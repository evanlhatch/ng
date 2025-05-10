use std::fs;
use std::path::{PathBuf};
use std::process::Command as StdCommand;
use color_eyre::eyre::{bail, eyre, Result};
use nix;
use walkdir::WalkDir;
use rayon::prelude::*;
use tracing::{info, warn, debug};

// Interface and core imports
use crate::interface::{OsArgs, OsRebuildArgs, OsSubcommand, OsReplArgs, OsGenerationsArgs};
use crate::installable::Installable;
use crate::commands::{Command, Build};
use crate::util::get_hostname;
use crate::generations;
use crate::update::update;

// Workflow refactoring imports
use crate::workflow_executor::execute_rebuild_workflow;
use crate::nixos_strategy::NixosPlatformStrategy;
use crate::workflow_strategy::ActivationMode;
use crate::context::OperationContext;
use crate::nix_interface::NixInterface;
use crate::workflow_types::CommonRebuildArgs;

// Constants
const SYSTEM_PROFILE: &str = "/nix/var/nix/profiles/system";
const CURRENT_PROFILE: &str = "/run/current-system";
const SPEC_LOCATION: &str = "/etc/specialisation";

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
            &core_common_args,
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

#[derive(Debug)]
enum OsRebuildVariant {
    Build,
    Switch,
    Boot,
    Test,
}

impl OsRebuildArgs {
    fn rebuild(self, variant: OsRebuildVariant, verbose_count: u8) -> Result<()> {
        use OsRebuildVariant::*;

        let elevate = if self.bypass_root_check {
            warn!("Bypassing root check, now running nix as root");
            false
        } else {
            if nix::unistd::Uid::effective().is_root() {
                bail!("Don't run nh os as root. I will call sudo internally as needed");
            }
            true
        };

        // Add pre-flight checks
        let run_preflight = !self.common.common.no_preflight;
        if run_preflight {
            // Git Check
            let pb = crate::progress::start_spinner("[🔍 Git] Checking status...");
            match crate::check_git::run_git_check_warning_only() {
                Ok(_) => {
                    // Git check passed or issued warnings
                    crate::progress::finish_spinner_success(&pb, "[✅ Git] Check complete (warnings above if any).");
                },
                Err(e) => {
                    crate::progress::finish_spinner_fail(&pb);
                    crate::error_handler::report_failure(
                        "Git Check",
                        "Failed to check Git status",
                        Some(e.to_string()),
                        vec![
                            "Ensure git is installed and accessible".to_string(),
                            "Check if this is a git repository".to_string(),
                        ]
                    );
                    bail!("Git check failed");
                }
            }
            
            // Parse Check
            let pb = crate::progress::start_spinner("[🔍 Parse] Checking Nix syntax...");
            match run_parallel_parse_check(verbose_count) {
                Ok(_) => {
                    crate::progress::finish_spinner_success(&pb, "[✅ Parse] Syntax check passed");
                },
                Err(e) => {
                    crate::progress::finish_spinner_fail(&pb);
                    crate::error_handler::report_failure(
                        "Parse Check", 
                        "Nix syntax errors found",
                        Some(e),
                        vec!["Fix the syntax errors reported above".to_string()]
                    );
                    bail!("Parse check failed");
                }
            }
            
            // Lint Check
            let pb = crate::progress::start_spinner("[🔍 Lint] Running formatters and linters...");
            
            let use_strict_lint = self.common.common.strict_lint || self.common.common.full || self.common.common.medium;
            
            match crate::lint::run_lint_checks(use_strict_lint, verbose_count) {
                Ok(lint_summary) => {
                    if matches!(lint_summary.outcome, Some(crate::lint::LintOutcome::CriticalFailure(_))) {
                        crate::progress::finish_spinner_fail(&pb);
                        crate::error_handler::report_failure(
                            "Lint", 
                            "Linting failed in strict mode",
                            None,
                            vec![
                                "Fix the linting issues reported above".to_string(),
                                "Use --no-preflight to skip linting checks".to_string()
                            ]
                        );
                        bail!("Lint check failed");
                    } else {
                        crate::progress::finish_spinner_success(&pb, &format!(
                            "[✅ Lint] Check {}",
                            if matches!(lint_summary.outcome, Some(crate::lint::LintOutcome::Warnings)) {
                                "completed with warnings"
                            } else {
                                "passed"
                            }
                        ));
                    }
                }
                Err(e) => {
                    crate::progress::finish_spinner_fail(&pb);
                    crate::error_handler::report_failure(
                        "Lint",
                        "Failed to run linters",
                        Some(e.to_string()),
                        vec![
                            "Ensure formatters/linters are installed".to_string(),
                            "Use --no-preflight to skip linting".to_string()
                        ]
                    );
                    bail!("Lint check failed");
                }
            }
        }

        // Determine hostname
        let hostname = match self.hostname.clone() {
            Some(h) => h,
            None => get_hostname().map_err(|e| eyre!("Failed to get hostname: {}", e))?,
        };
        debug!("Using hostname: {}", hostname);

        // Determine installable
        let installable = toplevel_for(self.common.installable.clone(), &hostname, &self)?;

        // Handle update
        if self.update_args.update || self.update_args.update_input.is_some() {
            update(&installable, self.update_args.update_input.clone())?;
        }

        // Build
        let is_just_build_variant = matches!(variant, Build);
        let dry = self.common.common.dry;
        let ask = self.common.common.ask;

        if is_just_build_variant && (ask || dry) {
            warn!("`--ask` and `--dry` have no effect for `nh os build`");
        }

        // Determine out_link
        let out_link = self.common.common.out_link.clone();

        // Build the configuration
        // Use a simpler approach for building
        let mut build_cmd = Command::new("nix")
            .arg("build")
            .arg(&installable.to_args().join(" "));
        
        // Add --no-link if no_nom is false
        if !self.common.common.no_nom {
            build_cmd = build_cmd.arg("--no-link");
        }
        
        // Add extra args one by one
        for arg in &self.extra_args {
            build_cmd = build_cmd.arg(arg);
        }

        // Add out_link if specified
        if let Some(out_link) = &out_link {
            build_cmd = build_cmd.arg("--out-link").arg(out_link);
        }

        let result_path = if !dry || is_just_build_variant {
            build_cmd.run()?;
            out_link.unwrap_or_else(|| PathBuf::from("./result"))
        } else {
            info!("Dry run, skipping build");
            PathBuf::from("/tmp/dry-run-result")
        };

        // Show diff
        if !is_just_build_variant && !dry {
            if let Ok(current_system) = fs::canonicalize(CURRENT_PROFILE) {
                Command::new("nvd")
                    .arg("diff")
                    .arg(current_system)
                    .arg(&result_path)
                    .run()
                    .unwrap_or_else(|e| {
                        warn!("Failed to run nvd diff: {}", e);
                    });
            }
        }

        // Confirm
        if ask && !is_just_build_variant && !dry {
            info!("Confirm activation? [y/N] ");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                bail!("Activation cancelled");
            }
        }

        // Activate
        if !is_just_build_variant && !dry {
            let action = match variant {
                Switch => "switch",
                Boot => "boot",
                Test => "test",
                Build => unreachable!(),
            };

            Command::new("sudo")
                .arg(format!("{}/bin/switch-to-configuration", result_path.display()))
                .arg(action)
                .message(format!("Activating configuration ({})", action))
                .run()?;
        }

        // Cleanup
        if self.common.common.clean && !is_just_build_variant && !dry {
            perform_cleanup(verbose_count)?;
        }

        Ok(())
    }
}

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

/// Performs cleanup after a successful rebuild
pub fn perform_cleanup(_verbose_count: u8) -> Result<()> {
    info!("Performing cleanup...");
    
    Command::new("sudo")
        .args(["nix-store", "--gc"])
        .message("Running garbage collection")
        .run()?;
    
    Ok(())
}

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
    pub fn run(self, verbose_count: u8) -> Result<()> {
        let hostname = match self.hostname.clone() {
            Some(h) => h,
            None => get_hostname().map_err(|e| eyre!("Failed to get hostname: {}", e))?,
        };
        debug!("Using hostname: {}", hostname);

        let installable = self.installable.clone();

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
