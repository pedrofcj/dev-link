use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const DEFAULT_ITEMS: &[&str] = &[".planning", "docs", ".docs", ".omc"];

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    pub central: Option<String>,
    pub items: Option<Vec<String>>,
}

/// `~/.config/dev-link/config.toml` on every OS (Windows uses %USERPROFILE%).
pub fn config_path() -> PathBuf {
    home_dir().join(".config").join("dev-link").join("config.toml")
}

fn home_dir() -> PathBuf {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn load() -> Result<Config> {
    let path = config_path();
    if !path.exists() {
        return Ok(Config::default());
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading config {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing config {}", path.display()))
}

/// central: flag > config > error.
pub fn resolve_central(flag: Option<&str>, cfg: &Config) -> Result<PathBuf> {
    if let Some(c) = flag {
        return Ok(PathBuf::from(c));
    }
    if let Some(c) = &cfg.central {
        return Ok(PathBuf::from(c));
    }
    bail!("no central configured: run 'dev-link init --central <path>' or pass --central");
}

/// items: flag (non-empty) > config (non-empty) > built-in default.
pub fn resolve_items(flag: Option<&[String]>, cfg: &Config) -> Vec<String> {
    if let Some(items) = flag {
        if !items.is_empty() {
            return items.to_vec();
        }
    }
    if let Some(items) = &cfg.items {
        if !items.is_empty() {
            return items.clone();
        }
    }
    DEFAULT_ITEMS.iter().map(|s| s.to_string()).collect()
}

/// Write a config template (creates parent dirs). Used by `dev-link init`.
pub fn write_template(central: Option<&str>) -> Result<()> {
    let path = config_path();
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let central = central.unwrap_or("/path/to/dev-docs").replace('\\', "\\\\");
    let body = format!(
        "central = \"{central}\"\nitems   = [\".planning\", \"docs\", \".docs\", \".omc\"]\n"
    );
    std::fs::write(&path, body).with_context(|| format!("writing {}", path.display()))?;
    println!("wrote {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(central: Option<&str>, items: Option<Vec<&str>>) -> Config {
        Config {
            central: central.map(|s| s.to_string()),
            items: items.map(|v| v.into_iter().map(|s| s.to_string()).collect()),
        }
    }

    #[test]
    fn central_flag_beats_config() {
        let c = cfg(Some("/from-config"), None);
        assert_eq!(resolve_central(Some("/from-flag"), &c).unwrap(), PathBuf::from("/from-flag"));
    }

    #[test]
    fn central_falls_back_to_config() {
        let c = cfg(Some("/from-config"), None);
        assert_eq!(resolve_central(None, &c).unwrap(), PathBuf::from("/from-config"));
    }

    #[test]
    fn central_errors_when_unset() {
        assert!(resolve_central(None, &Config::default()).is_err());
    }

    #[test]
    fn items_default_when_unset() {
        let got = resolve_items(None, &Config::default());
        assert_eq!(got, vec![".planning", "docs", ".docs", ".omc"]);
    }

    #[test]
    fn items_flag_beats_config() {
        let c = cfg(None, Some(vec!["a", "b"]));
        let got = resolve_items(Some(&["x".to_string()]), &c);
        assert_eq!(got, vec!["x"]);
    }
}
