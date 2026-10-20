//! Zed theme parsing, alpha blending, capture fallback, bundled themes.

use std::collections::{BTreeMap, HashMap};
use std::io;
use std::path::{Path, PathBuf};

use etcetera::BaseStrategy;
use ratatui::style::{Color, Modifier, Style};
use serde::Deserialize;
use serde_json::Value;

use crate::defaults::default_theme;

/// Alpha of the added/deleted color blended onto the background for diff tints (`ui.tint`).
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tint {
    /// Changed lines.
    pub line: f32,
    /// Changed tokens.
    pub token: f32,
    /// Selection fallback when the theme has no `players[0].selection`: `foreground` blended
    /// onto `active_line_background`.
    pub selection: f32,
}

#[derive(Debug, thiserror::Error)]
pub enum ThemeError {
    #[error("theme `{0}` not found")]
    NotFound(String),
    #[error("theme file {} needs `ui.theme_variant`", .0.display())]
    NoVariant(PathBuf),
    #[error("theme `{theme}` is missing `{key}`")]
    MissingKey { theme: String, key: String },
    #[error("theme `{theme}`: invalid color {value:?} for `{key}`")]
    BadColor {
        theme: String,
        key: String,
        value: String,
    },
    #[error("invalid theme file {}: {source}", path.display())]
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("reading {}: {source}", path.display())]
    Io { path: PathBuf, source: io::Error },
    #[error(transparent)]
    Home(#[from] etcetera::HomeDirError),
}

/// One resolved theme variant, all colors opaque RGB.
#[derive(Debug)]
pub struct Theme {
    pub name: String,
    pub background: Color,
    pub foreground: Color,
    pub line_number: Color,
    pub active_line_background: Color,
    /// Visual-mode selection background (`players[0].selection`).
    pub selection: Color,
    pub added: Color,
    pub deleted: Color,
    pub modified: Color,
    pub conflict: Color,
    pub line_added_bg: Color,
    pub line_deleted_bg: Color,
    pub token_added_bg: Color,
    pub token_deleted_bg: Color,
    pub syntax: BTreeMap<String, Style>,
}

impl Theme {
    /// `theme` is a variant name, or a path to a theme file (see [`is_path`]) whose variant is `variant`.
    pub fn resolve(theme: &str, variant: Option<&str>, tint: Tint) -> Result<Theme, ThemeError> {
        if is_path(theme) {
            let path = Path::new(theme);
            let variant = variant.ok_or_else(|| ThemeError::NoVariant(path.into()))?;
            return find(&[read_family(path)?], variant, tint);
        }
        let mut families = user_families()?;
        families.push(bundled_family());
        find(&families, theme, tint)
    }

    pub fn bundled_default(tint: Tint) -> Theme {
        find(&[bundled_family()], default_theme::VARIANT, tint).expect("default theme is bundled")
    }
}

/// True if `ui.theme` names a file rather than a variant.
pub fn is_path(theme: &str) -> bool {
    theme.contains('/') || theme.ends_with(".json")
}

#[derive(Deserialize)]
struct Family {
    themes: Vec<Variant>,
}

#[derive(Deserialize)]
struct Variant {
    name: String,
    style: VariantStyle,
}

#[derive(Deserialize)]
struct VariantStyle {
    #[serde(default)]
    syntax: BTreeMap<String, SyntaxStyle>,
    #[serde(flatten)]
    colors: HashMap<String, Value>,
}

#[derive(Deserialize)]
struct SyntaxStyle {
    color: Option<String>,
    font_style: Option<String>,
    font_weight: Option<f32>,
}

fn bundled_family() -> Family {
    serde_json::from_str(default_theme::FAMILY).expect("bundled theme is valid")
}

fn read_family(path: &Path) -> Result<Family, ThemeError> {
    let text = std::fs::read_to_string(path).map_err(|source| ThemeError::Io {
        path: path.into(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| ThemeError::Json {
        path: path.into(),
        source,
    })
}

/// Families in `<config dir>/reviewy/themes/*.json`, sorted by file name.
fn user_families() -> Result<Vec<Family>, ThemeError> {
    let dir = etcetera::choose_base_strategy()?
        .config_dir()
        .join("reviewy/themes");
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(ThemeError::Io { path: dir, source }),
    };
    let mut paths = entries
        .map(|entry| entry.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| ThemeError::Io {
            path: dir.clone(),
            source,
        })?;
    paths.retain(|p| p.extension().is_some_and(|ext| ext == "json"));
    paths.sort();
    paths.iter().map(|p| read_family(p)).collect()
}

fn find(families: &[Family], name: &str, tint: Tint) -> Result<Theme, ThemeError> {
    families
        .iter()
        .flat_map(|f| &f.themes)
        .find(|v| v.name == name)
        .ok_or_else(|| ThemeError::NotFound(name.into()))?
        .to_theme(tint)
}

impl Variant {
    fn to_theme(&self, tint: Tint) -> Result<Theme, ThemeError> {
        let bad = |key: &str, value: &str| ThemeError::BadColor {
            theme: self.name.clone(),
            key: key.into(),
            value: value.into(),
        };
        let (key, value) = self.lookup(&["editor.background"])?;
        let bg = parse_hex(value).ok_or_else(|| bad(key, value))?.0;
        let blended = |key: &str, value: &str| -> Result<[u8; 3], ThemeError> {
            let (rgb, alpha) = parse_hex(value).ok_or_else(|| bad(key, value))?;
            Ok(blend(rgb, bg, alpha))
        };
        let color = |keys: &[&'static str]| {
            let (key, value) = self.lookup(keys)?;
            blended(key, value)
        };

        let foreground = color(&["editor.foreground"])?;
        let active_line = color(&["editor.active_line.background"])?;
        let selection = match self.selection() {
            Some(value) => blended("players[0].selection", value)?,
            None => blend(foreground, active_line, tint.selection),
        };
        let added = color(&["version_control.added", "created"])?;
        let deleted = color(&["version_control.deleted", "deleted"])?;
        let syntax = self
            .style
            .syntax
            .iter()
            .map(|(key, s)| {
                let mut style = Style::new();
                if let Some(value) = &s.color {
                    style = style.fg(rgb(blended(key, value)?));
                }
                if s.font_style.as_deref() == Some("italic") {
                    style = style.add_modifier(Modifier::ITALIC);
                }
                if s.font_weight.is_some_and(|w| w >= 700.0) {
                    style = style.add_modifier(Modifier::BOLD);
                }
                Ok((key.clone(), style))
            })
            .collect::<Result<_, ThemeError>>()?;

        Ok(Theme {
            name: self.name.clone(),
            background: rgb(bg),
            foreground: rgb(foreground),
            line_number: rgb(color(&["editor.line_number"])?),
            active_line_background: rgb(active_line),
            selection: rgb(selection),
            added: rgb(added),
            deleted: rgb(deleted),
            modified: rgb(color(&["version_control.modified", "modified"])?),
            conflict: rgb(color(&["version_control.conflict", "conflict"])?),
            line_added_bg: rgb(blend(added, bg, tint.line)),
            line_deleted_bg: rgb(blend(deleted, bg, tint.line)),
            token_added_bg: rgb(blend(added, bg, tint.token)),
            token_deleted_bg: rgb(blend(deleted, bg, tint.token)),
            syntax,
        })
    }

    fn selection(&self) -> Option<&str> {
        self.style
            .colors
            .get("players")?
            .get(0)?
            .get("selection")?
            .as_str()
    }

    /// First of `keys` with a non-null value, so `version_control.*` can fall back.
    fn lookup(&self, keys: &[&'static str]) -> Result<(&'static str, &str), ThemeError> {
        keys.iter()
            .find_map(|&k| Some((k, self.style.colors.get(k)?.as_str()?)))
            .ok_or_else(|| ThemeError::MissingKey {
                theme: self.name.clone(),
                key: keys[0].into(),
            })
    }
}

/// `#RRGGBB` or `#RRGGBBAA` → (rgb, alpha in 0..=1).
fn parse_hex(value: &str) -> Option<([u8; 3], f32)> {
    let hex = value.strip_prefix('#')?;
    if !matches!(hex.len(), 6 | 8) || !hex.is_ascii() {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    let alpha = if hex.len() == 8 { byte(6)? } else { 255 };
    Some(([byte(0)?, byte(2)?, byte(4)?], f32::from(alpha) / 255.0))
}

fn blend(fg: [u8; 3], bg: [u8; 3], alpha: f32) -> [u8; 3] {
    std::array::from_fn(|i| {
        (f32::from(fg[i]) * alpha + f32::from(bg[i]) * (1.0 - alpha)).round() as u8
    })
}

fn rgb([r, g, b]: [u8; 3]) -> Color {
    Color::Rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TINT: Tint = Tint {
        line: 0.1,
        token: 0.2,
        selection: 0.15,
    };

    #[test]
    fn bundled_family_parses() {
        assert_eq!(bundled_family().themes.len(), 4);
    }

    #[test]
    fn moon_diff_colors_fall_back_to_created() {
        let moon = Theme::bundled_default(TINT);
        assert_eq!(moon.background, Color::Rgb(0x22, 0x24, 0x36));
        assert_eq!(moon.added, Color::Rgb(0xc3, 0xe8, 0x8d));
    }

    #[test]
    fn alpha_blends_onto_background() {
        let (rgb, alpha) = parse_hex("#ffffff80").unwrap();
        assert_eq!(blend(rgb, [0, 0, 0], alpha), [128, 128, 128]);
    }

    #[test]
    fn font_style_and_weight_map_to_modifiers() {
        let family: Family = serde_json::from_str(
            r##"{"themes": [{"name": "T", "appearance": "dark", "style": {
                "editor.background": "#000000", "editor.foreground": "#ffffff",
                "editor.line_number": "#808080", "editor.active_line.background": "#101010",
                "created": "#00ff00", "deleted": "#ff0000", "modified": "#ffff00", "conflict": "#ff00ff",
                "syntax": {
                    "comment": {"color": "#808080", "font_style": "italic", "font_weight": null},
                    "keyword": {"color": null, "font_style": null, "font_weight": 700}
                }}}]}"##,
        )
        .unwrap();
        let theme = find(&[family], "T", TINT).unwrap();
        assert_eq!(
            theme.syntax["comment"],
            Style::new()
                .fg(Color::Rgb(128, 128, 128))
                .add_modifier(Modifier::ITALIC)
        );
        assert_eq!(
            theme.syntax["keyword"],
            Style::new().add_modifier(Modifier::BOLD)
        );
    }
}
