# nh Enhancement Implementation Plan

This guide provides a detailed, step-by-step approach to implementing robust pre-flight checks, error reporting, and progress indicators in the nh tool. Each step is designed to be implemented and tested incrementally to ensure reliability and ease of debugging.

## Phase 1: Setup and Dependencies

### 1.1 Update Dependencies
- [x] 1.1.1 Update Cargo.toml with new dependencies:
```toml
[dependencies]
# ... existing dependencies

# New dependencies
indicatif = { version = "0.17", features = ["improved_unicode"] }
cli-table = "0.4"
rayon = "1"
lazy_static = "1.4"  # If not already included
atty = "0.2"
```
- [x] 1.1.2 Run `cargo check` to verify dependency resolution
- [x] 1.1.3 Create a simple `cargo run -- --help` test to ensure binary still builds

### 1.2 Update Main Struct and Verbosity Flag
- [x] 1.2.1 Modify `src/interface.rs`: Change verbose field in Main struct from boolean to u8 count
```rust
#[derive(Parser, Debug)]
pub struct Main {
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    /// Increase verbosity (can be used multiple times)
    pub verbose: u8,
    
    #[command(subcommand)]
    pub command: NHCommand,
}
```
- [x] 1.2.2 Test: `cargo run -- -v` and `cargo run -- -vvv` to verify count works

### 1.3 Update Command Run Method
- [x] 1.3.1 Modify `src/interface.rs`: Update NHCommand::run to accept verbosity count
```rust
impl NHCommand {
    pub fn run(self, verbose_count: u8) -> Result<()> {
        match self {
            Self::Os(args) => {
                std::env::set_var("NH_CURRENT_COMMAND", "os");
                args.run(verbose_count)
            }
            Self::Search(args) => args.run(verbose_count),
            Self::Clean(proxy) => proxy.command.run(verbose_count),
            Self::Completions(args) => args.run(verbose_count),
            Self::Home(args) => {
                std::env::set_var("NH_CURRENT_COMMAND", "home");
                args.run(verbose_count)
            }
            Self::Darwin(args) => {
                std::env::set_var("NH_CURRENT_COMMAND", "darwin");
                args.run(verbose_count)
            }
        }
    }
}
```

### 1.4 Update Main Function
- [x] 1.4.1 Modify `src/main.rs`: Update main() to pass verbose_count
```rust
fn main() -> Result<()> {
    // ... existing FLAKE warning logic ...

    let args = <crate::interface::Main as clap::Parser>::parse();
    let verbose_count = args.verbose; // Extract verbosity count
    
    crate::logging::setup_logging(verbose_count > 0)?; // For now, just pass bool compatibility
    debug!("{args:#?}");
    debug!(%NH_VERSION, ?NH_REV);

    // ... existing FLAKE warning logic ...

    args.command.run(verbose_count)
}
```
- [x] 1.4.2 Test: `cargo run -- -vv os --help` to verify compilation

### 1.5 Update Command Handlers in Each Module
- [x] 1.5.1 Modify `src/nixos.rs`: Update OsArgs::run
```rust
impl interface::OsArgs {
    pub fn run(self, verbose_count: u8) -> Result<()> {
        // ... update calls to include verbose_count
        match self.subcommand {
            OsSubcommand::Boot(args) => args.rebuild(Boot, verbose_count),
            OsSubcommand::Test(args) => args.rebuild(Test, verbose_count),
            OsSubcommand::Switch(args) => args.rebuild(Switch, verbose_count),
            OsSubcommand::Build(args) => {
                if args.common.ask || args.common.dry {
                    warn!("`--ask` and `--dry` have no effect for `nh os build`");
                }
                args.rebuild(Build, verbose_count)
            }
            OsSubcommand::Repl(args) => args.run(verbose_count),
            OsSubcommand::Info(args) => args.info(),
        }
    }
}
```
- [x] 1.5.2 Modify rebuild signatures to accept verbose_count
```rust
impl OsRebuildArgs {
    fn rebuild(self, variant: OsRebuildVariant, verbose_count: u8) -> Result<()> {
        // ... existing implementation (use verbose_count later)
    }
}
```
- [x] 1.5.3 Repeat similar updates for `src/home.rs` and `src/darwin.rs`
- [x] 1.5.4 Update `src/search.rs`, `src/clean.rs`, etc. to handle verbose_count
- [x] 1.5.5 Test: `cargo run -- -vv os --help` again to verify compilation

### 1.6 Update Logging Setup
- [x] 1.6.1 Modify `src/logging.rs`: Enhance setup_logging to use verbosity levels
```rust
pub(crate) fn setup_logging(verbose_level: u8) -> Result<()> {
    // ... existing color_eyre setup ...
    
    let is_debug = verbose_level > 0;
    let is_trace = verbose_level > 1;
    
    let layer_debug = fmt::layer()
        .with_writer(std::io::stderr)
        .without_time()
        .compact()
        .with_line_number(true)
        .with_filter(
            EnvFilter::from_env("NH_LOG").or(filter_fn(move |meta| {
                let level = *meta.level();
                (is_debug && level == Level::DEBUG) ||
                (is_trace && level == Level::TRACE)
            }))
        );
    
    // ... rest of existing function ...
}
```
- [x] 1.6.2 Test: Run with different verbosity levels to verify output differences:
  - `cargo run -- search nixpkgs`
  - `cargo run -- -v search nixpkgs` (should show DEBUG)
  - `cargo run -- -vv search nixpkgs` (should show TRACE)

## Phase 2: Core Utility Functions

### 2.1 Add Basic Command Execution Utilities
- [x] 2.1.1 Create/modify `src/util.rs`: Add TTY detection and command utilities
```rust
pub fn is_stdout_tty() -> bool {
    atty::is(atty::Stream::Stdout)
}

pub fn add_verbosity_flags(cmd: &mut std::process::Command, verbosity_level: u8) {
    let effective_level = std::cmp::min(verbosity_level, 7);
    for _ in 0..effective_level {
        cmd.arg("-v");
    }
}

pub fn run_cmd(command: &mut std::process::Command) -> Result<std::process::Output> {
    debug!("Executing: {:?}", command);
    let output = command.output()
        .wrap_err_with(|| format!("Failed to execute command: {:?}", command))?;
    
    if !output.status.success() {
        warn!("Command failed: {:?}", command);
        return Err(color_eyre::eyre::eyre!(
            "Command exited with non-zero status: {:?}",
            output.status.code()
        ));
    }
    Ok(output)
}

pub fn run_cmd_inherit_stdio(command: &mut std::process::Command) -> Result<std::process::ExitStatus> {
    debug!("Executing: {:?}", command);
    let status = command
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .wrap_err_with(|| {
            format!("Failed to execute command with inherited stdio: {:?}", command)
        })?;

    if !status.success() {
        warn!("Command failed: {:?}", command);
        return Err(color_eyre::eyre::eyre!(
            "Command exited with non-zero status: {:?}",
            status.code()
        ));
    }
    Ok(status)
}
```
- [x] 2.1.2 Create a basic test function that uses the utilities:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_add_verbosity_flags() {
        let mut cmd = std::process::Command::new("echo");
        add_verbosity_flags(&mut cmd, 3);
        // Check internal state via debug format
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("-v"));
    }
}
```
- [x] 2.1.3 Test: Run `cargo test util::tests::test_add_verbosity_flags`

### 2.2 Create Progress Indicator Module
- [x] 2.2.1 Create `src/progress.rs`: Implement spinner functions
```rust
use indicatif::{ProgressBar, ProgressStyle};
use crate::util::is_stdout_tty;
use tracing::info;

pub fn start_spinner(message: &str) -> ProgressBar {
    if !is_stdout_tty() {
        // If not a TTY, use a hidden spinner but still print the message
        info!("{}", message);
        ProgressBar::hidden()
    } else {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.blue} {msg}")
                .unwrap()
        );
        pb.set_message(message.to_string());
        pb.tick();
        pb
    }
}

pub fn update_spinner_message(pb: &ProgressBar, message: &str) {
    if pb.is_hidden() {
        info!("{}", message);
    }
    pb.set_message(message.to_string());
}

pub fn finish_spinner_success(pb: &ProgressBar, message: &str) {
    if pb.is_hidden() {
        info!("✓ {}", message);
        return;
    }
    
    pb.finish_with_message(format!("✓ {}", message));
}

pub fn finish_spinner_fail(pb: &ProgressBar) {
    if !pb.is_hidden() {
        pb.finish_and_clear();
    }
}
```
- [x] 2.2.2 Add module declaration in `src/main.rs`
```rust
mod progress;
```
- [x] 2.2.3 Create a sample program to test spinners
```rust
// src/bin/test_spinner.rs
use nh::progress;
use std::{thread, time::Duration};

fn main() {
    let pb = progress::start_spinner("Starting work...");
    thread::sleep(Duration::from_millis(1000));
    
    progress::update_spinner_message(&pb, "Processing data...");
    thread::sleep(Duration::from_millis(1000));
    
    progress::finish_spinner_success(&pb, "Work completed successfully");
}
```
- [x] 2.2.4 Test: `cargo run --bin test_spinner` (build may fail until lib.rs is created)

### 2.3 Create Error Handler Module
- [x] 2.3.1 Create `src/error_handler.rs`: Implement basic error reporting
```rust
use crate::util::{add_verbosity_flags, run_cmd};
use color_eyre::eyre::{Context, Result};
use lazy_static::lazy_static;
use owo_colors::OwoColorize;
use regex::Regex;
use std::process::Command;
use tracing::{error, info};

// Define regex patterns for parsing error messages
lazy_static! {
    static ref RE_BUILDER_FAILED: Regex =
        Regex::new(r"error: builder for '(/nix/store/.*?\.drv)' failed").unwrap();
}

pub fn find_failed_derivations(stderr_summary: &str) -> Vec<String> {
    let mut failed_drvs = Vec::new();
    
    for cap in RE_BUILDER_FAILED.captures_iter(stderr_summary) {
        failed_drvs.push(cap[1].to_string());
    }
    
    failed_drvs
}

pub fn fetch_and_format_nix_log(drv_path: &str, verbose_count: u8) -> Result<String> {
    info!("-> Fetching build log for {} using 'nix log'...", drv_path.cyan());
    
    let mut cmd = Command::new("nix");
    cmd.arg("log");
    add_verbosity_flags(&mut cmd, verbose_count);
    cmd.arg(drv_path);
    
    let output = run_cmd(&mut cmd)
        .wrap_err_with(|| format!("Failed to fetch log for {}", drv_path))?;
    
    let log_content = String::from_utf8_lossy(&output.stdout).to_string();
    
    // Format log with headers/separators
    let header = format!("\n{}\n", "═".repeat(80).cyan());
    let title = format!(" Build Log: {} ", drv_path);
    let centered_title = format!("{:^80}", title);
    
    let mut formatted_log = String::new();
    formatted_log.push_str(&header);
    formatted_log.push_str(&centered_title);
    formatted_log.push_str(&header);
    formatted_log.push_str("\n");
    formatted_log.push_str(&log_content);
    formatted_log.push_str("\n");
    formatted_log.push_str(&header);
    
    Ok(formatted_log)
}

pub fn report_failure(stage: &str, reason: &str, details: Option<String>, recommendations: Vec<String>) {
    let width = 80;
    let border = "═".repeat(width).red();
    
    eprintln!("\n{}", border);
    error!("{:^width$}", format!("🛑 COMMAND FAILED: {} 🛑", stage.bold()));
    eprintln!("{}", border);
    error!("{}", reason.bold());
    
    if !recommendations.is_empty() {
        info!("\n🔍 Possible solutions:");
        for rec in recommendations {
            info!(" • {}", rec);
        }
    }
    
    if let Some(detail) = details {
        eprintln!("\n{}", detail);
    }
    
    eprintln!("{}\n", border);
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_find_failed_derivations() {
        let sample_error = r#"error: builder for '/nix/store/abc123.drv' failed
with exit code 1
error: 1 dependencies of derivation '/nix/store/def456.drv' failed to build
error: builder for '/nix/store/ghi789.drv' failed"#;
        
        let drvs = find_failed_derivations(sample_error);
        assert_eq!(drvs.len(), 2);
        assert_eq!(drvs[0], "/nix/store/abc123.drv");
        assert_eq!(drvs[1], "/nix/store/ghi789.drv");
    }
}
```
- [x] 2.3.2 Add module declaration in `src/main.rs`
```rust
mod error_handler;
```
- [x] 2.3.3 Test: Run `cargo test error_handler::tests::test_find_failed_derivations`

### 2.4 Create Git Check Module
- [x] 2.4.1 Create `src/check_git.rs`: Implement basic Git status check
```rust
use crate::util::run_cmd;
use color_eyre::eyre::{Context, Result};
use std::path::Path;
use std::process::Command;
use tracing::{debug, info, warn};
use owo_colors::OwoColorize;

fn git_command() -> Command {
    Command::new("git")
}

fn is_git_repo() -> bool {
    Path::new(".git").exists() && which::which("git").is_ok()
}

pub fn run_git_check_warning_only() -> Result<()> {
    if !is_git_repo() {
        debug!("Not a git repository or git not found, skipping check.");
        return Ok(());
    }
    
    info!("🔍 Running Git status check...");
    let mut cmd = git_command();
    cmd.args(["status", "--porcelain=v1"]);
    let output = run_cmd(&mut cmd).context("Failed to run 'git status'")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let problematic_files: Vec<String> = stdout
        .lines()
        .filter_map(|line| {
            // Focus on ?? (untracked) .nix / flake.lock
            if line.starts_with("??") && (line.ends_with(".nix") || line.ends_with("flake.lock")) {
                Some(line[3..].trim().to_string())
            } else {
                None
            }
        })
        .collect();

    if !problematic_files.is_empty() {
        warn!("{}", "------------------------------------------------------------".yellow());
        warn!("{}", "⚠️ Git Warning: Untracked files detected:".yellow().bold());
        for file in &problematic_files {
            warn!("   - {}", file.yellow());
        }
        warn!("{}", "   These files are not tracked by Git and will".yellow());
        warn!("{}", "   likely NOT be included in the Nix build.".yellow());
        warn!("{}", "   Consider running 'git add <files>' if they are needed.".yellow());
        warn!("{}", "------------------------------------------------------------".yellow());
    }

    // Check ignored flake.lock
    if Path::new("flake.lock").exists() {
        let mut ignore_cmd = git_command();
        ignore_cmd.args(["check-ignore", "-q", "flake.lock"]);
        if let Ok(status) = ignore_cmd.status() {
            if status.success() {
                warn!("⚠️ Git Warning: {} is ignored by Git (in .gitignore).", "flake.lock".italic());
            }
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // This is difficult to unit test without mocking git
    // We'll implement a simple command validation test
    #[test]
    fn test_git_command() {
        let cmd = git_command();
        assert_eq!(format!("{:?}", cmd).contains("git"), true);
    }
}
```
- [x] 2.4.2 Add module declaration in `src/main.rs`
```rust
mod check_git;
```
- [x] 2.4.3 Test: Run `cargo test check_git::tests::test_git_command`

### 2.5 Create Lint Module
- [x] 2.5.1 Create `src/lint.rs`: Implement basic linting framework
```rust
use crate::util::{add_verbosity_flags, run_cmd};
use color_eyre::eyre::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use tracing::{debug, info};
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckStatus {
    Passed,
    Failed,
    Skipped,
    Warnings,
}

#[derive(Debug, PartialEq, Eq)]
pub enum LintOutcome {
    Passed,
    Warnings,
    CriticalFailure(String),
}

#[derive(Debug)]
pub struct LintSummary {
    pub outcome: LintOutcome,
    pub details: HashMap<String, CheckStatus>,
    pub message: String,
    pub files_formatted: u32,
    pub fixes_applied: u32,
}

impl Default for LintSummary {
    fn default() -> Self {
        Self {
            outcome: LintOutcome::Passed,
            details: HashMap::new(),
            message: String::new(),
            files_formatted: 0,
            fixes_applied: 0,
        }
    }
}

fn find_nix_files() -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(".")
        .follow_links(true)
        .into_iter()
        .filter_entry(|e| !is_hidden(e))
    {
        let entry = entry?;
        if entry.file_type().is_file() && entry.path().extension().map_or(false, |ext| ext == "nix") {
            files.push(entry.path().to_path_buf());
        }
    }
    Ok(files)
}

fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry.file_name()
        .to_str()
        .map_or(false, |s| s.starts_with('.'))
        || entry.path().components().any(|c| {
            c.as_os_str().to_str().map_or(false, |s| s.starts_with('.'))
        })
}

pub fn run_lint_checks(strict_mode: bool, verbose_count: u8) -> Result<LintSummary> {
    info!("🧹 Running Linters and Formatters...");
    let mut summary = LintSummary::default();
    
    let nix_files = find_nix_files()?;
    if nix_files.is_empty() {
        summary.message = "No .nix files found.".to_string();
        return Ok(summary);
    }
    
    let nix_file_args: Vec<String> = nix_files.iter()
        .filter_map(|p| p.to_str().map(String::from))
        .collect();
    
    // Try formatters in order of preference
    let formatters = ["alejandra", "nixpkgs-fmt", "nixfmt"];
    for formatter in &formatters {
        if which::which(formatter).is_ok() {
            info!("-> Running {} for formatting...", formatter);
            
            let mut cmd = Command::new(formatter);
            cmd.args(&nix_file_args);
            
            match run_cmd(&mut cmd) {
                Ok(_) => {
                    summary.details.insert(format!("Format ({})", formatter), CheckStatus::Passed);
                    summary.files_formatted += nix_file_args.len() as u32;
                    break;
                }
                Err(e) => {
                    debug!("Formatter error (non-critical): {:?}", e);
                    summary.details.insert(format!("Format ({})", formatter), CheckStatus::Failed);
                    if strict_mode {
                        summary.outcome = LintOutcome::CriticalFailure(
                            format!("Formatter {} failed", formatter)
                        );
                    } else {
                        summary.outcome = LintOutcome::Warnings;
                    }
                }
            }
        }
    }
    
    // Basic implementation - we'll expand more later
    summary.message = format!(
        "{} files formatted, {} fixes applied",
        summary.files_formatted, summary.fixes_applied
    );
    
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_lint_summary_default() {
        let summary = LintSummary::default();
        assert_eq!(summary.files_formatted, 0);
        assert_eq!(summary.fixes_applied, 0);
        assert!(matches!(summary.outcome, LintOutcome::Passed));
    }
}
```
- [x] 2.5.2 Add module declaration in `src/main.rs`
```rust
mod lint;
```
- [x] 2.5.3 Test: Run `cargo test lint::tests::test_lint_summary_default`

### 2.6 Create Library Configuration
- [x] 2.6.1 Create `src/lib.rs` to expose the modules as a library
```rust
// New modules
pub mod check_git;
pub mod error_handler;
pub mod lint;
pub mod progress;
pub mod util;

// Existing modules
pub mod commands;
pub mod interface;
pub mod logging;
pub mod nixos;
pub mod home;
pub mod darwin;
pub mod clean;
pub mod search;
pub mod update;
pub mod generations;
pub mod installable;
pub mod json;
pub mod completion;

// Re-export color_eyre::Result for convenience
pub use color_eyre::Result;

// Constants and functions from main.rs
pub const NH_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NH_REV: Option<&str> = option_env!("NH_REV");

/// Elevate privileges using sudo
pub fn self_elevate() -> ! {
    use std::os::unix::process::CommandExt;

    let mut cmd = std::process::Command::new("sudo");
    cmd.args(std::env::args());
    tracing::debug!("{:?}", cmd);
    let err = cmd.exec();
    panic!("{}", err);
}
```
- [x] 2.6.2 Test: Run `cargo build` to verify library structure

## Phase 3: Update Interface for New Features

### 3.1 Define CommonArgs to Replace CommonRebuildArgs
- [x] 3.1.1 Modify `src/interface.rs`: Add new CommonArgs struct
```rust
#[derive(Debug, Args, Clone)]
pub struct CommonArgs {
    /// Skip pre-flight checks (Git, Parse, Flake, Lint)
    #[arg(long, global = true)]
    pub no_preflight: bool,
    
    /// Abort on any lint warning/error (after attempting fixes)
    #[arg(long, global = true)]
    pub strict_lint: bool,
    
    /// Run deeper checks: Nix Eval
    #[arg(long, global = true)]
    pub medium: bool,
    
    /// Run deepest checks: Nix Eval and Dry Run Build
    #[arg(long, global = true)]
    pub full: bool,
    
    /// Only print actions, without performing them
    #[arg(long, short = 'n', global = true)]
    pub dry: bool,
    
    /// Ask for confirmation before activating/committing changes
    #[arg(long, short, global = true)]
    pub ask: bool,
    
    /// Don't use nix-output-monitor for the build process
    #[arg(long, global = true)]
    pub no_nom: bool,
    
    /// Path to save the build result link
    #[arg(long, short, global = true)]
    pub out_link: Option<PathBuf>,

    /// Run cleanup after successful activation
    #[arg(long, global = true)]
    pub clean: bool,
}
```
- [x] 3.1.2 Modify OsRebuildArgs, HomeRebuildArgs, DarwinRebuildArgs to use CommonArgs
```rust
#[derive(Debug, Args)]
pub struct OsRebuildArgs {
    #[command(flatten)]
    pub common: CommonRebuildArgs,
    
    #[command(flatten)]
    pub update_args: UpdateArgs,
    
    // ... other fields ...
}
```
- [x] 3.1.3 Test: Run `cargo check` to verify interface changes

### 3.2 Update Run Methods for Compatibility
- [x] 3.2.1 Modify `src/nixos.rs`: Update rebuild method temporarily for compatibility
```rust
impl OsRebuildArgs {
    fn rebuild(self, variant: OsRebuildVariant, verbose_count: u8) -> Result<()> {
        // Access common args fields and run the old logic
        // Make minimal changes to maintain compatibility while structure changes
        // ...
        
        // For testing - print verbosity level
        debug!("Verbosity level: {}", verbose_count);
        
        // Continue with existing implementation...
    }
}
```
- [x] 3.2.2 Make similar updates to home.rs and darwin.rs
- [x] 3.2.3 Test: Run `cargo run -- -vv os info` to verify debug output shows verbosity

### 3.3 Unit Tests
- [x] 3.3.1 Run unit tests for all implemented components
- [x] 3.3.2 Fix doctest for add_verbosity_flags function
- [x] 3.3.3 Verify all tests pass

## Phase 4: Implement Parse Check

### 4.1 Implement Parallel Parse Check
- [x] 4.1.1 Add a test file with syntax error for testing
```nix
# test/syntax_error.nix
{
  # Missing closing brace
  foo = {
    bar = "baz";
```
- [x] 4.1.2 Add implementation of parse check function in `src/nixos.rs`
```rust
fn run_parallel_parse_check(verbose_count: u8) -> Result<(), String> {
    use rayon::prelude::*;
    use walkdir::WalkDir;
    use std::path::PathBuf;
    
    info!("Running parallel syntax check on .nix files...");
    
    // Find .nix files
    let nix_files: Vec<PathBuf> = WalkDir::new(".")
        .follow_links(true)
        .into_iter()
        .filter_entry(|e| !crate::lint::is_hidden(e))
        .filter_map(|entry| {
            entry.ok().filter(|e| {
                e.file_type().is_file() && 
                e.path().extension().map_or(false, |ext| ext == "nix")
            }).map(|e| e.path().to_owned())
        })
        .collect();
        
    if nix_files.is_empty() {
        info!("No .nix files found to check.");
        return Ok(());
    }
    
    debug!("Found {} .nix files to check", nix_files.len());
    
    // Use rayon to run nix-instantiate in parallel
    let parse_errors: Vec<(PathBuf, String)> = nix_files.par_iter()
        .filter_map(|path| {
            let mut cmd = std::process::Command::new("nix-instantiate");
            cmd.args(["--parse", path.to_str().unwrap()]);
            crate::util::add_verbosity_flags(&mut cmd, verbose_count);
            
            match cmd.output() {
                Ok(output) => {
                    if !output.status.success() {
                        let error = String::from_utf8_lossy(&output.stderr).to_string();
                        Some((path.clone(), error))
                    } else {
                        None
                    }
                },
                Err(e) => Some((path.clone(), format!("Failed to run nix-instantiate: {}", e)))
            }
        })
        .collect();
        
    if parse_errors.is_empty() {
        Ok(())
    } else {
        let mut combined_error = format!("Found {} file(s) with syntax errors:\n", parse_errors.len());
        for (path, error) in parse_errors {
            combined_error.push_str(&format!("\nError in {}: \n{}\n", path.display(), error));
        }
        Err(combined_error)
    }
}
```
- [x] 4.1.3 Test: Run a basic parse check:
```bash
# Command to test the parse check manually
cd nh && touch syntax_error.nix
echo "{" > syntax_error.nix
cargo run -- -vv os switch --hostname test
```

### 4.2 Integrate Parse Check into Rebuild
- [x] 4.2.1 Start refactoring rebuild in `src/nixos.rs` to include parse check
```rust
impl OsRebuildArgs {
    fn rebuild(self, variant: OsRebuildVariant, verbose_count: u8) -> Result<()> {
        // Early part of the function...
        
        // Add this near the beginning, before the build
        let run_preflight = !self.common.no_preflight;
        if run_preflight {
            let pb = progress::start_spinner("[Syntax] Checking .nix files...");
            match run_parallel_parse_check(verbose_count) {
                Ok(_) => {
                    progress::finish_spinner_success(&pb, "Syntax check passed");
                },
                Err(e) => {
                    progress::finish_spinner_fail(&pb);
                    error_handler::report_failure(
                        "Parse Check", 
                        "Nix syntax errors found",
                        Some(e),
                        vec!["Fix the syntax errors reported above".to_string()]
                    );
                    bail!("Parse check failed");
                }
            }
        }
        
        // Continue with the rest of the rebuild...
    }
}
```
- [x] 4.2.2 Test: Run with a syntax error file and test both with preflight enabled and disabled
```bash
cd nh && touch syntax_error.nix
echo "{" > syntax_error.nix
cargo run -- -vv os switch --hostname test  # Should fail with error
cargo run -- -vv os switch --hostname test --no-preflight  # Should bypass check
```

## Phase 5: Git Check Integration

### 5.1 Integrate Git Check
- [x] 5.1.1 Add Git check to rebuild in `src/nixos.rs`
```rust
impl OsRebuildArgs {
    fn rebuild(self, variant: OsRebuildVariant, verbose_count: u8) -> Result<()> {
        // Early part of the function...
        
        let run_preflight = !self.common.no_preflight;
        if run_preflight {
            // Add Git check first
            let pb = progress::start_spinner("[Git] Checking status...");
            match check_git::run_git_check_warning_only() {
                Ok(_) => {
                    // Git check passed or issued warnings
                    progress::finish_spinner_success(&pb, "Git check complete (warnings above if any)");
                },
                Err(e) => {
                    progress::finish_spinner_fail(&pb);
                    error_handler::report_failure(
                        "Git Check",
                        "Failed to check Git status",
                        Some(e.to_string()),
                        vec![
                            "Ensure git is installed and accessible".to_string(),
                            "Check if this is a git repository".to_string(),
                        ]
                    );
                    bail!("Git check failed");
                }
            }
            
            // Then the parse check we added earlier...
        }
        
        // Continue with the rest of the rebuild...
    }
}
```
- [x] 5.1.2 Test: Run in a Git repo with untracked .nix files
```bash
cd nh
# Create an untracked test file
touch untracked.nix
echo "{ test = 1; }" > untracked.nix
cargo run -- -vv os switch --hostname test  # Should show warning but not fail
```

## Phase 6: Lint Check Integration

### 6.1 Integrate Lint Check
- [x] 6.1.1 Add Lint check to rebuild in `src/nixos.rs`
```rust
impl OsRebuildArgs {
    fn rebuild(self, variant: OsRebuildVariant, verbose_count: u8) -> Result<()> {
        // Early part with Git and Parse checks...
        
        // Add Lint check
        if run_preflight {
            let pb = progress::start_spinner("[Lint] Running formatters and linters...");
            
            let use_strict_lint = self.common.strict_lint || self.common.full || self.common.medium;
            
            match lint::run_lint_checks(use_strict_lint, verbose_count) {
                Ok(lint_summary) => {
                    if matches!(lint_summary.outcome, lint::LintOutcome::CriticalFailure(_)) {
                        progress::finish_spinner_fail(&pb);
                        error_handler::report_failure(
                            "Lint", 
                            "Linting failed in strict mode",
                            None,
                            vec![
                                "Fix the linting issues reported above".to_string(),
                                "Use --no-preflight to skip linting checks".to_string()
                            ]
                        );
                        bail!("Lint check failed");
                    } else {
                        progress::finish_spinner_success(&pb, &format!(
                            "Lint check {}",
                            if matches!(lint_summary.outcome, lint::LintOutcome::Warnings) {
                                "completed with warnings"
                            } else {
                                "passed"
                            }
                        ));
                    }
                }
                Err(e) => {
                    progress::finish_spinner_fail(&pb);
                    error_handler::report_failure(
                        "Lint",
                        "Failed to run linters",
                        Some(e.to_string()),
                        vec![
                            "Ensure formatters/linters are installed".to_string(),
                            "Use --no-preflight to skip linting".to_string()
                        ]
                    );
                    bail!("Lint check failed");
                }
            }
        }
        
        // Continue with the rest of the rebuild...
    }
}
```
- [x] 6.1.2 Test: Run with --strict-lint to see how it behaves
```bash
cd nh
cargo run -- -vv os switch --hostname test --strict-lint  # May pass or fail depending on linters
```

## Phase 7: Medium and Full Mode Checks 

### 7.1 Implement Medium Mode (Eval Check)
- [x] 7.1.1 Add Eval check to rebuild in `src/nixos.rs` for medium mode
```rust
impl OsRebuildArgs {
    fn rebuild(self, variant: OsRebuildVariant, verbose_count: u8) -> Result<()> {
        // Early checks...
        
        // After other preflight checks, add:
        if self.common.medium || self.common.full {
            let pb = progress::start_spinner("[Eval] Evaluating configuration...");
            
            // For simplicity in this test phase, determine a basic installable to evaluate
            let flake_ref = ".";  // Current directory
            let attribute_path = vec![
                "nixosConfigurations".to_string(),
                "test".to_string(), // Use hostname when integrating fully
                "config".to_string(), 
            ];
            
            let mut cmd = std::process::Command::new("nix");
            cmd.args(["eval", "--json"]);
            crate::util::add_verbosity_flags(&mut cmd, verbose_count);
            
            // Construct flake argument
            let flake_arg = format!("{}#{}", flake_ref, attribute_path.join("."));
            cmd.arg(flake_arg);
            
            match crate::util::run_cmd(&mut cmd) {
                Ok(_) => {
                    progress::finish_spinner_success(&pb, "Configuration evaluated successfully");
                }
                Err(e) => {
                    progress::finish_spinner_fail(&pb);
                    
                    // Try to get detailed trace
                    let trace = error_handler::fetch_nix_trace(
                        flake_ref, &attribute_path, verbose_count
                    ).unwrap_or_else(|_| "".to_string());
                    
                    error_handler::report_failure(
                        "Eval",
                        "Failed to evaluate configuration",
                        Some(format!("{}\n\n{}", e, trace)),
                        vec![
                            "Fix the evaluation errors in your configuration".to_string(),
                            "Run with --no-medium to skip this check".to_string()
                        ]
                    );
                    bail!("Evaluation failed");
                }
            }
        }
        
        // Continue with the rest of the rebuild...
    }
}
```

### 7.2 Implement Full Mode (Dry Run Build)
- [x] 7.2.1 Add Dry Run check to rebuild in `src/nixos.rs` for full mode
```rust
impl OsRebuildArgs {
    fn rebuild(self, variant: OsRebuildVariant, verbose_count: u8) -> Result<()> {
        // Earlier checks...
        
        // After medium check, add:
        if self.common.full {
            let pb = progress::start_spinner("[Dry Run] Performing dry-run build...");
            
            // Similar to the eval check, use a simplified approach for testing
            let flake_ref = ".";  // Current directory
            let attribute_path = vec![
                "nixosConfigurations".to_string(),
                "test".to_string(), // Use hostname when integrating fully
                "config".to_string(),
                "system".to_string(),
                "build".to_string(),
            ];
            
            let mut cmd = std::process::Command::new("nix");
            cmd.args(["build", "--dry-run"]);
            crate::util::add_verbosity_flags(&mut cmd, verbose_count);
            
            // Construct flake argument
            let flake_arg = format!("{}#{}", flake_ref, attribute_path.join("."));
            cmd.arg(flake_arg);
            
            match crate::util::run_cmd(&mut cmd) {
                Ok(_) => {
                    progress::finish_spinner_success(&pb, "Dry run build successful");
                }
                Err(e) => {
                    progress::finish_spinner_fail(&pb);
                    
                    // Try to extract failed derivation paths
                    let stderr = e.to_string();
                    let failed_drvs = error_handler::find_failed_derivations(&stderr);
                    
                    let mut details = stderr;
                    
                    // If we found failed derivations, try to fetch their logs
                    if !failed_drvs.is_empty() {
                        // Get the first failed derivation log for now
                        if let Ok(log) = error_handler::fetch_and_format_nix_log(
                            &failed_drvs[0], verbose_count
                        ) {
                            details.push_str(&format!("\n\n{}", log));
                        }
                    }
                    
                    error_handler::report_failure(
                        "Dry Run",
                        "Dry run build failed",
                        Some(details),
                        vec![
                            "Fix the build errors reported above".to_string(),
                            "Run with --no-full to skip this check".to_string()
                        ]
                    );
                    bail!("Dry run build failed");
                }
            }
        }
        
        // Continue with the rest of the rebuild...
    }
}
```
- [x] 7.2.2 Test: Run with --medium and --full to verify functionality (with --dry for safety)
```bash
cd nh
# Use --dry to prevent actual system changes
cargo run -- -vv os switch --hostname test --medium --dry  # Should run eval check
cargo run -- -vv os switch --hostname test --full --dry    # Should run eval and dry-run
```

## Phase 8: Build Error Handling

### 8.1 Enhance Build Error Handling
- [x] 8.1.1 Update build section in rebuild to use progress indicators and error handling
```rust
impl OsRebuildArgs {
    fn rebuild(self, variant: OsRebuildVariant, verbose_count: u8) -> Result<()> {
        // Earlier checks...
        
        // Update the build section
        let pb_build = progress::start_spinner("[Build] Building configuration...");
        
        // Use the existing build mechanism but enhance error handling
        let build_result = commands::Build::new(toplevel.clone())
            .extra_arg("--out-link")
            .extra_arg(out_path.get_path())
            .extra_args(&self.extra_args)
            .message("Building NixOS configuration")
            .nom(!self.common.no_nom)
            .run();
            
        if let Err(e) = build_result {
            progress::finish_spinner_fail(&pb_build);
            
            // Try to extract failed derivation paths from error message
            let error_msg = e.to_string();
            let failed_drvs = error_handler::find_failed_derivations(&error_msg);
            
            let mut details = error_msg;
            
            // If we found any failed derivations, try to fetch their logs
            if !failed_drvs.is_empty() {
                if let Ok(log) = error_handler::fetch_and_format_nix_log(&failed_drvs[0], verbose_count) {
                    details.push_str(&format!("\n\n{}", log));
                }
            }
            
            error_handler::report_failure(
                "Build",
                "Failed to build NixOS configuration",
                Some(details),
                vec![
                    "Fix the build errors reported above".to_string(),
                    "Try running with --verbose for more details".to_string()
                ]
            );
            bail!("Build failed");
        }
        
        progress::finish_spinner_success(&pb_build, &format!(
            "Configuration built successfully: {}", 
            out_path.get_path().display()
        ));
        
        // Continue with the rest of the rebuild...
    }
}
```

## Phase 9: Progress Indicators for Existing Steps

### 9.1 Add Progress for Diff and Activation
- [x] 9.1.1 Update diff and activation sections to use progress indicators
```rust
impl OsRebuildArgs {
    fn rebuild(self, variant: OsRebuildVariant, verbose_count: u8) -> Result<()> {
        // Earlier parts with checks and build...
        
        // Update diff section
        let pb_diff = progress::start_spinner("[Diff] Comparing changes...");
        
        let diff_result = Command::new("nvd")
            .arg("diff")
            .arg(CURRENT_PROFILE)
            .arg(&target_profile)
            .message("Comparing changes")
            .run();
            
        if let Err(e) = diff_result {
            progress::finish_spinner_fail(&pb_diff);
            error_handler::report_failure(
                "Diff",
                "Failed to compare configurations",
                Some(e.to_string()),
                vec![
                    "Ensure nvd is installed".to_string(),
                    "Check if the current profile exists".to_string()
                ]
            );
            // Don't bail here, we can proceed without the diff
            warn!("Failed to show diff, continuing anyway");
        } else {
            progress::finish_spinner_success(&pb_diff, "Configuration differences displayed");
        }
        
        // Add remaining progress indicators for activation, etc.
        // ... similar pattern for other steps ...
        
        // Complete the function
        info!("🏆 NixOS {} completed successfully!", 
            match variant {
                Switch => "switch",
                Boot => "boot",
                Test => "test", 
                Build => "build"
            }
        );
        
        Ok(())
    }
}
```

## Phase 10: Clean Mode

### 10.1 Implement Cleanup After Successful Build
- [x] 10.1.1 Add cleanup logic at end of rebuild when --clean flag is used
```rust
impl OsRebuildArgs {
    fn rebuild(self, variant: OsRebuildVariant, verbose_count: u8) -> Result<()> {
        // Earlier parts of rebuild...
        
        // After successful build and activation, add cleanup if requested
        if self.common.clean {
            let pb_clean = progress::start_spinner("[Clean] Cleaning up old generations...");
            
            // Run basic gc with nix store
            let mut gc_cmd = std::process::Command::new("nix");
            gc_cmd.args(["store", "gc"]);
            crate::util::add_verbosity_flags(&mut gc_cmd, verbose_count);
            
            match crate::util::run_cmd(&mut gc_cmd) {
                Ok(_) => {
                    progress::finish_spinner_success(&pb_clean, "Cleanup completed");
                }
                Err(e) => {
                    progress::finish_spinner_fail(&pb_clean);
                    warn!("Cleanup failed: {}", e);
                    // Don't abort on cleanup failure
                }
            }
        }
        
        // Complete the function...
    }
}
```
- [x] 10.1.2 Test: Run with --clean flag to verify functionality (with --dry for safety)
```bash
cd nh
# Use --dry to prevent actual system changes
cargo run -- -vv os switch --hostname test --clean --dry
```

## Phase 11: Apply Changes to Other Modules

### 11.1 Update Home and Darwin Modules
- [x] 11.1.1 Apply similar changes to Home and Darwin modules
- [x] 11.1.2 Test each without and with --medium, --full, --clean flags (with --dry for safety)
```bash
# Test home module with dry run
cargo run -- -vv home switch --dry
cargo run -- -vv home switch --medium --dry
cargo run -- -vv home switch --full --dry
cargo run -- -vv home switch --clean --dry

# Test darwin module with dry run
cargo run -- -vv darwin switch --hostname test --dry
cargo run -- -vv darwin switch --hostname test --medium --dry
cargo run -- -vv darwin switch --hostname test --full --dry
cargo run -- -vv darwin switch --hostname test --clean --dry
```

## Phase 12: Final Refactoring and Optimizations

### 12.1 Refactor Common Code
- [ ] 12.1.1 Move shared logic between nixos.rs, home.rs and darwin.rs to shared utility functions

### 12.2 Error Message Improvements
- [ ] 12.2.1 Review and enhance error messages for clarity and helpfulness

### 12.3 Documentation
- [ ] 12.3.1 Update README.md with new features
- [ ] 12.3.2 Add clear examples of using the new flags

## Final Testing Checklist
- [ ] Verify all flags work correctly
- [ ] Test on real NixOS, home-manager, and nix-darwin configurations
- [ ] Test behavior on non-git directories
- [ ] Test with syntax errors in different files
- [ ] Test with invalid flake references
- [ ] Test on systems with and without formatters/linters installed

## Important Testing Note

**⚠️ VM/Isolated Environment Testing Required**

Some of the tests in this plan involve commands that could potentially modify the host system configuration, particularly:
- `nh os switch` commands that activate NixOS configurations
- `nh home switch` commands that modify home-manager configurations
- `nh darwin switch` commands that modify nix-darwin configurations
- Commands using the `--clean` flag that run garbage collection

These tests should be performed in a virtual machine or other isolated environment to avoid unintended changes to your production system. The code changes themselves can be developed on any system, but the actual execution of system-modifying commands should be done in a safe, isolated environment.

For development purposes, you can:
1. Use the `--dry` flag to preview changes without applying them
2. Create minimal test configurations that don't affect critical system components
3. Use a NixOS VM for testing the full switch process
4. Consider using `nixos-test` framework for automated testing

----

This implementation plan is designed to build the new features in small, testable increments. Each step includes verification to ensure the changes are working as expected before proceeding to the next step. The approach minimizes debugging difficulty by implementing one feature at a time and focusing on clear error reporting.

## Important Note on Testing

When testing commands that would normally modify the system (like `switch` or `boot`), always use the `--dry` flag to prevent actual system changes. This is especially important for:

1. Testing the `os switch`, `home switch`, and `darwin switch` commands
2. Testing the `--clean` flag which runs garbage collection
3. Testing on production systems

The `--dry` flag makes the commands print what they would do without actually doing it, making it safe for testing.

For example:
```bash
# Safe test command that won't modify your system
cargo run -- -vv os switch --hostname test --medium --dry

# Unsafe command that will actually modify your system - use with caution!
cargo run -- -vv os switch --hostname test
```

## Phase 13: UI/UX Enhancements

### 13.1 Create UI Style Module
- [ ] 13.1.1 Create `src/ui_style.rs`: Implement centralized styling utilities
```rust
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
    pub fn success<D: Display>(text: D) -> impl Display {
        text.green()
    }

    /// Error/Failure color (red)
    pub fn error<D: Display>(text: D) -> impl Display {
        text.red()
    }

    // ... other color functions ...
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

    // ... other symbol functions ...
}

/// Helper functions for printing styled messages
pub struct Print;

impl Print {
    /// Print a success message
    pub fn success(msg: &str) {
        eprintln!("{} {}", Colors::success(Symbols::success()), msg);
    }

    // ... other print functions ...
}
```
- [ ] 13.1.2 Add module declaration in `src/lib.rs`
```rust
pub mod ui_style;
```
- [ ] 13.1.3 Create test file for ui_style module
```rust
#[cfg(test)]
mod tests {
    use crate::ui_style::{Colors, Symbols, Print};

    #[test]
    fn test_colors() {
        // Test color functions
    }

    #[test]
    fn test_symbols() {
        // Test symbol functions
    }

    // ... other tests ...
}
```
- [ ] 13.1.4 Add test module declaration in `src/main.rs`
```rust
#[cfg(test)]
mod ui_style_test;
```

### 13.2 Update Progress Module
- [ ] 13.2.1 Modify `src/progress.rs`: Use ui_style for consistent styling
```rust
use crate::ui_style::{Colors, Symbols};

pub fn finish_spinner_success(spinner: &ProgressBar, message: &str) {
    if is_stdout_tty() {
        spinner.finish_with_message(format!("{} {}", Colors::success(Symbols::success()), message));
    } else {
        tracing::info!("{} {}", Colors::success(Symbols::success()), message);
    }
}

// ... other functions ...
```

### 13.3 Update Error Handler Module
- [ ] 13.3.1 Modify `src/error_handler.rs`: Use ui_style for consistent styling
```rust
use crate::ui_style::{Colors, Symbols, Print, separator, header};

pub fn report_failure(stage: &str, reason: &str, details: Option<String>, recommendations: Vec<String>) {
    // Print header with consistent styling
    eprintln!("{}", header(&format!("NH COMMAND ABORTED at Stage: {}", Colors::emphasis(stage))));
    
    // Print reason
    error!("{} {}", Symbols::error(), Colors::emphasis(reason));
    
    // ... rest of function ...
}

// ... other functions ...
```

### 13.4 Update Git Check Module
- [ ] 13.4.1 Modify `src/check_git.rs`: Use ui_style for consistent styling
```rust
use crate::ui_style::{Colors, Symbols, Print};

// ... in run_git_check_warning_only function ...
warn!("{}", Colors::warning("------------------------------------------------------------"));
warn!("{} {}", Symbols::warning(), Colors::warning(Colors::emphasis("Git Warning: Untracked files detected:")));
// ... rest of function ...
```

### 13.5 Update Lint Module
- [ ] 13.5.1 Modify `src/lint.rs`: Use ui_style for consistent styling
```rust
use crate::ui_style::{Colors, Symbols, Print};

// ... in run_lint_checks function ...
info!("{} {}", Symbols::cleanup(), Colors::info("Running Linters and Formatters (Attempting Fixes)..."));
// ... rest of function ...
```

### 13.6 Update CLI Table Formatting
- [ ] 13.6.1 Create helper functions for table formatting
```rust
pub fn format_lint_results_table(summary: &LintSummary) -> String {
    use cli_table::{print_stdout, Table, WithTitle};

    #[derive(Table)]
    struct LintResultRow {
        #[table(title = "Linter")]
        linter: String,
        #[table(title = "Status")]
        status: String,
        #[table(title = "Details")]
        details: String,
    }

    // ... implementation ...
}
```

## Final Testing Checklist (Updated)
- [ ] Verify all flags work correctly
- [ ] Test on real NixOS, home-manager, and nix-darwin configurations
- [ ] Test behavior on non-git directories
- [ ] Test with syntax errors in different files
- [ ] Test with invalid flake references
- [ ] Test on systems with and without formatters/linters installed
- [ ] Verify consistent UI styling across all commands
- [ ] Test in different terminal emulators and with different color schemes
- [ ] Test with and without TTY (cargo run -- ... | cat)