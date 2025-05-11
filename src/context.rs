use crate::workflow_types::CommonRebuildArgs;
use crate::interface::UpdateArgs;
use crate::nix_interface::NixInterface;
use crate::config::NgConfig;
use std::sync::Arc;
use std::path::PathBuf; // For project_root

/// Holds the operational context for a given ng command run.
/// Passed around to workflow steps and strategies.
#[derive(Debug)]
pub struct OperationContext<'a> {
    pub common_args: CommonRebuildArgs,
    pub update_args: &'a UpdateArgs,
    pub verbose_count: u8,
    pub nix_interface: NixInterface,
    pub config: Arc<NgConfig>,
    pub project_root: Option<PathBuf>, // Added for specifying root in tests
}

impl<'a> OperationContext<'a> {
    pub fn new(
        common_args: CommonRebuildArgs,
        update_args: &'a UpdateArgs,
        verbose_count: u8,
        nix_interface: NixInterface,
        config: Arc<NgConfig>, // Pass config in
        project_root: Option<PathBuf>, // Added
    ) -> Self {
        Self {
            common_args,
            update_args,
            verbose_count,
            nix_interface,
            config,
            project_root,
        }
    }
    
    // Helper to get the effective project root
    pub fn get_effective_project_root(&self) -> PathBuf {
        self.project_root.clone().unwrap_or_else(|| PathBuf::from("."))
    }
}