use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use chrono::{DateTime, Local, TimeZone, Utc};
use tracing::debug;

#[derive(Debug)]
pub struct GenerationInfo {
    /// Number of a generation
    pub number: String,

    /// Date on switch a generation was built
    pub date: String,

    /// NixOS version derived from `nixos-version`
    pub nixos_version: String,

    /// Version of the bootable kernel for a given generation
    pub kernel_version: String,

    /// Revision for a configuration. This will be the value
    /// set in `config.system.configurationRevision`
    pub configuration_revision: String,

    /// Specialisations, if any.
    pub specialisations: Vec<String>,

    /// Whether a given generation is the current one.
    pub current: bool,
}

pub fn from_dir(generation_dir: &Path) -> Option<u64> {
    generation_dir
        .file_name()
        .and_then(|os_str| os_str.to_str())
        .and_then(|generation_base| {
            let no_link_gen = generation_base.trim_end_matches("-link");
            no_link_gen
                .rsplit_once('-')
                .and_then(|(_, gen)| gen.parse::<u64>().ok())
        })
}

pub fn describe(generation_dir: &Path, current_profile: &Path) -> Option<GenerationInfo> {
    let generation_number = from_dir(generation_dir)?;

    // Get metadata once and reuse for both date and existence checks
    let metadata = fs::metadata(generation_dir).ok()?;
    let build_date = metadata
        .created()
        .or_else(|_| metadata.modified())
        .map(|system_time| {
            let duration = system_time
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            DateTime::<Utc>::from(std::time::UNIX_EPOCH + duration).to_rfc3339()
        })
        .unwrap_or_else(|_| "Unknown".to_string());

    let nixos_version = fs::read_to_string(generation_dir.join("nixos-version"))
        .unwrap_or_else(|_| "Unknown".to_string());

    let kernel_dir = generation_dir
        .join("kernel")
        .canonicalize()
        .ok()
        .and_then(|path| path.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("Unknown"));

    let kernel_modules_dir = kernel_dir.join("lib/modules");
    let kernel_version = if kernel_modules_dir.exists() {
        match fs::read_dir(&kernel_modules_dir) {
            Ok(entries) => {
                let mut versions = Vec::with_capacity(4);
                for entry in entries.filter_map(Result::ok) {
                    if let Some(name) = entry.file_name().to_str() {
                        versions.push(name.to_string());
                    }
                }
                versions.join(", ")
            }
            Err(_) => "Unknown".to_string(),
        }
    } else {
        "Unknown".to_string()
    };

    let configuration_revision = {
        let nixos_version_path = generation_dir.join("sw/bin/nixos-version");
        if nixos_version_path.exists() {
            process::Command::new(&nixos_version_path)
                .arg("--configuration-revision")
                .output()
                .ok()
                .and_then(|output| String::from_utf8(output.stdout).ok())
                .unwrap_or_default()
                .trim()
                .to_string()
        } else {
            String::new()
        }
    };

    let specialisations = {
        let specialisation_path = generation_dir.join("specialisation");
        if specialisation_path.exists() {
            fs::read_dir(specialisation_path)
                .map(|entries| {
                    let mut specs = Vec::with_capacity(5);
                    for entry in entries.filter_map(Result::ok) {
                        if let Some(name) = entry.file_name().to_str() {
                            specs.push(name.to_string());
                        }
                    }
                    specs
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    };

    let canonical_gen_dir = generation_dir.canonicalize().ok()?;
    let current = current_profile
        .canonicalize()
        .ok()
        .map(|canonical_current| canonical_gen_dir == canonical_current)
        .unwrap_or(false);

    Some(GenerationInfo {
        number: generation_number.to_string(),
        date: build_date,
        nixos_version,
        kernel_version,
        configuration_revision,
        specialisations,
        current,
    })
}

pub fn print_info(mut generations: Vec<GenerationInfo>) {
    let closure = {
        // Get path information for the *current generation* from /run/current-system
        // and split it by whitespace to get the size (second part). This should be
        // safe enough, in theory.
        let path_info = process::Command::new("nix")
            .arg("path-info")
            .arg("-Sh")
            .arg("/run/current-system")
            .output();

        if let Ok(output) = path_info {
            let size_info = String::from_utf8_lossy(&output.stdout);
            // Improve error handling with more descriptive message
            let size = size_info
                .split_whitespace()
                .nth(1)
                .unwrap_or_else(|| {
                    debug!("Unexpected format in nix path-info output: {}", size_info);
                    "Unknown size"
                });
            size.to_string()
        } else {
            debug!("Failed to get closure size information");
            "Unknown size".to_string()
        }
    };

    // Parse all dates at once and cache them
    let mut parsed_dates = HashMap::with_capacity(generations.len());
    for gen in &generations {
        let date = DateTime::parse_from_rfc3339(&gen.date)
            .map(|dt| dt.with_timezone(&Local))
            .unwrap_or_else(|_| Local.timestamp_opt(0, 0).unwrap());
        parsed_dates.insert(
            gen.date.clone(),
            date.format("%Y-%m-%d %H:%M:%S").to_string(),
        );
    }

    // Sort generations by numeric value of the generation number
    generations.sort_by_key(|gen| gen.number.parse::<u64>().unwrap_or(0));
    let current_generation = generations
        .iter()
        .max_by_key(|gen| gen.number.parse::<u64>().unwrap_or(0));
    debug!(?current_generation);

    if let Some(current) = current_generation {
        println!("NixOS {}", current.nixos_version);
    } else {
        println!("Error getting current generation!");
    }

    println!("Closure Size: {}", closure);
    println!();

    // Determine column widths for pretty printing
    // Use more descriptive variable names and safer defaults
    let max_nixos_version_len = generations
        .iter()
        .map(|g| g.nixos_version.len())
        .max()
        .unwrap_or_else(|| {
            // Default to header length if no generations
            "NixOS Version".len()
        });

    let max_kernel_len = generations
        .iter()
        .map(|g| g.kernel_version.len())
        .max()
        .unwrap_or_else(|| {
            // Default to header length if no generations
            "Kernel".len()
        });

    println!(
        "{:<13} {:<20} {:<width_nixos$} {:<width_kernel$} {:<22} Specialisations",
        "Generation No",
        "Build Date",
        "NixOS Version",
        "Kernel",
        "Configuration Revision",
        width_nixos = max_nixos_version_len,
        width_kernel = max_kernel_len
    );

    // Print generations in descending order
    for generation in generations.iter().rev() {
        // Improve error handling for date formatting
        let formatted_date = parsed_dates
            .get(&generation.date)
            .cloned()
            .unwrap_or_else(|| {
                debug!("Missing parsed date for generation {}", generation.number);
                "Unknown date".to_string()
            });

        let specialisations = if generation.specialisations.is_empty() {
            String::new()
        } else {
            generation
                .specialisations
                .iter()
                .map(|s| format!("*{}", s))
                .collect::<Vec<String>>()
                .join(" ")
        };

        println!(
            "{:<13} {:<20} {:<width_nixos$} {:<width_kernel$} {:<25} {}",
            format!(
                "{}{}",
                generation.number,
                if generation.current { " (current)" } else { "" }
            ),
            formatted_date,
            generation.nixos_version,
            generation.kernel_version,
            generation.configuration_revision,
            specialisations,
            width_nixos = max_nixos_version_len,
            width_kernel = max_kernel_len
        );
    }
}
