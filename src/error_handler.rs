use crate::util::{add_verbosity_flags, run_cmd};
use crate::ui_style::{Colors, Symbols, separator, header};
use color_eyre::eyre::{Context, Result};
use lazy_static::lazy_static;
use owo_colors::OwoColorize;
use regex::Regex;
use std::process::Command;
use tracing::{error, info};

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
        Colors::warning("─".repeat(80)),
        Colors::warning(format!(" BUILD LOG FOR: {} ", Colors::emphasis(drv_path.to_string()))),
        Colors::warning("─".repeat(80)),
        log_content.trim(),
        Colors::warning("─".repeat(80)),
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
    // Print header with consistent styling
    eprintln!("\n{}", header(&format!("NH COMMAND ABORTED at Stage: {}", Colors::emphasis(stage.to_string()))));
    
    // Print reason with improved styling
    error!("{} {}", Symbols::error(), Colors::emphasis(reason.to_string()));
    
    // Print separator
    eprintln!("{}", separator());
    
    // Print details if provided
    if let Some(ref d) = details {
        // For syntax errors, we'll enhance the formatting
        if stage == "Parse Check" {
            let enhanced_details = enhance_syntax_error_output(d);
            eprintln!("\n{}\n", enhanced_details);
        } else {
            eprintln!("\n{}\n", d.trim());
        }
    }
    
    // Print recommendations if any
    if !recommendations.is_empty() {
        // For syntax errors, we'll generate more specific recommendations
        let final_recommendations = if stage == "Parse Check" && details.is_some() {
            generate_syntax_error_recommendations(details.as_ref().unwrap())
        } else {
            recommendations
        };
        
        if !final_recommendations.is_empty() {
            info!("{} {}", Symbols::info(), Colors::info(Colors::emphasis("Possible Issues & Recommendations:")));
            for rec in final_recommendations {
                info!("   • {}", rec);
            }
        }
    }
}

/// Enhances the formatting of syntax error output
pub fn enhance_syntax_error_output(error_details: &str) -> String {
    let mut enhanced = String::new();
    let mut file_count = 0;
    
    // Convert to owned string to avoid lifetime issues
    let error_details_owned = error_details.to_string();
    for line in error_details_owned.lines() {
        if line.starts_with("Error in ") {
            if file_count > 0 {
                enhanced.push_str("\n");
            }
            file_count += 1;
            
            // Extract file path and highlight it
            let file_path = line.trim_start_matches("Error in ").trim_end_matches(": ");
            enhanced.push_str(&format!("{}:\n", Colors::info(Colors::emphasis(file_path))));
        } else if line.contains("error: syntax error") {
            // Highlight the error message with improved styling
            enhanced.push_str(&format!("{} {}\n",
                Symbols::error(),
                Colors::error(Colors::emphasis(line))));
        } else if line.contains("at ") && line.contains(".nix:") {
            // Highlight the location with improved styling
            enhanced.push_str(&format!("{} {}\n",
                Symbols::info(),
                Colors::warning(line)));
        } else if line.contains("|") {
            // Code snippet - keep as is but indent slightly and highlight
            if line.contains("^") {
                // This is the error indicator line
                enhanced.push_str(&format!("    {}\n", Colors::error(line)));
            } else {
                enhanced.push_str(&format!("    {}\n", Colors::code(line)));
            }
        } else {
            enhanced.push_str(&format!("{}\n", line));
        }
    }
    
    enhanced
}

/// Generates specific recommendations based on syntax error messages
pub fn generate_syntax_error_recommendations(error_details: &str) -> Vec<String> {
    let mut recommendations = Vec::new();
    
    // Common syntax errors and their recommendations
    if error_details.contains("unexpected end of file, expecting INHERIT")
       || error_details.contains("unexpected end of file, expecting }")
    {
        recommendations.push("Add missing closing brace '}' to complete the attribute set".to_string());
    } else if error_details.contains("unexpected end of file, expecting ]")
    {
        recommendations.push("Add missing closing bracket ']' to complete the list".to_string());
    } else if error_details.contains("unexpected end of file, expecting )")
    {
        recommendations.push("Add missing closing parenthesis ')' to complete the expression".to_string());
    } else if error_details.contains("unexpected ;")
    {
        recommendations.push("Remove extra semicolon ';' or add an expression after it".to_string());
    } else if error_details.contains("unexpected =")
    {
        recommendations.push("Check attribute name before '=' or ensure proper nesting of attribute sets".to_string());
    } else if error_details.contains("unexpected }")
    {
        recommendations.push("Remove extra closing brace '}' or add a matching opening brace".to_string());
    } else if error_details.contains("unexpected ]")
    {
        recommendations.push("Remove extra closing bracket ']' or add a matching opening bracket".to_string());
    } else if error_details.contains("unexpected )")
    {
        recommendations.push("Remove extra closing parenthesis ')' or add a matching opening parenthesis".to_string());
    } else if error_details.contains("unexpected in")
    {
        recommendations.push("Ensure 'let' expression has a matching 'in' keyword".to_string());
    } else if error_details.contains("unexpected let")
    {
        recommendations.push("Ensure 'let' expression is properly formatted with 'in' keyword".to_string());
    } else
    {
        // Generic recommendation if we can't identify the specific error
        recommendations.push("Fix the syntax error according to the error message".to_string());
    }
    
    // Add general recommendations
    recommendations.push("Consider using a Nix formatter like 'alejandra' or 'nixpkgs-fmt' to automatically fix formatting issues".to_string());
    
    recommendations
}