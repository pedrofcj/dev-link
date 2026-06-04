use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use tempfile::tempdir;

fn git_init(dir: &Path) {
    let out = StdCommand::new("git").arg("-C").arg(dir).arg("init").output().unwrap();
    assert!(out.status.success(), "git init failed in {}: {:?}", dir.display(), out);
}

/// A `dev-link` command with HOME/USERPROFILE pointed at an empty dir, so the
/// real `~/.config/dev-link/config.toml` (and the user's global git excludesFile)
/// can never poison a test.
fn dev_link(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("dev-link").unwrap();
    cmd.env("HOME", home).env("USERPROFILE", home);
    cmd
}

/// An empty home dir under `root` for config + git-global isolation.
fn empty_home(root: &Path) -> PathBuf {
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    home
}

#[test]
fn link_moves_bytes_and_creates_link() {
    let root = tempdir().unwrap();
    let home = empty_home(root.path());
    let project = root.path().join("proj");
    let central = root.path().join("central");
    fs::create_dir_all(project.join(".planning")).unwrap();
    fs::write(project.join(".planning").join("PLAN.md"), "hi").unwrap();
    git_init(&project);
    fs::write(project.join(".gitignore"), ".planning\n").unwrap();

    dev_link(&home)
        .args(["link", "--items", ".planning", "--project"])
        .arg(&project)
        .arg("--central")
        .arg(&central)
        .assert()
        .success()
        .stdout(predicates::str::contains("Done."));

    assert!(central.join("proj").join(".planning").join("PLAN.md").exists());
    assert_eq!(
        fs::read_to_string(project.join(".planning").join("PLAN.md")).unwrap(),
        "hi"
    );
}

#[test]
fn link_is_idempotent() {
    let root = tempdir().unwrap();
    let home = empty_home(root.path());
    let project = root.path().join("proj");
    let central = root.path().join("central");
    fs::create_dir_all(project.join(".planning")).unwrap();
    git_init(&project);
    fs::write(project.join(".gitignore"), ".planning\n").unwrap();

    dev_link(&home)
        .args(["link", "--items", ".planning", "--project"]).arg(&project)
        .arg("--central").arg(&central).assert().success();

    dev_link(&home)
        .args(["link", "--items", ".planning", "--project"]).arg(&project)
        .arg("--central").arg(&central)
        .assert()
        .success()
        .stderr(predicates::str::contains("already a link"));
}

#[test]
fn conflict_when_both_have_real_copy() {
    let root = tempdir().unwrap();
    let home = empty_home(root.path());
    let project = root.path().join("proj");
    let central = root.path().join("central");
    fs::create_dir_all(project.join(".planning")).unwrap();
    fs::create_dir_all(central.join("proj").join(".planning")).unwrap();
    git_init(&project);

    dev_link(&home)
        .args(["link", "--items", ".planning", "--project"]).arg(&project)
        .arg("--central").arg(&central)
        .assert()
        .failure()
        .stderr(predicates::str::contains("CONFLICT"));
}

#[test]
fn relink_rebuilds_without_moving() {
    let root = tempdir().unwrap();
    let home = empty_home(root.path());
    let project = root.path().join("proj");
    let central = root.path().join("central");
    fs::create_dir_all(project.join(".planning")).unwrap();
    fs::write(project.join(".planning").join("PLAN.md"), "hi").unwrap();
    git_init(&project);
    fs::write(project.join(".gitignore"), ".planning\n").unwrap();

    dev_link(&home).args(["link", "--items", ".planning", "--project"]).arg(&project)
        .arg("--central").arg(&central).assert().success();

    let link = project.join(".planning");
    if link.is_dir() { fs::remove_dir_all(&link).ok(); } else { fs::remove_file(&link).ok(); }

    dev_link(&home)
        .args(["relink", "--items", ".planning", "--project"]).arg(&project)
        .arg("--central").arg(&central)
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(project.join(".planning").join("PLAN.md")).unwrap(),
        "hi"
    );
}

#[test]
fn nested_project_mirrors_subpath() {
    let root = tempdir().unwrap();
    let home = empty_home(root.path());
    let repo = root.path().join("myRepo");
    let nested = repo.join("src").join("frontend");
    let central = root.path().join("central");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join(".env"), "SECRET=1").unwrap();
    git_init(&repo);
    fs::write(repo.join(".gitignore"), ".env\n").unwrap();

    // .env is a FILE -> on Windows needs Developer Mode (ON here and in CI).
    dev_link(&home)
        .args(["link", "--items", ".env", "--project"]).arg(&nested)
        .arg("--central").arg(&central)
        .assert()
        .success();

    // central got the bytes at the mirrored nested subpath
    assert!(central.join("myRepo").join("src").join("frontend").join(".env").exists());
    // project side is a LINK (not a real copy left behind), readable through to the bytes
    let project_env = nested.join(".env");
    assert_eq!(fs::read_to_string(&project_env).unwrap(), "SECRET=1");
    assert!(
        fs::symlink_metadata(&project_env).unwrap().file_type().is_symlink(),
        "project .env must be a symlink, not a real copy"
    );
}

#[test]
fn leak_risk_warns_when_item_not_gitignored() {
    let root = tempdir().unwrap();
    let home = empty_home(root.path());
    let project = root.path().join("proj");
    let central = root.path().join("central");
    fs::create_dir_all(project.join("leaky")).unwrap();
    fs::write(project.join("leaky").join("f.txt"), "x").unwrap();
    git_init(&project);
    // no .gitignore entry for `leaky` (and HOME is isolated, so no global ignore)
    // -> dev-link must warn LEAK RISK after linking it.

    dev_link(&home)
        .args(["link", "--items", "leaky", "--project"]).arg(&project)
        .arg("--central").arg(&central)
        .assert()
        .success()
        .stderr(predicates::str::contains("LEAK RISK"));
}

#[test]
fn errors_without_central() {
    // HOME/USERPROFILE point at an empty dir -> config::load() finds no config
    // and returns the default (no central) -> "no central configured".
    let root = tempdir().unwrap();
    let home = empty_home(root.path());
    let project = root.path().join("proj");
    fs::create_dir_all(&project).unwrap();
    git_init(&project);

    dev_link(&home)
        .args(["link", "--items", ".planning", "--project"]).arg(&project)
        .assert()
        .failure()
        .stderr(predicates::str::contains("no central configured"));
}
