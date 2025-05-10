#[cfg(test)]
mod tests {
    use crate::nixos_strategy::NixosPlatformStrategy;
    use crate::workflow_strategy::{PlatformRebuildStrategy};
    use crate::context::OperationContext;
    use crate::workflow_types::CommonRebuildArgs;
    use crate::installable::Installable;
    use crate::interface::{OsRebuildArgs, CommonRebuildArgs as InterfaceCommonRebuildArgs, CommonArgs, UpdateArgs};
    use std::path::PathBuf;
    use std::ffi::OsString;

    // Helper function to create a test OperationContext
    fn create_test_context<'a>(
        installable: Installable,
        update_args: &'a UpdateArgs,
        verbose_count: u8,
    ) -> OperationContext<'a> {
        let common_args = CommonRebuildArgs {
            installable,
            no_preflight: true,
            strict_lint: false,
            medium_checks: false,
            full_checks: false,
            dry_run: true,
            ask_confirmation: false,
            no_nom: true,
            out_link: None,
            clean_after: false,
            extra_build_args: Vec::<OsString>::new(),
        };
        let nix_interface = crate::nix_interface::NixInterface::new(verbose_count);
        OperationContext::new(
            common_args, // Pass by value
            update_args,
            verbose_count,
            nix_interface,
        )
    }

    // Helper function to create test OsRebuildArgs
    fn create_test_os_rebuild_args(hostname: Option<String>, bypass_root_check: bool) -> OsRebuildArgs {
        let common_args = CommonArgs {
            no_preflight: true,
            strict_lint: false,
            medium: false,
            full: false,
            dry: true,
            ask: false,
            no_nom: true,
            out_link: None,
            clean: false,
        };

        let interface_common_rebuild_args = InterfaceCommonRebuildArgs {
            common: common_args,
            installable: Installable::Flake {
                reference: ".".to_string(),
                attribute: Vec::new(),
            },
        };

        OsRebuildArgs {
            common: interface_common_rebuild_args,
            update_args: UpdateArgs {
                update: false,
                update_input: None,
            },
            hostname,
            specialisation: None,
            no_specialisation: false,
            extra_args: Vec::<String>::new(),
            bypass_root_check,
        }
    }

    #[test]
    fn test_get_toplevel_installable_with_empty_attribute() {
        // Create a strategy
        let strategy = NixosPlatformStrategy;

        // Create an installable with empty attribute
        let installable = Installable::Flake {
            reference: ".".to_string(),
            attribute: Vec::new(),
        };

        // Create update args
        let update_args = UpdateArgs {
            update: false,
            update_input: None,
        };

        // Create operation context
        let op_ctx = create_test_context(installable, &update_args, 0);

        // Create platform args with hostname
        let platform_args = create_test_os_rebuild_args(Some("test-host".to_string()), true);

        // Get toplevel installable
        let result = strategy.get_toplevel_installable(&op_ctx, &platform_args);

        // Check that it succeeded
        assert!(result.is_ok(), "get_toplevel_installable failed: {:?}", result.err());

        // Check that the attribute path was correctly populated
        if let Ok(Installable::Flake { reference: _, attribute }) = result {
            assert_eq!(attribute, vec![
                "nixosConfigurations".to_string(),
                "test-host".to_string(),
                "config".to_string(),
                "system".to_string(),
                "build".to_string(),
                "toplevel".to_string(),
            ]);
        } else {
            panic!("Expected Flake installable, got: {:?}", result);
        }
    }

    #[test]
    fn test_get_toplevel_installable_with_existing_attribute() {
        // Create a strategy
        let strategy = NixosPlatformStrategy;

        // Create an installable with existing attribute
        let installable = Installable::Flake {
            reference: ".".to_string(),
            attribute: vec!["nixosConfigurations".to_string(), "custom-host".to_string()],
        };

        // Create update args
        let update_args = UpdateArgs {
            update: false,
            update_input: None,
        };

        // Create operation context
        let op_ctx = create_test_context(installable, &update_args, 0);

        // Create platform args with hostname (should be ignored since attribute is already set)
        let platform_args = create_test_os_rebuild_args(Some("test-host".to_string()), true);

        // Get toplevel installable
        let result = strategy.get_toplevel_installable(&op_ctx, &platform_args);

        // Check that it succeeded
        assert!(result.is_ok(), "get_toplevel_installable failed: {:?}", result.err());

        // Check that the attribute path was correctly populated
        if let Ok(Installable::Flake { reference: _, attribute }) = result {
            assert_eq!(attribute, vec![
                "nixosConfigurations".to_string(),
                "custom-host".to_string(),
                "config".to_string(),
                "system".to_string(),
                "build".to_string(),
                "toplevel".to_string(),
            ]);
        } else {
            panic!("Expected Flake installable, got: {:?}", result);
        }
    }

    #[test]
    fn test_get_current_profile_path() {
        // Create a strategy
        let strategy = NixosPlatformStrategy;

        // Create an installable
        let installable = Installable::Flake {
            reference: ".".to_string(),
            attribute: Vec::new(),
        };

        // Create update args
        let update_args = UpdateArgs {
            update: false,
            update_input: None,
        };

        // Create operation context
        let op_ctx = create_test_context(installable, &update_args, 0);

        // Create platform args
        let platform_args = create_test_os_rebuild_args(Some("test-host".to_string()), true);

        // Get current profile path
        let result = strategy.get_current_profile_path(&op_ctx, &platform_args);

        // Check that it returned a path
        assert!(result.is_some(), "get_current_profile_path returned None");

        // Check that the path is correct
        assert_eq!(result.unwrap(), PathBuf::from("/run/current-system"));
    }
}