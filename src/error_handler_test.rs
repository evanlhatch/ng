#[cfg(test)]
mod tests {
    use crate::error_handler::{parse_nix_eval_error, find_failed_derivations, scan_log_for_recommendations};

    #[test]
    fn test_parse_nix_eval_error() {
        let stderr = "error: undefined variable 'pkgs' at /home/user/config.nix:10:5";
        let result = parse_nix_eval_error(stderr);
        assert!(result.is_some());
        
        if let Some((message, file, line, column)) = result {
            assert_eq!(message, "undefined variable 'pkgs'");
            assert_eq!(file, "/home/user/config.nix");
            assert_eq!(line, 10);
            assert_eq!(column, 5);
        }
        
        // Test with no match
        let stderr = "some other error message";
        let result = parse_nix_eval_error(stderr);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_failed_derivations() {
        let stderr = "error: builder for '/nix/store/abcdef123456.drv' failed\nsome other text\nerror: builder for '/nix/store/xyz789.drv' failed";
        let result = find_failed_derivations(stderr);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "/nix/store/abcdef123456.drv");
        assert_eq!(result[1], "/nix/store/xyz789.drv");
        
        // Test with no match
        let stderr = "some other error message";
        let result = find_failed_derivations(stderr);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_scan_log_for_recommendations() {
        // Test with package not found
        let log = "error: package foo not found";
        let result = scan_log_for_recommendations(log);
        assert!(result.iter().any(|r| r.contains("package dependency")));
        
        // Test with permission error
        let log = "error: permission denied";
        let result = scan_log_for_recommendations(log);
        assert!(result.iter().any(|r| r.contains("Permission errors")));
        
        // Test with network error
        let log = "error: network timeout";
        let result = scan_log_for_recommendations(log);
        assert!(result.iter().any(|r| r.contains("Network-related errors")));
        
        // Test with syntax error
        let log = "error: syntax error";
        let result = scan_log_for_recommendations(log);
        assert!(result.iter().any(|r| r.contains("Syntax errors")));
        
        // Test with no specific error
        let log = "some generic error";
        let result = scan_log_for_recommendations(log);
        assert!(result.iter().any(|r| r.contains("Review the full log")));
    }
}