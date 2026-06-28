use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "nextzip", about = "Structural program compression archive")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Pack {
        input: PathBuf,
        output: PathBuf,
        #[arg(long)]
        exact: bool,
        #[arg(long, default_value_t = 3)]
        level: i32,
    },
    Unpack {
        input: PathBuf,
        output: PathBuf,
    },
    Inspect {
        input: PathBuf,
    },
    Bench {
        input: PathBuf,
    },
}
