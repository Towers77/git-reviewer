//! The user's settings file, `config.toml` in the platform's config folder:
//! `%APPDATA%\git-reviewer\` on Windows, `~/Library/Application Support/git-reviewer/`
//! on macOS, `~/.config/git-reviewer/` on Linux.
//!
//! ```toml
//! root = "C:\\Users\\me\\repos"     # the folder whose repos the app shows
//!
//! [bases]                            # optional: compare a repo against this branch
//! web-client = "origin/development"  # instead of its origin/HEAD
//! ```
//!
//! The app rewrites the file only when it's started with a different root folder, and
//! a rewrite keeps the settings but not comments.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const APP_DIR: &str = "git-reviewer";
const FILE_NAME: &str = "config.toml";

// `deny_unknown_fields`: a misspelled key (`rot = ...`) makes the file invalid, which is
// reported, instead of being silently ignored and then dropped by the next save.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// The folder whose repos are shown. Set from the command line the first time;
    /// remembered so the app can be opened without arguments afterwards.
    pub root: Option<PathBuf>,
    /// Per-repo (by folder name) base branch for the Del. branch check, for repos whose
    /// `origin/HEAD` isn't the branch work merges into.
    pub bases: BTreeMap<String, String>,
}

#[derive(Debug)]
pub enum ConfigError {
    /// No config folder on this platform (e.g. no home directory).
    NoConfigDir,
    Io(PathBuf, io::Error),
    Parse(PathBuf, toml::de::Error),
    Serialize(toml::ser::Error),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::NoConfigDir => write!(f, "no config folder found on this system"),
            ConfigError::Io(path, err) => write!(f, "{}: {err}", path.display()),
            ConfigError::Parse(path, err) => {
                write!(f, "{} is not valid TOML: {err}", path.display())
            }
            ConfigError::Serialize(err) => write!(f, "could not write the config: {err}"),
        }
    }
}

impl Config {
    /// The app's folder in the platform's config folder, which also holds `state.toml`.
    pub fn dir() -> Result<PathBuf, ConfigError> {
        dirs::config_dir()
            .map(|dir| dir.join(APP_DIR))
            .ok_or(ConfigError::NoConfigDir)
    }

    pub fn path() -> Result<PathBuf, ConfigError> {
        Ok(Config::dir()?.join(FILE_NAME))
    }

    /// Reads the config. A missing file is a normal first run and gives the defaults;
    /// a file that exists but can't be read or parsed is an error, so a typo is
    /// reported instead of silently ignored.
    pub fn load() -> Result<Config, ConfigError> {
        let path = Config::path()?;

        match fs::read_to_string(&path) {
            Ok(text) => Config::parse(&text).map_err(|err| ConfigError::Parse(path, err)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Config::default()),
            Err(err) => Err(ConfigError::Io(path, err)),
        }
    }

    fn parse(text: &str) -> Result<Config, toml::de::Error> {
        toml::from_str(text)
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        write_toml(&Config::path()?, self)
    }

    /// The configured base branch for the repo in folder `repo_name`, if any.
    pub fn base_for(&self, repo_name: &str) -> Option<&str> {
        self.bases.get(repo_name).map(String::as_str)
    }
}

/// Writes `value` as TOML to `path`, creating its folder first if needed.
pub fn write_toml(path: &Path, value: &impl Serialize) -> Result<(), ConfigError> {
    let text = toml::to_string_pretty(value).map_err(ConfigError::Serialize)?;

    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|err| ConfigError::Io(dir.to_path_buf(), err))?;
    }
    fs::write(path, text).map_err(|err| ConfigError::Io(path.to_path_buf(), err))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_full() {
        let config = Config::parse(
            r#"
            root = 'C:\Users\me\repos'

            [bases]
            web-client = "origin/development"
            "#,
        )
        .unwrap();

        assert_eq!(config.root, Some(PathBuf::from(r"C:\Users\me\repos")));
        assert_eq!(config.base_for("web-client"), Some("origin/development"));
        assert_eq!(config.base_for("api"), None);
    }

    #[test]
    fn test_parse_empty_gives_defaults() {
        assert_eq!(Config::parse("").unwrap(), Config::default());
    }

    #[test]
    fn test_parse_rejects_wrong_types() {
        assert!(Config::parse("root = 3").is_err());
    }

    #[test]
    fn test_parse_rejects_unknown_keys() {
        assert!(Config::parse("rot = 'C:\\repos'").is_err());
    }

    #[test]
    fn test_round_trip() {
        let mut config = Config {
            root: Some(PathBuf::from(r"C:\Users\me\repos")),
            ..Config::default()
        };
        config
            .bases
            .insert("web-client".into(), "origin/development".into());

        let text = toml::to_string_pretty(&config).unwrap();
        assert_eq!(Config::parse(&text).unwrap(), config);
    }
}
