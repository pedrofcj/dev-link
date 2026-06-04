use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

use crate::{git, linkfs, paths};

/// Canonicalize project, find repo root (fallback = project), compute dest.
/// Returns (canonical_project, repo_name, dest_dir).
pub fn resolve_dest(project: &Path, central: &Path) -> Result<(PathBuf, String, PathBuf)> {
    if !project.exists() {
        bail!("project not found: {}", project.display());
    }
    let project = dunce::canonicalize(project)?;
    let repo_root = git::repo_root(&project)
        .and_then(|r| dunce::canonicalize(&r).ok())
        .unwrap_or_else(|| project.clone());
    let name = paths::repo_name(&repo_root);
    let dest = paths::dest_dir(central, &repo_root, &project);
    Ok((project, name, dest))
}

/// Move each item out of the project into central + link back.
pub fn link(project: &Path, central: &Path, items: &[String]) -> Result<()> {
    let (project, repo_name, dest) = resolve_dest(project, central)?;
    std::fs::create_dir_all(&dest)?;

    for item in items {
        let src = project.join(item);
        let tgt = dest.join(item);

        // already a link in the project -> nothing to do
        if linkfs::is_link(&src) {
            eprintln!("skip {item} (already a link)");
            continue;
        }

        let src_exists = src.exists();
        let tgt_exists = tgt.exists();

        // both hold a real copy -> ambiguous, refuse loudly
        if src_exists && tgt_exists {
            bail!(
                "CONFLICT: '{item}' exists in BOTH project ({}) and central ({}). \
                 Keep one, delete the other, then re-run.",
                src.display(),
                tgt.display()
            );
        }

        // central has it but the project link is gone -> rebuild link
        if !src_exists && tgt_exists {
            if let Some(p) = src.parent() {
                std::fs::create_dir_all(p)?;
            }
            linkfs::make_link(&src, &tgt)?;
            println!("relinked  {item}  ->  {}   (central already had it)", tgt.display());
            continue;
        }

        if !src_exists && !tgt_exists {
            eprintln!("skip {item} (not in project)");
            continue;
        }

        // normal: real item in project, nothing in central -> move + link atomically
        if let Some(p) = tgt.parent() {
            std::fs::create_dir_all(p)?;
        }
        linkfs::move_item(&src, &tgt)?;
        if let Err(e) = linkfs::make_link(&src, &tgt) {
            let _ = linkfs::move_item(&tgt, &src); // roll back
            bail!("link failed for '{item}' (move rolled back, project intact): {e}");
        }
        println!("linked  {item}  ->  {}", tgt.display());

        // safety: the link must stay ignored by the project's repo or it can leak
        if !git::is_ignored(&project, item) {
            let base = Path::new(item)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| item.clone());
            eprintln!(
                "LEAK RISK: '{item}' is NOT ignored by {repo_name}'s git. Add no-slash \
                 '{base}' to ~/.config/git/ignore BEFORE committing the project repo."
            );
        }
    }

    println!();
    println!("Done. Commit in {} to capture history:", central.display());
    println!(
        "  git -C \"{c}\" add -A; git -C \"{c}\" commit -m \"docs: {repo_name}\"",
        c = central.display()
    );
    Ok(())
}
