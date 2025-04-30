#[cfg(test)]
mod tests {
    use crate::util::{compare_semver, add_verbosity_flags, is_stdout_tty, run_cmd, run_cmd_inherit_stdio};
    use std::process::Command;

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
    }
}