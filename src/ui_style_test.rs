#[cfg(test)]
mod tests {
    use crate::ui_style::{Colors, Symbols, Print, spinner_message, success_message, separator, header};

    #[test]
    fn test_colors() {
        // Just test that the functions compile and return something
        let success = Colors::success("Success").to_string();
        let error = Colors::error("Error").to_string();
        let warning = Colors::warning("Warning").to_string();
        let info = Colors::info("Info").to_string();
        let prompt = Colors::prompt("Prompt").to_string();
        let code = Colors::code("Code").to_string();
        let emphasis = Colors::emphasis("Emphasis").to_string();
        
        // Verify the strings contain the original text
        assert!(success.contains("Success"));
        assert!(error.contains("Error"));
        assert!(warning.contains("Warning"));
        assert!(info.contains("Info"));
        assert!(prompt.contains("Prompt"));
        assert!(code.contains("Code"));
        assert!(emphasis.contains("Emphasis"));
    }

    #[test]
    fn test_symbols() {
        // Verify symbols are non-empty
        assert!(!Symbols::success().is_empty());
        assert!(!Symbols::error().is_empty());
        assert!(!Symbols::warning().is_empty());
        assert!(!Symbols::info().is_empty());
        assert!(!Symbols::progress().is_empty());
        assert!(!Symbols::cleanup().is_empty());
        assert!(!Symbols::prompt().is_empty());
        assert!(!Symbols::check().is_empty());
        assert!(!Symbols::build().is_empty());
        assert!(!Symbols::activate().is_empty());
        assert!(!Symbols::success_check().is_empty());
        assert!(!Symbols::skip().is_empty());
    }

    #[test]
    fn test_spinner_message() {
        let msg = spinner_message("Build", "Building configuration...");
        assert!(msg.contains("Build"));
        assert!(msg.contains("Building configuration..."));
    }

    #[test]
    fn test_success_message() {
        let msg = success_message("Build", "Configuration built successfully");
        assert!(msg.contains("Build"));
        assert!(msg.contains("Configuration built successfully"));
        assert!(msg.contains(Symbols::success_check()));
    }

    #[test]
    fn test_separator() {
        let sep = separator();
        assert!(!sep.is_empty());
        assert!(sep.len() >= 80);
    }

    #[test]
    fn test_header() {
        let title = "Test Header";
        let h = header(title);
        assert!(h.contains(title));
        assert!(h.contains(&separator()));
    }
}