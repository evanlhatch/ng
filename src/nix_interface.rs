// Phase 3: This is a stub implementation that will be replaced in Phase 3
// with the actual nil integration.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use color_eyre::eyre::eyre;
use tracing::info;

// use nix_interop; // Not used in current implementation
use crate::commands::Command;
use crate::installable::Installable;
use crate::Result;

#[derive(Debug)]
pub struct NixInterface {
    verbose_count: u8,
    dry_run: bool,
} // **** CLOSING BRACE FOR struct NixInterface ****

// **** IMPL BLOCK MOVED OUTSIDE AND AFTER THE STRUCT DEFINITION ****
impl NixInterface {
    pub fn new(verbose_count: u8, dry_run: bool) -> Self {
        Self {
            verbose_count,
            dry_run,
        }
    }

    pub fn build_configuration(
        &self,
        installable: &Installable,
        extra_build_args: &[OsString],
        no_nom: bool,
        out_link: Option<&Path>,
    ) -> Result<PathBuf> {
        let mut build_cmd = Command::new("nix")
            .arg("build")
            .arg(&installable.to_args().join(" "))
            .add_verbosity_flags(self.verbose_count)
            .dry(self.dry_run);

        let mut capture_stdout_for_path = false;

        if !no_nom {
            build_cmd = build_cmd.arg("--no-link");
            if out_link.is_none() && !self.dry_run {
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

        if self.dry_run {
            let cmd_str = build_cmd.to_command_string();
            info!(
                "[Dry Run] NixInterface: Would build: {}",
                cmd_str
            );
            
            // Record the command in test mode even for dry runs
            #[cfg(test)]
            if crate::commands::test_support::is_test_mode_enabled() {
                crate::commands::test_support::record_command(&cmd_str);
            }
            
            if let Some(link_path) = out_link {
                return Ok(link_path.to_path_buf());
            } else if no_nom {
                return Ok(PathBuf::from("./result"));
            } else {
                return Ok(PathBuf::from(
                    "/tmp/dry-run-no-link-no-out-link-placeholder",
                ));
            }
        }

        if capture_stdout_for_path {
            match build_cmd.run_capture()? {
                Some(stdout) => {
                    if let Some(path_str) = stdout.lines().find(|s| !s.trim().is_empty()) {
                        Ok(PathBuf::from(path_str.trim()))
                    } else {
                        Err(eyre!(
                            "nix build --print-out-paths produced no parsable output path"
                        ))
                    }
                }
                None => Err(eyre!("nix build --print-out-paths did not produce stdout")),
            }
        } else {
            build_cmd.run()?;
            if let Some(link_path) = out_link {
                Ok(link_path.to_path_buf())
            } else if no_nom {
                Ok(PathBuf::from("./result"))
            } else {
                eprintln!(
                    "Warning: build_configuration in unexpected state. no_nom={}, out_link={:?}. \
                    Returning conventional \'./result\'.",
                    no_nom, out_link
                );
                Ok(PathBuf::from("./result"))
            }
        }
    }

    pub fn run_gc(&self, dry_run_param: bool) -> Result<()> {
        let gc_cmd = Command::new("sudo")
            .args(["nix-store", "--gc"])
            .add_verbosity_flags(self.verbose_count)
            .dry(self.dry_run);

        if dry_run_param || self.dry_run {
            info!(
                "[Dry Run] NixInterface: Would run GC: {:?}",
                gc_cmd.to_command_string()
            );
            Ok(())
        } else {
            info!(
                "Running garbage collection: {:?}",
                gc_cmd.to_command_string()
            );
            gc_cmd.message("Running garbage collection").run()?;
            Ok(())
        }
    }
} // **** THIS CLOSING BRACE IS FOR impl NixInterface ****

#[cfg(test)]
mod tests {
    use super::*;
    use crate::installable::Installable;
    // OsString is already imported at the top level of nix_interface.rs
    // PathBuf is already imported at the top level of nix_interface.rs

    fn get_test_installable() -> Installable {
        Installable::Flake {
            reference: ".#testPackage".to_string(),
            attribute: Vec::new(),
        }
    }

    #[test]
    fn test_build_configuration_dry_run_with_out_link() {
        let interface = NixInterface::new(0, true); // verbose=0, dry_run=true
        let installable = get_test_installable();
        let out_link = PathBuf::from("/tmp/my-out-link");
        let result = interface.build_configuration(&installable, &[], false, Some(&out_link));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), out_link);
    }

    #[test]
    fn test_build_configuration_dry_run_no_out_link_no_nom_true() {
        // no_nom = true means --no-link is NOT used, so ./result is expected
        let interface = NixInterface::new(0, true);
        let installable = get_test_installable();
        let result = interface.build_configuration(&installable, &[], true, None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PathBuf::from("./result"));
    }

    #[test]
    fn test_build_configuration_dry_run_no_out_link_no_nom_false() {
        // no_nom = false means --no-link IS used. With no out_link, and dry_run, expect placeholder.
        let interface = NixInterface::new(0, true);
        let installable = get_test_installable();
        let result = interface.build_configuration(&installable, &[], false, None);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            PathBuf::from("/tmp/dry-run-no-link-no-out-link-placeholder")
        );
    }

    #[test]
    fn test_run_gc_dry_run_true_param() {
        let interface = NixInterface::new(0, false); // Interface itself not in dry_run mode
        let result = interface.run_gc(true); // Call with dry_run_param = true
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_gc_dry_run_interface_mode() {
        let interface = NixInterface::new(0, true); // Interface IS in dry_run mode
        let result = interface.run_gc(false); // Call with dry_run_param = false
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_config_capture_print_out_paths() {
        crate::commands::test_support::enable_test_mode();
        let expected_path_str = "/nix/store/somehash-package";
        crate::commands::test_support::set_mock_capture_stdout(expected_path_str.to_string());

        let interface = NixInterface::new(1, false);
        let installable = get_test_installable();

        let result = interface.build_configuration(&installable, &[], false, None);

        assert!(
            result.is_ok(),
            "build_configuration failed: {:?}",
            result.err()
        );
        assert_eq!(result.unwrap(), PathBuf::from(expected_path_str));

        let recorded_commands = crate::commands::test_support::get_recorded_commands();
        assert_eq!(recorded_commands.len(), 1, "Expected exactly one command to be recorded in this test");
        let cmd_to_check = recorded_commands.first().expect("No command was recorded").clone();
        assert!(cmd_to_check.contains("nix build"));
        assert!(cmd_to_check.contains(&installable.to_args().join(" ")));
        assert!(cmd_to_check.contains("--no-link"));
        assert!(cmd_to_check.contains("--print-out-paths"));
        assert!(cmd_to_check.contains("-v")); // from verbose_count = 1

        crate::commands::test_support::disable_test_mode();
    }

    #[test]
    fn test_build_config_capture_failure() {
        crate::commands::test_support::enable_test_mode();
        let error_message = "mocked capture failure from nix build"; // Changed message for clarity from duplicates
        crate::commands::test_support::set_mock_capture_error(error_message.to_string());

        let interface = NixInterface::new(0, false); // Not in dry_run
        let installable = get_test_installable();
        let result = interface.build_configuration(&installable, &[], false, None); // Trigger capture

        assert!(result.is_err(), "Expected build_configuration to fail");
        if let Err(e) = result {
            assert!(e.to_string().contains(error_message));
        }

        let recorded_commands = crate::commands::test_support::get_recorded_commands();
        assert_eq!(recorded_commands.len(), 1, "Expected exactly one command to be recorded in this test");
        let cmd_to_check = recorded_commands.first().expect("No command was recorded").clone();
        assert!(cmd_to_check.contains("--print-out-paths")); // Ensure it tried the capture path
        crate::commands::test_support::disable_test_mode();
    }

    #[test]
    fn test_build_config_capture_empty_stdout() {
        crate::commands::test_support::enable_test_mode();
        crate::commands::test_support::set_mock_capture_stdout("\n   \n  ".to_string()); // Empty/whitespace lines

        let interface = NixInterface::new(0, false);
        let installable = get_test_installable();
        let result = interface.build_configuration(&installable, &[], false, None); // Trigger capture

        assert!(
            result.is_err(),
            "Expected build_configuration to fail for empty stdout"
        );
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("produced no parsable output path"));
        let recorded_commands = crate::commands::test_support::get_recorded_commands();
        assert_eq!(recorded_commands.len(), 1, "Expected exactly one command to be recorded in this test");
        let cmd_to_check = recorded_commands.first().expect("No command was recorded").clone();
        assert!(cmd_to_check.contains("--print-out-paths")); // Ensure it tried the capture path
        crate::commands::test_support::disable_test_mode();
    }

    #[test]
    fn test_build_config_run_failure() {
        crate::commands::test_support::enable_test_mode();
        let error_message = "mocked nix build direct run failure";
        crate::commands::test_support::set_mock_run_result(Err(eyre!(error_message.to_string())));

        let interface = NixInterface::new(0, false); // Not in dry_run
        let installable = get_test_installable();
        let out_link = PathBuf::from("/tmp/some-link");
        // Trigger direct run by providing an out_link and no_nom = true
        let result = interface.build_configuration(&installable, &[], true, Some(&out_link));

        assert!(
            result.is_err(),
            "Expected build_configuration to fail on run error"
        );
        assert!(result.unwrap_err().to_string().contains(error_message));
        let recorded_commands = crate::commands::test_support::get_recorded_commands();
        assert_eq!(recorded_commands.len(), 1, "Expected exactly one command to be recorded in this test");
        let cmd_to_check = recorded_commands.first().expect("No command was recorded").clone();
        assert!(!cmd_to_check.contains("--print-out-paths")); // Ensure it did NOT try the capture path
        crate::commands::test_support::disable_test_mode();
    }

    #[test]
    fn test_run_gc_non_dry_run_success() {
        crate::commands::test_support::enable_test_mode();
        crate::commands::test_support::set_mock_run_result(Ok(()));

        let interface = NixInterface::new(0, false); // Interface not in dry_run
        let result = interface.run_gc(false); // Call with dry_run_param = false

        assert!(result.is_ok(), "run_gc failed: {:?}", result.err());
        let recorded_commands = crate::commands::test_support::get_recorded_commands();
        assert_eq!(recorded_commands.len(), 1, "Expected exactly one command to be recorded in this test");
        let cmd_to_check = recorded_commands.first().expect("No command was recorded").clone();
        assert!(cmd_to_check.contains("sudo nix-store --gc"));
        crate::commands::test_support::disable_test_mode();
    }

    #[test]
    fn test_run_gc_non_dry_run_failure() {
        crate::commands::test_support::enable_test_mode();
        let error_message = "mocked gc failure";
        crate::commands::test_support::set_mock_run_result(Err(eyre!(error_message.to_string())));

        let interface = NixInterface::new(0, false); // Interface not in dry_run
        let result = interface.run_gc(false); // Call with dry_run_param = false

        assert!(result.is_err(), "Expected run_gc to fail");
        assert!(result.unwrap_err().to_string().contains(error_message));
        let recorded_commands = crate::commands::test_support::get_recorded_commands();
        assert_eq!(recorded_commands.len(), 1, "Expected exactly one command to be recorded in this test");
        let cmd_to_check = recorded_commands.first().expect("No command was recorded").clone();
        assert!(cmd_to_check.contains("sudo nix-store --gc"));
        crate::commands::test_support::disable_test_mode();
    }
    // Removed duplicated test blocks that were present in the original code
} // **** THIS IS THE FINAL CLOSING BRACE for mod tests ****
