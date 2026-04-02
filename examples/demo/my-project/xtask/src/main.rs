use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use xtask_patch::Manifest;

#[derive(Clone, Debug, Parser)]
struct Cli {
    #[arg(long, short)]
    show: bool,

    #[command(subcommand)]
    subcommands: Option<Subcommands>,
}

#[derive(Subcommand, Clone, Debug)]
enum Subcommands {
    Add(Add),
    Toggle { package: String },
    Remove { package: String },
}

#[derive(Args, Clone, Debug)]
struct Add {
    #[arg(long, short)]
    source: Option<String>,
    package: String,
    path: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut manifest = Manifest::new(None).expect("can read manifest");

    if cli.show {
        let patches = manifest.patches();
        if patches.is_empty() {
            println!("No available patches");
        } else {
            println!("{}", patches);
        }
    }

    if let Some(command) = cli.subcommands {
        match command {
            Subcommands::Add(add) => manifest.add(add.source, add.package, add.path),
            Subcommands::Toggle { package } => manifest.toggle(package),
            Subcommands::Remove { package } => manifest.remove(package),
        }

        manifest.write()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
