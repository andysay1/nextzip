use std::fs;

use anyhow::Context;
use clap::Parser;
use nextzip::bench::bench_path;
use nextzip::cli::{Cli, Command};
use nextzip::{inspect_archive, pack, unpack, PackOptions};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Pack {
            input,
            output,
            exact,
            level,
        } => {
            let bytes = fs::read(&input).with_context(|| format!("read {}", input.display()))?;
            let archive = pack(&bytes, PackOptions { exact, level })?;
            fs::write(&output, archive).with_context(|| format!("write {}", output.display()))?;
        }
        Command::Unpack { input, output } => {
            let bytes = fs::read(&input).with_context(|| format!("read {}", input.display()))?;
            let restored = unpack(&bytes)?;
            fs::write(&output, restored).with_context(|| format!("write {}", output.display()))?;
        }
        Command::Inspect { input } => {
            let bytes = fs::read(&input).with_context(|| format!("read {}", input.display()))?;
            println!("{}", inspect_archive(&bytes)?);
        }
        Command::Bench { input, json } => {
            println!("{}", bench_path(&input, json.as_deref())?);
        }
    }

    Ok(())
}
