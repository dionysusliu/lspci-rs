mod cli;

use clap::Parser;
use cli::Cli;

fn main() {
    let cli = Cli::parse();

    match cli.command {
        cli::Command::List { format } => {
            println!("format: {format:?}")
        }
    }
}
