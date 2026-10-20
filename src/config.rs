//! reviewy.toml loading and merging over the built-in defaults.

use std::io;
use std::path::{Path, PathBuf};

use color_eyre::Result;
use color_eyre::eyre::WrapErr;
use etcetera::BaseStrategy;
use serde::Deserialize;
use toml::{Table, Value};

use crate::highlight::theme;

const DEFAULTS: &str = include_str!("defaults/defaults.toml");

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code, reason = "all fields read by task 09; remove then")]
pub struct Config {
    pub ui: UiConfig,
    pub limits: LimitsConfig,
    pub ignore: IgnoreConfig,
    pub comments: CommentsConfig,
    pub difft: DifftConfig,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code, reason = "all fields read by task 07; remove then")]
pub struct UiConfig {
    pub theme: Option<String>,
    pub theme_variant: Option<String>,
    pub wrap: bool,
    pub tab_width: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LimitsConfig {
    pub max_lines: usize,
    pub max_bytes: usize,
    pub max_line_length: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IgnoreConfig {
    pub paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code, reason = "all fields read by task 10; remove then")]
pub struct CommentsConfig {
    pub context_lines: usize,
    pub preamble: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DifftConfig {
    pub binary: PathBuf,
}

/// Defaults, then the global XDG config, then `<repo_root>/.reviewy/reviewy.toml`.
pub fn load(repo_root: &Path) -> Result<Config> {
    let global = etcetera::choose_base_strategy()?
        .config_dir()
        .join("reviewy/reviewy.toml");
    let project = repo_root.join(".reviewy/reviewy.toml");
    let mut sources = Vec::new();
    for path in [global, project] {
        match std::fs::read_to_string(&path) {
            Ok(text) => sources.push((path, text)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e).wrap_err_with(|| format!("reading {}", path.display())),
        }
    }
    let sources: Vec<_> = sources
        .iter()
        .map(|(p, t)| (p.as_path(), t.as_str()))
        .collect();
    from_sources(&sources)
}

/// `sources` are `(path, toml)` pairs layered over the defaults, later overriding earlier; the path only labels errors.
fn from_sources(sources: &[(&Path, &str)]) -> Result<Config> {
    let mut merged: Table = toml::from_str(DEFAULTS)?;
    let mut config = merged.clone().try_into()?;
    for (path, text) in sources {
        let context = || format!("invalid {}", path.display());
        let mut table = toml::from_str(text).wrap_err_with(context)?;
        resolve_theme_path(&mut table, path);
        merge(&mut merged, table);
        // Everything merged before was valid, so any error comes from this file.
        config = merged.clone().try_into().wrap_err_with(context)?;
    }
    Ok(config)
}

/// Makes a relative theme path in `ui.theme` relative to the config file that sets it.
fn resolve_theme_path(table: &mut Table, file: &Path) {
    if let Some(Value::String(theme)) = table.get_mut("ui").and_then(|ui| ui.get_mut("theme"))
        && theme::is_path(theme)
        && Path::new(theme).is_relative()
        && let Some(dir) = file.parent()
    {
        *theme = dir.join(&*theme).to_string_lossy().into_owned();
    }
}

/// Deep-merges tables; any other value (arrays included) replaces.
fn merge(base: &mut Table, over: Table) {
    for (key, value) in over {
        match (base.get_mut(&key), value) {
            (Some(Value::Table(b)), Value::Table(o)) => merge(b, o),
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GLOBAL: &str = "/global/reviewy.toml";
    const PROJECT: &str = "/repo/.reviewy/reviewy.toml";

    fn load(global: &str, project: &str) -> Result<Config> {
        from_sources(&[(Path::new(GLOBAL), global), (Path::new(PROJECT), project)])
    }

    #[test]
    fn no_files_gives_defaults() {
        let c = from_sources(&[]).unwrap();
        assert_eq!(c.ui.theme, None);
        assert!(c.ui.wrap);
        assert_eq!(c.ui.tab_width, 4);
        assert_eq!(c.limits.max_bytes, 1_048_576);
        assert_eq!(c.comments.context_lines, 3);
        assert_eq!(c.difft.binary, Path::new("difft"));
    }

    #[test]
    fn project_overrides_global_per_key() {
        let c = load("[ui]\ntab_width = 8\nwrap = false", "[ui]\ntab_width = 2").unwrap();
        assert_eq!(c.ui.tab_width, 2);
        assert!(!c.ui.wrap);
    }

    #[test]
    fn arrays_replace() {
        let c = load(
            "[ignore]\npaths = [\"*.lock\", \"dist/**\"]",
            "[ignore]\npaths = [\"*.md\"]",
        )
        .unwrap();
        assert_eq!(c.ignore.paths, ["*.md"]);
    }

    #[test]
    fn relative_theme_path_is_relative_to_config_file() {
        let c = load("", "[ui]\ntheme = \"themes/x.json\"").unwrap();
        assert_eq!(c.ui.theme.unwrap(), "/repo/.reviewy/themes/x.json");
    }

    #[test]
    fn unknown_key_names_file() {
        let err = load("", "[ui]\ntabwidth = 2").unwrap_err();
        assert!(format!("{err:#}").contains(PROJECT), "{err:#}");
    }
}
