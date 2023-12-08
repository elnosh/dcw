use crate::wallet::Wallet;
use clap::{Parser, Subcommand};
use futures::executor::block_on;
use std::process;

pub mod wallet;

#[derive(Parser)]
#[command()]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Balance,
    Mint {
        amount: u64,
        //invoice: Option<String>,
    },
    Send {
        amount: u64,
    },
    Receive {
        token: String,
    },
}

fn main() {
    let wallet = Wallet::build("http://127.0.0.1:3338").unwrap_or_else(|err| {
        eprintln!("{err}");
        process::exit(1);
    });

    let cli = Cli::parse();

    match cli.command {
        Commands::Balance => println!("{} sats", wallet.get_balance()),
        Commands::Mint { amount } => match block_on(wallet.request_mint(amount)) {
            Ok(mint_res) => println!("invoice: {}", mint_res.pr),
            Err(e) => println!("could not generate invoice: {}", e.to_string()),
        },
        _ => {}
    }
}
