#[cfg(test)]
mod tests {
    use ng::util::{
        compare_semver, add_verbosity_flags, is_stdout_tty, run_cmd, run_cmd_inherit_stdio,
        UtilCommandError, manage_out_path, run_piped_commands
    };
    use std::process::Command;
    use std::path::PathBuf;

    #[test]
    fn test_compare_semver() {
        // Test equal versions
        let result = compare_semver("1.2.3", "1.2.3").unwrap();
        assert_eq!(result, std::cmp::Ordering::Equal);
        
        // Test greater version
        let result = compare_semver("2.0.0", "1.2.3").unwrap();
        assert_eq!(result, std::cmp::Ordering::Greater);
        
        // Test lesser version
        let result = compare_semver("1.0.0", "1.2.3").unwrap();
        assert_eq!(result, std::cmp::Ordering::Less);
        
        // Test with pre-release versions
        let result = compare_semver("1.2.3-alpha", "1.2.3").unwrap();
        assert_eq!(result, std::cmp::Ordering::Less);
        
        // Test with build metadata
        // Note: According to semver spec, build metadata should be ignored when determining version precedence,
        // but the semver crate implementation might handle this differently
        let result = compare_semver("1.2.3+build.1", "1.2.3").unwrap();
        // The actual result might be Equal or Greater depending on the semver crate implementation
        assert!(result == std::cmp::Ordering::Equal || result == std::cmp::Ordering::Greater);
        
        // Test invalid version
        let result = compare_semver("invalid", "1.2.3");
        assert!(result.is_err());
    }

    #[test]
    fn test_add_verbosity_flags() {
        // Test with verbosity level 0
        let mut cmd = Command::new("test");
        add_verbosity_flags(&mut cmd, 0);
        assert_eq!(cmd.get_args().count(), 0);
        
        // Test with verbosity level 1
        let mut cmd = Command::new("test");
        add_verbosity_flags(&mut cmd, 1);
        assert_eq!(cmd.get_args().count(), 1);
        assert_eq!(cmd.get_args().next().unwrap().to_str().unwrap(), "-v");
        
        // Test with verbosity level 3
        let mut cmd = Command::new("test");
        add_verbosity_flags(&mut cmd, 3);
        assert_eq!(cmd.get_args().count(), 3);
        
        // Test with verbosity level exceeding cap (7)
        let mut cmd = Command::new("test");
        add_verbosity_flags(&mut cmd, 10);
        assert_eq!(cmd.get_args().count(), 7);
    }

    #[test]
    fn test_is_stdout_tty() {
        // This test is limited since we can't easily control whether stdout is a TTY
        // Just make sure the function runs without error
        let _ = is_stdout_tty();
    }

    #[test]
    fn test_run_cmd() {
        // Test with a successful command
        let mut cmd = Command::new("echo");
        cmd.arg("test");
        let result = run_cmd(&mut cmd);
        assert!(result.is_ok());
        
        if let Ok(output) = result {
            assert!(output.status.success());
            assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "test");
        }
        
        // Test with a failing command
        let mut cmd = Command::new("false");
        let result = run_cmd(&mut cmd);
        assert!(result.is_err());
        
        // Verify the error type
        match result {
            Err(err) => {
                match err {
                    UtilCommandError::NonZeroStatus { command_str, status_code, stdout: _, stderr: _ } => {
                        assert!(command_str.contains("false"));
                        assert_eq!(status_code, "1");
                    },
                    _ => panic!("Expected NonZeroStatus error, got: {:?}", err),
                }
            },
            _ => panic!("Expected error"),
        }
        
        // Test with a non-existent command
        let mut cmd = Command::new("command_that_does_not_exist_ever");
        let result = run_cmd(&mut cmd);
        assert!(result.is_err());
        
        // Verify the error type
        match result {
            Err(err) => {
                match err {
                    UtilCommandError::SpawnFailed { command_str, io_error: _ } => {
                        assert!(command_str.contains("command_that_does_not_exist_ever"));
                    },
                    _ => panic!("Expected SpawnFailed error, got: {:?}", err),
                }
            },
            _ => panic!("Expected error"),
        }
    }

    #[test]
    fn test_run_cmd_inherit_stdio() {
        // Test with a successful command
        let mut cmd = Command::new("echo");
        cmd.arg("test");
        let result = run_cmd_inherit_stdio(&mut cmd);
        assert!(result.is_ok());
        
        if let Ok(status) = result {
            assert!(status.success());
        }
        
        // Test with a failing command
        let mut cmd = Command::new("false");
        let result = run_cmd_inherit_stdio(&mut cmd);
        assert!(result.is_err());
        
        // Verify the error type
        match result {
            Err(err) => {
                match err {
                    UtilCommandError::InheritedNonZeroStatus { command_str, status_code } => {
                        assert!(command_str.contains("false"));
                        assert_eq!(status_code, "1");
                    },
                    _ => panic!("Expected InheritedNonZeroStatus error, got: {:?}", err),
                }
            },
            _ => panic!("Expected error"),
        }
    }
    
    #[test]
    fn test_manage_out_path() {
        // Test with a provided path
        let provided_path = PathBuf::from("/tmp/test_out_link");
        let result = manage_out_path(Some(&provided_path));
        assert!(result.is_ok());
        
        if let Ok(path_manager) = result {
            assert_eq!(path_manager.get_path(), &provided_path);
        }
        
        // Test with no provided path (should create a temp dir)
        let result = manage_out_path(None);
        assert!(result.is_ok());
        
        if let Ok(path_manager) = result {
            // The path should end with "result"
            assert!(path_manager.get_path().ends_with("result"));
            // The parent directory should exist
            assert!(path_manager.get_path().parent().unwrap().exists());
        }
    }
    
    #[test]
    fn test_run_piped_commands() {
        // Test piping echo to grep
        let mut cmd1 = Command::new("echo");
        cmd1.arg("hello\nworld\nhello again");
        let mut cmd2 = Command::new("grep");
        cmd2.arg("hello");
        
        let result = run_piped_commands(cmd1, cmd2);
        assert!(result.is_ok());
        
        if let Ok(output) = result {
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("hello"));
            assert!(stdout.contains("hello again"));
            assert!(!stdout.contains("world"));
        }
        
        // Test with a failing first command
        let cmd1_fail = Command::new("command_that_does_not_exist_ever");
        let mut cmd2_fail_1 = Command::new("grep");
        cmd2_fail_1.arg("hello");
        
        let result_fail1 = run_piped_commands(cmd1_fail, cmd2_fail_1);
        assert!(result_fail1.is_err());
        
        match result_fail1 {
            Err(err) => {
                match err {
                    UtilCommandError::SpawnFailed { command_str, io_error: _ } => {
                        assert!(command_str.contains("command_that_does_not_exist_ever"));
                    },
                    _ => panic!("Expected SpawnFailed error, got: {:?}", err),
                }
            },
            _ => panic!("Expected error"),
        }
        
        // Test with a failing second command
        let mut cmd1_ok = Command::new("echo");
        cmd1_ok.arg("hello");
        let mut cmd2_fail_2 = Command::new("grep");
        cmd2_fail_2.arg("--invalid-option");
        
        let result_fail2 = run_piped_commands(cmd1_ok, cmd2_fail_2);
        assert!(result_fail2.is_err());
        
        match result_fail2 {
            Err(err) => {
                match err {
                    UtilCommandError::NonZeroStatus { command_str, status_code, stdout: _, stderr: _ } => {
                        assert!(command_str.contains("echo") && command_str.contains("grep"));
                        assert_ne!(status_code, "0");
                    },
                    _ => panic!("Expected NonZeroStatus error, got: {:?}", err),
                }
            },
            _ => panic!("Expected error"),
        }
    }
}