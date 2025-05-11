#[cfg(test)]
mod tests {
    use ng::util::{is_hidden_path, command_exists};
    use ng::lint::{LintSummary, CheckStatus, LintOutcome}; // Keep these specific to lint
    use std::path::Path;

    #[test]
    fn test_is_hidden() {
        assert!(is_hidden_path(Path::new(".hidden_file")));
        assert!(!is_hidden_path(Path::new(".hidden_dir/file.txt")));
        assert!(!is_hidden_path(Path::new("visible_file.txt")));
        assert!(!is_hidden_path(Path::new("visible_dir/file.txt")));
    }

    #[test]
    fn test_command_exists() {
        // Test with a command that should exist on most systems
        assert!(command_exists("ls"));
        
        // Test with a command that shouldn't exist
        assert!(!command_exists("this_command_should_not_exist_12345"));
    }

    #[test]
    fn test_lint_summary_default() {
        let summary = LintSummary::default();
        assert!(summary.outcome.is_none());
        assert_eq!(summary.details.len(), 0);
        assert_eq!(summary.message, "");
        assert_eq!(summary.files_formatted, 0);
        assert_eq!(summary.fixes_applied, 0);
    }

    #[test]
    fn test_check_status_variants() {
        // Just verify we can create all variants
        let statuses = vec![
            CheckStatus::Passed,
            CheckStatus::Failed,
            CheckStatus::Skipped,
            CheckStatus::Warnings,
        ];
        
        assert_eq!(statuses.len(), 4);
    }

    #[test]
    fn test_lint_outcome_variants() {
        // Just verify we can create all variants
        let outcomes = vec![
            LintOutcome::Passed,
            LintOutcome::Warnings,
            LintOutcome::CriticalFailure("Test error".to_string()),
        ];
        
        assert_eq!(outcomes.len(), 3);
    }
}