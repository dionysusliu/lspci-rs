mod cli;
mod output;

use clap::Parser;
use cli::Cli;
use pci::{PciAddress, PciSession};

use crate::cli::{Command, OutputFormat};

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::List { format } => match run_list(format) {
            Ok(output) => print!("{output}"),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        },

        Command::Show { address, format } => match run_show(address, format) {
            Ok(output) => print!("{output}"),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        },
    }
}

fn run_list(format: OutputFormat) -> Result<String, Box<dyn std::error::Error>> {
    let mut session = PciSession::new()?;
    let snapshot = session.scan()?;

    match format {
        OutputFormat::Text => Ok(output::render_text(&snapshot)),
        OutputFormat::Json => Ok(output::render_json(&snapshot)?),
    }
}

fn run_show(
    address: PciAddress,
    format: OutputFormat,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut session = PciSession::new()?;
    let inspection = session.inspect(address)?;

    match format {
        OutputFormat::Text => Ok(output::render_inspection_text(&inspection)),
        OutputFormat::Json => Ok(output::render_inspection_json(&inspection)?),
    }
}
