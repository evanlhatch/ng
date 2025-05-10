use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use color_eyre::eyre::{bail, Context};
use color_eyre::eyre::{eyre, Result};
use tracing::{debug, info, warn};
use walkdir::WalkDir;
use rayon::prelude::*;

use crate::commands;
use crate::commands::Command;
use crate::generations;
use crate::installable::Installable;
use crate::interface::OsSubcommand::{self};
use crate::interface::{self, OsGenerationsArgs, OsRebuildArgs, OsReplArgs};
use crate::update::update;
use crate::util::get_hostname;

const SYSTEM_PROFILE: &str = "/nix/var/nix/profiles/system";
const CURRENT_PROFILE: &str = "/run/current-system";

const SPEC_LOCATION: &str = "/etc/specialisation";

impl interface::OsArgs {
    pub fn run(self, verbose_count: u8) -> Result<()> {
        use OsRebuildVariant::*;
        match self.subcommand {
            OsSubcommand::Boot(args) => args.rebuild(Boot, verbose_count),
            OsSubcommand::Test(args) => args.rebuild(Test, verbose_count),
            OsSubcommand::Switch(args) => args.rebuild(Switch, verbose_count),
            OsSubcommand::Build(args) => {
                if args.common.common.ask || args.common.common.dry {
                    warn!("`--ask` and `--dry` have no effect for `nh os build`");
                }
                args.rebuild(Build, verbose_count)
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
            let pb = crate::progress::start_spinner("[🧩 Parse] Checking syntax...");
            match self.run_parallel_parse_check(verbose_count) {
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

        // Get hostname early for eval and dry-run checks
        let hostname = self.hostname.ok_or(()).or_else(|()| get_hostname())?;

        // Eval Check (Medium Mode)
        if run_preflight && (self.common.common.medium || self.common.common.full) {
            let pb = crate::progress::start_spinner("[⚙️ Eval] Evaluating configuration...");
            
            // Determine the flake reference and attribute path
            let flake_ref = "."; // Current directory
            let attribute_path = match &self.common.installable {
                Installable::Flake { reference, attribute } => {
                    let mut attr_path = vec!["nixosConfigurations".to_string()];
                    attr_path.push(hostname.to_string());
                    if !attribute.is_empty() {
                        attr_path.extend(attribute.iter().cloned());
                    }
                    (reference.clone(), attr_path)
                },
                _ => {
                    // For non-flake installables, we can't easily do an eval check
                    crate::progress::finish_spinner_success(&pb, "[⏭️ Eval] Skipped (non-flake installable).");
                    (".".to_string(), vec![])
                }
            };
            
            // Only proceed with eval if we have a valid attribute path
            if !attribute_path.1.is_empty() {
                // Build and run the nix eval command
                let mut eval_cmd = std::process::Command::new("nix");
                eval_cmd.args(["eval", "--show-trace", &format!("{}#{}", attribute_path.0, attribute_path.1.join("."))]);
                
                // Add verbosity flags
                crate::util::add_verbosity_flags(&mut eval_cmd, verbose_count);
                
                match crate::util::run_cmd(&mut eval_cmd) {
                    Ok(_) => {
                        crate::progress::finish_spinner_success(&pb, "[✅ Eval] Configuration evaluated successfully.");
                    },
                    Err(e) => {
                        crate::progress::finish_spinner_fail(&pb);
                        
                        // Try to get a trace for better error reporting
                        let trace = match crate::error_handler::fetch_nix_trace(&attribute_path.0, &attribute_path.1, verbose_count) {
                            Ok(trace_output) => Some(trace_output),
                            Err(_) => None,
                        };
                        
                        crate::error_handler::report_failure(
                            "Eval",
                            "Configuration evaluation failed",
                            trace,
                            vec![
                                "Check your configuration for errors".to_string(),
                                "Use --no-preflight to skip evaluation".to_string()
                            ]
                        );
                        bail!("Eval check failed");
                    }
                }
            }
        }
        
        // Dry Run Build (Full Mode)
        if run_preflight && self.common.common.full {
            let pb = crate::progress::start_spinner("[🧪 Dry Run] Performing dry-run build...");
            
            // Build the dry-run command
            let mut dry_run_cmd = std::process::Command::new("nix");
            
            // Determine the installable
            let dry_run_target = match &self.common.installable {
                Installable::Flake { reference, attribute } => {
                    let mut attr_path = vec!["nixosConfigurations".to_string()];
                    attr_path.push(hostname.to_string());
                    if !attribute.is_empty() {
                        attr_path.extend(attribute.iter().cloned());
                    }
                    format!("{}#{}", reference, attr_path.join("."))
                },
                _ => {
                    // For non-flake installables, we can't easily do a dry-run
                    crate::progress::finish_spinner_success(&pb, "[⏭️ Dry Run] Skipped (non-flake installable).");
                    String::new()
                }
            };
            
            if !dry_run_target.is_empty() {
                dry_run_cmd.args(["build", "--dry-run", &dry_run_target]);
                
                // Add verbosity flags
                crate::util::add_verbosity_flags(&mut dry_run_cmd, verbose_count);
                
                match crate::util::run_cmd(&mut dry_run_cmd) {
                    Ok(_) => {
                        crate::progress::finish_spinner_success(&pb, "[✅ Dry Run] Build simulation successful.");
                    },
                    Err(e) => {
                        crate::progress::finish_spinner_fail(&pb);
                        
                        // Extract failed derivations from stderr if possible
                        let stderr = String::from_utf8_lossy(&e.to_string().as_bytes()).to_string();
                        let failed_drvs = crate::error_handler::find_failed_derivations(&stderr);
                        
                        // Try to get logs for the failed derivations
                        let mut logs = Vec::new();
                        for drv in &failed_drvs {
                            if let Ok(log) = crate::error_handler::fetch_and_format_nix_log(drv, verbose_count) {
                                logs.push(log);
                            }
                        }
                        
                        let combined_logs = if !logs.is_empty() {
                            Some(logs.join("\n\n"))
                        } else {
                            Some(stderr)
                        };
                        
                        crate::error_handler::report_failure(
                            "Dry Run",
                            "Build simulation failed",
                            combined_logs,
                            vec![
                                "Check the build logs for errors".to_string(),
                                "Use --no-preflight or --medium instead of --full to skip dry-run".to_string()
                            ]
                        );
                        bail!("Dry run check failed");
                    }
                }
            }
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
        
        // Use NH_OS_FLAKE if available, otherwise use the provided installable
        let installable = if let Ok(os_flake) = env::var("NH_OS_FLAKE") {
            debug!("Using NH_OS_FLAKE: {}", os_flake);
        
            let mut elems = os_flake.splitn(2, '#');
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
        
        let toplevel = toplevel_for(hostname, installable);
        
        // Add progress indicator for build
        let pb_build = crate::progress::start_spinner("[🔨 Build] Building configuration...");
        
        // Use the existing build mechanism but enhance error handling
        let build_result = commands::Build::new(toplevel)
            .extra_arg("--out-link")
            .extra_arg(out_path.get_path())
            .extra_args(&self.extra_args)
            .message("Building NixOS configuration")
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
                "Failed to build NixOS configuration",
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

        let current_specialisation = std::fs::read_to_string(SPEC_LOCATION).ok();

        let target_specialisation = if self.no_specialisation {
            None
        } else {
            current_specialisation.or_else(|| self.specialisation.to_owned())
        };

        debug!("target_specialisation: {target_specialisation:?}");

        let target_profile = match &target_specialisation {
            None => out_path.get_path().to_owned(),
            Some(spec) => out_path.get_path().join("specialisation").join(spec),
        };

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

        if let Test | Switch = variant {
            // Add progress indicator for activation
            let pb_activate = crate::progress::start_spinner("[🚀 Activate] Activating configuration...");
            
            // !! Use the target profile aka spec-namespaced
            let switch_to_configuration =
                target_profile.join("bin").join("switch-to-configuration");
            let switch_to_configuration = switch_to_configuration.to_str().unwrap();
        
            let activate_result = Command::new(switch_to_configuration)
                .arg("test")
                .message("Activating configuration")
                .elevate(elevate)
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
        }

        if let Boot | Switch = variant {
            // Add progress indicator for bootloader update
            let pb_boot = crate::progress::start_spinner("[⚙️ Boot] Updating bootloader...");
            
            let boot_result = Command::new("nix")
                .elevate(elevate)
                .args(["build", "--no-link", "--profile", SYSTEM_PROFILE])
                .arg(out_path.get_path())
                .run();
                
            if let Err(e) = boot_result {
                crate::progress::finish_spinner_fail(&pb_boot);
                crate::error_handler::report_failure(
                    "Boot",
                    "Failed to update bootloader",
                    Some(e.to_string()),
                    vec![
                        "Check if you have the necessary permissions".to_string(),
                        "Ensure the system profile path is writable".to_string()
                    ]
                );
                bail!("Bootloader update failed");
            }
        
            // !! Use the base profile aka no spec-namespace
            let switch_to_configuration = out_path
                .get_path()
                .join("bin")
                .join("switch-to-configuration");

            let boot_config_result = Command::new(switch_to_configuration)
                .arg("boot")
                .elevate(elevate)
                .message("Adding configuration to bootloader")
                .run();
                
            if let Err(e) = boot_config_result {
                crate::progress::finish_spinner_fail(&pb_boot);
                crate::error_handler::report_failure(
                    "Boot Configuration",
                    "Failed to add configuration to bootloader",
                    Some(e.to_string()),
                    vec![
                        "Check if the bootloader configuration is valid".to_string(),
                        "Ensure you have the necessary permissions".to_string()
                    ]
                );
                bail!("Bootloader configuration failed");
            }
            
            crate::progress::finish_spinner_success(&pb_boot, "[✅ Boot] Bootloader updated successfully");
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
        info!("🏆 NixOS {} completed successfully!",
            match variant {
                OsRebuildVariant::Switch => "switch",
                OsRebuildVariant::Boot => "boot",
                OsRebuildVariant::Test => "test",
                OsRebuildVariant::Build => "build"
            }
        );
        
        Ok(())
    }

    // Helper method to run parallel parse check on all .nix files
    fn run_parallel_parse_check(&self, verbose_count: u8) -> Result<(), String> {
        info!("Running parallel syntax check on .nix files...");
        
        // Print current working directory
        if let Ok(cwd) = std::env::current_dir() {
            debug!("Current working directory: {:?}", cwd);
            
            // List files in current directory
            if let Ok(entries) = std::fs::read_dir(".") {
                debug!("Files in current directory:");
                for entry in entries {
                    if let Ok(entry) = entry {
                        debug!("  {:?}", entry.path());
                    }
                }
            }
        }
        
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
                        
                        if is_file {
                            debug!("Found file: {:?}, is_nix: {}", e.path(), is_nix);
                        }
                        
                        if is_file && is_nix {
                            Some(e.path().to_owned())
                        } else {
                            None
                        }
                    },
                    Err(err) => {
                        debug!("Error accessing entry: {:?}", err);
                        None
                    }
                }
            })
            .collect();
            
        if nix_files.is_empty() {
            info!("No .nix files found to check.");
            return Ok(());
        }
        
        debug!("Found {} .nix files to check", nix_files.len());
        
        // Print out the files found for debugging
        for file in &nix_files {
            debug!("Found .nix file: {:?}", file);
        }
        
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
}

pub fn toplevel_for<S: AsRef<str>>(hostname: S, installable: Installable) -> Installable {
    let mut res = installable;
    let hostname = hostname.as_ref().to_owned();

    let toplevel = ["config", "system", "build", "toplevel"]
        .into_iter()
        .map(String::from);

    match res {
        Installable::Flake {
            ref mut attribute, ..
        } => {
            // If user explicitly selects some other attribute, don't push nixosConfigurations
            if attribute.is_empty() {
                attribute.push(String::from("nixosConfigurations"));
                attribute.push(hostname);
            }
            attribute.extend(toplevel);
        }
        Installable::File {
            ref mut attribute, ..
        } => {
            attribute.extend(toplevel);
        }
        Installable::Expression {
            ref mut attribute, ..
        } => {
            attribute.extend(toplevel);
        }
        Installable::Store { .. } => {}
    }

    res
}

impl OsReplArgs {
    fn run(self, verbose_count: u8) -> Result<()> {
        // Use NH_OS_FLAKE if available, otherwise use the provided installable
        let mut target_installable = if let Ok(os_flake) = env::var("NH_OS_FLAKE") {
            debug!("Using NH_OS_FLAKE: {}", os_flake);

            let mut elems = os_flake.splitn(2, '#');
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
                attribute.push(String::from("nixosConfigurations"));
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

impl OsGenerationsArgs {
    fn info(&self) -> Result<()> {
        let profile = match self.profile {
            Some(ref p) => PathBuf::from(p),
            None => bail!("Profile path is required"),
        };

        if !profile.is_symlink() {
            return Err(eyre!(
                "No profile `{:?}` found",
                profile.file_name().unwrap_or_default()
            ));
        }

        let profile_dir = profile.parent().unwrap_or_else(|| Path::new("."));

        let generations: Vec<_> = fs::read_dir(profile_dir)?
            .filter_map(|entry| {
                entry.ok().and_then(|e| {
                    let path = e.path();
                    if path
                        .file_name()?
                        .to_str()?
                        .starts_with(profile.file_name()?.to_str()?)
                    {
                        Some(path)
                    } else {
                        None
                    }
                })
            })
            .collect();

        let descriptions: Vec<generations::GenerationInfo> = generations
            .iter()
            .filter_map(|gen_dir| generations::describe(gen_dir, &profile))
            .collect();

        generations::print_info(descriptions);

        Ok(())
    }
}
