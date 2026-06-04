use std::path::{Path, PathBuf};
use std::process::Command;

/// Repo root via `git -C <project> rev-parse --show-toplevel`.
/// `None` when not a git repo (caller falls back to the project path).
pub fn repo_root(project: &Path) -> Option<PathBuf> {
    let out = Command::new("git")
        .arg("-C")
        .arg(project)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(PathBuf::from(s))
    }
}

/// True if `item` is ignored by the project's git (`check-ignore` exits 0).
pub fn is_ignored(project: &Path, item: &str) -> bool {
    let item = item.replace('\\', "/");
    Command::new("git")
        .arg("-C")
        .arg(project)
        .args(["check-ignore", &item])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::tempdir;

    fn git(dir: &Path, args: &[&str]) {
        let out = Command::new("git").arg("-C").arg(dir).args(args).output().unwrap();
        assert!(out.status.success(), "git {args:?} failed: {out:?}");
    }

    #[test]
    fn finds_root_and_reads_ignore() {
        let dir = tempdir().unwrap();
        let root = dunce::canonicalize(dir.path()).unwrap();
        git(&root, &["init"]);
        std::fs::write(root.join(".gitignore"), ".planning\n").unwrap();

        let found = repo_root(&root).unwrap();
        assert_eq!(dunce::canonicalize(found).unwrap(), root);
        assert!(is_ignored(&root, ".planning"));
        assert!(!is_ignored(&root, "src"));
    }
}
