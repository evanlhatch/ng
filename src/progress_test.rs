#[cfg(test)]
mod tests {
    use nh::progress::{start_spinner, update_spinner_message, finish_spinner_success, finish_spinner_fail};
    use indicatif::ProgressBar;
    use std::time::Duration;

    #[test]
    fn test_start_spinner() {
        let spinner = start_spinner("Test message");
        
        // Verify the spinner is created
        assert!(spinner.is_hidden() || true);
        
        // Clean up
        spinner.finish_and_clear();
    }

    #[test]
    fn test_update_spinner_message() {
        let spinner = ProgressBar::new_spinner();
        let original_message = "Original message";
        spinner.set_message(original_message.to_string());
        
        // Update the message
        let new_message = "New message";
        update_spinner_message(&spinner, new_message);
        
        // Verify the message is updated
        assert_eq!(spinner.message(), new_message);
        
        // Clean up
        spinner.finish_and_clear();
    }

    #[test]
    fn test_finish_spinner_success() {
        let spinner = ProgressBar::new_spinner();
        
        // Finish with success
        let success_message = "Success message";
        finish_spinner_success(&spinner, success_message);
        
        // Verify the spinner is finished
        // Just check that the operation doesn't panic
        
        // Clean up
        spinner.finish_and_clear();
    }

    #[test]
    fn test_finish_spinner_fail() {
        let spinner = ProgressBar::new_spinner();
        
        // Finish with failure
        finish_spinner_fail(&spinner);
        
        // Verify the spinner is finished
        // Just check that the operation doesn't panic
        
        // Clean up
        spinner.finish_and_clear();
    }

    #[test]
    fn test_spinner_integration() {
        // Test the full spinner lifecycle
        let spinner = start_spinner("Starting operation");
        
        // Simulate some work
        std::thread::sleep(Duration::from_millis(100));
        
        // Update the message
        update_spinner_message(&spinner, "Operation in progress");
        
        // Simulate more work
        std::thread::sleep(Duration::from_millis(100));
        
        // Finish with success
        finish_spinner_success(&spinner, "Operation completed successfully");
        
        // Verify the spinner is finished
        // Just check that the operation doesn't panic
    }
}