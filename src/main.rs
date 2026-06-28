use std::fs;

use anyhow::Context;
use clap::Parser;
use nextzip::bench::bench_path;
use nextzip::cli::{Cli, Command};
use nextzip::{inspect_archive, inspect_archive_json, pack_file, unpack_file, PackOptions};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Pack {
            input,
            output,
            exact,
            level,
        } => {
            pack_file(&input, &output, PackOptions { exact, level })?;
        }
        Command::Unpack { input, output } => {
            unpack_file(&input, &output)?;
        }
        Command::Inspect { input, json } => {
            let bytes = fs::read(&input).with_context(|| format!("read {}", input.display()))?;
            if json {
                println!("{}", inspect_archive_json(&bytes)?);
            } else {
                println!("{}", inspect_archive(&bytes)?);
            }
        }
        Command::Bench { input, json } => {
            println!("{}", bench_path(&input, json.as_deref())?);
        }
    }

    Ok(())
}
