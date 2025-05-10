//! UI styling utilities for consistent visual presentation
//!
//! This module provides a centralized set of styling utilities to ensure
//! consistent visual presentation throughout the application. It defines
//! semantic color palettes, standardized output prefixes/symbols, and
//! helper functions for printing styled messages.

use owo_colors::OwoColorize;
use std::fmt::Display;

/// Semantic color palette for consistent UI styling
pub struct Colors;

impl Colors {
    /// Success color (green)
    pub fn success<D: Display>(text: D) -> String {
        format!("{}", text.green())
    }

    /// Error/Failure color (red)
    pub fn error<D: Display>(text: D) -> String {
        format!("{}", text.red())
    }

    /// Warning color (yellow)
    pub fn warning<D: Display>(text: D) -> String {
        format!("{}", text.yellow())
    }

    /// Informational/Progress color (cyan)
    pub fn info<D: Display>(text: D) -> String {
        format!("{}", text.cyan())
    }

    /// User Input/Prompts color (magenta)
    pub fn prompt<D: Display>(text: D) -> String {
        format!("{}", text.magenta())
    }

    /// Code/Paths/Commands color (bright black)
    pub fn code<D: Display>(text: D) -> String {
        format!("{}", text.bright_black())
    }

    /// Emphasis (bold)
    pub fn emphasis<D: Display>(text: D) -> String {
        format!("{}", text.bold())
    }
}

/// Standardized output prefixes/symbols
pub struct Symbols;

impl Symbols {
    /// Success symbol (✓)
    pub fn success() -> &'static str {
        "✓"
    }

    /// Error symbol (❌)
    pub fn error() -> &'static str {
        "❌"
    }

    /// Warning symbol (⚠️)
    pub fn warning() -> &'static str {
        "⚠️"
    }

    /// Info symbol (ℹ️)
    pub fn info() -> &'static str {
        "ℹ️"
    }

    /// Progress symbol (⚙️)
    pub fn progress() -> &'static str {
        "⚙️"
    }

    /// Cleanup symbol (🧹)
    pub fn cleanup() -> &'static str {
        "🧹"
    }

    /// Prompt symbol (❓)
    pub fn prompt() -> &'static str {
        "❓"
    }

    /// Check symbol (🔍)
    pub fn check() -> &'static str {
        "🔍"
    }

    /// Build symbol (🔨)
    pub fn build() -> &'static str {
        "🔨"
    }

    /// Activate symbol (🚀)
    pub fn activate() -> &'static str {
        "🚀"
    }

    /// Success check symbol (✅)
    pub fn success_check() -> &'static str {
        "✅"
    }

    /// Skip symbol (⏭️)
    pub fn skip() -> &'static str {
        "⏭️"
    }
}

/// Helper functions for printing styled messages
pub struct Print;

impl Print {
    /// Print a success message
    pub fn success(msg: &str) {
        eprintln!("{} {}", Colors::success(Symbols::success()), msg);
    }

    /// Print an error message
    pub fn error(msg: &str) {
        eprintln!("{} {}", Colors::error(Symbols::error()), msg);
    }

    /// Print a warning message
    pub fn warning(msg: &str) {
        eprintln!("{} {}", Colors::warning(Symbols::warning()), msg);
    }

    /// Print an info message
    pub fn info(msg: &str) {
        eprintln!("{} {}", Colors::info(Symbols::info()), msg);
    }

    /// Print a prompt message
    pub fn prompt(msg: &str) {
        eprintln!("{} {}", Colors::prompt(Symbols::prompt()), msg);
    }

    /// Print a section header
    pub fn section(title: impl Display) {
        eprintln!("\n{}", Colors::info(format!("❯ {}", Colors::emphasis(title))));
    }
}

/// Create a styled spinner message with appropriate prefix
pub fn spinner_message(stage: &str, action: &str) -> String {
    format!("[{} {}] {}", stage_symbol(stage), stage, action)
}

/// Create a styled success message with appropriate prefix
pub fn success_message(stage: &str, message: &str) -> String {
    format!("[{} {}] {}", Symbols::success_check(), stage, message)
}

/// Get the appropriate symbol for a stage
fn stage_symbol(stage: &str) -> &'static str {
    match stage.to_lowercase().as_str() {
        "git" => Symbols::check(),
        "parse" => "🧩",
        "lint" => "🎨",
        "eval" => "⚙️",
        "dry run" => "🧪",
        "build" => Symbols::build(),
        "diff" => Symbols::check(),
        "activate" => Symbols::activate(),
        "boot" => "⚙️",
        "clean" => Symbols::cleanup(),
        "profile" => "📦",
        "user" => "👤",
        "system" => "🖥️",
        _ => Symbols::info(),
    }
}

/// Create a styled separator
pub fn separator() -> String {
    Colors::info("─".repeat(80)).to_string()
}

/// Create a styled header
pub fn header(title: &str) -> String {
    format!(
        "{}\n{}\n{}",
        separator(),
        format!(" {} ", title).bold(),
        separator()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_colors() {
        // Just test that the functions compile and return something
        let _success = Colors::success("Success").to_string();
        let _error = Colors::error("Error").to_string();
        let _warning = Colors::warning("Warning").to_string();
        let _info = Colors::info("Info").to_string();
        let _prompt = Colors::prompt("Prompt").to_string();
        let _code = Colors::code("Code").to_string();
        let _emphasis = Colors::emphasis("Emphasis").to_string();
    }

    #[test]
    fn test_symbols() {
        assert_eq!(Symbols::success(), "✓");
        assert_eq!(Symbols::error(), "❌");
        assert_eq!(Symbols::warning(), "⚠️");
        assert_eq!(Symbols::info(), "ℹ️");
        assert_eq!(Symbols::progress(), "⚙️");
        assert_eq!(Symbols::cleanup(), "🧹");
        assert_eq!(Symbols::prompt(), "❓");
    }

    #[test]
    fn test_spinner_message() {
        let msg = spinner_message("Build", "Building configuration...");
        assert!(msg.contains("Build"));
        assert!(msg.contains("Building configuration..."));
    }

    #[test]
    fn test_success_message() {
        let msg = success_message("Build", "Configuration built successfully");
        assert!(msg.contains("Build"));
        assert!(msg.contains("Configuration built successfully"));
        assert!(msg.contains("✅"));
    }
}