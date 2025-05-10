use crate::workflow_types::CommonRebuildArgs;
use crate::interface::UpdateArgs;
use crate::nix_interface::NixInterface;

/// Holds the operational context for a given ng command run.
/// Passed around to workflow steps and strategies.
#[derive(Debug)] // Add Clone if needed
pub struct OperationContext<'a> { // Lifetime 'a is now only for update_args
    /// Common rebuild arguments (Owned)
    pub common_args: CommonRebuildArgs,
    
    /// Arguments for flake update operations
    pub update_args: &'a UpdateArgs,
    
    /// Verbosity level (number of -v flags)
    pub verbose_count: u8,
    
    /// Interface for Nix operations
    pub nix_interface: NixInterface,
}

impl<'a> OperationContext<'a> {
    /// Creates a new OperationContext with the given parameters
    pub fn new(
        common_args: CommonRebuildArgs, // Pass by value
        update_args: &'a UpdateArgs,
        verbose_count: u8,
        nix_interface: NixInterface,
    ) -> Self {
        Self {
            common_args,
            update_args,
            verbose_count,
            nix_interface,
        }
    }
}