//! Review comments: model, persistence and markdown output.

pub mod model;
pub mod render_md;
#[allow(dead_code, reason = "used by task 10; remove then")]
pub mod store;

use std::io;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum CommentsError {
    #[error("{path}: {source}", path = path.display())]
    Io { path: PathBuf, source: io::Error },
    #[error("invalid {path}: {source}", path = path.display())]
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("unsupported comments.json version {0}; expected {expected}", expected = model::VERSION)]
    Version(u32),
}
