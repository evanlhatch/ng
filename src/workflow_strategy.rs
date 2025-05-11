use crate::installable::Installable;
use crate::Result; // from color_eyre
use crate::context::OperationContext;
use std::path::{Path, PathBuf};

/// Represents the different modes of activation for a configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivationMode {
    /// Activate and make default (if applicable)
    Switch,
    
    /// Prepare for next boot (e.g., NixOS, Darwin)
    Boot,
    
    /// Activate but don't make default (e.g., NixOS, Darwin)
    Test,
    
    /// Just build, no activation (replaces is_just_build_variant)
    Build,
}

/// Trait defining the platform-specific strategy for rebuilding configurations
pub trait PlatformRebuildStrategy {
    /// Platform-specific arguments, typically the clap::Args struct for the platform's rebuild command
    /// e.g., crate::interface::OsRebuildArgs
    type PlatformArgs: std::fmt::Debug;

    /// Returns the name of the platform (e.g., "NixOS", "HomeManager", "Darwin")
    fn name(&self) -> &str;

    /// Initial platform-specific checks (e.g., root user for NixOS).
    /// Called before any shared pre-flight checks.
    fn pre_rebuild_hook(&self, op_ctx: &OperationContext, platform_args: &Self::PlatformArgs) -> Result<()>;

    /// Determines the final Nix Installable target for the build operation.
    /// This incorporates hostname, user-specified attributes, specialisations, etc.
    fn get_toplevel_installable(&self, op_ctx: &OperationContext, platform_args: &Self::PlatformArgs) -> Result<Installable>;

    /// Path to the current system profile for diffing against the new build.
    /// Returns None if not applicable or not found.
    fn get_current_profile_path(&self, op_ctx: &OperationContext, platform_args: &Self::PlatformArgs) -> Option<PathBuf>;

    /// Performs the platform-specific activation.
    /// `built_profile_path` is the /nix/store path of the newly built configuration.
    fn activate_configuration(
        &self,
        op_ctx: &OperationContext,
        platform_args: &Self::PlatformArgs,
        built_profile_path: &Path,
        activation_mode: &ActivationMode,
    ) -> Result<()>;

    /// Final platform-specific actions after activation/cleanup.
    fn post_rebuild_hook(&self, op_ctx: &OperationContext, platform_args: &Self::PlatformArgs) -> Result<()>;

    // Optional: For platform-specific pre-flight checks that don't fit the shared model.
    // fn platform_specific_pre_flight_checks(&self, op_ctx: &OperationContext, platform_args: &Self::PlatformArgs) -> Result<()>;
}


#[cfg(test)]
#[derive(Debug)]
pub struct MockPlatformStrategy;

#[cfg(test)]
impl PlatformRebuildStrategy for MockPlatformStrategy {
    type PlatformArgs = (); // Simple unit type for args if not needed

    fn name(&self) -> &str { "MockPlatform" }
    fn pre_rebuild_hook(&self, _op_ctx: &OperationContext, _platform_args: &Self::PlatformArgs) -> Result<()> { 
        // Use tracing::debug or println! for test output if needed
        // println!("MockPlatform: pre_rebuild_hook called"); 
        Ok(())
    }
    fn get_toplevel_installable(&self, _op_ctx: &OperationContext, _platform_args: &Self::PlatformArgs) -> Result<Installable> {
        // println!("MockPlatform: get_toplevel_installable called"); 
        Ok(Installable::Flake { reference: ".#mock".to_string(), attribute: Vec::new() })
    }
    fn get_current_profile_path(&self, _op_ctx: &OperationContext, _platform_args: &Self::PlatformArgs) -> Option<PathBuf> {
        None
    }
    fn activate_configuration(
        &self,
        _op_ctx: &OperationContext,
        _platform_args: &Self::PlatformArgs,
        _built_profile_path: &Path,
        _activation_mode: &ActivationMode,
    ) -> Result<()> { 
        // println!("MockPlatform: activate_configuration called"); 
        Ok(())
    }
    fn post_rebuild_hook(&self, _op_ctx: &OperationContext, _platform_args: &Self::PlatformArgs) -> Result<()> { 
        // println!("MockPlatform: post_rebuild_hook called"); 
        Ok(())
    }
}