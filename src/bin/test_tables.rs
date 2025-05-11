//! Test program for table formatting

use ng::lint::{CheckStatus, LintOutcome, LintSummary, format_lint_results_table};
use ng::tables::format_package_diff_table;

fn main() {
    println!("=== Testing Table Formatting ===\n");
    
    // Test lint results table
    println!("Lint Results Table:");
    let mut lint_summary = LintSummary::default();
    lint_summary.outcome = Some(LintOutcome::Passed);
    lint_summary.files_formatted = 3;
    lint_summary.fixes_applied = 2;
    
    // Add some test data
    lint_summary.details.insert("Format (alejandra)".to_string(), CheckStatus::Passed);
    lint_summary.details.insert("Statix Fix".to_string(), CheckStatus::Failed);
    lint_summary.details.insert("Deadnix Fix".to_string(), CheckStatus::Skipped);
    
    // Format and print the table
    let table = format_lint_results_table(&lint_summary);
    println!("{}", table);
    
    // Test package diff table
    println!("\nPackage Diff Table:");
    
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
    
    // Format and print the table
    let table = format_package_diff_table(&added, &removed, &changed);
    println!("{}", table);
    
    println!("\nTable Formatting Test Completed!");
}