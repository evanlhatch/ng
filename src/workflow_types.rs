use crate::installable::Installable;
use std::path::PathBuf;
use std::ffi::OsString;

/// This struct centralizes common arguments for any rebuild-like operation.
/// It's derived from CLI parsing and environment variables.
#[derive(Debug, Clone)]
pub struct CommonRebuildArgs {
    /// The installable to build/rebuild
    pub installable: Installable,
    
    /// Skip pre-flight checks
    pub no_preflight: bool,
    
    /// Enforce strict linting
    pub strict_lint: Option<bool>, // Changed
    
    /// Enforce strict formatting
    pub strict_format: Option<bool>, // Changed
    
    /// Run medium-level checks
    pub medium_checks: bool, // Renamed from 'medium' for clarity
    
    /// Run full checks
    pub full_checks: bool,   // Renamed from 'full' for clarity
    
    /// Perform a dry run without making changes
    pub dry_run: bool,       // Renamed from 'dry'
    
    /// Ask for confirmation before applying changes
    pub ask_confirmation: bool, // Renamed from 'ask'
    
    /// Skip using nom for build output formatting
    pub no_nom: bool,
    
    /// Path to create the output link at
    pub out_link: Option<PathBuf>,
    
    /// Clean up after rebuilding
    pub clean_after: bool,   // Renamed from 'clean'
    
    /// Extra arguments to pass to nix build
    pub extra_build_args: Vec<OsString>,
}