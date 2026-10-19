//! Application state and actions.

pub mod actions;
pub mod state;

#[derive(Default)]
pub struct App {
    pub dirty: bool,
    pub should_quit: bool,
}
