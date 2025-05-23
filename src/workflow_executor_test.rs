#[cfg(test)]
mod tests {
    use crate::workflow_executor::execute_rebuild_workflow;
    use crate::workflow_strategy::{PlatformRebuildStrategy, ActivationMode};
    use crate::context::OperationContext;
    use crate::workflow_types::CommonRebuildArgs;
    use crate::installable::Installable;
    use crate::Result;
    use std::path::{Path, PathBuf};
    use std::ffi::OsString;
    use std::sync::Arc; // Added for Arc<NgConfig>
    use crate::config::NgConfig; // Added for NgConfig::default()

    // Mock platform strategy for testing
    struct MockPlatformStrategy;

    // Mock platform args for testing
    #[derive(Debug)]
    struct MockPlatformArgs {
        _hostname: Option<String>,
        _bypass_root_check: bool,
    }

    impl PlatformRebuildStrategy for MockPlatformStrategy {
        type PlatformArgs = MockPlatformArgs;

        fn name(&self) -> &str { "MockPlatform" }

        fn pre_rebuild_hook(&self, _op_ctx: &OperationContext, _platform_args: &Self::PlatformArgs) -> Result<()> {
            println!("MockPlatform: pre_rebuild_hook called");
            Ok(())
        }

        fn get_toplevel_installable(&self, op_ctx: &OperationContext, _platform_args: &Self::PlatformArgs) -> Result<Installable> {
            println!("MockPlatform: get_toplevel_installable called");
            // Just return the installable from common_args
            Ok(op_ctx.common_args.installable.clone())
        }

        fn get_current_profile_path(&self, _op_ctx: &OperationContext, _platform_args: &Self::PlatformArgs) -> Option<PathBuf> {
            println!("MockPlatform: get_current_profile_path called");
            Some(PathBuf::from("/tmp/mock-current-profile"))
        }

        fn activate_configuration(
            &self,
            _op_ctx: &OperationContext,
            _platform_args: &Self::PlatformArgs,
            built_profile_path: &Path,
            activation_mode: &ActivationMode,
        ) -> Result<()> {
            println!("MockPlatform: activate_configuration called with path: {} and mode: {:?}", 
                built_profile_path.display(), activation_mode);
            Ok(())
        }

        fn post_rebuild_hook(&self, _op_ctx: &OperationContext, _platform_args: &Self::PlatformArgs) -> Result<()> {
            println!("MockPlatform: post_rebuild_hook called");
            Ok(())
        }
    }

    #[test]
    fn test_execute_rebuild_workflow() {
        // Create mock update args
        let update_args = crate::interface::UpdateArgs {
            update: false,
            update_input: None,
        };

        // Create mock installable
        let installable = Installable::Flake {
            reference: ".".to_string(),
            attribute: vec!["nixosConfigurations".to_string(), "test-host".to_string()],
        };

        // Create common rebuild args
        let common_args = CommonRebuildArgs {
            installable,
            no_preflight: true, // Skip preflight for test
            strict_lint: None,
            strict_format: None, 
            medium_checks: false,
            full_checks: false,
            dry_run: true, // Use dry run for test
            ask_confirmation: false,
            no_nom: true,
            out_link: None,
            clean_after: false,
            extra_build_args: Vec::<OsString>::new(),
        };

        // Create operation context
        let dry_run_for_test = true; // Or common_args.dry_run if common_args is accessible here for this decision
        let nix_interface = crate::nix_interface::NixInterface::new(0, dry_run_for_test);
        let op_ctx = OperationContext::new(
            common_args, // Pass by value
            &update_args,
            0, // verbose_count
            nix_interface,
            Arc::new(NgConfig::default()), // Added config
            None, // Added project_root
        );

        // Create mock platform args
        let platform_args = MockPlatformArgs {
            _hostname: Some("test-host".to_string()),
            _bypass_root_check: true,
        };

        // Create mock platform strategy
        let strategy = MockPlatformStrategy;

        // Execute the workflow
        let result = execute_rebuild_workflow(
            &strategy,
            &op_ctx,
            &platform_args,
            ActivationMode::Switch,
        );

        // Check that the workflow executed successfully
        assert!(result.is_ok(), "Workflow execution failed: {:?}", result.err());
    }
}