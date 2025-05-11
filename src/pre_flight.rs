use std::collections::HashMap; // Added for the map
use std::path::PathBuf; // ADDED
// use std::path::PathBuf; // Removed
use std::sync::Arc;

use color_eyre::eyre::bail;
use regex::Regex; // Added for Deadnix parsing
use serde_json; // Added for Statix JSON parsing
use tracing::{debug, info, warn};

use crate::context::OperationContext;
use crate::error_handler;
use crate::external_linter_types; // Added for Statix types
use crate::nix_analyzer::{NgDiagnostic, NgSeverity, NixAnalysisContext}; // Ensure NgDiagnostic is here
use crate::progress;
use crate::util; // For command_exists and find_nix_files_walkdir
use crate::workflow_strategy::PlatformRebuildStrategy;
use crate::Result;

/// Represents the status of a pre-flight check
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatusReport {
    Passed,
    PassedWithWarnings,
    FailedCritical,
}

/// Trait for pre-flight checks
pub trait PreFlightCheck: std::fmt::Debug + Send + Sync {
    fn name(&self) -> &str;
    fn run<S: PlatformRebuildStrategy>(
        &self,
        op_ctx: &OperationContext,
        platform_strategy: &S,
        platform_args: &S::PlatformArgs,
    ) -> Result<CheckStatusReport>;
}

// Enum wrapper to make PreFlightCheck usable without dyn Trait
#[derive(Debug)]
pub enum AnyPreFlightCheck {
    NixParse(NixParsePreFlightCheck),
    Semantic(SemanticPreFlightCheck),
    NixFormat(NixFormatPreFlightCheck),
    ExternalLinters(ExternalLintersPreFlightCheck), // ADDED
}

impl AnyPreFlightCheck {
    pub fn name(&self) -> &str {
        match self {
            AnyPreFlightCheck::NixParse(c) => c.name(),
            AnyPreFlightCheck::Semantic(c) => c.name(),
            AnyPreFlightCheck::NixFormat(c) => c.name(),
            AnyPreFlightCheck::ExternalLinters(c) => c.name(), // ADDED
        }
    }

    pub fn run<S: PlatformRebuildStrategy>(
        &self,
        op_ctx: &OperationContext,
        platform_strategy: &S,
        platform_args: &S::PlatformArgs,
    ) -> Result<CheckStatusReport> {
        match self {
            AnyPreFlightCheck::NixParse(c) => c.run(op_ctx, platform_strategy, platform_args),
            AnyPreFlightCheck::Semantic(c) => c.run(op_ctx, platform_strategy, platform_args),
            AnyPreFlightCheck::NixFormat(c) => c.run(op_ctx, platform_strategy, platform_args),
            AnyPreFlightCheck::ExternalLinters(c) => {
                c.run(op_ctx, platform_strategy, platform_args)
            } // ADDED
        }
    }
}

/// Pre-flight check for Nix syntax using nil-syntax
#[derive(Debug)]
pub struct NixParsePreFlightCheck;

impl PreFlightCheck for NixParsePreFlightCheck {
    fn name(&self) -> &str {
        "Nix Syntax Parse"
    }

    fn run<S: PlatformRebuildStrategy>(
        &self,
        op_ctx: &OperationContext,
        _platform_strategy: &S,
        _platform_args: &S::PlatformArgs,
    ) -> Result<CheckStatusReport> {
        debug!("Running NixParsePreFlightCheck...");
        let mut analyzer = NixAnalysisContext::new();
        let nix_files = util::find_nix_files_walkdir(&op_ctx.get_effective_project_root())?;
        if nix_files.is_empty() {
            info!("No .nix files found for syntax check.");
            return Ok(CheckStatusReport::Passed);
        }
        debug!("Found {} .nix files for syntax check", nix_files.len());
        let mut all_diagnostics = Vec::new();
        for path in nix_files {
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => Arc::new(c),
                Err(e) => {
                    warn!(
                        "Failed to read Nix file {} for parsing: {}",
                        path.display(),
                        e
                    );
                    continue;
                }
            };
            let (file_id, _source_file, syntax_errors) =
                analyzer.parse_file_with_syntax(&path, content);
            for error in &syntax_errors {
                all_diagnostics
                    .push(analyzer.convert_nil_syntax_error_to_ng(error, file_id, &path));
            }
        }
        if !all_diagnostics.is_empty() {
            error_handler::report_ng_diagnostics("Syntax Check", &all_diagnostics, Some(&analyzer));
            return Ok(CheckStatusReport::FailedCritical);
        }
        Ok(CheckStatusReport::Passed)
    }
}

/// Pre-flight check for Nix semantics using nil-ide
#[derive(Debug)]
pub struct SemanticPreFlightCheck;

impl PreFlightCheck for SemanticPreFlightCheck {
    fn name(&self) -> &str {
        "Nix Semantic Check"
    }

    fn run<S: PlatformRebuildStrategy>(
        &self,
        op_ctx: &OperationContext,
        _platform_strategy: &S,
        _platform_args: &S::PlatformArgs,
    ) -> Result<CheckStatusReport> {
        debug!("Running SemanticPreFlightCheck...");
        let mut analyzer = NixAnalysisContext::new();
        let nix_files = util::find_nix_files_walkdir(&op_ctx.get_effective_project_root())?;
        if nix_files.is_empty() {
            info!("No .nix files found for semantic check.");
            return Ok(CheckStatusReport::Passed);
        }
        debug!("Found {} .nix files for semantic check", nix_files.len());
        let mut all_diagnostics = Vec::new();
        for path in nix_files {
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => Arc::new(c),
                Err(e) => {
                    warn!(
                        "Failed to read Nix file {} for semantic analysis: {}",
                        path.display(),
                        e
                    );
                    continue;
                }
            };
            let (file_id, _source_file, syntax_errors) =
                analyzer.parse_file_with_syntax(&path, content);
            if !syntax_errors.is_empty() {
                debug!(
                    "Skipping semantic analysis for {} due to syntax errors",
                    path.display()
                );
                for error in &syntax_errors {
                    all_diagnostics
                        .push(analyzer.convert_nil_syntax_error_to_ng(error, file_id, &path));
                }
                continue;
            }
            match analyzer.get_semantic_diagnostics(file_id) {
                Ok(semantic_diagnostics_vec) => {
                    for diag in &semantic_diagnostics_vec {
                        all_diagnostics
                            .push(analyzer.convert_nil_diagnostic_to_ng(diag, file_id, &path));
                    }
                }
                Err(_cancelled) => {
                    warn!("Semantic analysis for {} was cancelled.", path.display());
                }
            }
        }
        if all_diagnostics.is_empty() {
            return Ok(CheckStatusReport::Passed);
        }
        let has_errors = all_diagnostics
            .iter()
            .any(|d| matches!(d.severity, NgSeverity::Error));
        error_handler::report_ng_diagnostics("Semantic Check", &all_diagnostics, Some(&analyzer));
        // Use strict_lint from config, falling back to CommonArgs, then false.
        let use_strict_lint = match op_ctx.common_args.strict_lint {
            Some(cli_value) => cli_value, // CLI override
            None => op_ctx.config.pre_flight.strict_lint.unwrap_or(false), // Config, then default
        };
        if has_errors && use_strict_lint {
            Ok(CheckStatusReport::FailedCritical)
        } else if has_errors {
            warn!("Semantic errors found, but continuing due to non-strict mode");
            Ok(CheckStatusReport::PassedWithWarnings)
        } else {
            Ok(CheckStatusReport::PassedWithWarnings)
        }
    }
}

/// Pre-flight check for Nix code formatting using nixfmt
#[derive(Debug)]
pub struct NixFormatPreFlightCheck;

#[derive(Debug, Clone, Copy)] // ADDED struct
pub struct ExternalLintersPreFlightCheck;

impl PreFlightCheck for NixFormatPreFlightCheck {
    fn name(&self) -> &str {
        "Nix Code Format"
    }

    fn run<S: PlatformRebuildStrategy>(
        &self,
        op_ctx: &OperationContext,
        _platform_strategy: &S,
        _platform_args: &S::PlatformArgs,
    ) -> Result<CheckStatusReport> {
        debug!("Running NixFormatPreFlightCheck...");

        // Determine formatter tool from config, default to "auto" then "nixfmt"
        let formatter_tool_config = op_ctx
            .config
            .pre_flight
            .format
            .tool
            .as_deref()
            .unwrap_or("auto");
        let formatter_bin = match formatter_tool_config {
            "auto" => "nixfmt", // For now, auto just means nixfmt. Later: detect alejandra, etc.
            other => other,
        };

        // Check if formatter exists by trying a benign command like --version.
        // This uses crate::commands::Command and is thus mockable.
        let existence_check_cmd = crate::commands::Command::new(formatter_bin).arg("--version");
        // In tests, the first mock set by set_mock_run_result will apply to this.
        match existence_check_cmd.run() {
            Ok(_) => { /* Formatter likely exists and --help ran ok */ }
            Err(_e) => {
                // Could be "not found" or "help failed for other reasons"
                warn!(
                    "Formatter '{}' not found or non-functional (e.g., --help failed). Skipping format check. Error: {}. Please install it or configure via ng.toml.",
                    formatter_bin, _e
                );
                return Ok(CheckStatusReport::PassedWithWarnings);
            }
        }

        let effective_root = op_ctx.get_effective_project_root();
        let nix_files = match util::find_nix_files_walkdir(&effective_root) {
            Ok(files) => files,
            Err(e) => {
                warn!("Failed to find .nix files for formatting check: {}", e); // Restored warn!
                return Ok(CheckStatusReport::PassedWithWarnings);
            }
        };

        if nix_files.is_empty() {
            info!("No .nix files found for format check.");
            return Ok(CheckStatusReport::Passed);
        }

        debug!(
            "Found {} .nix files for format check with {}",
            nix_files.len(),
            formatter_bin
        );
        let mut unformatted_files = Vec::new();

        for path in &nix_files {
            let mut cmd = crate::commands::Command::new(formatter_bin);
            cmd = cmd.arg("--check").arg(path);

            match cmd.run() {
                Ok(_) => { /* File is formatted */ }
                Err(_e) => {
                    debug!(
                        "File considered unformatted by '{}': {}",
                        formatter_bin,
                        path.display()
                    ); // Back to debug!
                    unformatted_files.push(path.clone());
                }
            }
        }

        if !unformatted_files.is_empty() {
            warn!("Found unformatted Nix files:");
            for file_path in &unformatted_files {
                warn!("  - {}", file_path.display());
            }
            warn!(
                "Please run '{} <file>' to format them or use 'nh fix format'.",
                formatter_bin
            );

            // Use strict_format from config, falling back to CommonArgs.strict_format.
            // The CommonArgs.strict_format itself would default to false if not set by CLI.
            let use_strict_format = match op_ctx.common_args.strict_format {
                Some(cli_value) => cli_value, // CLI override
                None => op_ctx.config.pre_flight.strict_format.unwrap_or(false), // Config, then default
            };

            if use_strict_format {
                return Ok(CheckStatusReport::FailedCritical);
            }
            return Ok(CheckStatusReport::PassedWithWarnings);
        }

        Ok(CheckStatusReport::Passed)
    }
}

// ADDED PreFlightCheck impl for ExternalLintersPreFlightCheck
impl PreFlightCheck for ExternalLintersPreFlightCheck {
    fn name(&self) -> &str {
        "External Linters"
    }

    fn run<S: PlatformRebuildStrategy>(
        &self,
        op_ctx: &OperationContext,
        _platform_strategy: &S,
        _platform_args: &S::PlatformArgs,
    ) -> Result<CheckStatusReport, color_eyre::Report> {
        debug!("Running ExternalLintersPreFlightCheck...");
        let mut diagnostics: Vec<NgDiagnostic> = Vec::new(); // Initialize diagnostics vector

        let enabled_linters = op_ctx
            .config
            .pre_flight
            .external_linters
            .enable
            .as_deref()
            .unwrap_or(&[]);

        if enabled_linters.is_empty() {
            info!("No external linters enabled in ng.toml ([pre_flight.external_linters.enable]). Skipping External Linters check.");
            return Ok(CheckStatusReport::Passed);
        }

        info!(
            "External linters to run based on config: {:?}",
            enabled_linters
        );
        let mut ran_any_linter = false;
        let _had_errors = false; // This can be determined from diagnostics severity later

        let project_root_path = op_ctx
            .project_root
            .as_ref()
            .map_or_else(|| PathBuf::from("."), |p| p.clone());

        // --- Statix ---
        if enabled_linters
            .iter()
            .any(|name| name.eq_ignore_ascii_case("statix"))
        {
            ran_any_linter = true;
            info!("Checking for Statix...");
            if util::command_exists("statix") {
                // let project_root_path = op_ctx.project_root.as_ref().map_or_else(|| PathBuf::from("."), |p| p.clone()); // Defined above
                info!(
                    "Running Statix check on project root: {:?}",
                    project_root_path
                );
                let statix_path = op_ctx.config.pre_flight.external_linters.statix_path.as_deref().unwrap_or("statix");
                let cmd = crate::commands::Command::new(statix_path)
                    .arg("check")
                    .arg("--json") // Added --json flag
                    .arg(&project_root_path);

                match cmd.run_capture_output() {
                    Ok(output) => {
                        if !output.status.success() {
                            // Statix found issues, try to parse JSON output
                            match serde_json::from_slice::<
                                Vec<external_linter_types::StatixDiagnostic>,
                            >(&output.stdout)
                            {
                                Ok(statix_diagnostics) => {
                                    if statix_diagnostics.is_empty() && !output.status.success() {
                                        warn!("Statix command failed but returned empty JSON. Stderr: {}. Stdout: {}",
                                            String::from_utf8_lossy(&output.stderr),
                                            String::from_utf8_lossy(&output.stdout)
                                        );
                                        diagnostics.push(NgDiagnostic {
                                            tool_name: Some("statix".to_string()),
                                            file_path: project_root_path.to_path_buf(), // Use PathBuf directly
                                            message: format!("Statix failed with exit code {:?} but reported no specific issues in JSON. Stderr: {}. Stdout: {}",
                                                output.status.code(), String::from_utf8_lossy(&output.stderr), String::from_utf8_lossy(&output.stdout)),
                                            line: None, column: None,
                                            severity: NgSeverity::Error, // Default to error
                                        });
                                    } else {
                                        for diag_item in statix_diagnostics {
                                            let severity =
                                                if diag_item.severity.eq_ignore_ascii_case("error")
                                                {
                                                    NgSeverity::Error
                                                } else {
                                                    NgSeverity::Warning
                                                };
                                            diagnostics.push(NgDiagnostic {
                                                tool_name: Some("statix".to_string()),
                                                file_path: PathBuf::from(diag_item.file),
                                                message: diag_item.message,
                                                line: Some(diag_item.position.start_line),
                                                column: Some(diag_item.position.start_col),
                                                severity,
                                            });
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to parse Statix JSON output: {}. Statix exit code: {:?}. Stderr: {}. Stdout: {}",
                                        e, output.status.code(), String::from_utf8_lossy(&output.stderr), String::from_utf8_lossy(&output.stdout));
                                    diagnostics.push(NgDiagnostic {
                                        tool_name: Some("statix".to_string()),
                                        file_path: project_root_path.to_path_buf(),
                                        message: format!("Statix found issues (exit code {:?}), but output parsing failed: {}.\nStdout: {}\nStderr: {}",
                                            output.status.code().unwrap_or(-1), e,
                                            String::from_utf8_lossy(&output.stdout),
                                            String::from_utf8_lossy(&output.stderr)),
                                        line: None, column: None,
                                        severity: NgSeverity::Error, // Parsing failure implies an error
                                    });
                                }
                            }
                        } else {
                            if !output.stdout.is_empty() {
                                match serde_json::from_slice::<
                                    Vec<external_linter_types::StatixDiagnostic>,
                                >(&output.stdout)
                                {
                                    Ok(statix_diagnostics) => {
                                        for diag_item in statix_diagnostics {
                                            let severity =
                                                if diag_item.severity.eq_ignore_ascii_case("error")
                                                {
                                                    NgSeverity::Error
                                                } else {
                                                    NgSeverity::Warning
                                                };
                                            diagnostics.push(NgDiagnostic {
                                                tool_name: Some("statix".to_string()),
                                                file_path: PathBuf::from(diag_item.file),
                                                message: diag_item.message,
                                                line: Some(diag_item.position.start_line),
                                                column: Some(diag_item.position.start_col),
                                                severity,
                                            });
                                        }
                                        if !diagnostics.is_empty()
                                            && !diagnostics
                                                .iter()
                                                .any(|d| d.severity == NgSeverity::Error)
                                        {
                                            info!("Statix reported warnings but passed overall.");
                                        }
                                    }
                                    Err(e) => {
                                        warn!("Statix command succeeded but failed to parse potential JSON warnings: {}. Stdout: {}", e, String::from_utf8_lossy(&output.stdout));
                                    }
                                }
                            }
                            if diagnostics.is_empty() {
                                info!("Statix check passed.");
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to run Statix: {}. Skipping Statix check.", e);
                    }
                }
            } else {
                warn!("Statix is configured but not found in PATH.");
            }
        }

        // --- Deadnix ---
        if enabled_linters
            .iter()
            .any(|name| name.eq_ignore_ascii_case("deadnix"))
        {
            ran_any_linter = true;
            info!("Checking for Deadnix...");
            if util::command_exists("deadnix") {
                info!("Running Deadnix on project root: {:?}", project_root_path);
                let deadnix_path = op_ctx.config.pre_flight.external_linters.deadnix_path.as_deref().unwrap_or("deadnix");
                let cmd = crate::commands::Command::new(deadnix_path)
                    .current_dir(&project_root_path)
                    .arg("--check");

                match cmd.run_capture_output() {
                    Ok(output) => {
                        if !output.status.success() {
                            // Deadnix found issues or failed
                            let stdout_str = String::from_utf8_lossy(&output.stdout);
                            let stderr_str = String::from_utf8_lossy(&output.stderr);
                            warn!(
                                "Deadnix found issues (exit code: {:?}):\nStdout:\n{}\nStderr:\n{}",
                                output.status.code().unwrap_or(-1),
                                stdout_str,
                                stderr_str
                            );

                            let mut deadnix_found_specific_issues = false;
                            // Regex for lines like: path:line:col: message
                            // Example: src/main.nix:12:5: Unused binding `foo`
                            let re = Regex::new(r"^(.*?):(\d+):(\d+):\s*(.*)$").unwrap(); // Unwrap is fine for a static known-good regex
                            for line_content in stdout_str.lines() {
                                if let Some(caps) = re.captures(line_content.trim()) {
                                    if caps.len() == 5 {
                                        // Full match + 4 capture groups
                                        let file =
                                            caps.get(1).map_or("", |m| m.as_str()).to_string();
                                        let line_num_str = caps.get(2).map_or("0", |m| m.as_str());
                                        let col_num_str = caps.get(3).map_or("0", |m| m.as_str());
                                        let msg =
                                            caps.get(4).map_or("", |m| m.as_str()).to_string();

                                        let line_num = line_num_str.parse::<u32>().ok();
                                        let col_num = col_num_str.parse::<u32>().ok();

                                        // Deadnix usually reports errors
                                        diagnostics.push(NgDiagnostic {
                                            tool_name: Some("deadnix".to_string()),
                                            file_path: project_root_path.join(file), // Path from deadnix is relative to CWD
                                            message: msg,
                                            line: line_num,
                                            column: col_num,
                                            severity: NgSeverity::Error, // Assume errors from deadnix
                                        });
                                        deadnix_found_specific_issues = true;
                                    }
                                }
                            }

                            if !deadnix_found_specific_issues {
                                // If no specific issues parsed but command failed, add a generic one
                                diagnostics.push(NgDiagnostic {
                                    tool_name: Some("deadnix".to_string()),
                                    file_path: project_root_path.to_path_buf(),
                                    message: format!("Deadnix failed with exit code {:?}.\nStdout:\n{}\nStderr:\n{}",
                                        output.status.code().unwrap_or(-1), stdout_str, stderr_str),
                                    line: None, column: None,
                                    severity: NgSeverity::Error,
                                });
                            }
                        } else {
                            info!("Deadnix check passed.");
                        }
                    }
                    Err(e) => {
                        warn!("Failed to run Deadnix: {}. Skipping Deadnix check.", e);
                    }
                }
            } else {
                warn!("Deadnix is configured but not found in PATH.");
            }
        }

        if !ran_any_linter && diagnostics.is_empty() {
            info!("No configured external linters were found or enabled to run, and no prior diagnostics.");
            return Ok(CheckStatusReport::Passed);
        }

        let final_had_errors = diagnostics.iter().any(|d| d.severity == NgSeverity::Error);

        if !diagnostics.is_empty() {
            // Create a dummy NixAnalysisContext for now, as ExternalLinters don't use its specific features
            // for reporting (like file_id_for_db or complex range to line/col conversion within the reporter).
            // The NgDiagnostics from external linters already have file_path and line/col.
            error_handler::report_ng_diagnostics(
                "External Linters",
                &diagnostics,
                None,
            );
        }

        if final_had_errors {
            let use_strict_lint = match op_ctx.common_args.strict_lint {
                Some(cli_value) => cli_value,
                None => op_ctx.config.pre_flight.strict_lint.unwrap_or(false),
            };
            if use_strict_lint {
                return Ok(CheckStatusReport::FailedCritical);
            }
            return Ok(CheckStatusReport::PassedWithWarnings);
        } else if !diagnostics.is_empty() {
            return Ok(CheckStatusReport::PassedWithWarnings);
        }

        Ok(CheckStatusReport::Passed)
    }
}

// Helper to build a map of all available checks by their canonical names
fn get_available_checks_map() -> HashMap<String, AnyPreFlightCheck> {
    let mut checks_map = HashMap::new();

    let parse_check = NixParsePreFlightCheck;
    checks_map.insert(
        parse_check.name().to_string(),
        AnyPreFlightCheck::NixParse(parse_check),
    );

    let semantic_check = SemanticPreFlightCheck;
    checks_map.insert(
        semantic_check.name().to_string(),
        AnyPreFlightCheck::Semantic(semantic_check),
    );

    let format_check = NixFormatPreFlightCheck;
    checks_map.insert(
        format_check.name().to_string(),
        AnyPreFlightCheck::NixFormat(format_check),
    );

    let external_linters_check = ExternalLintersPreFlightCheck; // ADDED
    checks_map.insert(
        external_linters_check.name().to_string(),
        AnyPreFlightCheck::ExternalLinters(external_linters_check),
    ); // ADDED

    checks_map
}

/// Runs the shared pre-flight checks based on ng.toml configuration.
pub fn run_shared_pre_flight_checks<S: PlatformRebuildStrategy>(
    op_ctx: &OperationContext,
    platform_strategy: &S,
    platform_args: &S::PlatformArgs,
) -> Result<()> {
    if op_ctx.common_args.no_preflight {
        info!("[⏭️ Pre-flight] All checks skipped due to --no-preflight.");
        return Ok(());
    }

    let available_checks_map = get_available_checks_map();

    let default_checks = vec![
        NixParsePreFlightCheck.name().to_string(),
        SemanticPreFlightCheck.name().to_string(),
        NixFormatPreFlightCheck.name().to_string(),
    ];

    let checks_to_run_names: Vec<String> = op_ctx
        .config
        .pre_flight
        .checks
        .clone()
        .unwrap_or(default_checks);

    if checks_to_run_names.is_empty() {
        info!("[ℹ️ Pre-flight] No checks configured to run.");
        return Ok(());
    }

    info!(
        "🚀 Running pre-flight checks for {} based on configuration: {:?}...",
        platform_strategy.name(),
        checks_to_run_names
    );
    let mut overall_status = CheckStatusReport::Passed;

    for check_name in checks_to_run_names {
        if let Some(check) = available_checks_map.get(&check_name) {
            debug!("Executing pre-flight check: {}...", check.name());
            let pb =
                progress::start_spinner(&format!("[Pre-flight] Running {} check", check.name()));

            match check.run(op_ctx, platform_strategy, platform_args) {
                Ok(CheckStatusReport::Passed) => {
                    progress::finish_spinner_success(
                        &pb,
                        &format!("{} check passed", check.name()),
                    );
                }
                Ok(CheckStatusReport::PassedWithWarnings) => {
                    progress::finish_spinner_success(
                        &pb,
                        &format!("{} check passed with warnings", check.name()),
                    );
                    if overall_status == CheckStatusReport::Passed {
                        overall_status = CheckStatusReport::PassedWithWarnings;
                    }
                }
                Ok(CheckStatusReport::FailedCritical) => {
                    progress::finish_spinner_fail(&pb);
                    bail!(
                        "Critical pre-flight check '{}' failed. Aborting.",
                        check.name()
                    );
                }
                Err(e) => {
                    progress::finish_spinner_fail(&pb);
                    error_handler::report_failure(
                        "Pre-flight System Error",
                        &format!("Check '{}' failed to execute", check.name()),
                        Some(e.to_string()),
                        vec!["This indicates an issue with ng itself or its environment."
                            .to_string()],
                    );
                    bail!(
                        "Error executing pre-flight check '{}'. Aborting.",
                        check.name()
                    );
                }
            }
        } else {
            warn!("[⚠️ Pre-flight] Unknown check '{}' specified in ng.toml or default list. Skipping.", check_name);
        }
    }

    match overall_status {
        CheckStatusReport::Passed => info!(
            "✅ All pre-flight checks passed for {}.",
            platform_strategy.name()
        ),
        CheckStatusReport::PassedWithWarnings => info!(
            "⚠️ Pre-flight checks for {} completed with warnings.",
            platform_strategy.name()
        ),
        CheckStatusReport::FailedCritical => {} // Should have bailed already
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    // TODO: Add tests for config-driven pre-flight checks
    // Need to mock OperationContext with different NgConfig values.

    #[test]
    fn test_check_status_report() {
        assert_ne!(
            CheckStatusReport::Passed,
            CheckStatusReport::PassedWithWarnings
        );
        assert_ne!(CheckStatusReport::Passed, CheckStatusReport::FailedCritical);
        assert_ne!(
            CheckStatusReport::PassedWithWarnings,
            CheckStatusReport::FailedCritical
        );
    }
}
