mod cli; // Assuming your cli module

use cli::{Cli, Commands, FormatArgs}; // Adjust import
use color_eyre::eyre::{Result, WrapErr};
use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() -> Result<()> {
    color_eyre::install()?;
    let cli_args = Cli::parse();

    // Determine project root
    let project_root = env::current_dir().wrap_err("Failed to determine current directory")?;

    match cli_args.command {
        Commands::Format(format_args) => {
            format_nix_files(&project_root.join(format_args.path), format_args.apply)?;
        }
    }

    Ok(())
}

fn format_nix_files(start_path: &PathBuf, apply: bool) -> Result<()> {
    // 1. Use walkdir to find all .nix files
    use walkdir::WalkDir;
    let walker = WalkDir::new(start_path).into_iter();

    let mut files_to_format = Vec::new();
    for entry in walker.filter_map(Result::ok) {
        if entry.file_type().is_file() && entry.path().extension().map_or(false, |ext| ext == "nix") {
            files_to_format.push(entry.path().to_path_buf());
        }
    }

    if files_to_format.is_empty() {
        println!("No Nix files found in '{}'", start_path.display());
        return Ok(());
    }

    // 2. Run nix fmt (or nixfmt-rfc-style) on each file
    for file_path in &files_to_format {
        println!("Formatting: {}", file_path.display());
        let mut cmd = Command::new("nix"); // Or "nixfmt-rfc-style"
        cmd.arg("fmt").arg(file_path);

        if !apply {
            cmd.arg("--check"); // Just check if formatting is correct
        }

        let output = cmd.output().wrap_err_with(|| format!("Failed to execute nix fmt on {}", file_path.display()))?;

        if output.status.success() {
            if !apply {
                println!("{} is already formatted correctly.", file_path.display());
            }
        } else {
            eprintln!("{} formatting check failed:", file_path.display());
            eprintln!("{}", String::from_utf8_lossy(&output.stderr));
            if !apply {
                eprintln!("Run with --apply to format this file.");
            }
        }
    }

    Ok(())
}
