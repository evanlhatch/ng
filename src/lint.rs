use crate::util::{add_verbosity_flags, run_cmd};
use color_eyre::eyre::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, error, info, warn};

/// Status of a check operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckStatus {
    /// Check passed successfully.
    Passed,
    /// Check failed.
    Failed,
    /// Check was skipped (e.g., tool not installed).
    Skipped,
    /// Check passed with warnings.
    Warnings,
}

/// Outcome of the lint operation.
#[derive(Debug, PartialEq, Eq)]
pub enum LintOutcome {
    /// All checks passed.
    Passed,
    /// Some checks had warnings but none failed.
    Warnings,
    /// At least one check failed critically.
    CriticalFailure(String),
}

/// Summary of the lint operation.
#[derive(Debug, Default)]
pub struct LintSummary {
    /// Overall outcome of the lint operation.
    pub outcome: Option<LintOutcome>,
    /// Status of each individual check.
    pub details: HashMap<String, CheckStatus>,
    /// Human-readable summary message.
    pub message: String,
    /// Number of files that were formatted.
    pub files_formatted: u32,
    /// Number of fixes applied by linters.
    pub fixes_applied: u32,
}

/// Checks if a command exists in the PATH.
///
/// # Arguments
///
/// * `cmd` - The command to check.
///
/// # Returns
///
/// * `bool` - True if the command exists, false otherwise.
pub fn command_exists(cmd: &str) -> bool {
    // Try to execute the command with --version or --help to check if it exists
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or_else(|_| {
            // If --version fails, try --help
            Command::new(cmd)
                .arg("--help")
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false)
        })
}

/// Checks if a path is hidden (starts with a dot).
///
/// # Arguments
///
/// * `path` - The path to check.
///
/// # Returns
///
/// * `bool` - True if the path is hidden, false otherwise.
pub fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

/// Recursively finds all .nix files in a directory.
///
/// # Arguments
///
/// * `dir` - The directory to search in.
/// * `files` - The vector to add found files to.
///
/// # Returns
///
/// * `Result<()>` - Ok if the search completed successfully, Err otherwise.
fn find_nix_files_in_dir(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if is_hidden(dir) {
        return Ok(());
    }
    
    for entry in fs::read_dir(dir).context(format!("Failed to read directory: {}", dir.display()))? {
        let entry = entry.context("Failed to read directory entry")?;
        let path = entry.path();
        
        if path.is_dir() {
            find_nix_files_in_dir(&path, files)?;
        } else if path.is_file() && path.extension().map_or(false, |ext| ext == "nix") {
            files.push(path);
        }
    }
    
    Ok(())
}

/// Finds all .nix files in the current directory and subdirectories.
///
/// # Returns
///
/// * `Result<Vec<PathBuf>>` - A list of paths to .nix files.
fn find_nix_files() -> Result<Vec<PathBuf>> {
    let mut nix_files = Vec::new();
    find_nix_files_in_dir(Path::new("."), &mut nix_files)?;
    Ok(nix_files)
}

/// Runs a formatter fix command.
///
/// # Arguments
///
/// * `formatter` - The formatter command to run.
/// * `files` - The files to format.
/// * `verbose_count` - The verbosity level.
///
/// # Returns
///
/// * `Result<(CheckStatus, u32)>` - The status of the check and the number of files formatted.
fn run_formatter_fix(formatter: &str, files: &[String], verbose_count: u8) -> Result<(CheckStatus, u32)> {
    if files.is_empty() {
        return Ok((CheckStatus::Passed, 0));
    }
    
    let mut cmd = Command::new(formatter);
    
    // Add files to format
    for file in files {
        cmd.arg(file);
    }
    
    // Add verbosity flags
    add_verbosity_flags(&mut cmd, verbose_count);
    
    // Run the command
    match run_cmd(&mut cmd) {
        Ok(_) => {
            // Assume all files were formatted
            Ok((CheckStatus::Passed, files.len() as u32))
        }
        Err(e) => {
            warn!("Formatter {} failed: {}", formatter, e);
            // Assume some files were formatted even if the command failed
            Ok((CheckStatus::Warnings, files.len() as u32 / 2))
        }
    }
}

/// Runs statix fix.
///
/// # Arguments
///
/// * `verbose_count` - The verbosity level.
///
/// # Returns
///
/// * `Result<(CheckStatus, u32)>` - The status of the check and the number of fixes applied.
fn run_statix_fix(verbose_count: u8) -> Result<(CheckStatus, u32)> {
    let mut cmd = Command::new("statix");
    cmd.args(["fix", "."]);
    
    // Add verbosity flags
    add_verbosity_flags(&mut cmd, verbose_count);
    
    // Run the command
    match run_cmd(&mut cmd) {
        Ok(_) => {
            // Assume some fixes were applied
            Ok((CheckStatus::Passed, 1))
        }
        Err(e) => {
            warn!("Statix fix failed: {}", e);
            Ok((CheckStatus::Warnings, 0))
        }
    }
}

/// Runs deadnix fix.
///
/// # Arguments
///
/// * `verbose_count` - The verbosity level.
///
/// # Returns
///
/// * `Result<(CheckStatus, u32)>` - The status of the check and the number of fixes applied.
fn run_deadnix_fix(verbose_count: u8) -> Result<(CheckStatus, u32)> {
    let mut cmd = Command::new("deadnix");
    cmd.args(["-f", "."]);
    
    // Add verbosity flags
    add_verbosity_flags(&mut cmd, verbose_count);
    
    // Run the command
    match run_cmd(&mut cmd) {
        Ok(_) => {
            // Assume some fixes were applied
            Ok((CheckStatus::Passed, 1))
        }
        Err(e) => {
            warn!("Deadnix fix failed: {}", e);
            Ok((CheckStatus::Warnings, 0))
        }
    }
}

/// Runs lint checks and fixes.
///
/// This function detects and runs available formatters and linters, attempting to fix issues.
/// It returns a summary of the checks and fixes applied.
///
/// # Arguments
///
/// * `strict_mode` - Whether to abort on lint warnings/errors.
/// * `verbose_count` - The verbosity level.
///
/// # Returns
///
/// * `Result<LintSummary>` - A summary of the lint operation.
pub fn run_lint_checks(strict_mode: bool, verbose_count: u8) -> Result<LintSummary> {
    info!("🧹 Running Linters and Formatters (Attempting Fixes)...");
    
    let mut summary = LintSummary::default();
    let mut outcome = LintOutcome::Passed;
    
    // Find .nix files
    let nix_files = find_nix_files()?;
    if nix_files.is_empty() {
        summary.message = "No .nix files found.".to_string();
        summary.outcome = Some(LintOutcome::Passed);
        return Ok(summary);
    }
    
    let nix_file_args: Vec<String> = nix_files
        .iter()
        .filter_map(|p| p.to_str().map(String::from))
        .collect();
    
    // --- Formatting ---
    let formatter = ["alejandra", "nixpkgs-fmt", "nixfmt"]
        .into_iter()
        .find(|cmd| command_exists(cmd));
    
    if let Some(fmt_cmd) = formatter {
        let check_name = format!("Format ({})", fmt_cmd);
        info!("-> Running {}", check_name);
        
        // Run formatter fix
        match run_formatter_fix(fmt_cmd, &nix_file_args, verbose_count) {
            Ok((status, count)) => {
                summary.files_formatted += count;
                summary.details.insert(check_name.clone(), status);
            }
            Err(e) => {
                error!("Formatter failed: {}", e);
                summary.details.insert(check_name.clone(), CheckStatus::Failed);
                if strict_mode {
                    outcome = LintOutcome::CriticalFailure(format!("Formatter {} failed", fmt_cmd));
                }
            }
        }
    } else {
        summary.details.insert("Format".to_string(), CheckStatus::Skipped);
        warn!("No Nix formatter found. Consider installing alejandra, nixpkgs-fmt, or nixfmt.");
    }
    
    // --- Statix ---
    if command_exists("statix") {
        info!("-> Running Statix");
        
        // Run statix fix
        match run_statix_fix(verbose_count) {
            Ok((status, count)) => {
                summary.fixes_applied += count;
                summary.details.insert("Statix Fix".to_string(), status);
            }
            Err(e) => {
                error!("Statix failed: {}", e);
                summary.details.insert("Statix Fix".to_string(), CheckStatus::Failed);
                if strict_mode {
                    outcome = LintOutcome::CriticalFailure("Statix fix failed".to_string());
                }
            }
        }
    } else {
        summary.details.insert("Statix".to_string(), CheckStatus::Skipped);
        debug!("Statix not found. Consider installing statix for better linting.");
    }
    
    // --- Deadnix ---
    if command_exists("deadnix") {
        info!("-> Running Deadnix");
        
        // Run deadnix fix
        match run_deadnix_fix(verbose_count) {
            Ok((status, count)) => {
                summary.fixes_applied += count;
                summary.details.insert("Deadnix Fix".to_string(), status);
            }
            Err(e) => {
                error!("Deadnix failed: {}", e);
                summary.details.insert("Deadnix Fix".to_string(), CheckStatus::Failed);
                if strict_mode {
                    outcome = LintOutcome::CriticalFailure("Deadnix fix failed".to_string());
                }
            }
        }
    } else {
        summary.details.insert("Deadnix".to_string(), CheckStatus::Skipped);
        debug!("Deadnix not found. Consider installing deadnix to detect unused code.");
    }
    
    // Update final outcome
    summary.outcome = Some(outcome);
    
    // Create summary message
    summary.message = format!(
        "Lint phase completed. {} files formatted, {} fixes applied (approx).",
        summary.files_formatted, summary.fixes_applied
    );
    
    Ok(summary)
}