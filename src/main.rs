use crate::wallet::Wallet;
use clap::{Args, Parser, Subcommand};
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
    Mint(MintArgs),
    Send { amount: u64 },
    Receive { token: String },
}

#[derive(Args)]
struct MintArgs {
    amount: Option<u64>,
    #[arg(short, long)]
    /// specify paid invoice
    invoice: Option<String>,
}

fn main() {
    let wallet = Wallet::build("http://127.0.0.1:3338").unwrap_or_else(|err| {
        eprintln!("{err}");
        process::exit(1);
    });

    let cli = Cli::parse();

    match cli.command {
        Commands::Balance => println!("{} sats", wallet.get_balance()),
        Commands::Mint(mint_args) => {
            if mint_args.amount == None && mint_args.invoice == None {
                eprintln!("specify an amount to mint");
                process::exit(1);
            } else {
                match mint_args.amount {
                    Some(amount) => match block_on(wallet.request_mint(amount)) {
                        Ok(invoice) => println!("invoice to pay: {}", invoice.pr),
                        Err(e) => println!("could not generate invoice: {}", e.to_string()),
                    },
                    None => match mint_args.invoice {
                        Some(pr) => match block_on(wallet.mint_tokens(&pr)) {
                            Ok(_) => println!("tokens were successfully minted"),
                            Err(e) => println!("{}", e.to_string()),
                        },
                        _ => {}
                    },
                }
            }
        }
        Commands::Send { amount } => match block_on(wallet.send(amount)) {
            Ok(token) => println!("{}", token),
            Err(e) => println!("{}", e.to_string()),
        },
        Commands::Receive { token } => match block_on(wallet.receive(&token)) {
            Ok(_) => println!("token received"),
            Err(e) => println!("{}", e.to_string()),
        },
    }
}
