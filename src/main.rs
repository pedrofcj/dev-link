mod config;
mod git;
mod linkfs;
mod marker;
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
    /// Per-repo settings.
    #[command(subcommand)]
    Project(ProjectCmd),
}

#[derive(Subcommand)]
enum ProjectCmd {
    /// Write .dev-link.toml at the repo root, pinning links to central/<workspace>/<project>.
    Init {
        /// Workspace folder under central.
        #[arg(long, default_value = "default")]
        workspace: String,
        /// Project folder under the workspace (defaults to the repo folder name).
        #[arg(long)]
        project: Option<String>,
        /// Directory inside the repo (defaults to the current directory).
        #[arg(long)]
        path: Option<PathBuf>,
        /// Overwrite an existing .dev-link.toml.
        #[arg(long)]
        force: bool,
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
        Cmd::Project(ProjectCmd::Init { workspace, project, path, force }) => {
            project_init(workspace, project, path, force)
        }
    }
}

fn project_init(
    workspace: String,
    project: Option<String>,
    path: Option<PathBuf>,
    force: bool,
) -> Result<()> {
    let dir = match path {
        Some(p) => p,
        None => std::env::current_dir()?,
    };
    if !dir.exists() {
        anyhow::bail!("path not found: {}", dir.display());
    }
    let dir = dunce::canonicalize(&dir)?;
    let root = git::repo_root(&dir)
        .and_then(|r| dunce::canonicalize(&r).ok())
        .unwrap_or(dir);
    let project = project.unwrap_or_else(|| paths::repo_name(&root));
    let m = marker::Marker { workspace, project };
    let written = marker::write(&root, &m, force)?;
    println!("wrote {}", written.display());
    // Informational only: the marker is already written, so a bad config must not fail here.
    match config::load().ok().and_then(|c| c.central) {
        Some(c) => println!(
            "links will go to {}",
            PathBuf::from(c).join(&m.workspace).join(&m.project).display()
        ),
        None => println!("links will go to <central>/{}", m.label()),
    }
    Ok(())
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
