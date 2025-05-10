use crate::Result;
use crate::context::OperationContext;
use crate::workflow_strategy::PlatformRebuildStrategy;
use crate::error_handler;
// Phase 3: Will be uncommented when implementing nil integration
// use crate::nix_analyzer::{NixAnalysisContext, NgDiagnostic, NgSeverity};
use crate::progress;
// Phase 3: Will be uncommented when implementing nil integration
// use nil_ide::config::Config as NilConfig;
use tracing::{info, warn, debug, error};
use color_eyre::eyre::{bail, eyre};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use walkdir::WalkDir;

/// Represents the status of a pre-flight check
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatusReport {
    /// Check passed without issues
    Passed,
    
    /// Check passed but with warnings
    PassedWithWarnings,
    
    /// Check failed critically and should halt the process
    FailedCritical,
}

/// Trait for pre-flight checks
pub trait PreFlightCheck: std::fmt::Debug + Send + Sync {
    /// Returns the name of the check
    fn name(&self) -> &str;
    
    /// Runs the check
    ///
    /// # Arguments
    ///
    /// * `op_ctx` - The operation context
    /// * `platform_strategy` - The platform strategy
    /// * `platform_args` - The platform-specific arguments
    ///
    /// # Returns
    ///
    /// * `Result<CheckStatusReport>` - The status of the check
    fn run<S: PlatformRebuildStrategy>(
        &self,
        op_ctx: &OperationContext,
        platform_strategy: &S,
        platform_args: &S::PlatformArgs,
    ) -> Result<CheckStatusReport>;
}

/// Finds all .nix files in the current directory
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

/// Checks if a directory entry is hidden
fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry.file_name()
        .to_str()
        .map_or(false, |s| s.starts_with('.'))
        || entry.path().components().any(|c| {
            c.as_os_str().to_str().map_or(false, |s| s.starts_with('.'))
        })
}

/// Pre-flight check for Nix syntax using nil-syntax
#[derive(Debug)]
pub struct NixParsePreFlightCheck;

impl PreFlightCheck for NixParsePreFlightCheck {
    fn name(&self) -> &str { "Nix Syntax Parse" }
    
    fn run<S: PlatformRebuildStrategy>(
        &self,
        op_ctx: &OperationContext,
        _platform_strategy: &S,
        _platform_args: &S::PlatformArgs,
    ) -> Result<CheckStatusReport> {
        debug!("Running NixParsePreFlightCheck...");
        
        // Create a new NixAnalysisContext
        let mut analyzer = NixAnalysisContext::new();
        
        // Find all .nix files
        let nix_files = find_nix_files()?;
        if nix_files.is_empty() {
            info!("No .nix files found to check.");
            return Ok(CheckStatusReport::Passed);
        }
        
        debug!("Found {} .nix files to check", nix_files.len());
        
        // Parse each file and collect syntax errors
        let mut all_diagnostics = Vec::new();
        
        for path in nix_files {
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => Arc::new(c),
                Err(e) => {
                    warn!("Failed to read Nix file {} for parsing: {}", path.display(), e);
                    continue;
                }
            };
            
            let (file_id, _source_file, syntax_errors) = analyzer.parse_file_with_syntax(&path, content);
            
            // Convert syntax errors to NgDiagnostic
            for error in &syntax_errors {
                all_diagnostics.push(analyzer.convert_nil_syntax_error_to_ng(error, file_id, &path));
            }
        }
        
        // Report diagnostics if any
        if !all_diagnostics.is_empty() {
            error_handler::report_ng_diagnostics("Syntax Check", &all_diagnostics, &analyzer);
            return Ok(CheckStatusReport::FailedCritical);
        }
        
        Ok(CheckStatusReport::Passed)
    }
}

/// Pre-flight check for Nix semantics using nil-ide
#[derive(Debug)]
pub struct SemanticPreFlightCheck;

impl PreFlightCheck for SemanticPreFlightCheck {
    fn name(&self) -> &str { "Nix Semantic Check" }
    
    fn run<S: PlatformRebuildStrategy>(
        &self,
        op_ctx: &OperationContext,
        _platform_strategy: &S,
        _platform_args: &S::PlatformArgs,
    ) -> Result<CheckStatusReport> {
        debug!("Running SemanticPreFlightCheck...");
        
        // Create a new NixAnalysisContext
        let mut analyzer = NixAnalysisContext::new();
        
        // Find all .nix files
        let nix_files = find_nix_files()?;
        if nix_files.is_empty() {
            info!("No .nix files found to check.");
            return Ok(CheckStatusReport::Passed);
        }
        
        debug!("Found {} .nix files to check", nix_files.len());
        
        // Parse each file and collect semantic diagnostics
        let mut all_diagnostics = Vec::new();
        let nil_config = NilConfig::default();
        
        for path in nix_files {
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => Arc::new(c),
                Err(e) => {
                    warn!("Failed to read Nix file {} for semantic analysis: {}", path.display(), e);
                    continue;
                }
            };
            
            // First parse for syntax errors
            let (file_id, _source_file, syntax_errors) = analyzer.parse_file_with_syntax(&path, content);
            
            // Skip semantic analysis if there are syntax errors
            if !syntax_errors.is_empty() {
                debug!("Skipping semantic analysis for {} due to syntax errors", path.display());
                continue;
            }
            
            // Get semantic diagnostics
            let semantic_diagnostics = analyzer.get_semantic_diagnostics(file_id, &nil_config);
            
            // Convert semantic diagnostics to NgDiagnostic
            for diag in &semantic_diagnostics {
                all_diagnostics.push(analyzer.convert_nil_diagnostic_to_ng(diag, file_id, &path));
            }
        }
        
        // Determine check status based on diagnostics
        if all_diagnostics.is_empty() {
            return Ok(CheckStatusReport::Passed);
        }
        
        // Check if there are any errors (vs just warnings)
        let has_errors = all_diagnostics.iter().any(|d| matches!(d.severity, NgSeverity::Error));
        
        // Report diagnostics
        error_handler::report_ng_diagnostics("Semantic Check", &all_diagnostics, &analyzer);
        
        // Return appropriate status
        if has_errors && op_ctx.common_args.strict_lint {
            Ok(CheckStatusReport::FailedCritical)
        } else if has_errors {
            warn!("Semantic errors found, but continuing due to non-strict mode");
            Ok(CheckStatusReport::PassedWithWarnings)
        } else {
            Ok(CheckStatusReport::PassedWithWarnings)
        }
    }
}

/// Returns the core pre-flight checks to run
pub fn get_core_pre_flight_checks(_op_ctx: &OperationContext) -> Vec<Box<dyn PreFlightCheck>> {
    vec![
        Box::new(NixParsePreFlightCheck),
        Box::new(SemanticPreFlightCheck),
        // Add more checks here as needed
    ]
}

/// Runs the shared pre-flight checks
///
/// # Arguments
///
/// * `op_ctx` - The operation context
/// * `platform_strategy` - The platform strategy
/// * `platform_args` - The platform-specific arguments
/// * `checks_to_run` - The checks to run
///
/// # Returns
///
/// * `Result<()>` - Success or an error
pub fn run_shared_pre_flight_checks<S: PlatformRebuildStrategy>(
    op_ctx: &OperationContext,
    platform_strategy: &S,
    platform_args: &S::PlatformArgs,
    checks_to_run: &[Box<dyn PreFlightCheck>],
) -> Result<()> {
    if op_ctx.common_args.no_preflight {
        info!("[⏭️ Pre-flight] All checks skipped due to --no-preflight.");
        return Ok(());
    }
    
    info!("🚀 Running shared pre-flight checks for {}...", platform_strategy.name());
    let mut overall_status = CheckStatusReport::Passed;
    
    for check in checks_to_run {
        debug!("Executing pre-flight check: {}...", check.name());
        let pb = progress::start_spinner(&format!("[Pre-flight] Running {} check", check.name()));
        
        match check.run(op_ctx, platform_strategy, platform_args) {
            Ok(CheckStatusReport::Passed) => {
                progress::finish_spinner_success(&pb, &format!("{} check passed", check.name()));
            }
            Ok(CheckStatusReport::PassedWithWarnings) => {
                progress::finish_spinner_success(&pb, &format!("{} check passed with warnings", check.name()));
                if overall_status == CheckStatusReport::Passed {
                    overall_status = CheckStatusReport::PassedWithWarnings;
                }
            }
            Ok(CheckStatusReport::FailedCritical) => {
                progress::finish_spinner_fail(&pb);
                // Error message should have been printed by the check itself using ErrorHandler
                bail!("Critical pre-flight check '{}' failed. Aborting.", check.name());
            }
            Err(e) => {
                progress::finish_spinner_fail(&pb);
                error_handler::report_failure(
                    "Pre-flight System Error",
                    &format!("Check '{}' failed to execute", check.name()),
                    Some(e.to_string()),
                    vec!["This indicates an issue with ng itself or its environment.".to_string()]
                );
                bail!("Error executing pre-flight check '{}'. Aborting.", check.name());
            }
        }
    }
    
    match overall_status {
        CheckStatusReport::Passed => info!("✅ All pre-flight checks passed for {}.", platform_strategy.name()),
        CheckStatusReport::PassedWithWarnings => info!("⚠️ Pre-flight checks for {} completed with warnings.", platform_strategy.name()),
        CheckStatusReport::FailedCritical => {} // Should have bailed already
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_check_status_report() {
        assert_ne!(CheckStatusReport::Passed, CheckStatusReport::PassedWithWarnings);
        assert_ne!(CheckStatusReport::Passed, CheckStatusReport::FailedCritical);
        assert_ne!(CheckStatusReport::PassedWithWarnings, CheckStatusReport::FailedCritical);
    }
}