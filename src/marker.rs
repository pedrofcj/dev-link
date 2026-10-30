use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Per-repo marker that pins the central destination to `central/<workspace>/<project>`.
pub const MARKER_FILE: &str = ".dev-link.toml";

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Marker {
    pub workspace: String,
    pub project: String,
}

impl Marker {
    /// Central subpath label, e.g. `work/myRepo`.
    pub fn label(&self) -> String {
        format!("{}/{}", self.workspace, self.project)
    }
}

/// A workspace/project name becomes exactly one directory under central, so it
/// must be a single plain path segment (no separators, `.`, `..`, or drive).
pub fn validate_name(field: &str, value: &str) -> Result<()> {
    use std::path::Component;
    if value.trim().is_empty() {
        bail!("{field} must not be empty");
    }
    if value.contains('/') || value.contains('\\') {
        bail!("invalid {field} '{value}': must be a single folder name (no '/' or '\\')");
    }
    let mut comps = Path::new(value).components();
    match (comps.next(), comps.next()) {
        (Some(Component::Normal(_)), None) => Ok(()),
        _ => bail!("invalid {field} '{value}': must be a single folder name (not '.', '..' or a drive)"),
    }
}

/// Nearest marker at or above `start`. Returns the directory holding it plus its
/// parsed contents. A malformed marker is an error, never a silent fallback.
pub fn find(start: &Path) -> Result<Option<(PathBuf, Marker)>> {
    for dir in start.ancestors() {
        let path = dir.join(MARKER_FILE);
        if !path.is_file() {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let marker: Marker =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        validate_name("workspace", &marker.workspace)
            .and_then(|_| validate_name("project", &marker.project))
            .with_context(|| format!("in {}", path.display()))?;
        return Ok(Some((dir.to_path_buf(), marker)));
    }
    Ok(None)
}

/// Write the marker into `dir`. Refuses to overwrite unless `force`.
pub fn write(dir: &Path, marker: &Marker, force: bool) -> Result<PathBuf> {
    validate_name("workspace", &marker.workspace)?;
    validate_name("project", &marker.project)?;
    let path = dir.join(MARKER_FILE);
    if path.exists() && !force {
        bail!("{} already exists (pass --force to overwrite)", path.display());
    }
    let body = toml::to_string(marker)?;
    std::fs::write(&path, body).with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn accepts_plain_names() {
        for ok in ["work", "my-repo", "my.repo", ".hidden"] {
            assert!(validate_name("project", ok).is_ok(), "{ok} should be allowed");
        }
    }

    #[test]
    fn rejects_paths_and_dots() {
        for bad in ["", "  ", ".", "..", "a/b", "a\\b", "../x", "/abs"] {
            assert!(validate_name("project", bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn finds_nearest_marker_in_ancestor() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let m = Marker { workspace: "ws".into(), project: "proj".into() };
        write(root, &m, false).unwrap();
        let nested = root.join("src").join("frontend");
        std::fs::create_dir_all(&nested).unwrap();

        let (found_dir, found) = find(&nested).unwrap().unwrap();
        assert_eq!(found_dir, root);
        assert_eq!(found.label(), "ws/proj");
    }

    #[test]
    fn rejects_unsafe_names_in_file() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join(MARKER_FILE),
            "workspace = \"..\"\nproject = \"p\"\n",
        )
        .unwrap();
        assert!(find(dir.path()).is_err());
    }

    #[test]
    fn write_refuses_overwrite_without_force() {
        let dir = tempdir().unwrap();
        let m = Marker { workspace: "ws".into(), project: "p".into() };
        write(dir.path(), &m, false).unwrap();
        assert!(write(dir.path(), &m, false).is_err());
        assert!(write(dir.path(), &m, true).is_ok());
    }
}
