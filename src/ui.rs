//! Top-level drawing.

pub mod diff_view;
pub mod editor;
pub mod explorer;
pub mod modals;
pub mod status;

use ratatui::Frame;

use crate::app::App;

pub fn draw(_frame: &mut Frame, _app: &App) {}
