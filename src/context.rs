use crate::workflow_types::CommonRebuildArgs;
use crate::interface::UpdateArgs;
use crate::nix_interface::NixInterface;

/// Holds the operational context for a given ng command run.
/// Passed around to workflow steps and strategies.
#[derive(Debug)] // Add Clone if needed, but refs might be better
pub struct OperationContext<'a> {
    /// Common rebuild arguments
    pub common_args: &'a CommonRebuildArgs,
    
    /// Arguments for flake update operations
    pub update_args: &'a UpdateArgs,
    
    /// Verbosity level (number of -v flags)
    pub verbose_count: u8,
    
    /// Interface for Nix operations
    pub nix_interface: NixInterface,
    
    // The following fields will be added in later phases:
    // pub error_handler: ErrorHandler<'a>, // For structured error reporting
    // pub config: &'a NgConfig, // Loaded ng configuration file
}

impl<'a> OperationContext<'a> {
    /// Creates a new OperationContext with the given parameters
    pub fn new(
        common_args: &'a CommonRebuildArgs,
        update_args: &'a UpdateArgs,
        verbose_count: u8,
        nix_interface: NixInterface,
        // error_handler: ErrorHandler<'a>,
    ) -> Self {
        Self {
            common_args,
            update_args,
            verbose_count,
            nix_interface,
            // error_handler,
        }
    }
}