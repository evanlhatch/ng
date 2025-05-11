#[cfg(test)]
mod tests {
    use nh::check_git::{is_git_repo, git_command, run_git_check_warning_only};
    use std::fs;
    use std::path::Path;
    use std::process::Command;
    use tempfile::TempDir;

    // Helper function to create a temporary Git repository
    fn setup_git_repo() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        // Initialize Git repository
        let status = Command::new("git")
            .args(["init"])
            .current_dir(temp_path)
            .status()
            .expect("Failed to initialize Git repository");
        
        assert!(status.success());
        
        // Configure Git user
        let status = Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(temp_path)
            .status()
            .expect("Failed to configure Git user name");
        
        assert!(status.success());
        
        let status = Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(temp_path)
            .status()
            .expect("Failed to configure Git user email");
        
        assert!(status.success());
        
        temp_dir
    }

    #[test]
    fn test_is_git_repo() {
        // Test with a temporary Git repository
        let temp_dir = setup_git_repo();
        
        // Save the current directory path as a string to avoid potential issues
        let current_dir_str = std::env::current_dir()
            .ok()
            .and_then(|p| p.to_str().map(String::from))
            .unwrap_or_else(|| String::from("."));
        
        // Change to the temporary directory
        std::env::set_current_dir(temp_dir.path()).unwrap();
        
        // Test is_git_repo
        assert!(is_git_repo());
        
        // Try to change back to the original directory, but don't panic if it fails
        let _ = std::env::set_current_dir(Path::new(&current_dir_str));
        
        // Test with a non-Git directory
        let temp_dir = TempDir::new().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();
        
        assert!(!is_git_repo());
        
        // Try to change back to the original directory, but don't panic if it fails
        let _ = std::env::set_current_dir(Path::new(&current_dir_str));
    }

    #[test]
    fn test_git_command() {
        let cmd = git_command();
        assert_eq!(cmd.get_command_name(), std::ffi::OsStr::new("git"));
    }

    #[test]
    fn test_run_git_check_warning_only() {
        // Test with a temporary Git repository
        let temp_dir = setup_git_repo();
        
        // Save the current directory path as a string to avoid potential issues
        let current_dir_str = std::env::current_dir()
            .ok()
            .and_then(|p| p.to_str().map(String::from))
            .unwrap_or_else(|| String::from("."));
        
        // Change to the temporary directory
        std::env::set_current_dir(temp_dir.path()).unwrap();
        
        // Create an untracked .nix file
        fs::write(Path::new("test.nix"), "{ }: { }").unwrap();
        
        // Run the check
        let result = run_git_check_warning_only();
        assert!(result.is_ok());
        
        // Try to change back to the original directory, but don't panic if it fails
        let _ = std::env::set_current_dir(Path::new(&current_dir_str));
    }
}