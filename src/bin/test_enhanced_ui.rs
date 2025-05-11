//! Test program for enhanced UI elements

use color_eyre::eyre::Result;
use ng::ui_style::{Colors, Symbols, Print};
use tracing::info;

fn main() -> Result<()> {
    // Setup tracing
    tracing_subscriber::fmt::init();
    
    // Test basic styling
    println!("\n=== Testing Basic Styling ===\n");
    
    Print::section("Basic Styling Test");
    
    Print::success("This is a success message");
    Print::error("This is an error message");
    Print::warning("This is a warning message");
    Print::info("This is an info message");
    Print::prompt("This is a prompt message");
    
    // Test table display for lint results
    println!("\n=== Testing Lint Results Table ===\n");
    
    Print::section("Lint Results Table");
    
    let lint_results = vec![
        ("Format (alejandra)".to_string(), "Passed".to_string(), "No formatting issues found".to_string()),
        ("Statix Fix".to_string(), "Fixed".to_string(), "3 issues fixed".to_string()),
        ("Deadnix Fix".to_string(), "Failed".to_string(), "Failed to remove dead code".to_string()),
    ];
    
    // Handle errors without propagating
    if let Err(e) = ng::tables::display_lint_results(lint_results) {
        eprintln!("Error displaying lint results: {}", e);
    }
    
    // Test table display for package diff
    println!("\n=== Testing Package Diff Table ===\n");
    
    Print::section("Package Diff Table");
    
    let added = vec![
        "nixpkgs.ripgrep".to_string(),
        "nixpkgs.fd".to_string(),
    ];
    
    let removed = vec![
        "nixpkgs.grep".to_string(),
        "nixpkgs.find".to_string(),
    ];
    
    let changed = vec![
        ("nixpkgs.git".to_string(), "2.39.0".to_string(), "2.40.1".to_string()),
        ("nixpkgs.neovim".to_string(), "0.8.3".to_string(), "0.9.0".to_string()),
    ];
    
    // Handle errors without propagating
    if let Err(e) = ng::tables::display_package_diff(added, removed, changed) {
        eprintln!("Error displaying package diff: {}", e);
    }
    
    // Test Git status table
    println!("\n=== Testing Git Status Table ===\n");
    
    Print::section("Git Status Table");
    
    let untracked_files = vec![
        "flake.nix".to_string(),
        "flake.lock".to_string(),
        "configuration.nix".to_string(),
    ];
    
    let modified_files = vec![
        "home.nix".to_string(),
        "packages.nix".to_string(),
    ];
    
    // Handle errors without propagating
    if let Err(e) = ng::tables::display_git_status(untracked_files, modified_files) {
        eprintln!("Error displaying git status: {}", e);
    }
    
    // Test error reporting
    println!("\n=== Testing Error Reporting ===\n");
    
    Print::section("Error Reporting");
    
    let error_details = r#"Error in /home/user/nixos/configuration.nix:

error: syntax error, unexpected '}', expecting ';'

       at /home/user/nixos/configuration.nix:42:1:

       41|   };
       42| }
         | ^
"#;
    
    let enhanced = ng::error_handler::enhance_syntax_error_output(error_details);
    println!("{}", enhanced);
    
    let recommendations = vec![
        "Fix the syntax error by adding a missing semicolon".to_string(),
        "Use a formatter like 'alejandra' to automatically fix formatting issues".to_string(),
    ];
    
    info!("{} {}", Symbols::info(), Colors::info(Colors::emphasis("Possible Issues & Recommendations:")));
    for rec in recommendations {
        info!("   • {}", rec);
    }
    
    println!("\nUI Enhancement Test Completed Successfully!");
    
    Ok(())
}