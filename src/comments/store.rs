//! comments.json load and save.

use std::fs;
use std::io;
use std::path::Path;

use serde::Deserialize;

use super::CommentsError;
use super::model::{CommentSet, VERSION};
use super::render_md;

const JSON: &str = "comments.json";
const MD: &str = "comments.md";

/// Reads `dir/comments.json`; `None` if it doesn't exist.
pub fn load(dir: &Path) -> Result<Option<CommentSet>, CommentsError> {
    #[derive(Deserialize)]
    struct Header {
        version: u32,
    }

    let path = dir.join(JSON);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(CommentsError::Io { path, source }),
    };
    let json_err = |source| CommentsError::Json {
        path: path.clone(),
        source,
    };
    // Check the version first, so a future schema reports that rather than a field error.
    let header: Header = serde_json::from_str(&text).map_err(json_err)?;
    if header.version != VERSION {
        return Err(CommentsError::Version(header.version));
    }
    serde_json::from_str(&text).map(Some).map_err(json_err)
}

/// Writes `comments.json` and `comments.md` atomically, or deletes both if the set is empty.
pub fn save(dir: &Path, set: &CommentSet, preamble: &str) -> Result<(), CommentsError> {
    if set.comments.is_empty() {
        for name in [JSON, MD] {
            let path = dir.join(name);
            match fs::remove_file(&path) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => {
                    return Err(CommentsError::Io { path, source: e });
                }
                _ => {}
            }
        }
        return Ok(());
    }
    let json = serde_json::to_string_pretty(set).unwrap();
    write_atomic(dir, JSON, &json)?;
    write_atomic(dir, MD, &render_md::render(set, preamble))
}

fn write_atomic(dir: &Path, name: &str, text: &str) -> Result<(), CommentsError> {
    let tmp = dir.join(format!(".{name}.tmp"));
    let path = dir.join(name);
    fs::write(&tmp, text)
        .and_then(|()| fs::rename(&tmp, &path))
        .map_err(|source| CommentsError::Io { path, source })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comments::model::tests::sample_set;

    #[test]
    fn save_empty_set_deletes_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut set = sample_set();
        save(dir.path(), &set, "").unwrap();
        assert!(dir.path().join(JSON).exists() && dir.path().join(MD).exists());

        set.comments.clear();
        save(dir.path(), &set, "").unwrap();
        assert!(!dir.path().join(JSON).exists() && !dir.path().join(MD).exists());
    }
}
