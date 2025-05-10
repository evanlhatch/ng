//! Module for displaying structured data in tables

use cli_table::{print_stdout, Cell, Table, Style};
use cli_table::Color;

/// Display a table with lint results
pub fn display_lint_results(
    results: Vec<(String, String, String)>, // (Linter, Status, Details)
) -> Result<(), Box<dyn std::error::Error>> {
    let mut table = vec![];

    for (linter, status, details) in results {
        let status_cell = match status.as_str() {
            "Passed" => status.cell().foreground_color(Some(Color::Green)),
            "Fixed" => status.cell().foreground_color(Some(Color::Yellow)),
            "Failed" => status.cell().foreground_color(Some(Color::Red)),
            _ => status.cell(),
        };

        table.push(vec![
            linter.cell(),
            status_cell,
            details.cell(),
        ]);
    }

    let table = table.table()
        .title(vec![
            "Linter".cell().bold(true),
            "Status".cell().bold(true),
            "Details".cell().bold(true),
        ])
        .bold(true);

    print_stdout(table)?;
    Ok(())
}

/// Display a table with package diff summary
pub fn display_package_diff(
    added: Vec<String>,
    removed: Vec<String>,
    changed: Vec<(String, String, String)>, // (Package, Old Version, New Version)
) -> Result<(), Box<dyn std::error::Error>> {
    // Added packages
    if !added.is_empty() {
        let mut table = vec![];
        for package in added {
            table.push(vec![package.cell()]);
        }

        let table = table.table()
            .title(vec!["Packages Added".cell().bold(true)])
            .bold(true);

        print_stdout(table)?;
        println!();
    }

    // Removed packages
    if !removed.is_empty() {
        let mut table = vec![];
        for package in removed {
            table.push(vec![package.cell()]);
        }

        let table = table.table()
            .title(vec!["Packages Removed".cell().bold(true)])
            .bold(true);

        print_stdout(table)?;
        println!();
    }

    // Changed packages
    if !changed.is_empty() {
        let mut table = vec![];
        for (package, old_version, new_version) in changed {
            table.push(vec![
                package.cell(),
                old_version.cell(),
                new_version.cell(),
            ]);
        }

        let table = table.table()
            .title(vec![
                "Package".cell().bold(true),
                "Old Version".cell().bold(true),
                "New Version".cell().bold(true),
            ])
            .bold(true);

        print_stdout(table)?;
    }

    Ok(())
}

/// Format package diff as a table string
pub fn format_package_diff_table(
    added: &[String],
    removed: &[String],
    changed: &[(String, String, String)], // (Package, Old Version, New Version)
) -> String {
    use cli_table::Table;
    
    let mut output = String::new();
    
    // Added packages
    if !added.is_empty() {
        #[derive(Table)]
        struct AddedRow {
            #[table(title = "Packages Added")]
            package: String,
        }
        
        let mut rows = Vec::new();
        for package in added {
            rows.push(AddedRow {
                package: package.clone(),
            });
        }
        
        // Sort rows by package name for consistent output
        rows.sort_by(|a, b| a.package.cmp(&b.package));
        
        // Format as a simple table
        output.push_str("+-----------------+\n");
        output.push_str("| Packages Added  |\n");
        output.push_str("+-----------------+\n");
        
        for row in rows {
            output.push_str(&format!("| {:<15} |\n", row.package));
        }
        
        output.push_str("+-----------------+\n\n");
    }
    
    // Removed packages
    if !removed.is_empty() {
        #[derive(Table)]
        struct RemovedRow {
            #[table(title = "Packages Removed")]
            package: String,
        }
        
        let mut rows = Vec::new();
        for package in removed {
            rows.push(RemovedRow {
                package: package.clone(),
            });
        }
        
        // Sort rows by package name for consistent output
        rows.sort_by(|a, b| a.package.cmp(&b.package));
        
        // Format as a simple table
        output.push_str("+------------------+\n");
        output.push_str("| Packages Removed |\n");
        output.push_str("+------------------+\n");
        
        for row in rows {
            output.push_str(&format!("| {:<16} |\n", row.package));
        }
        
        output.push_str("+------------------+\n\n");
    }
    
    // Changed packages
    if !changed.is_empty() {
        #[derive(Table)]
        struct ChangedRow {
            #[table(title = "Package")]
            package: String,
            #[table(title = "Old Version")]
            old_version: String,
            #[table(title = "New Version")]
            new_version: String,
        }
        
        let mut rows = Vec::new();
        for (package, old_version, new_version) in changed {
            rows.push(ChangedRow {
                package: package.clone(),
                old_version: old_version.clone(),
                new_version: new_version.clone(),
            });
        }
        
        // Sort rows by package name for consistent output
        rows.sort_by(|a, b| a.package.cmp(&b.package));
        
        // Format as a simple table
        output.push_str("+----------------+-------------+-------------+\n");
        output.push_str("| Package        | Old Version | New Version |\n");
        output.push_str("+----------------+-------------+-------------+\n");
        
        for row in rows {
            output.push_str(&format!("| {:<14} | {:<11} | {:<11} |\n",
                row.package, row.old_version, row.new_version));
        }
        
        output.push_str("+----------------+-------------+-------------+\n");
    }
    
    output
}

/// Display a table with Git status information
pub fn display_git_status(
    untracked_files: Vec<String>,
    modified_files: Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if !untracked_files.is_empty() {
        let mut table = vec![];
        for file in untracked_files {
            table.push(vec![file.cell()]);
        }

        let table = table.table()
            .title(vec!["Untracked Files".cell().bold(true).foreground_color(Some(Color::Yellow))])
            .bold(true);

        print_stdout(table)?;
        println!();
    }

    if !modified_files.is_empty() {
        let mut table = vec![];
        for file in modified_files {
            table.push(vec![file.cell()]);
        }

        let table = table.table()
            .title(vec!["Modified Files".cell().bold(true).foreground_color(Some(Color::Yellow))])
            .bold(true);

        print_stdout(table)?;
    }

    Ok(())
}