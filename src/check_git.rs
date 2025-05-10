use color_eyre::eyre::{Context, Result};
use std::path::Path;
use std::process::Command;
use tracing::{debug, info, warn};
use owo_colors::OwoColorize;
use crate::ui_style::{Colors, Symbols};

#[cfg(not(test))]
use crate::util::run_cmd;

/// Checks if the current directory is a Git repository.
///
/// # Returns
///
/// * `bool` - True if the current directory is a Git repository, false otherwise.
pub fn is_git_repo() -> bool {
    // Check if .git directory exists
    if Path::new(".git").exists() {
        // Verify git command is available
        if let Ok(cmd) = Command::new("git").arg("--version").output() {
            return cmd.status.success();
        }
    }
    false
}

/// Creates a new Git command.
///
/// # Returns
///
/// * `Command` - A new Git command.
pub fn git_command() -> Command {
    Command::new("git")
}

/// Runs a Git check and warns about untracked or modified files that might affect the build.
///
/// This function checks for untracked .nix files and flake.lock, as well as whether flake.lock
/// is ignored by Git. It prints warnings but does not abort the build.
///
/// # Returns
///
/// * `Result<()>` - Ok if the check completed successfully, Err otherwise.
pub fn run_git_check_warning_only() -> Result<()> {
    if !is_git_repo() {
        debug!("Not a Git repository or git not found, skipping check.");
        return Ok(());
    }
    
    info!("🔍 Running Git status check (warnings only)...");
    
    // Check for untracked or modified files
    let mut cmd = git_command();
    cmd.args(["status", "--porcelain=v1"]);
    
    // Use direct Command execution in test environment to avoid issues with run_cmd
    #[cfg(test)]
    let output = cmd.output().context("Failed to run 'git status'")?;
    
    // Use run_cmd in non-test environment
    #[cfg(not(test))]
    let output = run_cmd(&mut cmd).context("Failed to run 'git status'")?;
    
    if !output.status.success() {
        warn!("'git status' command failed. Unable to check for uncommitted files.");
        return Ok(());
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let problematic_files: Vec<String> = stdout
        .lines()
        .filter_map(|line| {
            // Focus on untracked (??) .nix files and flake.lock
            if line.starts_with("??") && (line.ends_with(".nix") || line.ends_with("flake.lock")) {
                Some(line[3..].trim().to_string())
            } else {
                None
            }
        })
        .collect();
    
    if !problematic_files.is_empty() {
        warn!("{} {}", Symbols::warning(), Colors::warning(Colors::emphasis("Git Warning: Untracked files detected:")));
        
        // Clone the files to avoid lifetime issues
        let files_to_display: Vec<String> = problematic_files.iter().cloned().collect();
        
        // Use table display if available
        if let Err(e) = crate::tables::display_git_status(files_to_display, vec![]) {
            // Fallback to traditional display if table fails
            warn!("{}", Colors::warning("------------------------------------------------------------"));
            for file in problematic_files.iter() {
                warn!("   • {}", Colors::warning(file));
            }
            warn!("{}", Colors::warning("------------------------------------------------------------"));
            debug!("Failed to display git status table: {}", e);
        }
        
        warn!("{}", Colors::warning("   These files are not tracked by Git and will"));
        warn!("{}", Colors::warning("   likely NOT be included in the Nix build."));
        warn!("{}", Colors::warning("   Consider running 'git add <files> ...' if they are needed."));
    }
    
    // Check if flake.lock is ignored by Git
    if Path::new("flake.lock").exists() {
        let mut ignore_cmd = git_command();
        ignore_cmd.args(["check-ignore", "-q", "flake.lock"]);
        
        // Use direct Command execution in test environment
        #[cfg(test)]
        let status_result = ignore_cmd.status();
        
        // Use run_cmd in non-test environment
        #[cfg(not(test))]
        let status_result = ignore_cmd.status();
        
        if let Ok(status) = status_result {
            if status.success() {
                warn!("{} Git Warning: {} is ignored by Git (in .gitignore).",
                    Symbols::warning(),
                    Colors::code("flake.lock").italic());
            }
        }
    }
    
    debug!("Git check complete.");
    Ok(())
}