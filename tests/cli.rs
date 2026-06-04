use assert_cmd::Command;
use std::fs;
use std::path::Path;
use std::process::Command as StdCommand;
use tempfile::tempdir;

fn git_init(dir: &Path) {
    StdCommand::new("git").arg("-C").arg(dir).arg("init").output().unwrap();
}

fn dev_link() -> Command {
    Command::cargo_bin("dev-link").unwrap()
}

#[test]
fn link_moves_bytes_and_creates_link() {
    let root = tempdir().unwrap();
    let project = root.path().join("proj");
    let central = root.path().join("central");
    fs::create_dir_all(project.join(".planning")).unwrap();
    fs::write(project.join(".planning").join("PLAN.md"), "hi").unwrap();
    git_init(&project);
    fs::write(project.join(".gitignore"), ".planning\n").unwrap();

    dev_link()
        .args(["link", "--items", ".planning", "--project"])
        .arg(&project)
        .arg("--central")
        .arg(&central)
        .assert()
        .success();

    assert!(central.join("proj").join(".planning").join("PLAN.md").exists());
    assert_eq!(
        fs::read_to_string(project.join(".planning").join("PLAN.md")).unwrap(),
        "hi"
    );
}

#[test]
fn link_is_idempotent() {
    let root = tempdir().unwrap();
    let project = root.path().join("proj");
    let central = root.path().join("central");
    fs::create_dir_all(project.join(".planning")).unwrap();
    git_init(&project);
    fs::write(project.join(".gitignore"), ".planning\n").unwrap();

    dev_link()
        .args(["link", "--items", ".planning", "--project"]).arg(&project)
        .arg("--central").arg(&central).assert().success();

    dev_link()
        .args(["link", "--items", ".planning", "--project"]).arg(&project)
        .arg("--central").arg(&central)
        .assert()
        .success()
        .stderr(predicates::str::contains("already a link"));
}

#[test]
fn conflict_when_both_have_real_copy() {
    let root = tempdir().unwrap();
    let project = root.path().join("proj");
    let central = root.path().join("central");
    fs::create_dir_all(project.join(".planning")).unwrap();
    fs::create_dir_all(central.join("proj").join(".planning")).unwrap();
    git_init(&project);

    dev_link()
        .args(["link", "--items", ".planning", "--project"]).arg(&project)
        .arg("--central").arg(&central)
        .assert()
        .failure()
        .stderr(predicates::str::contains("CONFLICT"));
}

#[test]
fn relink_rebuilds_without_moving() {
    let root = tempdir().unwrap();
    let project = root.path().join("proj");
    let central = root.path().join("central");
    fs::create_dir_all(project.join(".planning")).unwrap();
    fs::write(project.join(".planning").join("PLAN.md"), "hi").unwrap();
    git_init(&project);
    fs::write(project.join(".gitignore"), ".planning\n").unwrap();

    dev_link().args(["link", "--items", ".planning", "--project"]).arg(&project)
        .arg("--central").arg(&central).assert().success();

    let link = project.join(".planning");
    if link.is_dir() { fs::remove_dir_all(&link).ok(); } else { fs::remove_file(&link).ok(); }

    dev_link()
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
    let repo = root.path().join("myRepo");
    let nested = repo.join("src").join("frontend");
    let central = root.path().join("central");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join(".env"), "SECRET=1").unwrap();
    git_init(&repo);
    fs::write(repo.join(".gitignore"), ".env\n").unwrap();

    // .env is a FILE -> on Windows needs Developer Mode (ON here and in CI).
    dev_link()
        .args(["link", "--items", ".env", "--project"]).arg(&nested)
        .arg("--central").arg(&central)
        .assert()
        .success();

    assert!(central.join("myRepo").join("src").join("frontend").join(".env").exists());
}

#[test]
fn errors_without_central() {
    // Hermetic: point HOME/USERPROFILE at an EMPTY dir so config::load() finds no
    // config file and returns the default (no central) -> "no central configured".
    // (Do NOT env_remove these — home lookup is fallible and would error first.)
    let root = tempdir().unwrap();
    let home = root.path().join("home");
    fs::create_dir_all(&home).unwrap();
    let project = root.path().join("proj");
    fs::create_dir_all(&project).unwrap();
    git_init(&project);

    dev_link()
        .args(["link", "--items", ".planning", "--project"]).arg(&project)
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .assert()
        .failure()
        .stderr(predicates::str::contains("no central configured"));
}
