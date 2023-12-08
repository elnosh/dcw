use std::error::Error;

use cashu_crab::{
    client::Client,
    nuts::nut00::{Proof, Proofs},
};

pub struct Wallet {
    db: sled::Db,
    mint_client: cashu_crab::client::Client,
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
        Ok(Self {
            db: db,
            mint_client: mint_client,
        })
    }

    pub fn get_balance(&self) -> u64 {
        let mut balance = 0;
        self.get_proofs()
            .iter()
            .for_each(|proof| balance += proof.amount.to_sat());
        balance
    }

    pub fn save_proof(&self, proof: &Proof) -> Result<(), Box<dyn Error>> {
        let proof_tree = self.db.open_tree("proofs")?;
        let json_proof = serde_json::to_vec(&proof)?;

        match proof_tree.insert(&proof.secret, json_proof) {
            Ok(_) => (),
            Err(e) => return Err(e.into()),
        };

        Ok(())
    }

    fn get_proofs(&self) -> Proofs {
        let proof_tree = match self.db.open_tree("proofs") {
            Ok(t) => t,
            Err(_) => return Vec::new(),
        };

        let proofs = proof_tree
            .iter()
            .map(|res| {
                let (_, value) = res.unwrap();
                serde_json::from_slice(&value).unwrap()
            })
            .collect();

        proofs
    }
}
