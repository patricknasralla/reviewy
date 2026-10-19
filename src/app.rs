//! Application state and actions.

pub mod actions;
pub mod state;

use crate::git::ChangedFile;
use crate::highlight::{Highlighter, Theme};

pub struct App {
    pub dirty: bool,
    pub should_quit: bool,
    #[expect(dead_code, reason = "read by task 07")]
    pub base_commit: String,
    #[expect(dead_code, reason = "read by task 07")]
    pub files: Vec<ChangedFile>,
    #[expect(dead_code, reason = "read by task 07")]
    pub theme: Theme,
    #[expect(dead_code, reason = "read by task 07")]
    pub highlighter: Highlighter,
    /// Status line message; set at startup for a theme fallback.
    #[expect(dead_code, reason = "read by task 07")]
    pub status: Option<String>,
}
