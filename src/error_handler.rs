use crate::util::{add_verbosity_flags, run_cmd};
use crate::ui_style::{Colors, Symbols, separator, header};
// Phase 3: Will be uncommented when implementing nil integration
// use crate::nix_analyzer::{NixAnalysisContext, NgDiagnostic, NgSeverity};
use color_eyre::eyre::{Context, Result};
use lazy_static::lazy_static;
use owo_colors::OwoColorize;
use regex::Regex;
use std::process::Command;
use std::sync::Arc;
use tracing::{error, info, debug};

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

// Phase 3: Documentation for nil integration functions
// /// Reports structured diagnostics from nil-syntax and nil-ide
// ///
// /// # Arguments
// ///
// /// * `stage` - The stage where the diagnostics were generated (e.g., "Syntax Check", "Semantic Check")
// /// * `diagnostics` - The list of diagnostics to report
// Phase 3: The following functions will be uncommented when implementing nil integration
/*
/// * `analyzer` - The NixAnalysisContext used to get file content
pub fn report_ng_diagnostics(stage: &str, diagnostics: &[NgDiagnostic], analyzer: &NixAnalysisContext) {
    if diagnostics.is_empty() {
        return;
    }

    // Group diagnostics by file
    let mut diagnostics_by_file: std::collections::HashMap<&std::path::Path, Vec<&NgDiagnostic>> = std::collections::HashMap::new();
    for diag in diagnostics {
        diagnostics_by_file
            .entry(diag.file_path.as_path())
            .or_default()
            .push(diag);
    }

    // Print header with consistent styling
    eprintln!("\n{}", header(&format!("NH COMMAND ABORTED at Stage: {}", Colors::emphasis(stage.to_string()))));
    
    // Print reason with improved styling
    let error_count = diagnostics.iter().filter(|d| matches!(d.severity, NgSeverity::Error)).count();
    let warning_count = diagnostics.iter().filter(|d| matches!(d.severity, NgSeverity::Warning)).count();
    
    let reason = if error_count > 0 {
        format!("Found {} error(s) and {} warning(s) in Nix files", error_count, warning_count)
    } else {
        format!("Found {} warning(s) in Nix files", warning_count)
    };
    
    error!("{} {}", Symbols::error(), Colors::emphasis(reason));
    
    // Print separator
    eprintln!("{}", separator());
    
    // Process each file's diagnostics
    for (file_path, file_diagnostics) in diagnostics_by_file {
        // Print file header
        eprintln!("\n{}:", Colors::info(Colors::emphasis(file_path.display().to_string())));
        
        // Get file content
        let file_content_opt = file_diagnostics.first()
            .and_then(|diag| analyzer.get_file_content(diag.file_id_for_db));
        
        if let Some(file_content) = file_content_opt {
            // Process each diagnostic
            for diag in file_diagnostics {
                // Print diagnostic message with severity
                let severity_symbol = match diag.severity {
                    NgSeverity::Error => Symbols::error(),
                    NgSeverity::Warning => Symbols::warning(),
                    NgSeverity::Info => Symbols::info(),
                    NgSeverity::Hint => Symbols::hint(),
                };
                
                let severity_color = match diag.severity {
                    NgSeverity::Error => Colors::error,
                    NgSeverity::Warning => Colors::warning,
                    NgSeverity::Info => Colors::info,
                    NgSeverity::Hint => Colors::info,
                };
                
                eprintln!("{} {}", severity_symbol, severity_color(Colors::emphasis(&diag.message)));
                
                // Extract line and column information from TextRange
                let range = diag.range;
                let start_line_col = line_col_from_offset(&file_content, range.start().into());
                let end_line_col = line_col_from_offset(&file_content, range.end().into());
                
                if let (Some((start_line, start_col)), Some((end_line, end_col))) = (start_line_col, end_line_col) {
                    eprintln!("{} At {}:{}:{}-{}:{}",
                        Symbols::info(),
                        Colors::warning(file_path.display().to_string()),
                        Colors::warning(start_line.to_string()),
                        Colors::warning(start_col.to_string()),
                        Colors::warning(end_line.to_string()),
                        Colors::warning(end_col.to_string())
                    );
                    
                    // Print code snippet with error highlighting
                    print_code_snippet(&file_content, start_line, start_col, end_line, end_col);
                }
                
                // Generate and print recommendations
                let recommendations = generate_nil_diagnostic_recommendations(diag);
                if !recommendations.is_empty() {
                    info!("   {} {}", Symbols::info(), Colors::info("Suggestions:"));
                    for rec in recommendations {
                        info!("      • {}", rec);
                    }
                }
                
                eprintln!();
            }
        } else {
            // If we couldn't get file content, just print the diagnostics without context
            for diag in file_diagnostics {
                eprintln!("{} {}",
                    match diag.severity {
                        NgSeverity::Error => Symbols::error(),
                        NgSeverity::Warning => Symbols::warning(),
                        NgSeverity::Info => Symbols::info(),
                        NgSeverity::Hint => Symbols::hint(),
                    },
                    match diag.severity {
                        NgSeverity::Error => Colors::error(&diag.message),
                        NgSeverity::Warning => Colors::warning(&diag.message),
                        NgSeverity::Info => Colors::info(&diag.message),
                        NgSeverity::Hint => Colors::info(&diag.message),
                    }
                );
            }
        }
    }
    
    // Print general recommendations
    eprintln!("\n{}", separator());
    info!("{} {}", Symbols::info(), Colors::info(Colors::emphasis("General Recommendations:")));
    info!("   • Fix the issues reported above and try again");
    info!("   • Use --no-preflight to skip these checks if needed");
    if stage == "Semantic Check" {
        info!("   • Consider using a Nix formatter like 'alejandra' or 'nixpkgs-fmt'");
    }
}

/// Converts a byte offset to line and column numbers
///
/// # Arguments
///
/// * `content` - The file content
/// * `offset` - The byte offset
///
/// # Returns
///
/// * `Option<(usize, usize)>` - The line and column numbers (1-based)
fn line_col_from_offset(content: &str, offset: usize) -> Option<(usize, usize)> {
    if offset > content.len() {
        return None;
    }
    
    let mut line = 1;
    let mut col = 1;
    
    for (i, c) in content.char_indices() {
        if i >= offset {
            break;
        }
        
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    
    Some((line, col))
}

/// Prints a code snippet with error highlighting
///
/// # Arguments
///
/// * `content` - The file content
/// * `start_line` - The starting line number (1-based)
/// * `start_col` - The starting column number (1-based)
/// * `end_line` - The ending line number (1-based)
/// * `end_col` - The ending column number (1-based)
fn print_code_snippet(content: &str, start_line: usize, start_col: usize, end_line: usize, end_col: usize) {
    // Get the lines around the error
    let lines: Vec<&str> = content.lines().collect();
    
    // Determine the context range (up to 2 lines before and after)
    let context_start = start_line.saturating_sub(2);
    let context_end = std::cmp::min(end_line + 2, lines.len());
    
    // Calculate the width needed for line numbers
    let line_num_width = context_end.to_string().len();
    
    // Print each line in the context
    for line_num in context_start..=context_end {
        if line_num == 0 || line_num > lines.len() {
            continue;
        }
        
        let line = lines[line_num - 1];
        
        // Format the line number with padding
        let line_num_str = format!("{:>width$}", line_num, width = line_num_width);
        
        // Determine if this is an error line
        let is_error_line = line_num >= start_line && line_num <= end_line;
        
        // Print the line with appropriate styling
        if is_error_line {
            eprintln!("    {} │ {}", Colors::code(line_num_str), Colors::code(line));
        } else {
            eprintln!("    {} │ {}", Colors::code(line_num_str), line);
        }
        
        // Add error indicator for error lines
        if is_error_line {
            let mut indicator = String::new();
            
            // Add spaces before the indicator
            let start_indicator_col = if line_num == start_line { start_col } else { 1 };
            for _ in 0..start_indicator_col + line_num_width + 5 {
                indicator.push(' ');
            }
            
            // Add carets for the error range
            let end_indicator_col = if line_num == end_line { end_col } else { line.len() + 1 };
            let indicator_length = end_indicator_col.saturating_sub(start_indicator_col);
            
            for _ in 0..indicator_length {
                indicator.push('^');
            }
            
            eprintln!("{}", Colors::error(indicator));
        }
    }
}

/// Generates recommendations based on nil diagnostic kind
///
/// # Arguments
///
/// * `diagnostic` - The diagnostic to generate recommendations for
///
/// # Returns
///
/// * `Vec<String>` - A list of recommendations
fn generate_nil_diagnostic_recommendations(diagnostic: &NgDiagnostic) -> Vec<String> {
    let mut recommendations = Vec::new();
    
    // Generate recommendations based on the diagnostic message
    // These are simplified patterns - in a real implementation, you would
    // use the actual DiagnosticKind from nil-ide
    
    if diagnostic.message.contains("undefined variable") {
        let var_name = diagnostic.message
            .trim_start_matches("undefined variable ")
            .trim_end_matches("'")
            .trim_start_matches("'");
        
        recommendations.push(format!("Check if '{}' is misspelled", var_name));
        recommendations.push(format!("Ensure '{}' is defined in scope (e.g., in let bindings or function parameters)", var_name));
        recommendations.push(format!("If '{}' should come from an input, add it to your inputs", var_name));
    } else if diagnostic.message.contains("unused") {
        recommendations.push("Remove the unused declaration or prefix it with '_' to indicate it's intentionally unused".to_string());
    } else if diagnostic.message.contains("expected type") && diagnostic.message.contains("but got") {
        recommendations.push("Fix the type mismatch according to the error message".to_string());
        recommendations.push("Ensure you're using the correct function or attribute for the given type".to_string());
    } else if diagnostic.message.contains("attribute") && diagnostic.message.contains("not found") {
        let attr_name = diagnostic.message
            .split("attribute '")
            .nth(1)
            .and_then(|s| s.split('\'').next())
            .unwrap_or("");
        
        if !attr_name.is_empty() {
            recommendations.push(format!("Check if attribute '{}' is misspelled", attr_name));
            recommendations.push(format!("Ensure the attribute '{}' exists in the set you're accessing", attr_name));
        } else {
            recommendations.push("Check if the attribute name is correct".to_string());
        }
    } else if diagnostic.message.contains("syntax error") {
        // For syntax errors, use our existing function
        return generate_syntax_error_recommendations(&diagnostic.message);
    }
    
    // If no specific recommendations, add a generic one
    if recommendations.is_empty() {
        recommendations.push("Review the error message and fix the issue accordingly".to_string());
    }
    
    recommendations
}
*/