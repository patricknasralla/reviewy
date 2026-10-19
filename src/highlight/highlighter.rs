//! tree-sitter-highlight to per-line style spans.

use std::cell::OnceCell;
use std::ops::Range;

use ratatui::style::Style;
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent};

use super::languages::Language;
use super::theme::Theme;
use crate::diff::model::FileDiff;
use crate::git::{ChangedFile, FileStatus};

/// Capture names the theme has no key for, and the theme key they use instead.
const REMAP: &[(&str, &str)] = &[
    ("escape", "string.escape"), // rust, python
];

#[derive(Debug, Clone)]
pub struct StyledSpan {
    /// Byte range into the tab-expanded line.
    pub range: Range<usize>,
    pub style: Style,
}

/// Highlights files with one theme; each language's grammar is configured on first use.
pub struct Highlighter {
    /// Theme syntax keys, then `REMAP` aliases.
    names: Vec<String>,
    styles: Vec<Style>,
    configs: [OnceCell<HighlightConfiguration>; 5],
}

impl Highlighter {
    pub fn new(theme: &Theme) -> Self {
        let (mut names, mut styles): (Vec<_>, Vec<_>) =
            theme.syntax.iter().map(|(k, s)| (k.clone(), *s)).unzip();
        for (alias, key) in REMAP {
            if let Some(i) = names.iter().position(|n| n == key) {
                names.push(alias.to_string());
                styles.push(styles[i]);
            }
        }
        Self {
            names,
            styles,
            configs: Default::default(),
        }
    }

    /// Fills `highlights` on each side whose path has a known language (old path for renames).
    pub fn apply(&self, diff: &mut FileDiff, file: &ChangedFile) {
        let old_path = match &file.status {
            FileStatus::Renamed { from } => from,
            _ => &file.path,
        };
        for (side, path) in [(&mut diff.lhs, old_path), (&mut diff.rhs, &file.path)] {
            if let (Some(side), Some(lang)) = (side, Language::detect(path)) {
                side.highlights = self.highlight(lang, &side.lines);
            }
        }
    }

    fn config(&self, lang: Language) -> &HighlightConfiguration {
        self.configs[lang as usize].get_or_init(|| lang.config(&self.names))
    }

    /// One span list per line; plain text (no spans) if highlighting fails.
    fn highlight(&self, lang: Language, lines: &[String]) -> Vec<Vec<StyledSpan>> {
        let source = lines.join("\n");
        self.spans(lang, &source, lines.len()).unwrap_or_else(|e| {
            tracing::warn!(?lang, %e, "highlighting failed");
            vec![Vec::new(); lines.len()]
        })
    }

    fn spans(
        &self,
        lang: Language,
        source: &str,
        line_count: usize,
    ) -> Result<Vec<Vec<StyledSpan>>, tree_sitter_highlight::Error> {
        let mut spans = vec![Vec::new(); line_count];
        if line_count == 0 {
            return Ok(spans);
        }
        let starts: Vec<usize> = std::iter::once(0)
            .chain(source.match_indices('\n').map(|(i, _)| i + 1))
            .collect();
        let mut highlighter = tree_sitter_highlight::Highlighter::new();
        let events =
            highlighter.highlight(self.config(lang), source.as_bytes(), None, None, |name| {
                (name == "rust").then(|| self.config(Language::Rust))
            })?;

        let mut stack = Vec::new();
        let mut line = 0;
        for event in events {
            match event? {
                HighlightEvent::HighlightStart(h) => stack.push(self.styles[h.0]),
                HighlightEvent::HighlightEnd => {
                    stack.pop();
                }
                HighlightEvent::Source { start, end } => {
                    // Split the source range at newlines into per-line spans.
                    let mut pos = start;
                    while pos < end {
                        while starts.get(line + 1).is_some_and(|&s| s <= pos) {
                            line += 1;
                        }
                        let line_end = starts.get(line + 1).map_or(source.len(), |s| s - 1);
                        let seg_end = end.min(line_end);
                        if let Some(&style) = stack.last()
                            && seg_end > pos
                        {
                            let range = pos - starts[line]..seg_end - starts[line];
                            spans[line].push(StyledSpan { range, style });
                        }
                        // Step over the newline.
                        pos = if seg_end == line_end {
                            line_end + 1
                        } else {
                            seg_end
                        };
                    }
                }
            }
        }
        Ok(spans)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Highlights `src` and asserts that `text` gets the style of theme key `key`.
    fn assert_styled(lang: Language, src: &str, text: &str, key: &str) {
        let theme = Theme::bundled_default();
        let lines: Vec<String> = src.lines().map(String::from).collect();
        let spans = Highlighter::new(&theme).highlight(lang, &lines);
        assert_eq!(spans.len(), lines.len());
        for (line, spans) in lines.iter().zip(&spans) {
            assert!(spans.iter().all(|s| s.range.end <= line.len()), "{spans:?}");
        }
        let found = lines.iter().zip(&spans).any(|(line, spans)| {
            spans
                .iter()
                .any(|s| line[s.range.clone()] == *text && s.style == theme.syntax[key])
        });
        assert!(found, "{text:?} not styled as {key}: {spans:?}");
    }

    #[test]
    fn rust() {
        let src = "/* multi\n   line */\nfn main() { let s = \"a\\n\"; }";
        assert_styled(Language::Rust, src, "fn", "keyword");
        assert_styled(Language::Rust, src, "\\n", "string.escape");
    }

    #[test]
    fn python() {
        assert_styled(Language::Python, "def f():\n    return 1", "def", "keyword");
    }

    #[test]
    fn csharp() {
        assert_styled(Language::CSharp, "class A {\n}", "class", "keyword");
    }

    #[test]
    fn typescript() {
        assert_styled(
            Language::TypeScript,
            "const x: number = 1;\n// done",
            "const",
            "keyword",
        );
    }

    #[test]
    fn tsx() {
        assert_styled(Language::Tsx, "const a = <div />;\n", "const", "keyword");
    }
}
