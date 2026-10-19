//! Command-line arguments.

use clap::Parser;

/// Review uncommitted changes side by side and leave comments for an AI agent.
#[derive(Parser)]
#[command(version, about)]
pub struct Cli {
    /// Reserved for git diff-style arguments (not supported in v1).
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub diff_args: Vec<String>,
}
