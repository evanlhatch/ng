#[cfg(test)]
mod tests {
    use crate::interface::{Main, OsSubcommand, NHCommand};
    
    use crate::commands::test_support;
    use clap::Parser;
    // Add other necessary imports here, e.g., for OperationContext, NixosPlatformStrategy if needed for setup.

    #[test]
    fn test_os_switch_clean_workflow() {
        test_support::enable_test_mode();
        assert!(test_support::is_test_mode_enabled(), "Test mode should be active right after enabling.");

        // Manually record a command to see if it persists and if others are added.
        test_support::record_command("MANUAL PRE-RUN HOOK COMMAND");

        test_support::set_mock_run_result(Ok(())); // For build and any other run() calls

        let args_vec = vec![
            "nh",
            "os",
            "switch",
            "--clean",
            "--no-ask",
            "--dry",
            ".#testHost",
            "--"
        ];

        let cli = Main::try_parse_from(args_vec.clone()).unwrap_or_else(|e| panic!("Failed to parse OS switch --clean args: {:#?}\nArgs: {:?}", e, args_vec));
        
        if let NHCommand::Os(os_args) = &cli.command {
            if let OsSubcommand::Switch(switch_args) = &os_args.subcommand {
                assert!(switch_args.common.common.dry, "Expected --dry flag to be parsed as true");
                assert!(!switch_args.common.common.ask, "Expected --no-ask flag to result in ask=false");
            } else {
                panic!("Expected OsSubcommand::Switch for this test case.");
            }
        } else {
            panic!("Expected NHCommand::Os subcommand after parsing arguments.");
        }

        let run_result = cli.command.run(cli.verbose);
        if let Err(e) = &run_result { 
            eprintln!("nh os switch --clean command failed with error: {:?}", e);
        }
        assert!(run_result.is_ok(), "nh os switch --clean command failed");

        let recorded_commands = test_support::get_recorded_commands();
        eprintln!("Recorded commands: {:#?}", recorded_commands);
        
        assert!(recorded_commands.contains(&"MANUAL PRE-RUN HOOK COMMAND".to_string()), "Manual pre-run command was not found! Commands: {:?}", recorded_commands);

        assert!(
            recorded_commands.len() > 1, 
            "Expected at least two commands (manual + build) to be recorded, got {}. Commands: {:?}", recorded_commands.len(), recorded_commands
        );

        let build_cmd_found = recorded_commands.iter().any(|cmd| 
            cmd.contains("nix build") && 
            (cmd.contains(".#testHost") || cmd.contains("testHost")) &&
            cmd.contains("--out-link")
        );
        assert!(build_cmd_found, "Expected nix build command not found in recorded commands: {:?}", recorded_commands);

        let gc_cmd_should_not_exist = recorded_commands.iter().any(|cmd| cmd.contains("sudo nix-store --gc"));
        assert!(!gc_cmd_should_not_exist, "GC command should NOT have been recorded in --dry mode, but was: {:?}", recorded_commands);

        test_support::disable_test_mode();
    }

    #[test]
    fn test_os_build_dry_run() {
        test_support::enable_test_mode();
        assert!(test_support::is_test_mode_enabled(), "Test mode should be active.");
        test_support::set_mock_run_result(Ok(()));

        let args_vec = vec![
            "nh",
            "os",
            "build",
            "--dry",
            ".#someHost",
        ];

        let cli = Main::try_parse_from(args_vec.clone()).unwrap_or_else(|e| {
            panic!("Failed to parse OS build --dry args: {:#?}\nArgs: {:?}", e, args_vec)
        });

        if let NHCommand::Os(os_args) = &cli.command {
            if let OsSubcommand::Build(build_cmd_args) = &os_args.subcommand {
                assert!(build_cmd_args.common.common.dry, "Expected --dry flag to be true in BuildArgs.common.common");
            } else {
                panic!("Expected OsSubcommand::Build for this test case.");
            }
        } else {
            panic!("Expected NHCommand::Os subcommand after parsing arguments.");
        }

        let run_result = cli.command.run(cli.verbose);
        if let Err(e) = &run_result {
            eprintln!("nh os build --dry command failed with error: {:?}", e);
        }
        assert!(run_result.is_ok(), "nh os build --dry command failed");

        let recorded_commands = test_support::get_recorded_commands();
        eprintln!("Recorded commands for os build --dry: {:#?}", recorded_commands);

        assert!(
            !recorded_commands.is_empty(),
            "Expected at least one command (the build command) to be recorded, got none."
        );

        assert_eq!(recorded_commands.len(), 1, "Expected exactly one command (build command), got {}. Commands: {:?}", recorded_commands.len(), recorded_commands);

        let build_cmd_found = recorded_commands.first().map_or(false, |cmd| 
            cmd.contains("nix build") && 
            (cmd.contains(".#someHost") || cmd.contains("someHost")) &&
            cmd.contains("--out-link")
        );
        assert!(build_cmd_found, "Expected nix build command with --out-link not found as the only command. Got: {:?}", recorded_commands);
        
        test_support::disable_test_mode();
    }

    #[test]
    fn test_os_boot_dry_run() {
        test_support::enable_test_mode();
        test_support::set_mock_run_result(Ok(()));

        let test_host_installable = ".#someHostForBoot";
        let args_vec = vec![
            "nh",
            "os",
            "boot",
            "--dry",
            test_host_installable,
        ];

        let cli = match Main::try_parse_from(args_vec.clone()) {
            Ok(c) => c,
            Err(e) => panic!("Failed to parse args: {}\nError:\n{}", args_vec.join(" "), e.render()),
        };

        if let NHCommand::Os(os_args) = &cli.command {
            if let OsSubcommand::Boot(boot_args) = &os_args.subcommand {
                assert!(
                    boot_args.common.common.dry,
                    "Expected --dry flag to be parsed as true for os boot"
                );
            } else {
                panic!("Expected OsSubcommand::Boot for this test case.");
            }
        } else {
            panic!("Expected NHCommand::Os subcommand after parsing arguments.");
        }

        let run_result = cli.command.run(cli.verbose);
        assert!(
            run_result.is_ok(),
            "cli.command.run() failed: {:?}",
            run_result.err()
        );

        let recorded_commands = test_support::get_recorded_commands();
        eprintln!("Recorded commands for os boot --dry: {:#?}", recorded_commands);

        assert!(
            !recorded_commands.is_empty(),
            "Expected at least one command (the build command) to be recorded, got none."
        );

        let build_cmd_expected_part = format!("nix build {}", test_host_installable);
        let build_cmd_exists = recorded_commands.iter().any(|cmd| {
            cmd.contains(&build_cmd_expected_part) && cmd.contains("--out-link")
        });
        assert!(
            build_cmd_exists,
            "Expected nix build command for '{}' not found in recorded commands: {:#?}",
            test_host_installable,
            recorded_commands
        );
        
        assert_eq!(
            recorded_commands.len(),
            1,
            "Expected only the build command to be recorded in dry mode for os boot. Got: {:#?}",
            recorded_commands
        );

        test_support::disable_test_mode();
    }
}
