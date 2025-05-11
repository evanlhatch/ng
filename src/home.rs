use std::env;
use std::path::PathBuf;

use color_eyre::eyre::bail;
use color_eyre::Result;
use tracing::{debug, info, warn};

use crate::commands;
use crate::commands::Command;
use crate::installable::Installable;
use crate::interface::{self, HomeRebuildArgs, HomeReplArgs, HomeSubcommand};
use crate::update::update;
use crate::util::get_hostname;

impl interface::HomeArgs {
    pub fn run(self, verbose_count: u8) -> Result<()> {
        use HomeRebuildVariant::*;
        match self.subcommand {
            HomeSubcommand::Switch(args) => args.rebuild(Switch, verbose_count),
            HomeSubcommand::Build(args) => {
                if args.common.common.ask || args.common.common.dry {
                    warn!("`--ask` and `--dry` have no effect for `nh home build`");
                }
                args.rebuild(Build, verbose_count)
            }
            HomeSubcommand::Repl(args) => args.run(verbose_count),
        }
    }
}

#[derive(Debug)]
enum HomeRebuildVariant {
    Build,
    Switch,
}

impl HomeRebuildArgs {
    fn rebuild(self, variant: HomeRebuildVariant, verbose_count: u8) -> Result<()> {
        use HomeRebuildVariant::*;
    
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
            
            let cli_strict_lint = self.common.common.strict_lint;
            // For home.rs, if CLI is None, this implies false (non-strict) unless overridden by config later (not handled here)
            // Or, if full/medium checks are on, imply strictness for this specific context.
            let use_strict_lint = cli_strict_lint.unwrap_or(false) || self.common.common.full || self.common.common.medium;
            
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

        let out_path: Box<dyn crate::util::MaybeTempPath> = match self.common.common.out_link {
            Some(ref p) => Box::new(p.clone()),
            None => Box::new({
                let dir = tempfile::Builder::new().prefix("nh-os").tempdir()?;
                (dir.as_ref().join("result"), dir)
            }),
        };

        debug!(?out_path);

        // Use NH_HOME_FLAKE if available, otherwise use the provided installable
        let installable = if let Ok(home_flake) = env::var("NH_HOME_FLAKE") {
            debug!("Using NH_HOME_FLAKE: {}", home_flake);

            let mut elems = home_flake.splitn(2, '#');
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

        let toplevel = toplevel_for(installable, true, &self.extra_args)?;
        
        // Add progress indicator for build
        let pb_build = crate::progress::start_spinner("[🔨 Build] Building configuration...");
        
        // Use the existing build mechanism but enhance error handling
        let build_result = commands::Build::new(toplevel)
            .extra_arg("--out-link")
            .extra_arg(out_path.get_path())
            .extra_args(&self.extra_args)
            .message("Building Home-Manager configuration")
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
                "Failed to build Home-Manager configuration",
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

        let prev_generation: Option<PathBuf> = [
            PathBuf::from("/nix/var/nix/profiles/per-user")
                .join(env::var("USER").expect("Couldn't get username"))
                .join("home-manager"),
            PathBuf::from(env::var("HOME").expect("Couldn't get home directory"))
                .join(".local/state/nix/profiles/home-manager"),
        ]
        .into_iter()
        .find(|next| next.exists());

        debug!(?prev_generation);

        let spec_location =
            PathBuf::from(std::env::var("HOME")?).join(".local/share/home-manager/specialisation");

        let current_specialisation = std::fs::read_to_string(spec_location.to_str().unwrap()).ok();

        let target_specialisation = if self.no_specialisation {
            None
        } else {
            current_specialisation.or(self.specialisation)
        };

        debug!("target_specialisation: {target_specialisation:?}");

        let target_profile = match &target_specialisation {
            None => out_path,
            Some(spec) => Box::new(out_path.get_path().join("specialisation").join(spec)),
        };

        // just do nothing for None case (fresh installs)
        if let Some(generation) = prev_generation {
            // Add progress indicator for diff
            let pb_diff = crate::progress::start_spinner("[🔍 Diff] Comparing changes...");
            
            let diff_result = Command::new("nvd")
                .arg("diff")
                .arg(generation)
                .arg(target_profile.get_path())
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
        }

        if self.common.common.dry || matches!(variant, Build) {
            if self.common.common.ask {
                warn!("--ask has no effect as dry run was requested");
            }
            return Ok(());
        }

        if self.common.common.ask {
            info!("Apply the config?");
            let confirmation = dialoguer::Confirm::new().default(false).interact()?;

            if !confirmation {
                bail!("User rejected the new config");
            }
        }

        if let Some(ext) = &self.backup_extension {
            info!("Using {} as the backup extension", ext);
            env::set_var("HOME_MANAGER_BACKUP_EXT", ext);
        }

        // Add progress indicator for activation
        let pb_activate = crate::progress::start_spinner("[🚀 Activate] Activating configuration...");
        
        let activate_result = Command::new(target_profile.get_path().join("activate"))
            .message("Activating configuration")
            .run();
            
        if let Err(e) = activate_result {
            crate::progress::finish_spinner_fail(&pb_activate);
            crate::error_handler::report_failure(
                "Activation",
                "Failed to activate configuration",
                Some(e.to_string()),
                vec![
                    "Check if the configuration is valid".to_string(),
                    "Ensure you have the necessary permissions".to_string()
                ]
            );
            bail!("Activation failed");
        }
        
        crate::progress::finish_spinner_success(&pb_activate, "[✅ Activate] Configuration activated successfully");

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
        drop(target_profile);
        
        // Final success message
        info!("🏆 Home-Manager {} completed successfully!",
            match variant {
                HomeRebuildVariant::Switch => "switch",
                HomeRebuildVariant::Build => "build"
            }
        );
        
        Ok(())
    }
}

fn toplevel_for<I, S>(
    installable: Installable,
    push_drv: bool,
    extra_args: I,
) -> Result<Installable>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let mut res = installable.clone();
    let extra_args = {
        let mut vec = Vec::new();
        for elem in extra_args.into_iter() {
            vec.push(elem.as_ref().to_owned());
        }
        vec
    };

    let toplevel = ["config", "home", "activationPackage"]
        .into_iter()
        .map(String::from);

    match res {
        Installable::Flake {
            ref reference,
            ref mut attribute,
        } => 'flake: {
            // If user explicitly selects some other attribute, don't push homeConfigurations
            if !attribute.is_empty() {
                break 'flake;
            }

            attribute.push(String::from("homeConfigurations"));

            // check for <user> and <user@hostname>
            let username = std::env::var("USER").expect("Couldn't get username");
            let hostname = get_hostname()?;

            let flake_reference = reference.clone();

            let mut tried = vec![];

            for attr in [format!("{username}@{hostname}"), username.to_string()] {
                let func = format!(r#" x: x ? "{}" "#, attr);
                let res = commands::Command::new("nix")
                    .arg("eval")
                    .args(&extra_args)
                    .arg("--apply")
                    .arg(func)
                    .args(
                        (Installable::Flake {
                            reference: flake_reference.clone(),
                            attribute: attribute.clone(),
                        })
                        .to_args(),
                    )
                    .run_capture()
                    .expect("Checking home-manager output");

                tried.push({
                    let mut attribute = attribute.clone();
                    attribute.push(attr.clone());
                    attribute
                });

                match res.map(|s| s.trim().to_owned()).as_deref() {
                    Some("true") => {
                        attribute.push(attr.clone());
                        if push_drv {
                            attribute.extend(toplevel);
                        }
                        break 'flake;
                    }
                    _ => {
                        continue;
                    }
                }
            }

            let tried_str = tried
                .into_iter()
                .map(|a| {
                    let f = Installable::Flake {
                        reference: flake_reference.clone(),
                        attribute: a,
                    };
                    f.to_args().join(" ")
                })
                .collect::<Vec<_>>()
                .join(", ");

            bail!("Couldn't find home-manager configuration, tried {tried_str}");
        }
        Installable::File {
            ref mut attribute, ..
        } => {
            if push_drv {
                attribute.extend(toplevel);
            }
        }
        Installable::Expression {
            ref mut attribute, ..
        } => {
            if push_drv {
                attribute.extend(toplevel);
            }
        }
        Installable::Store { .. } => {}
    }

    Ok(res)
}

impl HomeReplArgs {
    fn run(self, _verbose_count: u8) -> Result<()> {
        // Use NH_HOME_FLAKE if available, otherwise use the provided installable
        let installable = if let Ok(home_flake) = env::var("NH_HOME_FLAKE") {
            debug!("Using NH_HOME_FLAKE: {}", home_flake);

            let mut elems = home_flake.splitn(2, '#');
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

        let toplevel = toplevel_for(installable, false, &self.extra_args)?;

        Command::new("nix")
            .arg("repl")
            .args(toplevel.to_args())
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
