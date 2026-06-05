use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

use crate::{git, linkfs, paths};

/// Reject items that could escape the project/central dirs. Items are operator-
/// supplied, but an absolute path makes `Path::join` discard the base and `..`
/// escapes the directory — on a tool that moves bytes, fail fast instead.
fn validate_item(item: &str) -> Result<()> {
    use std::path::Component;
    let p = Path::new(item);
    if p.is_absolute() {
        bail!("unsafe item '{item}': must be a relative path inside the project");
    }
    for c in p.components() {
        match c {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir => bail!("unsafe item '{item}': '..' is not allowed"),
            Component::RootDir | Component::Prefix(_) => {
                bail!("unsafe item '{item}': absolute or drive-rooted paths are not allowed")
            }
        }
    }
    Ok(())
}

/// Canonicalize project, find repo root (fallback = project), compute dest.
/// `central` should already be absolute (callers normalize via `std::path::absolute`).
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

/// Warn if an externalized item is not gitignored by the project (leak risk).
fn warn_if_leak(project: &Path, repo_name: &str, item: &str) {
    if !git::is_ignored(project, item) {
        let base = Path::new(item)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| item.to_string());
        eprintln!(
            "LEAK RISK: '{item}' is NOT ignored by {repo_name}'s git. Add no-slash \
             '{base}' to ~/.config/git/ignore (or '{item}' to the repo's .git/info/exclude) \
             BEFORE committing the project repo."
        );
    }
}

/// Move each item out of the project into central + link back.
pub fn link(project: &Path, central: &Path, items: &[String]) -> Result<()> {
    for item in items {
        validate_item(item)?;
    }
    let central = std::path::absolute(central)?;
    let (project, repo_name, dest) = resolve_dest(project, &central)?;
    std::fs::create_dir_all(&dest)?;

    let mut changed = false;

    for item in items {
        let src = project.join(item);
        let tgt = dest.join(item);

        // already a link in the project
        if linkfs::is_link(&src) {
            if src.exists() {
                // healthy link (target resolves) -> nothing to do
                eprintln!("skip {item} (already a link)");
            } else {
                // dangling link: target missing. Do NOT fake success.
                eprintln!("skip {item} (dangling link; target missing — remove it and re-run)");
            }
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

        // central has it but the project link is gone -> rebuild link (no central bytes change)
        if !src_exists && tgt_exists {
            if let Some(p) = src.parent() {
                std::fs::create_dir_all(p)?;
            }
            linkfs::make_link(&src, &tgt)?;
            println!("relinked  {item}  ->  {}   (central already had it)", tgt.display());
            warn_if_leak(&project, &repo_name, item);
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
            // roll back; if the rollback ALSO fails, the data is stranded in central — say so.
            if let Err(rb) = linkfs::move_item(&tgt, &src) {
                bail!(
                    "link failed for '{item}': {e}\n\
                     ROLLBACK FAILED — your data is now at {} (not {}). \
                     Move it back manually to recover: {rb}",
                    tgt.display(),
                    src.display()
                );
            }
            bail!("link failed for '{item}' (move rolled back, project intact): {e}");
        }
        println!("linked  {item}  ->  {}", tgt.display());
        warn_if_leak(&project, &repo_name, item);
        changed = true;
    }

    if changed {
        println!();
        println!("Done. Commit in {} to capture history:", central.display());
        println!(
            "  git -C \"{c}\" add -A; git -C \"{c}\" commit -m \"docs: {repo_name}\"",
            c = central.display()
        );
    }
    Ok(())
}

/// Rebuild links from central without moving (fresh-machine case).
pub fn relink(project: &Path, central: &Path, items: &[String]) -> Result<()> {
    for item in items {
        validate_item(item)?;
    }
    let central = std::path::absolute(central)?;
    let (project, repo_name, dest) = resolve_dest(project, &central)?;
    if !dest.exists() {
        bail!("no central folder for project: {}", dest.display());
    }

    for item in items {
        let src = project.join(item);
        let tgt = dest.join(item);

        if !tgt.exists() {
            eprintln!("skip {item} (not in central)");
            continue;
        }
        if linkfs::is_link(&src) {
            if src.exists() {
                eprintln!("skip {item} (already a link)");
            } else {
                eprintln!("skip {item} (dangling link; target missing — remove it and re-run)");
            }
            continue;
        }
        if src.exists() {
            eprintln!("skip {item} (already present in project)");
            continue;
        }
        if let Some(p) = src.parent() {
            std::fs::create_dir_all(p)?;
        }
        linkfs::make_link(&src, &tgt)?;
        println!("relinked  {item}  ->  {}", tgt.display());
        warn_if_leak(&project, &repo_name, item);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_item;

    #[test]
    fn accepts_normal_relative_items() {
        for ok in [".planning", "docs", ".docs", ".omc", ".env", "docs/api"] {
            assert!(validate_item(ok).is_ok(), "{ok} should be allowed");
        }
    }

    #[test]
    fn rejects_parent_dir() {
        assert!(validate_item("../escape").is_err());
        assert!(validate_item("a/../../b").is_err());
    }

    #[test]
    fn rejects_absolute_and_drive() {
        // Absolute (leading-/ RootDir) is rejected on every OS.
        assert!(validate_item("/etc/passwd").is_err());
        // Drive prefixes only exist on Windows; on unix `C:\Windows` / `C:/Windows`
        // are legitimately *relative* filenames that cannot escape the project.
        #[cfg(windows)]
        {
            assert!(validate_item("C:\\Windows").is_err());
            assert!(validate_item("C:Windows").is_err());
        }
    }
}
