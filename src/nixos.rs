// use std::fs; // Unused
use std::path::{PathBuf};
// use std::process::Command as StdCommand; // Unused
use color_eyre::eyre::{eyre, Result}; // bail was unused
// use nix; // Unused
use walkdir::WalkDir;
// use rayon::prelude::*; // Unused
use tracing::{info, debug}; // warn was unused

// Interface and core imports
use crate::interface::{OsArgs, OsRebuildArgs, OsSubcommand, OsReplArgs, OsGenerationsArgs};
use crate::installable::Installable;
use crate::commands::Command; // Build was unused
use crate::util::get_hostname;
// use crate::generations; // Unused
// use crate::update::update; // Unused

// Workflow refactoring imports
use crate::workflow_executor::execute_rebuild_workflow;
use crate::nixos_strategy::NixosPlatformStrategy;
use crate::workflow_strategy::ActivationMode;
use crate::context::OperationContext;
use crate::nix_interface::NixInterface;
use crate::workflow_types::CommonRebuildArgs;

// Constants removed as they were only used in the deleted OsRebuildArgs::rebuild method
// const SYSTEM_PROFILE: &str = "/nix/var/nix/profiles/system";
// const CURRENT_PROFILE: &str = "/run/current-system";
// const SPEC_LOCATION: &str = "/etc/specialisation";

impl OsArgs {
    /// Helper function to create CommonRebuildArgs from OsRebuildArgs
    fn create_common_rebuild_args(cli_args: &OsRebuildArgs) -> CommonRebuildArgs {
        CommonRebuildArgs {
            installable: cli_args.common.installable.clone(),
            no_preflight: cli_args.common.common.no_preflight,
            strict_lint: cli_args.common.common.strict_lint,
            medium_checks: cli_args.common.common.medium,
            full_checks: cli_args.common.common.full,
            dry_run: cli_args.common.common.dry,
            ask_confirmation: cli_args.common.common.ask,
            no_nom: cli_args.common.common.no_nom,
            out_link: cli_args.common.common.out_link.clone(),
            clean_after: cli_args.common.common.clean,
            extra_build_args: cli_args.extra_args.iter().map(|s| std::ffi::OsString::from(s.clone())).collect(),
        }
    }

    /// Helper function to execute the rebuild workflow
    fn execute_os_workflow(cli_args: &OsRebuildArgs, verbose_count: u8, activation_mode: ActivationMode) -> Result<()> {
        let core_common_args = Self::create_common_rebuild_args(cli_args);
        
        let op_ctx = OperationContext::new(
            core_common_args, // Pass by value
            &cli_args.update_args,
            verbose_count,
            NixInterface::new(verbose_count),
        );
        
        let strategy = NixosPlatformStrategy;
        
        execute_rebuild_workflow(
            &strategy,
            &op_ctx,
            cli_args,
            activation_mode,
        )
    }

    pub fn run(self, verbose_count: u8) -> Result<()> {
        std::env::set_var("NH_CURRENT_COMMAND", "os"); // Good to keep for env var logic
        
        match self.subcommand {
            OsSubcommand::Switch(cli_args) => {
                Self::execute_os_workflow(&cli_args, verbose_count, ActivationMode::Switch)
            }
            OsSubcommand::Boot(cli_args) => {
                Self::execute_os_workflow(&cli_args, verbose_count, ActivationMode::Boot)
            }
            OsSubcommand::Test(cli_args) => {
                Self::execute_os_workflow(&cli_args, verbose_count, ActivationMode::Test)
            }
            OsSubcommand::Build(cli_args) => {
                Self::execute_os_workflow(&cli_args, verbose_count, ActivationMode::Build)
            }
            OsSubcommand::Repl(args) => args.run(verbose_count),
            OsSubcommand::Info(args) => args.info(),
        }
    }
}

// OsRebuildVariant enum removed (lines 93-98 of original)

// impl OsRebuildArgs { fn rebuild(...) } block removed (lines 100-301 of original)

/// Runs a parallel parse check on all .nix files
pub fn run_parallel_parse_check(verbose_count: u8) -> Result<(), String> {
    use rayon::prelude::*;
    
    info!("Running parallel syntax check on .nix files...");
    
    // Find .nix files
    let nix_files: Vec<PathBuf> = WalkDir::new(".")
        .follow_links(true)
        .into_iter()
        .filter_entry(|e| !is_hidden(e))
        .filter_map(|entry| {
            entry.ok().filter(|e| {
                e.file_type().is_file() && 
                e.path().extension().map_or(false, |ext| ext == "nix")
            }).map(|e| e.path().to_owned())
        })
        .collect();
        
    if nix_files.is_empty() {
        info!("No .nix files found to check.");
        return Ok(());
    }
    
    debug!("Found {} .nix files to check", nix_files.len());
    
    // Use rayon to run nix-instantiate in parallel
    let parse_errors: Vec<(PathBuf, String)> = nix_files.par_iter()
        .filter_map(|path| {
            let mut cmd = std::process::Command::new("nix-instantiate");
            cmd.args(["--parse", path.to_str().unwrap()]);
            crate::util::add_verbosity_flags(&mut cmd, verbose_count);
            
            match cmd.output() {
                Ok(output) => {
                    if !output.status.success() {
                        let error = String::from_utf8_lossy(&output.stderr).to_string();
                        Some((path.clone(), error))
                    } else {
                        None
                    }
                },
                Err(e) => Some((path.clone(), format!("Failed to run nix-instantiate: {}", e)))
            }
        })
        .collect();
        
    if parse_errors.is_empty() {
        Ok(())
    } else {
        let mut combined_error = format!("Found {} file(s) with syntax errors:\n", parse_errors.len());
        for (path, error) in parse_errors {
            combined_error.push_str(&format!("\nError in {}: \n{}\n", path.display(), error));
        }
        Err(combined_error)
    }
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

// perform_cleanup function removed (lines 358-366 of original)

/// Determines the toplevel installable for a NixOS configuration
pub fn toplevel_for<T>(installable: Installable, hostname: &str, _args: &T) -> Result<Installable> {
    match installable {
        Installable::Flake { reference, mut attribute } => {
            if attribute.is_empty() {
                attribute.push("nixosConfigurations".to_string());
                attribute.push(hostname.to_string());
            }
            
            // Add toplevel path
            attribute.push("config".to_string());
            attribute.push("system".to_string());
            attribute.push("build".to_string());
            attribute.push("toplevel".to_string());
            
            Ok(Installable::Flake { reference, attribute })
        }
        _ => Ok(installable),
    }
}

impl OsReplArgs {
    pub fn run(self, _verbose_count: u8) -> Result<()> {
        let hostname = match self.hostname.clone() {
            Some(h) => h,
            None => get_hostname().map_err(|e| eyre!("Failed to get hostname: {}", e))?,
        };
        debug!("Using hostname: {}", hostname);

        let _installable = self.installable.clone();

        Command::new("nix")
            .arg("repl")
            .arg("--file")
            .arg("<nixpkgs/nixos>")
            .run()?;

        Ok(())
    }
}

impl OsGenerationsArgs {
    pub fn info(self) -> Result<()> {
        // Assuming generations::list_generations takes a profile path and returns a Result
        // This is a placeholder implementation
        info!("Listing generations for profile: {:?}", self.profile);
        Ok(())
    }
}
