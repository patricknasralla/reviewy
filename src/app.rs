//! Application state and actions.

pub mod actions;
pub mod state;

use crate::git::ChangedFile;

#[derive(Default)]
pub struct App {
    pub dirty: bool,
    pub should_quit: bool,
    #[expect(dead_code, reason = "read by task 07")]
    pub base_commit: String,
    #[expect(dead_code, reason = "read by task 07")]
    pub files: Vec<ChangedFile>,
}
