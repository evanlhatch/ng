use crate::installable::Installable;
use crate::Result; // from color_eyre
use crate::context::OperationContext;
use crate::workflow_strategy::{PlatformRebuildStrategy, ActivationMode};
use crate::interface::DarwinRebuildArgs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct DarwinPlatformStrategy;

impl PlatformRebuildStrategy for DarwinPlatformStrategy {
    type PlatformArgs = DarwinRebuildArgs;

    fn name(&self) -> &str { "Darwin" }

    fn pre_rebuild_hook(&self, _op_ctx: &OperationContext, _platform_args: &Self::PlatformArgs) -> Result<()> {
        if !nix::unistd::Uid::effective().is_root() {
            // In future, more sophisticated checks or warnings might go here.
            // For now, actual sudo/root requirements are handled by specific commands.
            // println!("DarwinPlatform: pre_rebuild_hook - Not root, but that's okay for now.");
        }
        Ok(())
    }

    fn get_toplevel_installable(&self, _op_ctx: &OperationContext, platform_args: &Self::PlatformArgs) -> Result<Installable> { // Prefixed _op_ctx
        // Similar to NixOS, but might use different default attributes or NH_DARWIN_FLAKE logic.
        // For now, directly use what's provided, assuming it's correctly pointing to a darwinConfiguration.
        // TODO: Implement NH_DARWIN_FLAKE and default attribute logic similar to `rebuild_old`.
        Ok(platform_args.common.installable.clone())
    }

    fn get_current_profile_path(&self, _op_ctx: &OperationContext, _platform_args: &Self::PlatformArgs) -> Option<PathBuf> {
        // Darwin doesn't have a single system profile quite like NixOS's /nix/var/nix/profiles/system
        // or a /run/current-system that's universally the target for `nvd diff`.
        // Users often use `darwin-rebuild switch --flake .#myHost` which handles its own profile management.
        // For diffing purposes, we might need to be smarter or rely on user config.
        // Returning None for now, which means nvdiff might not be as useful out-of-the-box.
        None 
    }

    fn activate_configuration(
        &self,
        _op_ctx: &OperationContext, // Prefixed
        _platform_args: &Self::PlatformArgs, // Prefixed
        built_profile_path: &Path,
        activation_mode: &ActivationMode,
    ) -> Result<()> {
        match activation_mode {
            ActivationMode::Switch => {
                // Mimic `darwin-rebuild switch`
                // 1. Update /run/current-system (or equivalent) to point to the new profile.
                //    This is often done by the `activate` script in the Nix profile.
                // 2. Run the activation script.
                let activate_script = built_profile_path.join("activate");
                if !activate_script.exists() {
                    return Err(color_eyre::eyre::eyre!("activate script not found in built profile: {}", activate_script.display()));
                }

                println!("Simulating activation for Darwin (switch):");
                println!("  System profile would be updated (conceptually).");
                println!("  Running activation script: {}", activate_script.display());

                if !_op_ctx.common_args.dry_run {
                    // In a real scenario:
                    // Command::new(&activate_script).elevate(true).run()?;
                    // sudo /nix/var/nix/profiles/system/bin/darwin-activate (example of what it might do)
                    println!("DRY-RUN: Would execute {}", activate_script.display());
                }
                Ok(())
            }
            ActivationMode::Build => {
                // No activation, just build. Handled by workflow.
                Ok(())
            }
            ActivationMode::Boot | ActivationMode::Test => {
                // Boot activation is complex on Darwin (akin to NixOS bootloader changes).
                // Test activation might involve temporary profile linkage.
                // For now, these are not fully supported in this basic strategy.
                Err(color_eyre::eyre::eyre!("{:?} activation mode not yet fully supported for Darwin in this basic strategy.", activation_mode))
            }
        }
    }

    fn post_rebuild_hook(&self, _op_ctx: &OperationContext, _platform_args: &Self::PlatformArgs) -> Result<()> {
        Ok(())
    }
}
