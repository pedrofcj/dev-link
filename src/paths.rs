use std::path::{Path, PathBuf};

/// Non-UTF-8 bytes are replaced with U+FFFD; pass a canonicalized path on Windows.
/// Leaf name of the repo root, used as the central subfolder name.
pub fn repo_name(repo_root: &Path) -> String {
    repo_root
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "repo".to_string())
}

/// Central destination dir for a project: `central/<repo_name>[/<rel>]`,
/// where `rel` is `project` relative to `repo_root` (empty when equal).
/// If `project` is not inside `repo_root`, falls back to `central/<repo_name>`.
pub fn dest_dir(central: &Path, repo_root: &Path, project: &Path) -> PathBuf {
    let mut dest = central.join(repo_name(repo_root));
    if let Ok(rel) = project.strip_prefix(repo_root) {
        if !rel.as_os_str().is_empty() {
            dest = dest.join(rel);
        }
    }
    dest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dest_for_repo_root_project() {
        let central = Path::new("/central");
        let repo = Path::new("/code/myRepo");
        let proj = Path::new("/code/myRepo");
        assert_eq!(dest_dir(central, repo, proj), PathBuf::from("/central/myRepo"));
    }

    #[test]
    fn dest_for_nested_project() {
        // cd into src/frontend, link .env -> central/myRepo/src/frontend/.env
        let central = Path::new("/central");
        let repo = Path::new("/code/myRepo");
        let proj = Path::new("/code/myRepo/src/frontend");
        assert_eq!(
            dest_dir(central, repo, proj),
            PathBuf::from("/central/myRepo/src/frontend")
        );
    }

    #[test]
    fn repo_name_is_leaf() {
        assert_eq!(repo_name(Path::new("/a/b/myRepo")), "myRepo");
    }
}
