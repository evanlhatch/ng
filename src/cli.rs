use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[clap(author, version, about)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Format Nix files in the current directory and subdirectories.
    Format(FormatArgs),
    // Future: Add other commands here (lint, check-lock, etc.)
}

#[derive(Debug, Args)]
/// Format Nix files in the current directory and subdirectories.
pub struct FormatArgs {
    /// Whether to apply the formatting changes (default: check only).
    #[clap(long, short, action)]
    pub apply: bool,

    /// The path to start formatting from (defaults to the current directory)
    #[clap(default_value = ".")]
    pub path: String,
}