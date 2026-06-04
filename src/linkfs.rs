use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Create a link at `link` pointing to existing `target` (file or dir).
/// Windows dirs use a junction (no privilege) when same-volume; files and
/// cross-volume dirs use a real symlink (needs Developer Mode/admin).
pub fn make_link(link: &Path, target: &Path) -> Result<()> {
    let is_dir = target.is_dir();

    #[cfg(unix)]
    {
        let _ = is_dir;
        std::os::unix::fs::symlink(target, link)
            .with_context(|| format!("symlink {} -> {}", link.display(), target.display()))?;
        Ok(())
    }

    #[cfg(windows)]
    {
        if is_dir && same_volume(link, target) {
            junction::create(target, link)
                .with_context(|| format!("junction {} -> {}", link.display(), target.display()))?;
            return Ok(());
        }
        let res = if is_dir {
            std::os::windows::fs::symlink_dir(target, link)
        } else {
            std::os::windows::fs::symlink_file(target, link)
        };
        res.map_err(|e| win_link_err(e, link, target))
    }
}

/// True if `path` is a symlink (any OS) or a Windows junction.
pub fn is_link(path: &Path) -> bool {
    match fs::symlink_metadata(path) {
        Ok(m) => {
            if m.file_type().is_symlink() {
                return true;
            }
            #[cfg(windows)]
            {
                return junction::exists(path).unwrap_or(false);
            }
            #[allow(unreachable_code)]
            false
        }
        Err(_) => false,
    }
}

/// Move a file or dir; fall back to copy+remove across volumes.
pub fn move_item(src: &Path, dst: &Path) -> Result<()> {
    match fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(e) if is_cross_device(&e) => {
            copy_recursive(src, dst)?;
            remove_path(src)?;
            Ok(())
        }
        Err(e) => Err(anyhow::Error::new(e)
            .context(format!("move {} -> {}", src.display(), dst.display()))),
    }
}

fn is_cross_device(e: &std::io::Error) -> bool {
    #[cfg(unix)]
    {
        e.raw_os_error() == Some(18) // EXDEV
    }
    #[cfg(windows)]
    {
        e.raw_os_error() == Some(17) // ERROR_NOT_SAME_DEVICE
    }
}

fn copy_recursive(src: &Path, dst: &Path) -> Result<()> {
    if src.is_dir() {
        fs::create_dir_all(dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            copy_recursive(&entry.path(), &dst.join(entry.file_name()))?;
        }
    } else {
        if let Some(p) = dst.parent() {
            fs::create_dir_all(p)?;
        }
        fs::copy(src, dst)?;
    }
    Ok(())
}

fn remove_path(p: &Path) -> Result<()> {
    if p.is_dir() {
        fs::remove_dir_all(p)?;
    } else {
        fs::remove_file(p)?;
    }
    Ok(())
}

#[cfg(windows)]
fn same_volume(link: &Path, target: &Path) -> bool {
    fn vol(p: &Path) -> Option<String> {
        let base = if p.exists() {
            dunce::canonicalize(p).ok()?
        } else {
            dunce::canonicalize(p.parent()?).ok()?
        };
        match base.components().next()? {
            std::path::Component::Prefix(pre) => {
                Some(pre.as_os_str().to_string_lossy().to_uppercase())
            }
            _ => None,
        }
    }
    match (vol(link), vol(target)) {
        (Some(a), Some(b)) => a == b,
        _ => false, // unknown -> treat as cross-volume, use symlink
    }
}

#[cfg(windows)]
fn win_link_err(e: std::io::Error, link: &Path, target: &Path) -> anyhow::Error {
    if e.raw_os_error() == Some(1314) {
        // ERROR_PRIVILEGE_NOT_HELD
        anyhow::anyhow!(
            "file link needs Developer Mode ON (Settings > System > For developers) or an \
             admin shell. Directory items use junctions and don't need this. (failed: {} -> {})",
            link.display(),
            target.display()
        )
    } else {
        anyhow::Error::new(e)
            .context(format!("link {} -> {}", link.display(), target.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn link_dir_then_detect_and_resolve() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("real");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("f.txt"), "hi").unwrap();
        let link = dir.path().join("link");

        make_link(&link, &target).unwrap();

        assert!(is_link(&link), "link should be detected as a link");
        // reading through the link resolves to the real bytes
        assert_eq!(std::fs::read_to_string(link.join("f.txt")).unwrap(), "hi");
    }

    #[test]
    fn move_item_relocates_dir() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("a");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("f.txt"), "x").unwrap();
        let dst = dir.path().join("b");

        move_item(&src, &dst).unwrap();

        assert!(!src.exists());
        assert_eq!(std::fs::read_to_string(dst.join("f.txt")).unwrap(), "x");
    }
}
