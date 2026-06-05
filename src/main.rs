mod config;
mod git;
mod linkfs;
mod ops;
mod paths;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "dev-link", about = "Externalize gitignored docs to a central repo via links")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Move items out of the project into central + link back.
    Link(LinkArgs),
    /// Rebuild links from central (no move).
    Relink(LinkArgs),
    /// Write a config template to ~/.config/dev-link/config.toml.
    Init {
        #[arg(long)]
        central: Option<String>,
    },
}

#[derive(Args)]
struct LinkArgs {
    /// Project path (defaults to the current directory).
    #[arg(long)]
    project: Option<PathBuf>,
    /// Comma-separated items; replaces the default list.
    #[arg(long, value_delimiter = ',')]
    items: Option<Vec<String>>,
    /// Central docs repo; overrides config.
    #[arg(long)]
    central: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Link(a) => run(a, false),
        Cmd::Relink(a) => run(a, true),
        Cmd::Init { central } => config::write_template(central.as_deref()),
    }
}

fn run(a: LinkArgs, relink: bool) -> Result<()> {
    let cfg = config::load()?;
    let central = config::resolve_central(a.central.as_deref(), &cfg)?;
    let items = config::resolve_items(a.items.as_deref(), &cfg);
    let project = match a.project {
        Some(p) => p,
        None => std::env::current_dir()?,
    };
    if relink {
        ops::relink(&project, &central, &items)
    } else {
        ops::link(&project, &central, &items)
    }
}
