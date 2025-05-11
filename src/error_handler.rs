use crate::ui_style::{header, separator, Colors, Symbols}; // Added separator back
use crate::nix_analyzer::{NixAnalysisContext, NgDiagnostic};
use color_eyre::eyre::{Result, eyre}; // Removed Context
use lazy_static::lazy_static;
use owo_colors::OwoColorize;
use regex::Regex;
// StdCommand, StdProcessOutput, ExitStatusExt removed as create_mock_output is in test module
use crate::commands::Command as NhCommand;
use tracing::{error, info}; 

// Define Regexes for parsing error messages

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
    let attr_path_str = attribute_path_slice.join("."); // Used for info and final arg
    info!("-> Attempting to fetch evaluation trace for {}#{}...", flake_ref, attr_path_str);
    
    // Construct the full flake reference for the command
    let full_flake_arg = format!("{}#{}", flake_ref, attr_path_str);

    let cmd = NhCommand::new("nix") // Use NhCommand (crate::commands::Command)
        .args(["eval", &full_flake_arg, "--show-trace"])
        .add_verbosity_flags(verbose_count); // NhCommand's own method
    
    // Run the command and capture output
    match cmd.run_capture_output() { // Use the new method that returns Result<std::process::Output>
        Ok(output) => {
            // Nix eval --show-trace usually prints trace to stderr.
            // It might print the evaluated result to stdout if successful and not an error.
            let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
            if output.status.success() {
                // If successful, stderr contains the trace (or might be empty if no trace).
                // stdout might contain the value, but for --show-trace, stderr is primary for trace.
                Ok(stderr_str)
            } else {
                // If failed, stderr likely contains the error trace.
                // If stderr is empty, the error might be in stdout or a general failure.
                if stderr_str.is_empty() {
                    let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
                    Err(eyre!(
                        "nix eval --show-trace for {} failed with no stderr. stdout: {}",
                        full_flake_arg,
                        stdout_str
                    ).into())
                } else {
                    Err(eyre!(
                        "nix eval --show-trace for {} failed. Stderr: {}",
                        full_flake_arg,
                        stderr_str
                    ).into())
                }
            }
        }
        Err(e) => Err(eyre!("Failed to execute nix eval --show-trace for {}: {}", full_flake_arg, e)),
    }
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
    
    let cmd = NhCommand::new("nix") // Explicitly use NhCommand
        .args(["log", drv_path])
        .add_verbosity_flags(verbose_count);
    
    // Run the command and capture output
    let log_content = match cmd.run_capture()? {
        Some(stdout) => stdout,
        None => return Err(eyre!("nix log for {} did not produce stdout", drv_path)),
    };
    
    // Format the log with ui_style::header
    Ok(format!(
        "{}\n{}", // header() already includes top and bottom separators
        header(&format!("BUILD LOG FOR: {}", drv_path.bold())),
        log_content.trim()
    ))
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


pub fn report_ng_diagnostics(
    check_name: &str,
    diagnostics: &[NgDiagnostic],
    _analyzer_context: Option<&NixAnalysisContext>, // Analyzer context might be used for more details later
) {
    if diagnostics.is_empty() {
        return;
    }
    eprintln!(); // Add a blank line for spacing, use eprintln for errors
    eprintln!("{}", header(&format!("{} Found Issues:", check_name)).underline());

    for diag in diagnostics {
        let severity_str = match diag.severity {
            crate::nix_analyzer::NgSeverity::Error => "Error".red().bold().to_string(),
            crate::nix_analyzer::NgSeverity::Warning => "Warning".yellow().bold().to_string(),
        };
        
        let location_str = if let (Some(line), Some(col)) = (diag.line, diag.column) {
            format!("{}:{}:{}", diag.file_path.display(), line, col)
        } else if let Some(line) = diag.line {
            format!("{}:{}", diag.file_path.display(), line)
        } else {
            format!("{}", diag.file_path.display())
        };

        let output_str = format!(
            "  [{}] {} - {}",
            severity_str,
            location_str,
            diag.message
        );
        eprintln!("{}", output_str);
        // TODO: Print code snippet (requires file content access via NgDiagnostic or context)
    }
    eprintln!(); // Add a blank line for spacing
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

#[cfg(test)]
mod tests {
    use super::*; // Import functions from outer module
    use crate::commands::test_support;
     // For create_mock_output
     // For create_mock_output
     // For create_mock_output
    use color_eyre::eyre::eyre; // For create_mock_output in case of error from set_mock_process_output

    #[test]
    fn test_parse_nix_eval_error_valid() {
        let stderr = "error: attribute 'services.nonexistent' not found at /path/to/flake.nix:10:5 some other text";
        let expected = Some((
            "attribute 'services.nonexistent' not found".to_string(),
            "/path/to/flake.nix".to_string(),
            10,
            5,
        ));
        assert_eq!(parse_nix_eval_error(stderr), expected);
    }

    #[test]
    fn test_parse_nix_eval_error_no_match() {
        let stderr = "this is just some normal output";
        assert_eq!(parse_nix_eval_error(stderr), None);
    }

    #[test]
    fn test_parse_nix_eval_error_malformed_location() {
        let stderr = "error: some problem at file:not_a_line:col";
        assert_eq!(parse_nix_eval_error(stderr), None); // Regex should fail to capture digits for line/col
    }

    #[test]
    fn test_parse_nix_eval_error_empty_string() {
        let stderr = "";
        assert_eq!(parse_nix_eval_error(stderr), None);
    }

    #[test]
    fn test_parse_nix_eval_error_different_message() {
        let stderr = "error: undefined variable 'foobar' at /tmp/test.nix:1:1";
        let expected = Some((
            "undefined variable 'foobar'".to_string(),
            "/tmp/test.nix".to_string(),
            1,
            1,
        ));
        assert_eq!(parse_nix_eval_error(stderr), expected);
    }

    #[test]
    fn test_find_failed_derivations_single() {
        let stderr = "some preliminary output\nerror: builder for '/nix/store/abcd-foo.drv' failed with exit code 1\nmore output";
        let expected = vec!["/nix/store/abcd-foo.drv".to_string()];
        assert_eq!(find_failed_derivations(stderr), expected);
    }

    #[test]
    fn test_find_failed_derivations_multiple() {
        let stderr = "error: builder for '/nix/store/abcd-foo.drv' failed\nstuff in between\nerror: builder for '/nix/store/efgh-bar.drv' failed again";
        let expected = vec!["/nix/store/abcd-foo.drv".to_string(), "/nix/store/efgh-bar.drv".to_string()];
        assert_eq!(find_failed_derivations(stderr), expected);
    }

    #[test]
    fn test_find_failed_derivations_none() {
        let stderr = "everything is fine, no builder failed";
        let expected: Vec<String> = vec![];
        assert_eq!(find_failed_derivations(stderr), expected);
    }

    #[test]
    fn test_find_failed_derivations_empty_string() {
        let stderr = "";
        let expected: Vec<String> = vec![];
        assert_eq!(find_failed_derivations(stderr), expected);
    }

    #[test]
    fn test_find_failed_derivations_similar_no_match() {
        let stderr = "error: builder for some-other-thing failed";
        let expected: Vec<String> = vec![];
        assert_eq!(find_failed_derivations(stderr), expected);
    }

    #[test]
    fn test_scan_log_missing_package() {
        let log = "some log output... package 'openssl' not found ... more logs";
        let recs = scan_log_for_recommendations(log);
        assert!(recs.iter().any(|r| r.contains("package dependency appears to be missing")));
    }

    #[test]
    fn test_scan_log_permission_denied() {
        let log = "Error: permission denied while trying to access /nix/store";
        let recs = scan_log_for_recommendations(log);
        assert!(recs.iter().any(|r| r.contains("Permission errors detected")));
    }

    #[test]
    fn test_scan_log_network_error() {
        let log = "failed to download ... connection timed out ... blah";
        let recs = scan_log_for_recommendations(log);
        assert!(recs.iter().any(|r| r.contains("Network-related errors detected")));
    }

    #[test]
    fn test_scan_log_attribute_error() {
        let log = "error: attribute 'system' missing at /flake.nix:10:1";
        let recs = scan_log_for_recommendations(log);
        assert!(recs.iter().any(|r| r.contains("attribute error was detected")));
    }

    #[test]
    fn test_scan_log_syntax_error_keyword() {
        let log = "there is a syntax error in your configuration";
        let recs = scan_log_for_recommendations(log);
        assert!(recs.iter().any(|r| r.contains("Syntax errors detected")));
    }

    #[test]
    fn test_scan_log_multiple_issues() {
        let log = "package not found, also error: attribute 'foo' missing, and permission denied";
        let recs = scan_log_for_recommendations(log);
        assert_eq!(recs.len(), 3); // Missing package, attribute error, permission error
        assert!(recs.iter().any(|r| r.contains("package dependency")));
        assert!(recs.iter().any(|r| r.contains("attribute error")));
        assert!(recs.iter().any(|r| r.contains("Permission errors")));
    }

    #[test]
    fn test_scan_log_no_specific_issues() {
        let log = "this log has no specific keywords, just a general failure notice.";
        let recs = scan_log_for_recommendations(log);
        assert!(recs.iter().any(|r| r.contains("Review the full log")));
        assert!(recs.iter().any(|r| r.contains("Run 'nh doctor'")));
        assert_eq!(recs.len(), 2);
    }

    #[test]
    fn test_scan_log_empty_log() {
        let log = "";
        let recs = scan_log_for_recommendations(log);
        assert!(recs.iter().any(|r| r.contains("Review the full log")));
        assert!(recs.iter().any(|r| r.contains("Run 'nh doctor'")));
        assert_eq!(recs.len(), 2);
    }

    #[test]
    fn test_enhance_syntax_error_output_typical() {
        let error_details = "Error in /path/to/my/file.nix: \nerror: syntax error, unexpected ID, expecting SEMI or INHERIT at /path/to/my/file.nix:10:5\n\n   10|     some_attr = value: another_attr;
      |     ^";
        let enhanced = enhance_syntax_error_output(error_details);
        // Basic checks: contains file path, error, snippet parts. Coloring/symbols make exact match hard.
        assert!(enhanced.contains("/path/to/my/file.nix"));
        assert!(enhanced.contains("syntax error, unexpected ID"));
        assert!(enhanced.contains("some_attr = value"));
        assert!(enhanced.contains("^"));
    }

    #[test]
    fn test_enhance_syntax_error_output_no_match() {
        let error_details = "a generic build error, not a syntax error from nix-instantiate --parse";
        let enhanced = enhance_syntax_error_output(error_details);
        // Should return the string mostly as-is, with newlines normalized perhaps.
        assert!(enhanced.contains(error_details));
    }

    #[test]
    fn test_enhance_syntax_error_output_empty() {
        let error_details = "";
        let enhanced = enhance_syntax_error_output(error_details);
        assert_eq!(enhanced.trim(), ""); // Expect empty or just whitespace
    }

    #[test]
    fn test_enhance_syntax_error_output_multiple_files() {
        let error_details = "Error in file1.nix: \nerror: syntax error at file1.nix:1:1\n\nError in file2.nix: \nerror: syntax error at file2.nix:2:2";
        let enhanced = enhance_syntax_error_output(error_details);
        assert!(enhanced.contains("file1.nix"));
        assert!(enhanced.contains("file2.nix"));
        assert_eq!(enhanced.matches("Error in ").count(), 0); // Corrected: "Error in " should be removed
    }

    #[test]
    fn test_generate_recommendations_missing_brace() {
        let details = "error: syntax error, unexpected end of file, expecting } at /flake.nix:10:1";
        let recs = generate_syntax_error_recommendations(details);
        assert!(recs.iter().any(|r| r.contains("Add missing closing brace '}'")));
        assert!(recs.iter().any(|r| r.contains("Nix formatter"))); // General recommendation
    }

    #[test]
    fn test_generate_recommendations_unexpected_semicolon() {
        let details = "error: syntax error, unexpected ;, expecting end of file at /flake.nix:5:2";
        let recs = generate_syntax_error_recommendations(details);
        assert!(recs.iter().any(|r| r.contains("Remove extra semicolon")));
    }

    #[test]
    fn test_generate_recommendations_unexpected_equals() {
        let details = "error: syntax error, unexpected = at /flake.nix:3:4";
        let recs = generate_syntax_error_recommendations(details);
        assert!(recs.iter().any(|r| r.contains("Check attribute name before '='")));
    }

    #[test]
    fn test_generate_recommendations_generic_error() {
        let details = "error: some other syntax problem not specifically handled";
        let recs = generate_syntax_error_recommendations(details);
        assert!(recs.iter().any(|r| r.contains("Fix the syntax error according to the error message")));
        assert!(recs.iter().any(|r| r.contains("Nix formatter")));
        assert_eq!(recs.len(), 2);
    }

    // Tests for fetch_and_format_nix_log
    #[test]
    fn test_fetch_log_success() {
        test_support::enable_test_mode();
        let expected_log_raw = "This is the nix log output.".to_string();
        test_support::set_mock_capture_stdout(expected_log_raw.clone());
        
        let drv_path = "/nix/store/some-drv";
        let result = fetch_and_format_nix_log(drv_path, 0);
        
        use crate::ui_style::header; // Only header is directly used now
        use owo_colors::OwoColorize;

        // Expected format: header output (which includes its own separators) followed by the log content.
        let expected_formatted_log = format!(
            "{}\n{}",
            header(&format!("BUILD LOG FOR: {}", drv_path.bold())),
            expected_log_raw.trim()
        );

        assert!(result.is_ok(), "fetch_and_format_nix_log failed: {:?}", result.err());
        assert_eq!(result.unwrap(), expected_formatted_log);
        let recorded_commands = test_support::get_recorded_commands();
        assert_eq!(recorded_commands.len(), 1, "Expected exactly one command to be recorded in this test");
        let cmd_to_check = recorded_commands.first().expect("No command was recorded").clone();
        assert!(cmd_to_check.contains(&format!("nix log {}", drv_path)));
        test_support::disable_test_mode();
    }

    #[test]
    fn test_fetch_log_command_fails() {
        test_support::enable_test_mode();
        test_support::set_mock_capture_error("nix log command failed".to_string());

        let result = fetch_and_format_nix_log("/nix/store/some-drv", 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("nix log command failed"));
        test_support::disable_test_mode();
    }

    #[test]
    fn test_fetch_log_no_stdout() {
        test_support::enable_test_mode();
        // set_mock_capture_stdout with an empty string or rely on default None
        // get_mock_capture_result returns Ok(None) if MOCK_CAPTURE_STDOUT is None and MOCK_CAPTURE_ERROR is None
        // So, just ensure MOCK_CAPTURE_STDOUT is None (enable_test_mode resets it to None)

        let result = fetch_and_format_nix_log("/nix/store/some-drv", 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("did not produce stdout"));
        test_support::disable_test_mode();
    }

    // Tests for fetch_nix_trace
    fn create_mock_output(status_code: i32, stdout: &str, stderr: &str) -> Result<std::process::Output> {
        #[cfg(unix)]
        let status = std::os::unix::process::ExitStatusExt::from_raw(status_code);
        #[cfg(not(unix))]
        let status = {
            // Create a real ExitStatus for success or failure for non-unix
            let mut cmd = if status_code == 0 {
                std::process::Command::new("cmd") // or sh for general non-windows unix
                    .arg("/C")
                    .arg("exit 0")
            } else {
                std::process::Command::new("cmd")
                    .arg("/C")
                    .arg(format!("exit {}", status_code))
            };
            cmd.status().expect("Failed to create mock ExitStatus")
        };

        Ok(std::process::Output {
            status,
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        })
    }

    #[test]
    fn test_fetch_trace_success_with_stderr() {
        test_support::enable_test_mode();
        let expected_trace = "this is the trace on stderr".to_string();
        test_support::set_mock_process_output(create_mock_output(0, "stdout content", &expected_trace));
        
        let result = fetch_nix_trace("flake_ref", &["attr".to_string()], 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected_trace);
        let recorded_commands = test_support::get_recorded_commands();
        assert_eq!(recorded_commands.len(), 1, "Expected exactly one command to be recorded in this test");
        let cmd_to_check = recorded_commands.first().expect("No command was recorded").clone();
        assert!(cmd_to_check.contains("nix eval flake_ref#attr --show-trace"));
        test_support::disable_test_mode();
    }

    #[test]
    fn test_fetch_trace_success_empty_stderr_uses_stdout() {
        test_support::enable_test_mode();
        // This case was in fetch_nix_trace logic previously, let's ensure it's handled.
        // The refactored fetch_nix_trace now prioritizes stderr if status.success().
        // If stderr is empty on success, it still returns empty stderr.
        let expected_stdout_if_stderr_empty_on_success = "evaluated value".to_string();
        test_support::set_mock_process_output(create_mock_output(0, &expected_stdout_if_stderr_empty_on_success, ""));

        let result = fetch_nix_trace("flake_ref", &["attr".to_string()], 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ""); // Expects empty stderr now
        test_support::disable_test_mode();
    }

    #[test]
    fn test_fetch_trace_command_fails_with_stderr() {
        test_support::enable_test_mode();
        let error_output = "error trace on stderr".to_string();
        test_support::set_mock_process_output(create_mock_output(1, "some stdout", &error_output));
        
        let result = fetch_nix_trace("flake_ref", &["attr".to_string()], 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains(&error_output));
        test_support::disable_test_mode();
    }

    #[test]
    fn test_fetch_trace_command_fails_empty_stderr_uses_stdout() {
        test_support::enable_test_mode();
        let stdout_content = "failure info on stdout".to_string();
        test_support::set_mock_process_output(create_mock_output(1, &stdout_content, ""));

        let result = fetch_nix_trace("flake_ref", &["attr".to_string()], 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains(&format!("failed with no stderr. stdout: {}", stdout_content)));
        test_support::disable_test_mode();
    }

    #[test]
    fn test_fetch_trace_underlying_command_execution_error() {
        test_support::enable_test_mode();
        let exec_error_msg = "Failed to execute nix eval itself".to_string();
        // This simulates an error from cmd.run_capture_output() itself, not a Nix error.
        test_support::set_mock_process_output(Err(eyre!(exec_error_msg.clone())));

        let result = fetch_nix_trace("flake_ref", &["attr".to_string()], 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains(&exec_error_msg));
        test_support::disable_test_mode();
    }

    #[test]
    fn test_generate_recommendations_empty_details() {
        let details = "";
        let recs = generate_syntax_error_recommendations(details);
        // Should still give generic advice if details are empty but function is called
        assert!(recs.iter().any(|r| r.contains("Fix the syntax error according to the error message")));
        assert!(recs.iter().any(|r| r.contains("Nix formatter")));
        assert_eq!(recs.len(), 2);
    }
}
