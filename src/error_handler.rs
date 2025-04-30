use crate::util::{add_verbosity_flags, run_cmd};
use color_eyre::eyre::{Context, Result};
use lazy_static::lazy_static;
use owo_colors::OwoColorize;
use regex::Regex;
use std::process::Command;
use tracing::{error, info, warn};

// Define Regexes for parsing error messages
lazy_static! {
    // Match Nix evaluation errors: error: <message> at <file>:<line>:<column>
    static ref RE_EVAL_ERROR: Regex = Regex::new(r"error: (.*) at (.*?):(\d+):(\d+)").unwrap();
    
    // Match Nix builder failures: error: builder for '/nix/store/...drv' failed
    static ref RE_BUILDER_FAILED: Regex = Regex::new(r"error: builder for '(/nix/store/.*?\.drv)' failed").unwrap();
    
    // Match common error patterns in build logs
    static ref RE_MISSING_PACKAGE: Regex = Regex::new(r"package.*not found").unwrap();
    static ref RE_PERMISSION_ERROR: Regex = Regex::new(r"permission denied").unwrap();
    static ref RE_NETWORK_ERROR: Regex = Regex::new(r"(network|connection|timeout)").unwrap();
}

/// Parses Nix evaluation errors from stderr.
///
/// # Arguments
///
/// * `stderr` - The stderr output from a Nix command.
///
/// # Returns
///
/// * `Option<(String, String, usize, usize)>` - If an error is found, returns a tuple of
///   (error message, file path, line number, column number).
pub fn parse_nix_eval_error(stderr: &str) -> Option<(String, String, usize, usize)> {
    RE_EVAL_ERROR.captures(stderr).map(|caps| {
        (
            caps[1].to_string(),
            caps[2].to_string(),
            caps[3].parse().unwrap_or(0),
            caps[4].parse().unwrap_or(0),
        )
    })
}

/// Fetches the Nix trace for a failed evaluation.
///
/// # Arguments
///
/// * `flake_ref` - The flake reference.
/// * `attribute_path_slice` - The attribute path as a slice of strings.
/// * `verbose_count` - The verbosity level.
///
/// # Returns
///
/// * `Result<String>` - The trace output or an error.
pub fn fetch_nix_trace(flake_ref: &str, attribute_path_slice: &[String], verbose_count: u8) -> Result<String> {
    info!("-> Attempting to fetch evaluation trace for {}#{}...", flake_ref, attribute_path_slice.join("."));
    
    let mut cmd = Command::new("nix");
    cmd.arg("eval");
    cmd.arg("--show-trace");
    
    // Add verbosity flags
    add_verbosity_flags(&mut cmd, verbose_count);
    
    // Add flake reference and attribute path
    let attr_path = format!("{}#{}", flake_ref, attribute_path_slice.join("."));
    cmd.arg(attr_path);
    
    // Run the command and capture output
    let output = run_cmd(&mut cmd)
        .context("Failed to run nix eval --show-trace")?;
    
    Ok(String::from_utf8_lossy(&output.stderr).to_string())
}

/// Finds failed derivation paths in Nix build stderr.
///
/// # Arguments
///
/// * `stderr_summary` - The stderr output from a Nix build command.
///
/// # Returns
///
/// * `Vec<String>` - A list of failed derivation paths.
pub fn find_failed_derivations(stderr_summary: &str) -> Vec<String> {
    RE_BUILDER_FAILED
        .captures_iter(stderr_summary)
        .map(|caps| caps[1].to_string())
        .collect()
}

/// Fetches and formats the build log for a failed derivation.
///
/// # Arguments
///
/// * `drv_path` - The path to the derivation.
/// * `verbose_count` - The verbosity level.
///
/// # Returns
///
/// * `Result<String>` - The formatted log or an error.
pub fn fetch_and_format_nix_log(drv_path: &str, verbose_count: u8) -> Result<String> {
    info!("-> Fetching build log for {} using 'nix log'...", drv_path.cyan());
    
    let mut cmd = Command::new("nix");
    cmd.args(["log", drv_path]);
    
    // Add verbosity flags
    add_verbosity_flags(&mut cmd, verbose_count);
    
    // Run the command and capture output
    let output = run_cmd(&mut cmd)
        .context("Failed to run nix log")?;
    
    let log_content = String::from_utf8_lossy(&output.stdout).to_string();
    
    // Format the log with headers and footers
    let formatted_log = format!(
        "{}
{}
{}
{}
{}",
        "─".repeat(80).yellow(),
        format!(" BUILD LOG FOR: {} ", drv_path).yellow().bold(),
        "─".repeat(80).yellow(),
        log_content.trim(),
        "─".repeat(80).yellow(),
    );
    
    Ok(formatted_log)
}

/// Scans a log for common issues and provides recommendations.
///
/// # Arguments
///
/// * `log_content` - The log content to scan.
///
/// # Returns
///
/// * `Vec<String>` - A list of recommendations.
pub fn scan_log_for_recommendations(log_content: &str) -> Vec<String> {
    let mut recommendations = Vec::new();
    
    // Check for common error patterns
    if RE_MISSING_PACKAGE.is_match(log_content) {
        recommendations.push("A package dependency appears to be missing. Check your inputs and package names.".to_string());
    }
    
    if RE_PERMISSION_ERROR.is_match(log_content) {
        recommendations.push("Permission errors detected. Check file permissions or if you need elevated privileges.".to_string());
    }
    
    if RE_NETWORK_ERROR.is_match(log_content) {
        recommendations.push("Network-related errors detected. Check your internet connection or proxy settings.".to_string());
    }
    
    // Add general recommendations
    if log_content.contains("error: attribute") {
        recommendations.push("An attribute error was detected. Verify that all attribute paths exist in your configuration.".to_string());
    }
    
    if log_content.contains("syntax error") {
        recommendations.push("Syntax errors detected. Check for missing semicolons, brackets, or other syntax issues.".to_string());
    }
    
    // If no specific recommendations, add a generic one
    if recommendations.is_empty() {
        recommendations.push("Review the full log for specific error details.".to_string());
        recommendations.push("Run 'nh doctor' to check your Nix installation.".to_string());
    }
    
    recommendations
}

/// Reports a failure with structured error information.
///
/// # Arguments
///
/// * `stage` - The stage where the failure occurred.
/// * `reason` - The reason for the failure.
/// * `details` - Optional details (like logs or traces).
/// * `recommendations` - A list of recommendations to fix the issue.
pub fn report_failure(stage: &str, reason: &str, details: Option<String>, recommendations: Vec<String>) {
    let width = 80;
    let border_str = "─".repeat(width);
    
    // Print header
    eprintln!("{}", border_str.yellow());
    error!("│ {:<width$} │", format!("*** NH COMMAND ABORTED at Stage: {} ***", stage.bold()), width = width - 2);
    eprintln!("{}", border_str.yellow());
    
    // Print reason
    error!("│ {:<width$} │", format!("-> Reason: {}", reason.bold()), width = width - 2);
    
    // Print footer
    eprintln!("{}", border_str.yellow());
    
    // Print details if provided
    if let Some(d) = details {
        eprintln!("\n{}\n", d.trim());
    }
    
    // Print recommendations if any
    if !recommendations.is_empty() {
        info!("{}", "-> Possible Issues & Recommendations:".blue().bold());
        for rec in recommendations {
            info!("   - {}", rec);
        }
        println!();
    }
}