use std::env;
use std::path::PathBuf;

use color_eyre::eyre::{bail, Context};
use tracing::{debug, info, warn};

use crate::commands;
use crate::commands::Command;
use crate::installable::Installable;
use crate::interface::{DarwinArgs, DarwinRebuildArgs, DarwinReplArgs, DarwinSubcommand};
use crate::nixos::toplevel_for;
use crate::update::update;
use crate::util::get_hostname;
use crate::Result;

const SYSTEM_PROFILE: &str = "/nix/var/nix/profiles/system";
const CURRENT_PROFILE: &str = "/run/current-system";

impl DarwinArgs {
    pub fn run(self, verbose_count: u8) -> Result<()> {
        use DarwinRebuildVariant::*;
        match self.subcommand {
            DarwinSubcommand::Switch(args) => args.rebuild(Switch, verbose_count),
            DarwinSubcommand::Build(args) => {
                if args.common.common.ask || args.common.common.dry {
                    warn!("`--ask` and `--dry` have no effect for `nh darwin build`");
                }
                args.rebuild(Build, verbose_count)
            }
            DarwinSubcommand::Repl(args) => args.run(verbose_count),
        }
    }
}

enum DarwinRebuildVariant {
    Switch,
    Build,
}

impl DarwinRebuildArgs {
    fn rebuild(self, variant: DarwinRebuildVariant, verbose_count: u8) -> Result<()> {
        use DarwinRebuildVariant::*;
    
        if nix::unistd::Uid::effective().is_root() {
            bail!("Don't run nh os as root. I will call sudo internally as needed");
        }
    
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
            let pb = crate::progress::start_spinner("[🧩 Parse] Checking syntax...");
            match run_parallel_parse_check(verbose_count) {
                Ok(_) => {
                    crate::progress::finish_spinner_success(&pb, "[✅ Parse] OK.");
                },
                Err(details) => {
                    crate::progress::finish_spinner_fail(&pb);
                    crate::error_handler::report_failure(
                        "Parse Check",
                        "Syntax errors found",
                        Some(details),
                        vec![] // Empty vec since we'll generate recommendations based on the error message
                    );
                    bail!("Parse check failed");
                }
            }
        } else {
            info!("[⏭️ Parse] Check skipped.");
        }
    
        // Lint Check
        if run_preflight {
            let pb = crate::progress::start_spinner("[🎨 Lint] Running formatters and linters...");
            
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
                            "[✅ Lint] {}",
                            if matches!(lint_summary.outcome, Some(crate::lint::LintOutcome::Warnings)) {
                                "Completed with warnings"
                            } else {
                                "Passed"
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
        } else {
            info!("[⏭️ Lint] Check skipped.");
        }
    
        if self.update_args.update {
            update(&self.common.installable, self.update_args.update_input)?;
        }

        let hostname = self.hostname.ok_or(()).or_else(|()| get_hostname())?;

        let out_path: Box<dyn crate::util::MaybeTempPath> = match self.common.common.out_link {
            Some(ref p) => Box::new(p.clone()),
            None => Box::new({
                let dir = tempfile::Builder::new().prefix("nh-os").tempdir()?;
                (dir.as_ref().join("result"), dir)
            }),
        };

        debug!(?out_path);

        // Use NH_DARWIN_FLAKE if available, otherwise use the provided installable
        let installable = if let Ok(darwin_flake) = env::var("NH_DARWIN_FLAKE") {
            debug!("Using NH_DARWIN_FLAKE: {}", darwin_flake);

            let mut elems = darwin_flake.splitn(2, '#');
            let reference = elems.next().unwrap().to_owned();
            let attribute = elems
                .next()
                .map(crate::installable::parse_attribute)
                .unwrap_or_default();

            Installable::Flake {
                reference,
                attribute,
            }
        } else {
            self.common.installable.clone()
        };

        let mut processed_installable = installable;
        if let Installable::Flake {
            ref mut attribute, ..
        } = processed_installable
        {
            // If user explicitly selects some other attribute, don't push darwinConfigurations
            if attribute.is_empty() {
                attribute.push(String::from("darwinConfigurations"));
                attribute.push(hostname.clone());
            }
        }

        let toplevel = toplevel_for(hostname, processed_installable);

        // Add progress indicator for build
        let pb_build = crate::progress::start_spinner("[🔨 Build] Building configuration...");
        
        // Use the existing build mechanism but enhance error handling
        let build_result = commands::Build::new(toplevel)
            .extra_arg("--out-link")
            .extra_arg(out_path.get_path())
            .extra_args(&self.extra_args)
            .message("Building Darwin configuration")
            .nom(!self.common.common.no_nom)
            .run();
            
        if let Err(e) = build_result {
            crate::progress::finish_spinner_fail(&pb_build);
            
            // Try to extract failed derivation paths from error message
            let error_msg = e.to_string();
            let failed_drvs = crate::error_handler::find_failed_derivations(&error_msg);
            
            let mut details = error_msg;
            
            // If we found any failed derivations, try to fetch their logs
            if !failed_drvs.is_empty() {
                if let Ok(log) = crate::error_handler::fetch_and_format_nix_log(&failed_drvs[0], verbose_count) {
                    details.push_str(&format!("\n\n{}", log));
                }
            }
            
            crate::error_handler::report_failure(
                "Build",
                "Failed to build Darwin configuration",
                Some(details),
                vec![
                    "Fix the build errors reported above".to_string(),
                    "Try running with --verbose for more details".to_string()
                ]
            );
            bail!("Build failed");
        }
        
        crate::progress::finish_spinner_success(&pb_build, &format!(
            "[✅ Build] Configuration built successfully: {}",
            out_path.get_path().display()
        ));

        let target_profile = out_path.get_path().to_owned();

        target_profile.try_exists().context("Doesn't exist")?;

        // Add progress indicator for diff
        let pb_diff = crate::progress::start_spinner("[🔍 Diff] Comparing changes...");
        
        let diff_result = Command::new("nvd")
            .arg("diff")
            .arg(CURRENT_PROFILE)
            .arg(&target_profile)
            .message("Comparing changes")
            .run();
            
        if let Err(e) = diff_result {
            crate::progress::finish_spinner_fail(&pb_diff);
            crate::error_handler::report_failure(
                "Diff",
                "Failed to compare configurations",
                Some(e.to_string()),
                vec![
                    "Ensure nvd is installed".to_string(),
                    "Check if the current profile exists".to_string()
                ]
            );
            // Don't bail here, we can proceed without the diff
            warn!("Failed to show diff, continuing anyway");
        } else {
            crate::progress::finish_spinner_success(&pb_diff, "[✅ Diff] Configuration differences displayed");
        }

        if self.common.common.ask && !self.common.common.dry && !matches!(variant, Build) {
            info!("Apply the config?");
            let confirmation = dialoguer::Confirm::new().default(false).interact()?;

            if !confirmation {
                bail!("User rejected the new config");
            }
        }

        if let Switch = variant {
            // Add progress indicator for system profile update
            let pb_profile = crate::progress::start_spinner("[⚙️ Profile] Updating system profile...");
            
            let profile_result = Command::new("nix")
                .args(["build", "--no-link", "--profile", SYSTEM_PROFILE])
                .arg(out_path.get_path())
                .elevate(true)
                .dry(self.common.common.dry)
                .run();
                
            if let Err(e) = profile_result {
                crate::progress::finish_spinner_fail(&pb_profile);
                crate::error_handler::report_failure(
                    "Profile Update",
                    "Failed to update system profile",
                    Some(e.to_string()),
                    vec![
                        "Check if you have the necessary permissions".to_string(),
                        "Ensure the system profile path is writable".to_string()
                    ]
                );
                bail!("Profile update failed");
            }
            
            crate::progress::finish_spinner_success(&pb_profile, "[✅ Profile] System profile updated successfully");

            let switch_to_configuration = out_path.get_path().join("activate-user");

            // Add progress indicator for user activation
            let pb_user = crate::progress::start_spinner("[👤 User] Activating configuration for user...");
            
            let user_result = Command::new(switch_to_configuration)
                .message("Activating configuration for user")
                .dry(self.common.common.dry)
                .run();
                
            if let Err(e) = user_result {
                crate::progress::finish_spinner_fail(&pb_user);
                crate::error_handler::report_failure(
                    "User Activation",
                    "Failed to activate configuration for user",
                    Some(e.to_string()),
                    vec![
                        "Check if the configuration is valid".to_string(),
                        "Ensure you have the necessary permissions".to_string()
                    ]
                );
                bail!("User activation failed");
            }
            
            crate::progress::finish_spinner_success(&pb_user, "[✅ User] User configuration activated successfully");

            let switch_to_configuration = out_path.get_path().join("activate");

            // Add progress indicator for system activation
            let pb_system = crate::progress::start_spinner("[🚀 System] Activating system configuration...");
            
            let system_result = Command::new(switch_to_configuration)
                .elevate(true)
                .message("Activating configuration")
                .dry(self.common.common.dry)
                .run();
                
            if let Err(e) = system_result {
                crate::progress::finish_spinner_fail(&pb_system);
                crate::error_handler::report_failure(
                    "System Activation",
                    "Failed to activate system configuration",
                    Some(e.to_string()),
                    vec![
                        "Check if the configuration is valid".to_string(),
                        "Ensure you have the necessary permissions".to_string()
                    ]
                );
                bail!("System activation failed");
            }
            
            crate::progress::finish_spinner_success(&pb_system, "[✅ System] System configuration activated successfully");
        }

        // Add cleanup if requested
        if self.common.common.clean {
            let pb_clean = crate::progress::start_spinner("[🧹 Clean] Cleaning up old generations...");
            
            // Run basic gc with nix store
            let mut gc_cmd = std::process::Command::new("nix");
            gc_cmd.args(["store", "gc"]);
            crate::util::add_verbosity_flags(&mut gc_cmd, verbose_count);
            
            match crate::util::run_cmd(&mut gc_cmd) {
                Ok(_) => {
                    crate::progress::finish_spinner_success(&pb_clean, "[✅ Clean] Cleanup completed");
                }
                Err(e) => {
                    crate::progress::finish_spinner_fail(&pb_clean);
                    warn!("Cleanup failed: {}", e);
                    // Don't abort on cleanup failure
                }
            }
        }
        
        // Make sure out_path is not accidentally dropped
        // https://docs.rs/tempfile/3.12.0/tempfile/index.html#early-drop-pitfall
        drop(out_path);
        
        // Final success message
        info!("🏆 Darwin {} completed successfully!",
            match variant {
                DarwinRebuildVariant::Switch => "switch",
                DarwinRebuildVariant::Build => "build"
            }
        );
        
        Ok(())
    }
}

impl DarwinReplArgs {
    fn run(self, verbose_count: u8) -> Result<()> {
        // Use NH_DARWIN_FLAKE if available, otherwise use the provided installable
        let mut target_installable = if let Ok(darwin_flake) = env::var("NH_DARWIN_FLAKE") {
            debug!("Using NH_DARWIN_FLAKE: {}", darwin_flake);

            let mut elems = darwin_flake.splitn(2, '#');
            let reference = elems.next().unwrap().to_owned();
            let attribute = elems
                .next()
                .map(crate::installable::parse_attribute)
                .unwrap_or_default();

            Installable::Flake {
                reference,
                attribute,
            }
        } else {
            self.installable
        };

        if matches!(target_installable, Installable::Store { .. }) {
            bail!("Nix doesn't support nix store installables.");
        }

        let hostname = self.hostname.ok_or(()).or_else(|()| get_hostname())?;

        if let Installable::Flake {
            ref mut attribute, ..
        } = target_installable
        {
            if attribute.is_empty() {
                attribute.push(String::from("darwinConfigurations"));
                attribute.push(hostname);
            }
        }

        Command::new("nix")
            .arg("repl")
            .args(target_installable.to_args())
            .run()?;

        Ok(())
    }
}

// Helper method to run parallel parse check on all .nix files
fn run_parallel_parse_check(verbose_count: u8) -> Result<(), String> {
    use rayon::prelude::*;
    use walkdir::WalkDir;
    
    info!("Running parallel syntax check on .nix files...");
    
    // Find .nix files
    let nix_files: Vec<PathBuf> = WalkDir::new(".")
        .follow_links(true)
        .into_iter()
        .filter_entry(|e| {
            // Don't filter out the current directory
            if e.path() == std::path::Path::new(".") {
                return true;
            }
            
            let is_hidden = e.file_name().to_str().map_or(false, |s| s.starts_with('.'));
            if is_hidden {
                debug!("Skipping hidden entry: {:?}", e.path());
            }
            !is_hidden
        })
        .filter_map(|entry| {
            match entry {
                Ok(e) => {
                    let is_file = e.file_type().is_file();
                    let is_nix = e.path().extension().map_or(false, |ext| ext == "nix");
                    
                    if is_file && is_nix {
                        Some(e.path().to_owned())
                    } else {
                        None
                    }
                },
                Err(_) => None
            }
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
