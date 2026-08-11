use clap::{Parser, Subcommand, ValueEnum};
use pci::PciAddress;

use crate::color::ColorMode;

#[derive(Debug, Parser)]
#[command(name = "lspci-rs")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    #[arg(long, global = true, value_enum, default_value_t = ColorMode::Auto)]
    pub color: ColorMode,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    List {
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },

    Show {
        address: PciAddress,

        #[arg(long, value_enum)]
        config: Option<ConfigLevel>,

        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },

    Tree {
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ConfigLevel {
    Header,
    Standard,
    Extended,
}

impl From<ConfigLevel> for pci::ConfigReadLevel {
    fn from(level: ConfigLevel) -> Self {
        match level {
            ConfigLevel::Header => Self::Header,
            ConfigLevel::Standard => Self::Standard,
            ConfigLevel::Extended => Self::Extended,
        }
    }
}
