//! The bundled theme family, compiled in.

/// Zed theme family JSON; its variants are also selectable by name.
pub const FAMILY: &str = include_str!("tokyo-night.json");
/// Used when `ui.theme` is unset or fails to resolve.
pub const VARIANT: &str = "Tokyo Night Moon";
