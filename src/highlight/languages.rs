//! Language registry: extension to grammar and queries.

use std::path::Path;

use tree_sitter_highlight::HighlightConfiguration;
use tree_sitter_javascript as js;
use tree_sitter_typescript as ts;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Rust,
    Python,
    CSharp,
    TypeScript,
    Tsx,
}

impl Language {
    pub fn detect(path: &Path) -> Option<Language> {
        Some(match path.extension()?.to_str()? {
            "rs" => Language::Rust,
            "py" | "pyi" => Language::Python,
            "cs" => Language::CSharp,
            "ts" | "mts" | "cts" => Language::TypeScript,
            "tsx" => Language::Tsx,
            _ => return None,
        })
    }

    /// Fence tag for code blocks in comments.
    pub fn name(self) -> &'static str {
        match self {
            Language::Rust => "rust",
            Language::Python => "python",
            Language::CSharp => "csharp",
            Language::TypeScript => "typescript",
            Language::Tsx => "tsx",
        }
    }

    /// Builds the grammar's highlight configuration, with captures matched against `names`.
    pub(super) fn config(self, names: &[String]) -> HighlightConfiguration {
        // Later patterns win, so the order is JS, then JSX, then TS.
        let (language, highlights, injections, locals): (
            tree_sitter::Language,
            &[&str],
            &str,
            &[&str],
        ) = match self {
            Language::Rust => (
                tree_sitter_rust::LANGUAGE.into(),
                &[tree_sitter_rust::HIGHLIGHTS_QUERY],
                tree_sitter_rust::INJECTIONS_QUERY,
                &[],
            ),
            Language::Python => (
                tree_sitter_python::LANGUAGE.into(),
                &[tree_sitter_python::HIGHLIGHTS_QUERY],
                "",
                &[],
            ),
            Language::CSharp => (
                tree_sitter_c_sharp::LANGUAGE.into(),
                &[tree_sitter_c_sharp::HIGHLIGHTS_QUERY],
                "",
                &[],
            ),
            Language::TypeScript => (
                ts::LANGUAGE_TYPESCRIPT.into(),
                &[js::HIGHLIGHT_QUERY, ts::HIGHLIGHTS_QUERY],
                js::INJECTIONS_QUERY,
                &[js::LOCALS_QUERY, ts::LOCALS_QUERY],
            ),
            Language::Tsx => (
                ts::LANGUAGE_TSX.into(),
                &[
                    js::HIGHLIGHT_QUERY,
                    js::JSX_HIGHLIGHT_QUERY,
                    ts::HIGHLIGHTS_QUERY,
                ],
                js::INJECTIONS_QUERY,
                &[js::LOCALS_QUERY, ts::LOCALS_QUERY],
            ),
        };
        let mut config = HighlightConfiguration::new(
            language,
            self.name(),
            &highlights.join("\n"),
            injections,
            &locals.join("\n"),
        )
        .expect("bundled queries are valid");
        config.configure(names);
        config
    }
}
