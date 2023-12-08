use crate::wallet::Wallet;
use clap::{Parser, Subcommand};
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
    // setup wallet db here
    let wallet = Wallet::build("http://localhost:3338").unwrap_or_else(|err| {
        eprintln!("{err}");
        process::exit(1);
    });

    let cli = Cli::parse();

    match cli.command {
        Commands::Balance => println!("{}", wallet.get_balance()),
        Commands::Mint { amount } => println!("requesting mint for {}", amount),
        _ => {}
    }
}
