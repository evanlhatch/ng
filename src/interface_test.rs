#[cfg(test)]
mod tests {
    use crate::interface::{Main, NHCommand, OsArgs, OsSubcommand};
    use crate::installable::Installable; // For asserting Installable fields
    use clap::Parser;
    use std::path::PathBuf;

    #[test]
    fn test_main_verbosity() {
        let args = vec!["nh", "-vv", "search", "query"];
        let cli = Main::try_parse_from(args).unwrap();
        assert_eq!(cli.verbose, 2);
    }

    #[test]
    fn test_os_switch_basic() {
        let args = vec!["nh", "-v", "os", "switch", ".#myHost", "--hostname", "myHost", "--no-nom"]; // Removed --ask
        let cli = Main::try_parse_from(args).unwrap_or_else(|e| panic!("Failed to parse basic os switch args: {:?}", e));
        assert_eq!(cli.verbose, 1);

        if let NHCommand::Os(os_args) = cli.command { // Assuming NHCommand is alias for CliCommand
            if let OsSubcommand::Switch(rebuild_args) = os_args.subcommand {
                assert_eq!(rebuild_args.hostname.as_deref(), Some("myHost"));
                assert!(rebuild_args.common.common.no_nom);
                assert!(rebuild_args.common.common.ask); // ask is true by default
                assert_eq!(rebuild_args.update_args.update, false); // Default
                if let Installable::Flake { reference, attribute } = rebuild_args.common.installable {
                    assert_eq!(reference, ".");
                    assert_eq!(attribute, vec!["myHost"]);
                } else {
                    panic!("Expected Installable::Flake for os switch");
                }
            } else {
                panic!("Expected OsSubcommand::Switch");
            }
        } else {
            panic!("Expected CliCommand::Os");
        }
    }

    #[test]
    fn test_os_switch_with_common_args_and_out_link() {
        let args = vec![
            "nh", "os", "switch", "someFlake#configName",
            "--hostname", "overrideHost",
            "--dry", "--full", "--strict-lint",
            "--out-link", "/tmp/result-os",
            "--update",
            "--", // Explicitly mark end of options, start of positional args for extra_args
            "--extra-arg1", "val1"
        ];
        let cli = Main::try_parse_from(args).unwrap();

        if let NHCommand::Os(OsArgs { subcommand: OsSubcommand::Switch(rebuild_args) }) = cli.command {
            assert_eq!(rebuild_args.hostname.as_deref(), Some("overrideHost"));
            
            let common = &rebuild_args.common.common;
            assert!(common.dry);
            assert!(common.full);
            assert_eq!(common.strict_lint, Some(true)); // Changed
            assert_eq!(common.out_link, Some(PathBuf::from("/tmp/result-os")));
            
            assert!(rebuild_args.update_args.update);

            if let Installable::Flake { reference, attribute } = &rebuild_args.common.installable {
                assert_eq!(reference, "someFlake");
                assert_eq!(attribute, &vec!["configName"]);
            } else {
                panic!("Expected Installable::Flake");
            }
            assert_eq!(rebuild_args.extra_args, vec!["--extra-arg1", "val1"]);
        } else {
            panic!("Command parsing failed for os switch with common args");
        }
    }

    #[test]
    fn test_search_basic() {
        let args = vec!["nh", "search", "-l", "15", "myQuery", "anotherPart", "--channel", "nixos-23.11"];
        let cli = Main::try_parse_from(args).unwrap();

        if let NHCommand::Search(search_args) = cli.command {
            assert_eq!(search_args.limit, 15);
            assert_eq!(search_args.query, vec!["myQuery", "anotherPart"]);
            assert_eq!(search_args.channel, "nixos-23.11");
            assert_eq!(search_args.platforms, false); // Default
        } else {
            panic!("Expected NHCommand::Search");
        }
    }
    
    #[test]
    fn test_search_json_platforms() {
        let args = vec!["nh", "search", "--json", "--platforms", "testQuery"];
        let cli = Main::try_parse_from(args).unwrap();

        if let NHCommand::Search(search_args) = cli.command {
            assert!(search_args.json);
            assert!(search_args.platforms);
            assert_eq!(search_args.query, vec!["testQuery"]);
        } else {
            panic!("Expected NHCommand::Search for json/platforms");
        }
    }

    #[test]
    fn test_invalid_subcommand() {
        let args = vec!["nh", "os", "nonexistent_subcommand"];
        let result = Main::try_parse_from(args);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_required_arg_for_search() {
        let args = vec!["nh", "search"]; // Missing query
        let cli_result = Main::try_parse_from(args);
        assert!(cli_result.is_ok(), "Parsing 'nh search' without query should succeed");
        let cli = cli_result.unwrap();
        if let NHCommand::Search(search_args) = cli.command {
            assert!(search_args.query.is_empty(), "Expected query vector to be empty");
        } else {
            panic!("Expected NHCommand::Search after parsing 'nh search'");
        }
        // Note: The actual error for a missing query would now need to be handled
        // in the SearchArgs::run() method.
    }
    
    #[test]
    fn test_home_switch_basic() {
        let args = vec!["nh", "home", "switch", ".#user@host", "--configuration", "user@host", "--backup-extension", "bak"];
        let cli = Main::try_parse_from(args).unwrap();

        if let NHCommand::Home(home_args) = cli.command {
            if let crate::interface::HomeSubcommand::Switch(rebuild_args) = home_args.subcommand {
                assert_eq!(rebuild_args.configuration.as_deref(), Some("user@host"));
                assert_eq!(rebuild_args.backup_extension.as_deref(), Some("bak"));
                if let Installable::Flake { reference, attribute } = rebuild_args.common.installable {
                    assert_eq!(reference, ".");
                    assert_eq!(attribute, vec!["user@host"]);
                } else {
                    panic!("Expected Installable::Flake for home switch");
                }
            } else {
                panic!("Expected HomeSubcommand::Switch");
            }
        } else {
            panic!("Expected NHCommand::Home");
        }
    }

    // TODO: Add tests for DarwinArgs, CleanProxy/CleanArgs, CompletionsArgs
    // TODO: Add tests for more OsSubcommands like Repl, Info
    // TODO: Add tests for more HomeSubcommands like Build, Repl
    // TODO: Test interactions between global flags (like --dry) and subcommand flags
}