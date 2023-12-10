use std::{error::Error, str::FromStr};

use cashu_crab::{
    client::Client,
    dhke,
    nuts::{
        nut00::{BlindedMessages, Proof, Proofs, Token},
        nut02::KeySet,
        nut06::SplitRequest,
    },
    Amount,
};
use futures::executor::block_on;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize)]
struct MintRequestResponse {
    pr: String,
    hash: String,
}

#[derive(Deserialize, Serialize)]
pub struct Invoice {
    pub hash: String,
    pub pr: String,
    pub amount: Amount,
}

pub struct Wallet {
    db: sled::Db,
    mint_client: Client,
    current_keyset: KeySet,
}

impl Wallet {
    pub fn build(mint_url: &str) -> Result<Self, Box<dyn Error>> {
        let mut home_dir = match home::home_dir() {
            Some(path) => path,
            None => return Err("unable to setup wallet".into()),
        };
        home_dir.push(".cashuw");

        let db = sled::open(home_dir.as_path())?;
        let mint_client = Client::new(mint_url)?;
        let keys = block_on(mint_client.get_keys())?;

        Ok(Self {
            db: db,
            mint_client: mint_client,
            current_keyset: KeySet {
                id: keys.id(),
                keys: keys,
            },
        })
    }

    pub fn get_balance(&self) -> u64 {
        self.get_proofs()
            .iter()
            .map(|proof| proof.amount.to_sat())
            .sum()
    }

    pub async fn request_mint(&self, amount: u64) -> Result<Invoice, Box<dyn Error>> {
        let mut url = self.mint_client.mint_url.join("mint")?;
        url.query_pairs_mut()
            .append_pair("amount", &amount.to_string());

        let res = minreq::get(url).send()?.json::<Value>()?;

        let response: Result<MintRequestResponse, serde_json::Error> =
            serde_json::from_value(res.clone());

        match response {
            Ok(res) => {
                let invoice = Invoice {
                    hash: res.hash,
                    pr: res.pr,
                    amount: Amount::from_sat(amount),
                };
                self.save_invoice(&invoice)?;
                Ok(invoice)
            }
            Err(e) => Err(e.into()),
        }
    }

    // mint new tokens from invoice paid
    pub async fn mint_tokens(&self, pr: &String) -> Result<(), Box<dyn Error>> {
        let invoice = match self.get_invoice(pr) {
            Some(v) => v,
            None => return Err("invoice not found".into()),
        };

        // construct blinded messages
        let blinded_messages = BlindedMessages::random(invoice.amount)?;

        // send them to mint to get blinded signatures
        let mint_res = self
            .mint_client
            .mint(blinded_messages.clone(), &invoice.hash)
            .await?;

        // unblind the signatures to get the proofs
        let proofs = dhke::construct_proofs(
            mint_res.promises,
            blinded_messages.rs,
            blinded_messages.secrets,
            &self.current_keyset.keys,
        )?;

        // store proofs in db
        for proof in proofs {
            self.save_proof(&proof)?;
        }

        Ok(())
    }

    pub async fn send(&self, amount: u64) -> Result<String, Box<dyn Error>> {
        let proofs_to_send = select_proofs(self.get_proofs(), amount)?;
        let proofs_total: u64 = proofs_to_send
            .iter()
            .map(|proof| proof.amount.to_sat())
            .sum();

        let mut blinded_messages = BlindedMessages::random(Amount::from_sat(amount))?;
        let mut change = BlindedMessages::random(Amount::from_sat(proofs_total - amount))?;
        let send = blinded_messages.clone();

        blinded_messages
            .blinded_messages
            .append(&mut change.blinded_messages);
        blinded_messages.secrets.append(&mut change.secrets);
        blinded_messages.rs.append(&mut change.rs);
        blinded_messages.amounts.append(&mut change.amounts);

        blinded_messages = sort_blinded_messages(blinded_messages);

        let split_request = SplitRequest {
            proofs: proofs_to_send.clone(),
            outputs: blinded_messages.blinded_messages,
        };
        let split_res = self.mint_client.split(split_request).await?;

        // remove used proofs from db
        for proof in &proofs_to_send {
            self.delete_proof(&proof.secret)?;
        }

        let mut proofs = dhke::construct_proofs(
            split_res.promises,
            blinded_messages.rs,
            blinded_messages.secrets,
            &self.current_keyset.keys,
        )?;

        let mut proofs_to_send: Proofs = Vec::new();
        for message in send.blinded_messages {
            for (i, proof) in proofs.iter().enumerate() {
                if proof.amount == message.amount {
                    proofs_to_send.push(proof.clone());
                    proofs.swap_remove(i);
                    break;
                }
            }
        }

        // TODO: batch these instead of 1 by 1
        // remaining proofs are change to save
        for proof in &proofs {
            self.save_proof(&proof)?;
        }

        let token = Token::new(self.mint_client.mint_url.clone(), proofs_to_send, None)
            .convert_to_string()?;
        Ok(token)
    }

    pub async fn receive(&self, token: &str) -> Result<(), Box<dyn Error>> {
        let token = Token::from_str(token)?;

        let receive_amount: u64 = token
            .token
            .iter()
            .map(|proofs| {
                proofs
                    .proofs
                    .iter()
                    .map(|proof| proof.amount.to_sat())
                    .sum::<u64>()
            })
            .sum();

        let blinded_messages = BlindedMessages::random(Amount::from_sat(receive_amount))?;
        let proofs: Vec<Proof> = token
            .token
            .iter()
            .flat_map(|mint| mint.proofs.iter())
            .cloned()
            .collect();

        let split_request = SplitRequest {
            proofs: proofs,
            outputs: blinded_messages.blinded_messages,
        };
        let split_res = self.mint_client.split(split_request).await?;

        let proofs = dhke::construct_proofs(
            split_res.promises,
            blinded_messages.rs,
            blinded_messages.secrets,
            &self.current_keyset.keys,
        )?;

        for proof in proofs {
            self.save_proof(&proof)?;
        }

        Ok(())
    }

    pub fn save_proof(&self, proof: &Proof) -> Result<(), Box<dyn Error>> {
        let proof_tree = self.db.open_tree("proofs")?;
        let json_proof = serde_json::to_vec(&proof)?;
        proof_tree.insert(&proof.secret, json_proof)?;
        Ok(())
    }

    fn get_proofs(&self) -> Proofs {
        let proof_tree = match self.db.open_tree("proofs") {
            Ok(t) => t,
            Err(_) => return Vec::new(),
        };

        proof_tree
            .iter()
            .map(|res| {
                let (_, value) = res.unwrap();
                serde_json::from_slice(&value).unwrap()
            })
            .collect()
    }

    fn delete_proof(&self, secret: &String) -> Result<(), Box<dyn Error>> {
        let proof_tree = self.db.open_tree("proofs")?;
        proof_tree.remove(secret)?;
        Ok(())
    }

    fn save_invoice(&self, invoice: &Invoice) -> Result<(), Box<dyn Error>> {
        let invoice_tree = self.db.open_tree("invoices")?;
        let json_invoice = serde_json::to_vec(&invoice)?;
        invoice_tree.insert(&invoice.pr, json_invoice)?;
        Ok(())
    }

    fn get_invoice(&self, pr: &String) -> Option<Invoice> {
        let invoice_tree = match self.db.open_tree("invoices") {
            Ok(tree) => tree,
            Err(_) => return None,
        };

        let invoice: Invoice = match invoice_tree.get(&pr) {
            Ok(opt) => {
                let v = opt?;
                match serde_json::from_slice(&v) {
                    Ok(inv) => inv,
                    Err(_) => return None,
                }
            }
            Err(_) => return None,
        };
        Some(invoice)
    }
}

fn sort_blinded_messages(mut blinded: BlindedMessages) -> BlindedMessages {
    let mut order = blinded.clone();

    order
        .blinded_messages
        .sort_by(|a, b| a.amount.cmp(&b.amount));

    blinded.secrets = order.secrets.iter().map(|x| x.clone()).collect::<Vec<_>>();
    blinded.rs = order.rs.iter().map(|x| x.clone()).collect::<Vec<_>>();
    blinded.amounts = order.amounts.iter().map(|x| x.clone()).collect::<Vec<_>>();

    blinded
}

fn select_proofs(proofs: Proofs, amount: u64) -> Result<Proofs, Box<dyn Error>> {
    let total_proofs_amount = proofs.iter().map(|proof| proof.amount.to_sat()).sum();
    if amount > total_proofs_amount {
        return Err("insufficient funds".into());
    }

    let mut proofs_to_send: Proofs = Vec::new();
    let mut current_total = 0;
    for proof in proofs {
        current_total += proof.amount.to_sat();
        proofs_to_send.push(proof);

        if current_total > amount {
            break;
        }
    }

    Ok(proofs_to_send)
}
