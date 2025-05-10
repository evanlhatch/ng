use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;
use crate::util::is_stdout_tty;
use crate::ui_style::{Colors, Symbols, spinner_message, success_message};

/// Creates and returns a new spinner with the given message.
///
/// If stdout is not a TTY, the spinner will be hidden but the message will still be printed.
///
/// # Arguments
///
/// * `message` - The message to display next to the spinner.
///
/// # Returns
///
/// * `ProgressBar` - The created spinner.
pub fn start_spinner(message: &str) -> ProgressBar {
    let pb = if is_stdout_tty() {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                .template("{spinner:.cyan} {msg}")
                .unwrap(),
        );
        pb.enable_steady_tick(Duration::from_millis(80));
        pb
    } else {
        // Create a hidden spinner if not in a TTY
        let pb = ProgressBar::hidden();
        tracing::info!("{}", message);
        pb
    };
    
    pb.set_message(message.to_string());
    pb
}

/// Updates the message of an existing spinner.
///
/// # Arguments
///
/// * `spinner` - The spinner to update.
/// * `message` - The new message to display.
pub fn update_spinner_message(spinner: &ProgressBar, message: &str) {
    if !is_stdout_tty() {
        tracing::info!("{}", message);
    }
    spinner.set_message(message.to_string());
}

/// Finishes a spinner with a success message.
///
/// # Arguments
///
/// * `spinner` - The spinner to finish.
/// * `message` - The success message to display.
pub fn finish_spinner_success(spinner: &ProgressBar, message: &str) {
    if is_stdout_tty() {
        spinner.finish_with_message(format!("{} {}", Colors::success(Symbols::success()), message));
    } else {
        tracing::info!("{} {}", Colors::success(Symbols::success()), message);
    }
}

/// Finishes a spinner with a failure message.
///
/// # Arguments
///
/// * `spinner` - The spinner to finish.
pub fn finish_spinner_fail(spinner: &ProgressBar) {
    if is_stdout_tty() {
        spinner.finish_with_message(format!("{} Operation failed", Colors::error("✗")));
    } else {
        tracing::error!("{} Operation failed", Colors::error("✗"));
    }
}

/// Creates a spinner with a standardized message format
///
/// # Arguments
///
/// * `stage` - The stage name (e.g., "Git", "Parse", "Build")
/// * `action` - The action being performed (e.g., "Checking status", "Building configuration")
///
/// # Returns
///
/// * `ProgressBar` - The created spinner.
pub fn start_stage_spinner(stage: &str, action: &str) -> ProgressBar {
    start_spinner(&spinner_message(stage, action))
}

/// Finishes a spinner with a standardized success message format
///
/// # Arguments
///
/// * `spinner` - The spinner to finish.
/// * `stage` - The stage name (e.g., "Git", "Parse", "Build")
/// * `message` - The success message.
pub fn finish_stage_spinner_success(spinner: &ProgressBar, stage: &str, message: &str) {
    finish_spinner_success(spinner, &success_message(stage, message))
}