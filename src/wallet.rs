use std::error::Error;

use cashu_crab::{
    client::Client,
    dhke,
    nuts::{
        nut00::{BlindedMessages, Proof, Proofs},
        nut02::KeySet,
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
    mint_client: cashu_crab::client::Client,
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
