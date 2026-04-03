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
    Toggle(Pkg),
    Remove(Pkg),
}

#[derive(Args, Clone, Debug)]
struct Pkg {
    #[arg(long, short)]
    source: Option<String>,
    name: String,
}

#[derive(Args, Clone, Debug)]
struct Add {
    #[arg(long, short)]
    source: Option<String>,
    name: String,
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
            Subcommands::Add(add) => manifest.add(add.source, add.name, add.path)?,
            Subcommands::Toggle(pkg) => manifest.toggle(pkg.source, pkg.name)?,
            Subcommands::Remove(pkg) => manifest.remove(pkg.source, pkg.name)?,
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
