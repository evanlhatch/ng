// Phase 3: This is a stub implementation that will be replaced in Phase 3
// with the actual nil integration.

use crate::installable::Installable;
use crate::Result;
use std::path::{Path, PathBuf};
use std::ffi::OsString;
use color_eyre::eyre::eyre; // Added for eyre! macro
// use nix_interop; // Not used in current implementation

use crate::commands::Command; // For Nix command execution
use tracing::info; // For logging

#[derive(Debug)]
pub struct NixInterface {
    verbose_count: u8,
}

impl NixInterface {
    pub fn new(verbose_count: u8) -> Self {
        Self { verbose_count }
    }

    pub fn build_configuration(
        &self,
        installable: &Installable,
        extra_build_args: &[OsString],
        no_nom: bool, // if true, --no-link is NOT used. if false, --no-link IS used.
        out_link: Option<&Path>,
    ) -> Result<PathBuf> {
        let mut build_cmd = Command::new("nix")
            .arg("build")
            .arg(&installable.to_args().join(" "))
            .add_verbosity_flags(self.verbose_count);

        let mut capture_stdout_for_path = false;

        if !no_nom { // --no-link IS used
            build_cmd = build_cmd.arg("--no-link");
            if out_link.is_none() {
                // If --no-link is used and no --out-link, we need to get path from stdout
                build_cmd = build_cmd.arg("--print-out-paths");
                capture_stdout_for_path = true;
            }
        }

        for arg in extra_build_args {
            build_cmd = build_cmd.arg(arg);
        }

        if let Some(link_path) = out_link {
            build_cmd = build_cmd.arg("--out-link").arg(link_path);
        }

        if capture_stdout_for_path {
            match build_cmd.run_capture()? {
                Some(stdout) => {
                    // Nix usually prints one path per line to stdout for --print-out-paths
                    // We'll take the first non-empty line.
                    if let Some(path_str) = stdout.lines().find(|s| !s.trim().is_empty()) {
                        Ok(PathBuf::from(path_str.trim()))
                    } else {
                        Err(eyre!("nix build --print-out-paths produced no parsable output path"))
                    }
                }
                None => Err(eyre!("nix build --print-out-paths did not produce stdout (or was dry run unexpectedly)")),
            }
        } else {
            // Original behavior: command runs, errors if fails.
            // Result path determined by out_link or convention.
            build_cmd.run()?;
            if let Some(link_path) = out_link {
                Ok(link_path.to_path_buf())
            } else if no_nom { // --no-link is NOT used, so ./result should be created
                Ok(PathBuf::from("./result")) 
            } else {
                // This case should ideally not be reached if capture_stdout_for_path was true.
                // This implies --no-link was used, but we didn't capture stdout, and no out_link.
                // This is a fallback, but indicates a logic flaw if hit.
                eprintln!(
                    "Warning: build_configuration in unexpected state. no_nom={}, out_link={:?}. \
                    Returning conventional './result'.", no_nom, out_link
                );
                Ok(PathBuf::from("./result"))
            }
        }
    }

    pub fn run_gc(&self, dry_run: bool) -> Result<()> {
        let gc_cmd = Command::new("sudo").args(["nix-store", "--gc"])
            .add_verbosity_flags(self.verbose_count);

        if dry_run {
            info!("[Dry Run] Would run: {:?}", gc_cmd);
            Ok(())
        } else {
            info!("Running garbage collection: {:?}", gc_cmd);
            gc_cmd.message("Running garbage collection").run()?;
            Ok(())
        }
    }
}