use crate::workflow_strategy::{PlatformRebuildStrategy, ActivationMode};
use crate::context::OperationContext;
use crate::installable::Installable;
use crate::interface::OsRebuildArgs; // Your existing clap struct for 'nh os switch/boot/...'
use crate::util::{self, UtilCommandError};
use crate::Result; // from color_eyre
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand; // Alias to avoid conflict if Command struct exists
use tracing::{info, debug}; // warn was unused
use color_eyre::eyre::{bail, eyre}; // Context, Report were unused

/// Strategy implementation for NixOS platform
pub struct NixosPlatformStrategy;

impl PlatformRebuildStrategy for NixosPlatformStrategy {
    type PlatformArgs = OsRebuildArgs;

    fn name(&self) -> &str { "NixOS" }

    fn pre_rebuild_hook(&self, _op_ctx: &OperationContext, platform_args: &Self::PlatformArgs) -> Result<()> {
        debug!("NixOSStrategy: pre_rebuild_hook");
        if !platform_args.bypass_root_check && nix::unistd::Uid::effective().is_root() {
            bail!("Do not run `ng os` as root. Sudo will be used internally if needed based on the operation.");
        }
        Ok(())
    }

    fn get_toplevel_installable(&self, op_ctx: &OperationContext, platform_args: &Self::PlatformArgs) -> Result<Installable> {
        debug!("NixOSStrategy: get_toplevel_installable");
        let hostname = platform_args.hostname.clone()
            .or_else(|| util::get_hostname().ok()) // Try to get system hostname
            .ok_or_else(|| eyre!("Hostname could not be determined and was not provided via --hostname. Needed for NixOS configuration attribute path."))?;

        let mut final_installable = op_ctx.common_args.installable.clone();
        match &mut final_installable {
            Installable::Flake { reference: _, attribute } => {
                if attribute.is_empty() { 
                    attribute.push("nixosConfigurations".to_string());
                    attribute.push(hostname.clone()); 
                    attribute.extend(vec!["config".to_string(), "system".to_string(), "build".to_string(), "toplevel".to_string()]);
                } else if attribute.len() == 2 && attribute[0] == "nixosConfigurations" {
                    // If attribute is like ["nixosConfigurations", "some-host"], then append suffix.
                    attribute.extend(vec!["config".to_string(), "system".to_string(), "build".to_string(), "toplevel".to_string()]);
                }
                // TODO: Handle platform_args.specialisation and platform_args.no_specialisation here
            }
            Installable::File { attribute: _, .. } | Installable::Expression { attribute: _, .. } => {
            }
            Installable::Store { .. } => { /* Store path is already a toplevel */ }
        }
        debug!("NixOS toplevel: {:?}", final_installable);
        Ok(final_installable)
    }

    fn get_current_profile_path(&self, _op_ctx: &OperationContext, _platform_args: &Self::PlatformArgs) -> Option<PathBuf> {
        Some(PathBuf::from("/run/current-system"))
    }

    fn activate_configuration(
        &self,
        op_ctx: &OperationContext,
        platform_args: &Self::PlatformArgs,
        built_profile_path: &Path, // This IS the /nix/store path of the system derivation
        activation_mode: &ActivationMode,
    ) -> Result<()> {
        debug!("NixOSStrategy: activate_configuration (mode: {:?}, profile: {})", activation_mode, built_profile_path.display());

        if op_ctx.common_args.dry_run {
            info!("Dry-run: Would activate NixOS ({:?}) with profile: {}", activation_mode, built_profile_path.display());
            return Ok(());
        }

        let action_str = match activation_mode {
            ActivationMode::Switch => "switch",
            ActivationMode::Boot => "boot",
            ActivationMode::Test => "test",
            ActivationMode::Build => return Ok(()), // Should be caught by workflow_executor
        };

        let switch_script_path = built_profile_path.join("bin/switch-to-configuration");
        if !switch_script_path.exists() {
            bail!("Activation script 'bin/switch-to-configuration' not found in built profile: {}", built_profile_path.display());
        }

        let mut cmd = StdCommand::new(if platform_args.bypass_root_check {
            // If bypassing root check, assume we are already root or script handles it
            switch_script_path.as_os_str().to_os_string()
        } else {
            "sudo".into() // Otherwise, use sudo
        });

        if !platform_args.bypass_root_check {
            cmd.arg(switch_script_path);
        }
        cmd.arg(action_str);

        info!("Executing NixOS activation: {:?}", cmd);
        util::run_cmd_inherit_stdio(&mut cmd)
            .map_err(|e| {
                let context_msg = format!("NixOS activation with action '{}' failed.", action_str);
                // TODO: Pass full UtilCommandError to ErrorHandler in Part 3 for richer detail
                match e {
                    UtilCommandError::InheritedNonZeroStatus { command_str, status_code } => {
                        eyre!("{}. Command: '{}', Status: {}", context_msg, command_str, status_code)
                    }
                    _ => eyre!("{}: {}", context_msg, e),
                }
            })
            .map(|_| ())?;

        info!("NixOS configuration ({:?}) activated successfully.", activation_mode);
        Ok(())
    }

    fn post_rebuild_hook(&self, _op_ctx: &OperationContext, _platform_args: &Self::PlatformArgs) -> Result<()> {
        debug!("NixOSStrategy: post_rebuild_hook");
        // Example: could print info about rebooting if kernel changed, etc.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // use super::*; // Removed as unused
    
    // Test pre_rebuild_hook for root check logic
    // Test get_toplevel_installable with various Installable inputs, hostname, specialisation flags
    // Test activate_configuration by asserting the command string it would build for switch, boot, test
}