#[cfg(test)]
mod tests {
    use std::fs;
    
    use std::sync::Arc;

    use tempfile::TempDir;

    use crate::commands::test_support;
    use crate::config::NgConfig;
    use crate::context::OperationContext;
    use crate::installable::Installable;
    use crate::interface::{CommonArgs as ClapCommonArgs, UpdateArgs};
    use crate::nix_interface::NixInterface;
    use crate::pre_flight::run_shared_pre_flight_checks;
    use crate::workflow_strategy::MockPlatformStrategy;
    use crate::workflow_types;

    fn create_op_ctx_for_preflight(
        temp_dir: &TempDir, // Pass TempDir to use its path as project_root
        config_str: &str,
        cli_args_modifier: Option<fn(&mut ClapCommonArgs)>,
    ) -> OperationContext<'static> {
        let loaded_config = Arc::new(NgConfig::from_str(config_str).unwrap_or_else(|e| {
            panic!(
                "Failed to parse test NgConfig from string: {}\nContent:\n{}",
                e, config_str
            );
));
        let mut clap_common_args = ClapCommonArgs {
            no_preflight: false,
            strict_lint: None,   // Changed for Option<bool>
            strict_format: None, // Changed for Option<bool>
            medium: false,
            full: false,
            dry: true,
            // Note: clap_common_args.ask is bool, but its clap default is true.
            // This test helper sets it to false, which is fine if not testing 'ask' specifically.
            ask: false,
            no_nom: true,
            out_link: None,
            clean: false,
        };

        if let Some(modifier) = cli_args_modifier {
            modifier(&mut clap_common_args);
        }

        let workflow_common_args = workflow_types::CommonRebuildArgs {
            installable: Installable::Flake {
                reference: ".#dummy".to_string(),
                attribute: Vec::new(),
            },
            no_preflight: clap_common_args.no_preflight,
            strict_lint: clap_common_args.strict_lint,
            strict_format: clap_common_args.strict_format,
            medium_checks: clap_common_args.medium,
            full_checks: clap_common_args.full,
            dry_run: clap_common_args.dry,
            ask_confirmation: clap_common_args.ask,
            no_nom: clap_common_args.no_nom,
            out_link: clap_common_args.out_link.clone(),
            clean_after: clap_common_args.clean,
            extra_build_args: Vec::new(),
        };

        static DUMMY_UPDATE_ARGS: UpdateArgs = UpdateArgs {
            update: false,
            update_input: None,
        };

        // Create NixInterface before workflow_common_args is moved
        let nix_interface = NixInterface::new(0, workflow_common_args.dry_run);

        OperationContext::new(
            workflow_common_args, // Moved here
            &DUMMY_UPDATE_ARGS,
            0,             // verbose_count
            nix_interface, // Pass the created interface
            loaded_config,
            Some(temp_dir.path().to_path_buf()),
        )
    }

    #[test]
    fn test_config_default_checks_run_in_order() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("good.nix"), "{ value = 1; }").unwrap();
        test_support::enable_test_mode();
        test_support::set_mock_run_result(Ok(()));

        let op_ctx = create_op_ctx_for_preflight(&temp_dir, "[pre_flight]", None);
        let mock_strategy = MockPlatformStrategy;
        let dummy_platform_args = ();

        let result = run_shared_pre_flight_checks(&op_ctx, &mock_strategy, &dummy_platform_args);
        assert!(
            result.is_ok(),
            "run_shared_pre_flight_checks failed: {:?}",
            result.err()
        );

        test_support::disable_test_mode();
    }

    #[test]
    fn test_config_custom_check_order() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("good.nix"), "{ value = 1; }").unwrap();
        test_support::enable_test_mode();
        test_support::set_mock_run_result(Ok(()));

        let ng_toml_content = r#"
[pre_flight]
checks = ["Nix Code Format", "Nix Syntax Parse", "Nix Semantic Check"]
"#;
        let op_ctx = create_op_ctx_for_preflight(&temp_dir, ng_toml_content, None);
        let mock_strategy = MockPlatformStrategy;
        let dummy_platform_args = ();

        let result = run_shared_pre_flight_checks(&op_ctx, &mock_strategy, &dummy_platform_args);
        assert!(result.is_ok());

        test_support::disable_test_mode();
    }

    #[test]
    fn test_config_no_checks() {
        let temp_dir = TempDir::new().unwrap();
        let op_ctx = create_op_ctx_for_preflight(&temp_dir, "[pre_flight]\nchecks = []", None);
        let mock_strategy = MockPlatformStrategy;
        let dummy_platform_args = ();
        let result = run_shared_pre_flight_checks(&op_ctx, &mock_strategy, &dummy_platform_args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_config_unknown_check_is_skipped() {
        let temp_dir = TempDir::new().unwrap();
        let ng_toml_content = r#"
[pre_flight]
checks = ["unknown_check", "Nix Syntax Parse"]
"#;
        let op_ctx = create_op_ctx_for_preflight(&temp_dir, ng_toml_content, None);
        let mock_strategy = MockPlatformStrategy;
        let dummy_platform_args = ();
        let result = run_shared_pre_flight_checks(&op_ctx, &mock_strategy, &dummy_platform_args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_config_strict_lint_fails_critically() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("bad_semantic.nix"), "{ a = b; }").unwrap();
        test_support::enable_test_mode();

        let ng_toml_content = r#"
[pre_flight]
checks = ["Nix Semantic Check"]
strict_lint = true
"#;
        let op_ctx = create_op_ctx_for_preflight(&temp_dir, ng_toml_content, None);
        let mock_strategy = MockPlatformStrategy;
        let dummy_platform_args = ();

        let result = run_shared_pre_flight_checks(&op_ctx, &mock_strategy, &dummy_platform_args);
        assert!(
            result.is_err(),
            "Expected pre-flight checks to fail critically due to strict_lint"
        );
        if let Err(e) = result {
            assert!(e
                .to_string()
                .contains("Critical pre-flight check 'Nix Semantic Check' failed"));
        }

        test_support::disable_test_mode();
    }

    #[test]
    fn test_config_strict_format_fails_critically() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("bad_format.nix"), "{foo =bar;}").unwrap();
        test_support::enable_test_mode();
        // Mock 1: For 'nixfmt --version' (existence check) to succeed.
        test_support::set_mock_run_result(Ok(()));
        // Mock 2: For 'nixfmt --check bad_format.nix' to indicate an unformatted file.
        test_support::set_mock_run_result(Err(color_eyre::eyre::eyre!(
            crate::util::UtilCommandError::NonZeroStatus {
                command_str: "nixfmt --check bad_format.nix".to_string(),
                status_code: "1".to_string(),
                stdout: String::new(),
                stderr: "File would be reformatted".to_string(),
            }
        )));

        let ng_toml_content = r#"
[pre_flight]
checks = ["Nix Code Format"]
strict_format = true

[pre_flight.format]
tool = "nixfmt"
"#;
        let op_ctx = create_op_ctx_for_preflight(&temp_dir, ng_toml_content, None);
        let mock_strategy = MockPlatformStrategy;
        let dummy_platform_args = ();

        let result = run_shared_pre_flight_checks(&op_ctx, &mock_strategy, &dummy_platform_args);
        assert!(
            result.is_err(),
            "Expected pre-flight checks to fail critically due to strict_format"
        );
        if let Err(e) = result {
            assert!(e
                .to_string()
                .contains("Critical pre-flight check 'Nix Code Format' failed"));
        }

        test_support::disable_test_mode();
    }

    #[test]
    fn test_cli_strict_lint_overrides_config() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("bad_semantic.nix"), "{ લાભ =हानि; }").unwrap();
        test_support::enable_test_mode();

        let ng_toml_content = r#"
[pre_flight]
checks = ["Nix Semantic Check"]
strict_lint = false
"#;
        let op_ctx = create_op_ctx_for_preflight(
            &temp_dir,
            ng_toml_content,
            Some(|args| {
                args.strict_lint = Some(true);
            }),
        );
        let mock_strategy = MockPlatformStrategy;
        let dummy_platform_args = ();

        let result = run_shared_pre_flight_checks(&op_ctx, &mock_strategy, &dummy_platform_args);
        assert!(
            result.is_err(),
            "Expected CLI --strict-lint to cause critical failure"
        );
        if let Err(e) = result {
            assert!(e
                .to_string()
                .contains("Critical pre-flight check 'Nix Semantic Check' failed"));
        }

        test_support::disable_test_mode();
    }

    #[test]
    fn test_cli_strict_format_overrides_config() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("bad_format.nix"), "{bar= foo;}").unwrap();
        test_support::enable_test_mode();
        // Mock 1: For 'nixfmt --version' (existence check) to succeed.
        test_support::set_mock_run_result(Ok(()));
        // Mock 2: For 'nixfmt --check bad_format.nix' to indicate an unformatted file.
        test_support::set_mock_run_result(Err(color_eyre::eyre::eyre!(
            crate::util::UtilCommandError::NonZeroStatus {
                command_str: "nixfmt --check bad_format.nix".to_string(),
                status_code: "1".to_string(),
                stdout: String::new(),
                stderr: "File would be reformatted".to_string(),
            }
        )));

        let ng_toml_content = r#"
[pre_flight]
checks = ["Nix Code Format"]
strict_format = false

[pre_flight.format]
tool = "nixfmt"
"#;
        let op_ctx = create_op_ctx_for_preflight(
            &temp_dir,
            ng_toml_content,
            Some(|args| {
                args.strict_format = Some(true);
            }),
        );
        let mock_strategy = MockPlatformStrategy;
        let dummy_platform_args = ();

        let result = run_shared_pre_flight_checks(&op_ctx, &mock_strategy, &dummy_platform_args);
        assert!(
            result.is_err(),
            "Expected CLI --strict-format to cause critical failure"
        );
        if let Err(e) = result {
            assert!(e
                .to_string()
                .contains("Critical pre-flight check 'Nix Code Format' failed"));
        }

        test_support::disable_test_mode();
    }

    #[test]
    fn test_cli_true_overrides_config_false_for_lint() {
        // RENAMED
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("bad_semantic.nix"), "{ લાભ = हानि; }").unwrap();
        test_support::enable_test_mode();

        let ng_toml_content = r#"
[pre_flight]
checks = ["Nix Semantic Check"]
strict_lint = false
"#;
        let op_ctx = create_op_ctx_for_preflight(
            &temp_dir,
            ng_toml_content,
            Some(|args| {
                args.strict_lint = Some(true); // MODIFIED
            }),
        );
        let mock_strategy = MockPlatformStrategy;
        let dummy_platform_args = ();

        let result = run_shared_pre_flight_checks(&op_ctx, &mock_strategy, &dummy_platform_args);

        assert!(
            result.is_err(),
            "Expected CLI Some(true) to cause critical failure. Result: {:?}",
            result.err()
        ); // FLIPPED ASSERTION
        if let Err(e) = result {
            assert!(e
                .to_string()
                .contains("Critical pre-flight check 'Nix Semantic Check' failed"));
        }

        test_support::disable_test_mode();
    }

    #[test]
    fn test_cli_true_overrides_config_false_for_format() {
        // RENAMED
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("bad_format.nix"), "{foo =bar;}").unwrap();
        test_support::enable_test_mode();
        // Mock 1: For 'nixfmt --version' (existence check) to succeed.
        test_support::set_mock_run_result(Ok(()));
        // Mock 2: For 'nixfmt --check bad_format.nix' to indicate an unformatted file.
        test_support::set_mock_run_result(Err(color_eyre::eyre::eyre!(
            crate::util::UtilCommandError::NonZeroStatus {
                command_str: "nixfmt --check bad_format.nix".to_string(),
                status_code: "1".to_string(),
                stdout: String::new(),
                stderr: "File would be reformatted".to_string(),
            }
        )));

        let ng_toml_content = r#"
[pre_flight]
checks = ["Nix Code Format"]
strict_format = false

[pre_flight.format]
tool = "nixfmt"
"#;
        let op_ctx = create_op_ctx_for_preflight(
            &temp_dir,
            ng_toml_content,
            Some(|args| {
                args.strict_format = Some(true); // MODIFIED
            }),
        );
        let mock_strategy = MockPlatformStrategy;
        let dummy_platform_args = ();

        let result = run_shared_pre_flight_checks(&op_ctx, &mock_strategy, &dummy_platform_args);

        assert!(
            result.is_err(),
            "Expected CLI Some(true) to cause critical failure. Result: {:?}",
            result.err()
        ); // FLIPPED ASSERTION
        if let Err(e) = result {
            assert!(e
                .to_string()
                .contains("Critical pre-flight check 'Nix Code Format' failed"));
        }

        test_support::disable_test_mode();
    }

    #[test]
    fn test_cli_false_overrides_config_true_for_lint() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("bad_semantic.nix"), "{ લાભ = हानि; }").unwrap(); // Semantic error
        test_support::enable_test_mode();

        // Config sets strict_lint to true
        let ng_toml_content = r#"
[pre_flight]
checks = ["Nix Semantic Check"]
strict_lint = true
"#;
        // CLI overrides to false
        let op_ctx = create_op_ctx_for_preflight(
            &temp_dir,
            ng_toml_content,
            Some(|args| {
                args.strict_lint = Some(false);
            }),
        );
        let mock_strategy = MockPlatformStrategy;
        let dummy_platform_args = ();

        let result = run_shared_pre_flight_checks(&op_ctx, &mock_strategy, &dummy_platform_args);

        assert!(
            result.is_ok(),
            "Expected CLI Some(false) to make it non-critical. Result: {:?}",
            result.err()
        );

        test_support::disable_test_mode();
    }

    #[test]
    fn test_cli_false_overrides_config_true_for_format() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("bad_format.nix"), "{foo =bar;}").unwrap(); // Format error
        test_support::enable_test_mode();
        test_support::set_mock_run_result(Ok(())); // For existence check
        test_support::set_mock_run_result(Err(color_eyre::eyre::eyre!(
            crate::util::UtilCommandError::NonZeroStatus {
                command_str: "nixfmt --check bad_format.nix".to_string(),
                status_code: "1".to_string(),
                stdout: String::new(),
                stderr: "File would be reformatted".to_string(),
            }
        )));

        // Config sets strict_format to true
        let ng_toml_content = r#"
[pre_flight]
checks = ["Nix Code Format"]
strict_format = true
[pre_flight.format]
tool = "nixfmt"
"#;
        // CLI overrides to false
        let op_ctx = create_op_ctx_for_preflight(
            &temp_dir,
            ng_toml_content,
            Some(|args| {
                args.strict_format = Some(false);
            }),
        );
        let mock_strategy = MockPlatformStrategy;
        let dummy_platform_args = ();

        let result = run_shared_pre_flight_checks(&op_ctx, &mock_strategy, &dummy_platform_args);

        assert!(
            result.is_ok(),
            "Expected CLI Some(false) to make format check non-critical. Result: {:?}",
            result.err()
        );

        test_support::disable_test_mode();
    }

    #[test]
    fn test_external_linters_both_pass() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("good.nix"), "{ value = 1; }").unwrap();
        test_support::enable_test_mode();

        // Create mock executables
        fs::write(temp_dir.path().join("mock_statix"), "").unwrap();
        fs::write(temp_dir.path().join("mock_deadnix"), "").unwrap();

        // Mock for util::command_exists("statix") -> true (consumes one MOCK_RUN_RESULTS_QUEUE item)
        test_support::set_mock_run_result(Ok(()));
        // Mock for statix check . -> success (consumes one MOCK_PROCESS_OUTPUT_QUEUE item)
        test_support::set_mock_process_output(Ok(std::process::Output {
            status: std::os::unix::process::ExitStatusExt::from_raw(0),
            stdout: b"Statix: no issues found".to_vec(),
            stderr: Vec::new(),
        }));

        // Mock for util::command_exists("deadnix") -> true (consumes one MOCK_RUN_RESULTS_QUEUE item)
        test_support::set_mock_run_result(Ok(()));
        // Mock for deadnix . -> success (consumes one MOCK_PROCESS_OUTPUT_QUEUE item)
        test_support::set_mock_process_output(Ok(std::process::Output {
            status: std::os::unix::process::ExitStatusExt::from_raw(0),
            stdout: b"Deadnix: no issues found".to_vec(),
            stderr: Vec::new(),
        }));

        let ng_toml_content = r#"
[pre_flight]
checks = ["External Linters"]

[pre_flight.external_linters]
enable = ["statix", "deadnix"]
statix_path = "./mock_statix"
deadnix_path = "./mock_deadnix"
"#;
        let op_ctx = create_op_ctx_for_preflight(&temp_dir, ng_toml_content, None);
        let mock_strategy = MockPlatformStrategy;
        let dummy_platform_args = ();

        let result = run_shared_pre_flight_checks(&op_ctx, &mock_strategy, &dummy_platform_args);
        assert!(
            result.is_ok(),
            "External linters check failed when both should pass: {:?}",
            result.err()
        );

        let recorded_commands = test_support::get_recorded_commands();
        let temp_path_str = temp_dir.path().to_str().unwrap();

        // Check for statix --version (or --help)
        assert!(
            recorded_commands
                .iter()
                .any(|cmd| cmd.starts_with("./mock_statix --version") || cmd.starts_with("./mock_statix --help")),
            "statix existence check (version/help) not recorded. Recorded: {:?}",
            recorded_commands
        );
        // Check for statix check <temp_path>
        assert!(
            recorded_commands
                .iter()
                .any(|cmd| cmd == &format!("./mock_statix check {}", temp_path_str)),
            "statix check <temp_path> not recorded. Expected: 'statix check {}'. Recorded: {:?}",
            temp_path_str,
            recorded_commands
        );

        // Check for deadnix --version (or --help)
        assert!(
            recorded_commands.iter().any(
                |cmd| cmd.starts_with("./mock_deadnix --version") || cmd.starts_with("./mock_deadnix --help")
            ),
            "deadnix existence check (version/help) not recorded. Recorded: {:?}",
            recorded_commands
        );
        // Check for deadnix --check
        assert!(
            recorded_commands.iter().any(|cmd| cmd == "./mock_deadnix --check"),
            "deadnix --check not recorded. Recorded: {:?}",
            recorded_commands
        );

        test_support::disable_test_mode();
    }

    #[test]
    fn test_external_linters_statix_fails_not_strict() {
        let temp_dir = TempDir::new().unwrap();
        let bad_nix_path = temp_dir.path().join("bad.nix");
        fs::write(&bad_nix_path, "{ foo = bar; }").unwrap();
        test_support::enable_test_mode();

        // Create mock executables
        fs::write(temp_dir.path().join("mock_statix"), "").unwrap();
        fs::write(temp_dir.path().join("mock_deadnix"), "").unwrap();

        test_support::set_mock_run_result(Ok(())); // statix exists
        let statix_json_output = format!(
            r#"[
            {{
                "message": "Statix found an error here.",
                "file": "{}",
                "severity": "error",
                "position": {{ "start_line": 1, "start_col": 2, "end_line": 1, "end_col": 5 }}
            }}
        ]"#,
            bad_nix_path.to_str().unwrap().replace('\\', "/")
        ); // JSON escape path
        test_support::set_mock_process_output(Ok(std::process::Output {
            status: std::os::unix::process::ExitStatusExt::from_raw(1), // Statix exits non-zero on error
            stdout: statix_json_output.into_bytes(),
            stderr: Vec::new(),
        }));

        test_support::set_mock_run_result(Ok(())); // deadnix exists
        test_support::set_mock_process_output(Ok(std::process::Output {
            status: std::os::unix::process::ExitStatusExt::from_raw(0),
            stdout: b"Deadnix: no issues".to_vec(),
            stderr: Vec::new(),
        }));

        let ng_toml_content = r#"
[pre_flight]
checks = ["External Linters"]
strict_lint = false
[pre_flight.external_linters]
enable = ["statix", "deadnix"]
statix_path = "./mock_statix"
deadnix_path = "./mock_deadnix"
"#;
        let op_ctx = create_op_ctx_for_preflight(&temp_dir, ng_toml_content, None);
        let result = run_shared_pre_flight_checks(&op_ctx, &MockPlatformStrategy, &());

        assert!(
            result.is_ok(),
            "Expected PassedWithWarnings due to statix failure (not strict): {:?}",
            result.err()
        );
        // TODO: Capture stderr and check for formatted diagnostic output

        test_support::disable_test_mode();
    }

    #[test]
    fn test_external_linters_statix_fails_strict() {
        let temp_dir = TempDir::new().unwrap();
        let bad_nix_path = temp_dir.path().join("bad.nix");
        fs::write(&bad_nix_path, "{ foo = bar; }").unwrap();
        test_support::enable_test_mode();

        // Create mock executables
        fs::write(temp_dir.path().join("mock_statix"), "").unwrap();
        fs::write(temp_dir.path().join("mock_deadnix"), "").unwrap();

        test_support::set_mock_run_result(Ok(())); // statix exists
        let statix_json_output = format!(
            r#"[
            {{
                "message": "Critical statix error.",
                "file": "{}",
                "severity": "error",
                "position": {{ "start_line": 3, "start_col": 1, "end_line": 3, "end_col": 10 }}
            }}
        ]"#,
            bad_nix_path.to_str().unwrap().replace('\\', "/")
        );
        test_support::set_mock_process_output(Ok(std::process::Output {
            status: std::os::unix::process::ExitStatusExt::from_raw(1), // Statix exits non-zero
            stdout: statix_json_output.into_bytes(),
            stderr: Vec::new(),
        }));

        test_support::set_mock_run_result(Ok(())); // deadnix exists
        test_support::set_mock_process_output(Ok(std::process::Output {
            status: std::os::unix::process::ExitStatusExt::from_raw(0),
            stdout: b"Deadnix: no issues".to_vec(),
            stderr: Vec::new(),
        }));

        let ng_toml_content = r#"
[pre_flight]
checks = ["External Linters"]
strict_lint = true
[pre_flight.external_linters]
enable = ["statix", "deadnix"]
statix_path = "./mock_statix"
deadnix_path = "./mock_deadnix"
"#;
        let op_ctx = create_op_ctx_for_preflight(&temp_dir, ng_toml_content, None);
        let result = run_shared_pre_flight_checks(&op_ctx, &MockPlatformStrategy, &());

        assert!(
            result.is_err(),
            "Expected FailedCritical due to statix failure (strict mode)"
        );
        if let Err(e) = result {
            assert!(e
                .to_string()
                .contains("Critical pre-flight check 'External Linters' failed"));
        }
        // TODO: Capture stderr and check for formatted diagnostic output
        test_support::disable_test_mode();
    }

    #[test]
    fn test_external_linters_statix_not_found() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("good.nix"), "{ value = 1; }").unwrap();
        test_support::enable_test_mode();

        // mock_statix executable does NOT exist in temp_dir for this test.
        // mock_deadnix executable DOES exist.
        fs::write(temp_dir.path().join("mock_deadnix"), "").unwrap();

        // Mock statix command_exists (for ./mock_statix) returns false (Err for run())
        // If command_exists tries "./mock_statix --version" and it fails, it might try "./mock_statix --help"
        // So, provide two Err results.
        test_support::set_mock_run_result(Err(color_eyre::eyre::eyre!("statix not found mock (version attempt)")));
        test_support::set_mock_run_result(Err(color_eyre::eyre::eyre!("statix not found mock (help attempt)")));

        // Mock deadnix (./mock_deadnix) exists
        test_support::set_mock_run_result(Ok(()));
        // Mock deadnix passes
        test_support::set_mock_process_output(Ok(std::process::Output {
            status: std::os::unix::process::ExitStatusExt::from_raw(0),
            stdout: b"Deadnix: no issues".to_vec(),
            stderr: Vec::new(),
        }));

        let ng_toml_content = r#"
[pre_flight]
checks = ["External Linters"]
strict_lint = false // Doesn't matter for this test
[pre_flight.external_linters]
enable = ["statix", "deadnix"]
statix_path = "./mock_statix" // This path will be checked for existence
deadnix_path = "./mock_deadnix"
"#;

        let op_ctx = create_op_ctx_for_preflight(&temp_dir, ng_toml_content, None);
        let result = run_shared_pre_flight_checks(&op_ctx, &MockPlatformStrategy, &());

        assert!(
            result.is_ok(),
            "Expected check to pass (with warnings) when statix not found: {:?}",
            result.err()
        );

        let recorded_commands = test_support::get_recorded_commands();
        assert!(
            recorded_commands
                .iter()
                .any(|cmd| cmd.starts_with("./mock_statix --version") || cmd.starts_with("./mock_statix --help")),
            "mock_statix existence check not recorded. Recorded: {:?}", recorded_commands
        );
        assert!(
            !recorded_commands
                .iter()
                .any(|cmd| cmd.starts_with("./mock_statix check")),
            "mock_statix check command should not have been recorded. Recorded: {:?}", recorded_commands
        );
        assert!(
            recorded_commands
                .iter()
                .any(|cmd| cmd.starts_with("./mock_deadnix --version") || cmd.starts_with("./mock_deadnix --help")),
            "mock_deadnix existence check not recorded. Recorded: {:?}", recorded_commands
        );
        assert!(
            recorded_commands
                .iter()
                .any(|cmd| cmd == "./mock_deadnix --check"),
            "mock_deadnix --check not recorded. Recorded: {:?}", recorded_commands
        );

        test_support::disable_test_mode();
    }

    #[test]
    fn test_external_linters_none_enabled_in_config() {
        let temp_dir = TempDir::new().unwrap();
        test_support::enable_test_mode();

        // No linters specified in 'enable' list
        let ng_toml_content = r#"
[pre_flight]
checks = ["External Linters"]
[pre_flight.external_linters]
# enable = [] # This or missing 'enable' key
"#;
        let op_ctx = create_op_ctx_for_preflight(&temp_dir, ng_toml_content, None);
        let result = run_shared_pre_flight_checks(&op_ctx, &MockPlatformStrategy, &());

        assert!(
            result.is_ok(),
            "Expected check to pass when no external linters are enabled: {:?}",
            result.err()
        );
        test_support::disable_test_mode();
    }

    #[test]
    fn test_external_linters_empty_enable_list() {
        let temp_dir = TempDir::new().unwrap();
        test_support::enable_test_mode();

        let ng_toml_content = r#"
[pre_flight]
checks = ["External Linters"]
[pre_flight.external_linters]
enable = [] # Explicitly empty list
"#;
        let op_ctx = create_op_ctx_for_preflight(&temp_dir, ng_toml_content, None);
        let result = run_shared_pre_flight_checks(&op_ctx, &MockPlatformStrategy, &());

        assert!(
            result.is_ok(),
            "Expected check to pass with empty 'enable' list: {:?}",
            result.err()
        );
        test_support::disable_test_mode();
    }

    fn test_external_linters_deadnix_fails_not_strict() {
        let temp_dir = TempDir::new().unwrap();
        let bad_nix_path = temp_dir.path().join("bad.nix");
        fs::write(&bad_nix_path, "{ foo = 1; bar = 2; }").unwrap();
        test_support::enable_test_mode();

        // Mock deadnix exists
        test_support::set_mock_run_result(Ok(()));
        // Mock deadnix check fails (non-strict)
        let deadnix_stdout = format!("{}:1:7: Unused binding: 'foo'\n{}:2:7: Unused binding: 'bar'",
            bad_nix_path.to_str().unwrap().replace('\\', "/"),
            bad_nix_path.to_str().unwrap().replace('\\', "/"));
        test_support::set_mock_process_output(Ok(std::process::Output {
            status: std::os::unix::process::ExitStatusExt::from_raw(1),
            stdout: deadnix_stdout.into_bytes(),
            stderr: Vec::new(),
        }));

        let ng_toml_content = r#"
[pre_flight]
checks = ["External Linters"]
strict_lint = false
[pre_flight.external_linters]
enable = ["deadnix"]
"#;
        let op_ctx = create_op_ctx_for_preflight(&temp_dir, ng_toml_content, None);
        let result = run_shared_pre_flight_checks(&op_ctx, &MockPlatformStrategy, &());

        assert!(result.is_ok(), "Expected PassedWithWarnings in non-strict mode");

        test_support::disable_test_mode();
    }

    #[test]
    fn test_external_linters_deadnix_fails_strict() {
        let temp_dir = TempDir::new().unwrap();
        let bad_nix_path = temp_dir.path().join("bad.nix");
        fs::write(&bad_nix_path, "{ foo = 1; bar = 2; }").unwrap();
        test_support::enable_test_mode();

        // Mock deadnix exists
        test_support::set_mock_run_result(Ok(()));
        // Mock deadnix check fails (strict)
        let deadnix_stdout = format!("{}:1:7: Unused binding: 'foo'\n{}:2:7: Unused binding: 'bar'",
            bad_nix_path.to_str().unwrap().replace('\\', "/"),
            bad_nix_path.to_str().unwrap().replace('\\', "/"));
        test_support::set_mock_process_output(Ok(std::process::Output {
            status: std::os::unix::process::ExitStatusExt::from_raw(1),
            stdout: deadnix_stdout.into_bytes(),
            stderr: Vec::new(),
        }));

        let ng_toml_content = r#"
[pre_flight]
checks = ["External Linters"]
strict_lint = true
[pre_flight.external_linters]
enable = ["deadnix"]
"#;
        let op_ctx = create_op_ctx_for_preflight(&temp_dir, ng_toml_content, None);
        let result = run_shared_pre_flight_checks(&op_ctx, &MockPlatformStrategy, &());

        assert!(result.is_err(), "Expected FailedCritical in strict mode");
        if let Err(e) = result {
            assert!(e.to_string().contains("Critical pre-flight check 'External Linters' failed"));
        }
test_support::disable_test_mode();
}
}
    
