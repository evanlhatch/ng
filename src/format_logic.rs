// src/format_logic.rs
use crate::interface::FormatArgs;
use crate::Result;
use crate::ui_style;
use std::path::PathBuf;
use std::process::Command;
use walkdir::WalkDir;

pub fn run_formatting(args: &FormatArgs, verbose_count: u8) -> Result<()> {
    if verbose_count > 0 {
        println!(
            "Formatting with options: apply={}, path='{}', verbose_level={}",
            args.apply,
            args.path,
            verbose_count
        );
    }

    // 1. Determine the absolute path to format
    let path_to_format = PathBuf::from(&args.path);
    let absolute_path = if path_to_format.is_absolute() {
        path_to_format
    } else {
        std::env::current_dir()?.join(path_to_format)
    };

    if verbose_count > 1 {
        println!("Absolute path to format: {:?}", absolute_path);
    }

    // 2. Find all .nix files in the path
    let mut files_to_format = Vec::new();
    
    if absolute_path.is_file() && absolute_path.extension().map_or(false, |ext| ext == "nix") {
        files_to_format.push(absolute_path.clone());
    } else if absolute_path.is_dir() {
        for entry in WalkDir::new(&absolute_path).into_iter().filter_map(Result::ok) {
            if entry.file_type().is_file() && entry.path().extension().map_or(false, |ext| ext == "nix") {
                files_to_format.push(entry.path().to_path_buf());
            }
        }
    } else {
        println!("Path '{}' is not a valid file or directory", args.path);
        return Ok(());
    }

    if files_to_format.is_empty() {
        println!("No Nix files found in '{}'", args.path);
        return Ok(());
    }

    println!("Found {} Nix files to format", files_to_format.len());
    
    // 3. Format each file
    let mut had_errors = false;
    let mut formatted_count = 0;

    for file_path in &files_to_format {
        if verbose_count > 0 {
            println!("Processing: {}", file_path.display());
        }

        // Use nixfmt-rfc-style as the formatter
        let mut cmd = Command::new("nixfmt-rfc-style");
        
        if !args.apply {
            cmd.arg("--check");
        }
        
        cmd.arg(file_path);
        
        let output = cmd.output();
        
        match output {
            Ok(output) => {
                if output.status.success() {
                    if verbose_count > 0 {
                        if !args.apply {
                            println!("✓ {} is correctly formatted", file_path.display());
                        } else {
                            println!("✓ Formatted {}", file_path.display());
                            formatted_count += 1;
                        }
                    }
                } else {
                    had_errors = true;
                    if !args.apply {
                        println!("✗ {} needs formatting", file_path.display());
                        if verbose_count > 1 {
                            println!("{}", String::from_utf8_lossy(&output.stderr));
                        }
                    } else {
                        println!("✗ Failed to format {}", file_path.display());
                        println!("{}", String::from_utf8_lossy(&output.stderr));
                    }
                }
            },
            Err(e) => {
                had_errors = true;
                println!("Error executing formatter: {}", e);
                println!("Make sure nixfmt-rfc-style is installed and in your PATH");
                break;
            }
        }
    }

    // 4. Print summary
    if args.apply {
        if formatted_count > 0 {
            println!("{}", ui_style::success_message("Format", &format!("Formatted {} Nix files", formatted_count)));
        } else if !had_errors {
            println!("All files already formatted correctly");
        }
    } else {
        if had_errors {
            println!("Some files need formatting. Run with --apply to format them.");
            std::process::exit(1);
        } else {
            println!("{}", ui_style::success_message("Format", "All files are formatted correctly"));
        }
    }

    Ok(())
}
