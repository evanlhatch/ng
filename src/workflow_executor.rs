use crate::workflow_strategy::{PlatformRebuildStrategy, ActivationMode};
// use crate::workflow_types::CommonRebuildArgs; // Unused
use crate::context::OperationContext;
// use crate::installable::Installable; // Unused
use crate::util::{self}; // MaybeTempPath was unused, util::self might still be needed
// Phase 3: Will be uncommented when implementing nil integration
// use crate::pre_flight::{self, get_core_pre_flight_checks, run_shared_pre_flight_checks};
use crate::Result; // from color_eyre
use crate::progress;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{info, debug, warn, error};
use color_eyre::eyre::{bail, WrapErr};

// Stubs for functions to be implemented/refactored later or in other modules
// Phase 1: Temporary stubs for pre_flight until Phase 3
mod pre_flight {
    use crate::Result;
    use crate::context::OperationContext;
    use crate::workflow_strategy::PlatformRebuildStrategy;
    
    pub fn get_core_pre_flight_checks(_op_ctx: &OperationContext) -> Vec<()> {
        Vec::new()
    }
    
    pub fn run_shared_pre_flight_checks<S: PlatformRebuildStrategy>(
        _op_ctx: &OperationContext,
        _platform_strategy: &S,
        _platform_args: &S::PlatformArgs,
        _checks_to_run: &[()],
    ) -> Result<()> {
        Ok(())
    }
}

/// Shows the diff between two configurations using nvd
///
/// # Arguments
///
/// * `current_profile` - The path to the current profile
/// * `new_profile_path` - The path to the new profile
/// * `verbose_count` - The verbosity level
///
/// # Returns
///
/// * `Result<()>` - Success or an error
fn show_platform_diff(
    current_profile: &Path,
    new_profile_path: &Path,
    verbose_count: u8,
) -> Result<()> {
    info!("Comparing changes with nvd (current: {}, new: {})...", current_profile.display(), new_profile_path.display());
    let pb_diff = progress::start_spinner("Comparing configurations...");
    
    // Using nvd for diffing
    let mut nvd_cmd = Command::new("nvd");
    nvd_cmd.arg("diff")
           .arg(current_profile)
           .arg(new_profile_path);
    
    // Add verbosity flags
    util::add_verbosity_flags(&mut nvd_cmd, verbose_count);
    
    match util::run_cmd_inherit_stdio(&mut nvd_cmd) {
        Ok(_) => {
            progress::finish_spinner_success(&pb_diff, "Comparison displayed.");
            Ok(())
        }
        Err(e) => {
            progress::finish_spinner_fail(&pb_diff);
            warn!("Failed to run 'nvd diff': {}. This is non-critical. Ensure 'nvd' is installed and profiles are valid.", e);
            // Non-critical error, so we continue
            Ok(())
        }
    }
}

/// Executes the rebuild workflow for a given platform strategy.
///
/// This function orchestrates the entire rebuild process, including:
/// 1. Platform-specific pre-rebuild hook
/// 2. Shared pre-flight checks (Git, Parse, Lint, Eval, DryRun)
/// 3. Optional Flake Update
/// 4. Get Toplevel Derivation
/// 5. Build Configuration
/// 6. Show Diff
/// 7. Optional Confirmation
/// 8. Activate
/// 9. Optional Cleanup (Manual --clean flag)
/// 10. Automatic Lazy Cleanup (if configured)
/// 11. Platform-specific post-rebuild hook
///
/// # Arguments
///
/// * `platform_strategy` - The platform-specific strategy to use
/// * `op_ctx` - The operation context containing common arguments and state
/// * `platform_args` - The platform-specific arguments
/// * `activation_mode` - The activation mode to use
///
/// # Returns
///
/// * `Result<()>` - Success or an error
pub fn execute_rebuild_workflow<S: PlatformRebuildStrategy>(
    platform_strategy: &S,
    op_ctx: &OperationContext,
    platform_args: &S::PlatformArgs,
    activation_mode: ActivationMode,
) -> Result<()> {
    let workflow_span = tracing::info_span!("execute_rebuild_workflow", platform = platform_strategy.name(), mode = ?activation_mode);
    let _enter = workflow_span.enter();

    info!("🚀 Starting {} rebuild workflow ({:?})...", platform_strategy.name(), activation_mode);

    // 1. Platform-specific pre-rebuild hook
    platform_strategy.pre_rebuild_hook(op_ctx, platform_args)
        .wrap_err_with(|| format!("{} pre-rebuild hook failed", platform_strategy.name()))?;
    debug!("Pre-rebuild hook for {} completed.", platform_strategy.name());

    // 2. Shared Pre-flight checks (Git, Parse, Lint, Eval, DryRun)
    if !op_ctx.common_args.no_preflight {
        // Get the core checks (Phase 3 will use real pre-flight checks)
        let checks_to_run = pre_flight::get_core_pre_flight_checks(op_ctx);
        pre_flight::run_shared_pre_flight_checks(op_ctx, platform_strategy, platform_args, &checks_to_run)?;
        debug!("Shared pre-flight checks for {} completed.", platform_strategy.name());
    } else {
        info!("[⏭️ Pre-flight] All checks skipped due to --no-preflight.");
    }

    // 3. Optional Flake Update
    if op_ctx.update_args.update || op_ctx.update_args.update_input.is_some() {
        info!("Updating flake inputs as requested...");
        let pb_update = crate::progress::start_spinner("Updating flake inputs...");
        
        crate::update::update(&op_ctx.common_args.installable, op_ctx.update_args.update_input.clone())
            .wrap_err("Failed to update flake inputs")
            .map_err(|e| {
                crate::progress::finish_spinner_fail(&pb_update);
                error!("Flake update failed: {}", e);
                e
            })?;
            
        crate::progress::finish_spinner_success(&pb_update, "Flake inputs updated.");
        debug!("Flake input update for {} completed.", platform_strategy.name());
    }

    // 4. Get Toplevel Derivation
    let toplevel_installable = platform_strategy.get_toplevel_installable(op_ctx, platform_args)
        .wrap_err_with(|| format!("Failed to determine toplevel installable for {}", platform_strategy.name()))?;
    debug!("Resolved toplevel installable for {}: {:?}", platform_strategy.name(), toplevel_installable);

    // 5. Build Configuration
    let out_path_manager = util::manage_out_path(op_ctx.common_args.out_link.as_ref())?;
    let built_profile_path: PathBuf; // Actual path to the /nix/store derivation

    if op_ctx.common_args.dry_run && activation_mode == ActivationMode::Build {
        info!("Dry-run build-only mode: Simulating build for {:?}. Output would be at {}.",
            &toplevel_installable.to_args(),
            out_path_manager.get_path().display()
        );
        // For a pure dry-run build, we don't actually build or have a real result path.
        // Subsequent steps like diff/activate will be skipped anyway.
        built_profile_path = PathBuf::from("/dry_run_build_no_actual_path"); // Placeholder
    } else {
        // Use NixInterface to build the configuration
        built_profile_path = op_ctx.nix_interface.build_configuration(
            &toplevel_installable,
            &op_ctx.common_args.extra_build_args,
            op_ctx.common_args.no_nom,
            Some(out_path_manager.get_path()), // Pass out_link as Option
        )?;
    }


    // 6. Show Diff
    if activation_mode != ActivationMode::Build && !op_ctx.common_args.dry_run {
        if let Some(current_profile) = platform_strategy.get_current_profile_path(op_ctx, platform_args) {
            if current_profile.exists() {
                show_platform_diff(&current_profile, &built_profile_path, op_ctx.verbose_count)?;
            } else {
                info!("Current profile {} does not exist, skipping diff.", current_profile.display());
            }
        }
    } else if op_ctx.common_args.dry_run && activation_mode != ActivationMode::Build {
        info!("Dry-run: Skipping diff display.");
    }

    // 7. Optional Confirmation
    if op_ctx.common_args.ask_confirmation && activation_mode != ActivationMode::Build && !op_ctx.common_args.dry_run {
        info!("Confirmation for applying new configuration would be requested now.");
        if !dialoguer::Confirm::new()
            .with_prompt(format!("Apply the new {} configuration?", platform_strategy.name()))
            .default(false)
            .interact()?
        {
            bail!("User rejected the new configuration for {}.", platform_strategy.name());
        }
        info!("User confirmed action for {}.", platform_strategy.name());
    }

    // 8. Activate (if not dry run and not just a build variant)
    if activation_mode != ActivationMode::Build && !op_ctx.common_args.dry_run {
        info!("Attempting activation for {} (mode: {:?})...", platform_strategy.name(), activation_mode);
        platform_strategy.activate_configuration(op_ctx, platform_args, &built_profile_path, &activation_mode)
            .wrap_err_with(|| format!("Failed to activate {} configuration", platform_strategy.name()))?;
        info!("Activation for {} completed.", platform_strategy.name());
    } else if op_ctx.common_args.dry_run {
        info!("Dry-run: Skipping activation for {}.", platform_strategy.name());
    } else { // Build mode
        info!("Build-only mode: Result at {}", built_profile_path.display());
    }

    // 9. Optional Cleanup (triggered by --clean flag)
    if op_ctx.common_args.clean_after && activation_mode != ActivationMode::Build && !op_ctx.common_args.dry_run {
        info!("Performing cleanup as requested by --clean flag for {}...", platform_strategy.name());
        let pb_clean = crate::progress::start_spinner("Performing post-rebuild cleanup...");
        
        // Run garbage collection using NixInterface
        match op_ctx.nix_interface.run_gc(op_ctx.common_args.dry_run) {
            Ok(_) => {
                crate::progress::finish_spinner_success(&pb_clean, "Cleanup completed.");
                debug!("Manual cleanup for {} completed.", platform_strategy.name());
            },
            Err(e) => {
                crate::progress::finish_spinner_fail(&pb_clean);
                warn!("Manual cleanup step failed (non-critical): {}", e);
            }
        }
    }

    // 10. Automatic Lazy Cleanup (if configured) - STUBBED for Part 3
    // if should_auto_clean(...) { perform_auto_clean(...); }

    // 11. Platform-specific post-rebuild hook
    platform_strategy.post_rebuild_hook(op_ctx, platform_args)
        .wrap_err_with(|| format!("{} post-rebuild hook failed", platform_strategy.name()))?;
    debug!("Post-rebuild hook for {} completed.", platform_strategy.name());

    info!("🏆 {} rebuild workflow ({:?}) finished successfully!", platform_strategy.name(), activation_mode);
    Ok(())
}