# Part 1
## Part 1- Plan
Foundation First: Start with low-level utilities that everything else will depend on (command execution, error types).
Targeted Improvements: Address specific, self-contained improvements you've already identified (like Installable parsing).
Introduce Core Abstractions: Define the PlatformRebuildStrategy and the initial execute_rebuild_workflow.
Incremental Refactor of One Command: Migrate a single command (e.g., nh os switch) to use the new workflow, adding features to the workflow step-by-step.
Continuous, Granular Testing: After almost every single code modification or addition, write and run tests.
Assumptions:

You have a working development environment for ng set up.
You are comfortable with Rust's testing frameworks and potentially assert_cmd for integration tests and insta for snapshot tests (as per your Point 8).
Part 1: Foundational Refactoring and Workflow Implementation - Step-by-Step Moment Plan

Phase 0: Preparation & Setup

Moment 0.1: Version Control & Branching

Action: Ensure your project is under version control (Git). Create a new feature branch for this major refactoring (e.g., refactor/core-workflow-v1).
Test: N/A (Git operation).
Rationale: Isolate these significant changes.
Moment 0.2: Dependency Review & Addition (Chumsky)

Action: Review Cargo.toml. You've already added chumsky in your Point 3 conceptual solution. Ensure it's there. No other new major dependencies (like nil-syntax) are needed for this part of the plan, as we're focusing on internal refactoring and your existing proposals first.
Test: cargo check to ensure dependencies resolve.
Rationale: Prepare for Installable parsing.
Phase 1: Standardizing Command Execution & Error Handling (Your Points 2 & 4)

This phase focuses on util.rs and how ng runs external commands.

Moment 1.1: Define UtilCommandError

Action (Code - src/util.rs or src/error_handler.rs): Implement the UtilCommandError enum as you designed in Point 4:
Rust

// In util.rs or a dedicated errors.rs
#[derive(Debug, thiserror::Error)]
pub enum UtilCommandError {
    #[error("Failed to spawn command '{command_str}': {io_error}")]
    SpawnFailed {
        command_str: String,
        io_error: std::io::Error,
    },
    #[error(
        "Command '{command_str}' exited with status {status_code}.\nStdout:\n{stdout}\nStderr:\n{stderr}"
    )]
    NonZeroStatus {
        command_str: String,
        status_code: String, // String to handle cases where code might not be available
        stdout: String,
        stderr: String,
    },
}
Test (Unit Test - src/util_test.rs or new error_handler_test.rs):
Create instances of SpawnFailed and NonZeroStatus.
Use format!("{}", err) to ensure the error messages are rendered as expected.
Verify that color-eyre (if already integrated for Result<()> in main.rs) can consume and display these errors nicely (you might need to ensure UtilCommandError implements std::error::Error).
Rationale: Establishes a structured way to report command failures internally.
Moment 1.2: Refactor util::run_cmd

Action (Code - src/util.rs): Modify util::run_cmd to return Result<std::process::Output, UtilCommandError> and populate the error variants correctly, as per your Point 4.
Rust

// In util.rs
pub fn run_cmd(command: &mut std::process::Command) -> Result<std::process::Output, UtilCommandError> {
    let command_str = format!("{:?}", command); // For error reporting
    debug!("Executing command: {:?}", command_str); // Keep your debug log

    let output = command.output().map_err(|e| UtilCommandError::SpawnFailed {
        command_str: command_str.clone(),
        io_error: e,
    })?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        warn!( // Keep your warn log
            "Command failed: {:?} - Exit Code: {:?}",
            command_str,
            output.status.code()
        );
        debug!("Stderr: {}", stderr);
        debug!("Stdout: {}", stdout);
        Err(UtilCommandError::NonZeroStatus {
            command_str,
            status_code: output.status.code().map_or_else(|| "unknown".to_string(), |c| c.to_string()),
            stdout,
            stderr,
        })
    } else {
        Ok(output)
    }
}
Test (Unit Test - src/util_test.rs):
Test run_cmd with a command that succeeds (e.g., echo "hello"). Assert Ok and check output.stdout.
Test run_cmd with a command that fails with non-zero status (e.g., false on Unix, or a script that exits 1). Assert Err(UtilCommandError::NonZeroStatus) and inspect the captured stdout, stderr, and status_code.
Test run_cmd with a command that cannot be spawned (e.g., "command_that_does_not_exist_ever"). Assert Err(UtilCommandError::SpawnFailed).
Rationale: Centralizes basic command execution logic and error structure.
Moment 1.3: Refactor util::run_cmd_inherit_stdio

Action (Code - src/util.rs): Modify util::run_cmd_inherit_stdio similarly to return Result<std::process::ExitStatus, UtilCommandError>. For NonZeroStatus in this case, stdout and stderr won't be captured in the error type (as they were inherited), so the error message would just indicate failure and status code.
Rust

// In util.rs
pub fn run_cmd_inherit_stdio(command: &mut std::process::Command) -> Result<std::process::ExitStatus, UtilCommandError> {
    let command_str = format!("{:?}", command);
    debug!("Executing command with inherited stdio: {:?}", command_str);

    let status = command
        .status() // .stdin(Stdio::inherit()) etc. are set by caller or default
        .map_err(|e| UtilCommandError::SpawnFailed {
            command_str: command_str.clone(),
            io_error: e,
        })?;

    if !status.success() {
        warn!(
            "Command with inherited stdio failed: {:?} - Exit Code: {:?}",
            command_str,
            status.code()
        );
        Err(UtilCommandError::NonZeroStatus { // Stdout/Stderr are empty as they were inherited
            command_str,
            status_code: status.code().map_or_else(|| "unknown".to_string(), |c| c.to_string()),
            stdout: String::new(),
            stderr: String::new(),
        })
    } else {
        Ok(status)
    }
}
Test (Unit Test - src/util_test.rs): Similar tests as for run_cmd, focusing on success/failure status. Output will go to test runner's stdout/stderr.
Rationale: Consistency for commands where output is directly shown to user.
Moment 1.4: Gradually Replace subprocess and old commands.rs::Command execution.

Action (Code - incremental refactoring across the codebase):
Identify one usage of the old commands.rs::Command (which uses subprocess::Exec).
Refactor that specific call site to use std::process::Command built up and then executed via your new util::run_cmd or util::run_cmd_inherit_stdio.
Your existing commands::Command and commands::Build structs can remain as convenient builders, but their .run() / .run_capture() methods should internally delegate to the new util::run_cmd using std::process::Command. Example for commands::Command::run:
Rust

// src/commands.rs (conceptual change to your Command::run)
pub fn run(&self) -> Result<()> { // Assuming Result from color_eyre
    let mut cmd_builder = std::process::Command::new(&self.command);
    cmd_builder.args(&self.args);
    // ... handle self.elevate by prepending "sudo" if needed ...
    // (The sudo logic with --preserve-env in your commands.rs is complex;
    //  you'll need to replicate that with std::process::Command)

    if let Some(m) = &self.message {
        info!("{}", m);
    }
    debug!("Preparing to run: {:?}", cmd_builder);

    if !self.dry {
        // Use util::run_cmd_inherit_stdio if output is meant for user directly
        // or util::run_cmd if output needs to be captured/suppressed.
        // This example assumes inherit_stdio for simplicity here.
        util::run_cmd_inherit_stdio(&mut cmd_builder)
            .map_err(|e| { // Convert UtilCommandError to color_eyre::Report
                eyre!("Command execution failed: {}", e)
            })
            .map(|_| ()) // Discard ExitStatus if Ok
    } else {
        info!("Dry-run: Would execute: {:?}", cmd_builder);
        Ok(())
    }
}
Test (Integration Test): For the specific ng subcommand whose underlying command call you just refactored, run an existing integration test or add a new one to ensure it still behaves identically.
Repeat: Iterate this process for all usages of subprocess::Exec and internal execution methods of commands::Command and commands::Build. This is a significant, but crucial, part of standardizing.
Focus on commands.rs::Build::run: This one is complex due to nom piping. Your "piped commands" helper concept from Point 2 will be needed here.
Rust

// Conceptual util::run_piped_commands (Your Point 2)
// In util.rs
pub fn run_piped_commands(
    mut cmd1_builder: std::process::Command, // e.g., nix build ...
    mut cmd2_builder: std::process::Command, // e.g., nom --json
) -> Result<std::process::Output, UtilCommandError> {
    let cmd1_str = format!("{:?}", cmd1_builder);
    let cmd2_str = format!("{:?}", cmd2_builder);
    debug!("Executing piped command: {:?} | {:?}", cmd1_str, cmd2_str);

    let child1 = cmd1_builder
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped()) // Capture stderr from cmd1 too
        .spawn()
        .map_err(|e| UtilCommandError::SpawnFailed { command_str: cmd1_str.clone(), io_error: e })?;

    let child1_stdout = child1.stdout.ok_or_else(|| UtilCommandError::SpawnFailed {
        command_str: cmd1_str.clone(),
        io_error: std::io::Error::new(std::io::ErrorKind::Other, "cmd1 stdout missing"),
    })?;

    let child2 = cmd2_builder
        .stdin(child1_stdout)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| UtilCommandError::SpawnFailed { command_str: cmd2_str, io_error: e })?;

    let output2 = child2.wait_with_output().map_err(|e| UtilCommandError::SpawnFailed {
        command_str: format!("wait_with_output for {:?}", cmd2_builder), // A bit generic
        io_error: e,
    })?;

    // Optionally, capture stderr from child1 if needed
    // let child1_stderr_output = child1.wait_with_output()... (careful, stdout already taken)

    if !output2.status.success() {
        Err(UtilCommandError::NonZeroStatus {
            command_str: format!("{:?} | {:?}", cmd1_builder, cmd2_builder),
            status_code: output2.status.code().map_or_else(|| "unknown".to_string(), |c| c.to_string()),
            stdout: String::from_utf8_lossy(&output2.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output2.stderr).into_owned(),
        })
    } else {
        Ok(output2)
    }
}
Then commands.rs Build::run would use this.
Rationale: Achieve unified command execution and error handling across ng.
Phase 2: Robust Installable Attribute Parsing (Your Point 3)

This targets src/installable.rs.

Moment 2.1: Implement attribute_segment and attribute_path_parser using chumsky

Action (Code - src/installable.rs): Implement the chumsky parsers as outlined in your Point 3. Start with basic identifiers and quoted strings. Antiquotations can be a later enhancement if truly needed for attribute paths (they are rare in that context).
Rust

// In installable.rs
use chumsky::prelude::*;

fn nix_identifier_char_first() -> impl Parser<char, char, Error = Simple<char>> {
    filter(|c: &char| c.is_ascii_alphabetic() || *c == '_')
}

fn nix_identifier_char_rest() -> impl Parser<char, char, Error = Simple<char>> {
    filter(|c: &char| c.is_ascii_alphanumeric() || *c == '_' || *c == '\'' || *c == '-')
}

fn nix_identifier() -> impl Parser<char, String, Error = Simple<char>> {
    nix_identifier_char_first()
        .chain(nix_identifier_char_rest().repeated())
        .collect()
}

fn quoted_string_content() -> impl Parser<char, char, Error = Simple<char>> {
    filter(|c: &char| *c != '"' && *c != '\\').or(just('\\').ignore_then(any()))
}

fn quoted_string() -> impl Parser<char, String, Error = Simple<char>> {
    just('"')
        .ignore_then(quoted_string_content().repeated().collect())
        .then_ignore(just('"'))
}

fn attribute_segment() -> impl Parser<char, String, Error = Simple<char>> {
    nix_identifier().or(quoted_string())
}

pub fn attribute_path_parser() -> impl Parser<char, Vec<String>, Error = Simple<char>> {
    attribute_segment()
        .separated_by(just('.'))
        .allow_leading() // Check if Nix allows leading/trailing dots in attr paths (usually not)
        .allow_trailing()
}
Test (Unit Test - src/installable_test.rs or within installable.rs):
Test nix_identifier() with valid (a, a_b, _a, a-b) and invalid (1a, .a) identifiers.
Test quoted_string() with simple strings ("foo"), strings with spaces ("foo bar"), strings with dots ("foo.bar"), and escaped quotes ("foo\"bar").
Test attribute_segment() with both identifiers and quoted strings.
Test attribute_path_parser() with:
"foo" -> Ok(vec!["foo"])
"foo.bar" -> Ok(vec!["foo", "bar"])
"foo.\"bar.baz\"" -> Ok(vec!["foo", "bar.baz"])
"\"foo.bar\".baz" -> Ok(vec!["foo.bar", "baz"])
"" (empty string) -> Ok(vec![]) or decide desired behavior (your current parse_attribute returns empty vec). Chumsky might need adjustment for empty leading. Consider attribute_segment().separated_by(just('.')).at_least(0).collect() or similar if empty is valid. Or, wrap it:
Rust

pub fn parse_attribute_chumsky(s: &str) -> Result<Vec<String>, Vec<Simple<char>>> {
    if s.is_empty() { return Ok(Vec::new()); }
    attribute_path_parser().parse(s)
}
Invalid paths (e.g., "foo.", ".foo", "foo..bar") and check error reporting.
Rationale: Creates a robust parser for a well-defined part of your Installable structure.
Moment 2.2: Integrate Chumsky Parser into Installable::from_arg_matches and Installable::Flake etc.

Action (Code - src/installable.rs): Replace calls to the old parse_attribute with your new parse_attribute_chumsky (or whatever you name it) within Installable::from_arg_matches and where flake attributes are parsed from strings.
Modify join_attribute to correctly quote segments if they contain . or other characters that would make them invalid as unquoted Nix identifiers, ensuring symmetry with the parser.
Test (Unit/Integration):
Re-run existing tests for Installable::from_arg_matches that involve attribute paths.
Add new tests specifically targeting Installable creation with complex attribute paths (e.g., ng os switch .#foo."bar.baz" if your CLI parsing allows this directly, or test the internal Installable::Flake { attribute: ... } construction).
Test Installable::to_args() to ensure it correctly reconstructs the attribute path string using join_attribute.
Rationale: Uses the new robust parser in the actual program logic.
This concludes Part 1. We've laid a very strong foundation by:

Standardizing command execution and basic error reporting.
Making Installable attribute parsing robust.
Preparing for the major workflow refactor.
Crucially, every small change is testable. The next part will build upon this by implementing the PlatformRebuildStrategy and execute_rebuild_workflow, and then start integrating deeper Nix analysis.

## Part 1- Code
This will be a conceptual representation of the code, focusing on structure, new types, and the initial refactoring steps. You'll need to fill in details, handle all use statements, and, of course, thoroughly test and debug as you implement.

This first part will cover:

Setting up new error types and refactoring util.rs for command execution.
Refactoring installable.rs with chumsky.
Defining the core workflow types (CommonRebuildArgs, OperationContext).
Defining the PlatformRebuildStrategy trait and a skeletal execute_rebuild_workflow.
An initial implementation of NixosPlatformStrategy.
Modifying interface.rs and nixos.rs to start using the new workflow for ng os switch.
Ensuring ErrorHandler is callable.
I will indicate new files or significant refactors. For brevity, I will omit some existing code that doesn't directly change in this phase and focus on the new/modified structures. Assume necessary use statements are added.

Part 1 of 3: Foundational Codebase Structure and Initial Refactoring

1. Cargo.toml (Ensure chumsky is present)

Make sure chumsky (and thiserror if not already used beyond your proposal) are in your [dependencies]. Other oxalica/nil dependencies will be for later parts.

Ini, TOML

# Cargo.toml
# ... existing dependencies ...
[dependencies]
# ...
chumsky = "0.9" # Or latest compatible version
thiserror = "1.0" # If not already present and used
# ...
# Ensure color-eyre, clap, tracing, etc. are as they are in your provided Cargo.toml
2. New/Refactored: src/util.rs (Your Points 2 & 4)

This module centralizes command execution and defines the basic error type for it.

Rust

// src/util.rs
use color_eyre::eyre::{Result, WrapErr, eyre};
use std::process::{Command, Output, ExitStatus, Stdio};
use std::path::{Path, PathBuf};
use std::ffi::OsStr;
use tempfile::TempDir;
use tracing::{debug, warn};

// (Keep existing get_nix_version, MaybeTempPath, get_hostname, add_verbosity_flags, is_stdout_tty)
// ... (your existing util functions) ...

#[derive(Debug, thiserror::Error)]
pub enum UtilCommandError {
    #[error("Failed to spawn command '{command_str}': {io_error}")]
    SpawnFailed {
        command_str: String,
        io_error: std::io::Error,
    },
    #[error(
        "Command '{command_str}' exited with status {status_code}.\nStdout:\n{stdout}\nStderr:\n{stderr}"
    )]
    NonZeroStatus {
        command_str: String,
        status_code: String,
        stdout: String,
        stderr: String,
    },
    #[error("Command '{command_str}' (inheriting stdio) exited with status {status_code}.")]
    InheritedNonZeroStatus { // Variant for run_cmd_inherit_stdio
        command_str: String,
        status_code: String,
    },
}

pub fn run_cmd(command: &mut Command) -> Result<Output, UtilCommandError> {
    let command_str = format!("{:?}", command);
    debug!("Executing command: {:?}", command_str);

    let output = command.output().map_err(|e| UtilCommandError::SpawnFailed {
        command_str: command_str.clone(),
        io_error: e,
    })?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        warn!(
            "Command failed: {} - Exit Code: {:?}",
            command_str,
            output.status.code()
        );
        debug!("run_cmd Stderr: {}", stderr);
        debug!("run_cmd Stdout: {}", stdout);
        Err(UtilCommandError::NonZeroStatus {
            command_str,
            status_code: output.status.code().map_or_else(|| "unknown".to_string(), |c| c.to_string()),
            stdout,
            stderr,
        })
    } else {
        Ok(output)
    }
}

pub fn run_cmd_inherit_stdio(command: &mut Command) -> Result<ExitStatus, UtilCommandError> {
    let command_str = format!("{:?}", command);
    debug!("Executing command with inherited stdio: {:?}", command_str);

    let status = command
        .status() // Assumes .stdin(Stdio::inherit()) etc. are set by caller if needed
        .map_err(|e| UtilCommandError::SpawnFailed {
            command_str: command_str.clone(),
            io_error: e,
        })?;

    if !status.success() {
        warn!(
            "Command with inherited stdio failed: {} - Exit Code: {:?}",
            command_str,
            status.code()
        );
        Err(UtilCommandError::InheritedNonZeroStatus {
            command_str,
            status_code: status.code().map_or_else(|| "unknown".to_string(), |c| c.to_string()),
        })
    } else {
        Ok(status)
    }
}

// Your conceptual manage_out_path from Point 1 of your plan
// This seems like a good utility to have.
pub fn manage_out_path(out_link_opt: Option<&PathBuf>) -> Result<Box<dyn MaybeTempPath>> {
    match out_link_opt {
        Some(p) => {
            // Ensure parent directory exists if a specific out_link is given
            if let Some(parent_dir) = p.parent() {
                std::fs::create_dir_all(parent_dir)
                    .wrap_err_with(|| format!("Failed to create parent directory for out-link: {:?}", parent_dir))?;
            }
            Ok(Box::new(p.clone()))
        }
        None => {
            let temp_dir = TempDir::new_in(std::env::temp_dir()) // Or specific temp location
                .wrap_err("Failed to create temporary directory for build output")?;
            let path = temp_dir.path().join("result");
            Ok(Box::new((path, temp_dir)))
        }
    }
}

// (Keep your existing test module util_test.rs and add tests for new error variants)
3. Refactored: src/installable.rs (Your Point 3)

Incorporating chumsky for parse_attribute.

Rust

// src/installable.rs
use std::path::PathBuf;
use std::{env, fs};
use chumsky::prelude::*;
use clap::{Arg, ArgAction, Args, FromArgMatches}; // Assuming these are used as before
use color_eyre::owo_colors::OwoColorize;          // Assuming used as before

// (Keep your Installable Enum definition as is)
#[derive(Debug, Clone)]
pub enum Installable {
    Flake {
        reference: String,
        attribute: Vec<String>,
    },
    File {
        path: PathBuf,
        attribute: Vec<String>,
    },
    Store {
        path: PathBuf,
    },
    Expression {
        expression: String,
        attribute: Vec<String>,
    },
}


// --- Chumsky Parsers for Attribute Paths ---
fn nix_identifier_char_first() -> impl Parser<char, char, Error = Simple<char>> {
    filter(|c: &char| c.is_ascii_alphabetic() || *c == '_')
}

fn nix_identifier_char_rest() -> impl Parser<char, char, Error = Simple<char>> {
    filter(|c: &char| c.is_ascii_alphanumeric() || *c == '_' || *c == '\'' || *c == '-')
}

fn nix_identifier() -> impl Parser<char, String, Error = Simple<char>> {
    nix_identifier_char_first()
        .chain(nix_identifier_char_rest().repeated())
        .collect()
}

fn quoted_string_content() -> impl Parser<char, char, Error = Simple<char>> {
    filter(|c: &char| *c != '"' && *c != '\\').or(just('\\').ignore_then(any()))
}

fn quoted_string() -> impl Parser<char, String, Error = Simple<char>> {
    just('"')
        .ignore_then(quoted_string_content().repeated().collect())
        .then_ignore(just('"'))
}

fn attribute_segment() -> impl Parser<char, String, Error = Simple<char>> {
    nix_identifier().or(quoted_string())
}

pub fn attribute_path_parser() -> impl Parser<char, Vec<String>, Error = Simple<char>> {
    attribute_segment()
        .separated_by(just('.'))
        .allow_leading() // Consider if Nix allows this, if not, remove.
        .allow_trailing()// Consider if Nix allows this, if not, remove.
        .at_least(0) // To handle empty string correctly if it should parse to empty Vec
        .collect()
}

// New robust parse_attribute function
pub fn parse_attribute_robust(s: &str) -> Result<Vec<String>, Vec<Simple<char>>> {
    if s.is_empty() {
        return Ok(Vec::new());
    }
    attribute_path_parser().parse(s)
}

// Keep your old parse_attribute for now, or replace its usages one by one
// For example, in Installable::from_arg_matches:
// attribute: parse_attribute_robust(elems.next().map(|s| s.to_string()).unwrap_or_default())
//     .unwrap_or_else(|_err| {
//         // Handle chumsky parse error, maybe log it and fallback or return an error
//         tracing::warn!("Failed to parse attribute string '{}' robustly, falling back or erroring.", s_val);
//         // Fallback to old parser or return a clap::Error
//         super::installable::parse_attribute(s_val) // old parser
//     }),

// (The rest of Installable impl, FromArgMatches, Args impl, to_args, join_attribute, str_kind, tests)
// IMPORTANT: Update join_attribute to be the inverse of the chumsky parser,
// correctly quoting segments that contain '.' or would be invalid identifiers.
// Your existing join_attribute is a good start.
// ...

// Example of replacing in FromArgMatches (simplified)
impl FromArgMatches for Installable {
    fn from_arg_matches(matches: &clap::ArgMatches) -> std::result::Result<Self, clap::Error> {
        let mut matches = matches.clone();
        Self::from_arg_matches_mut(&mut matches)
    }

    fn from_arg_matches_mut(matches: &mut clap::ArgMatches) -> std::result::Result<Self, clap::Error> {
        let installable_str_opt = matches.get_one::<String>("installable").cloned();
        let file = matches.get_one::<String>("file").cloned();
        let expr = matches.get_one::<String>("expr").cloned();

        // Robust attribute parsing helper
        let parse_attr_for_installable = |attr_str_opt: Option<String>| -> Vec<String> {
            let attr_s = attr_str_opt.unwrap_or_default();
            parse_attribute_robust(&attr_s).unwrap_or_else(|parse_errs| {
                // Convert chumsky errors to a string or log them
                let err_msg = parse_errs.into_iter().map(|e| e.to_string()).collect::<Vec<_>>().join(", ");
                tracing::warn!("Failed to robustly parse attribute '{}': {}. Falling back to simple parsing.", attr_s, err_msg);
                // Fallback to your old simple parse_attribute for now during transition, or error out
                crate::installable::parse_attribute(&attr_s) // Assuming old one is still here
            })
        };

        if let Some(i_str) = &installable_str_opt {
            if let Ok(p) = fs::canonicalize(i_str) {
                if p.starts_with("/nix/store") {
                    return Ok(Self::Store { path: p });
                }
            }
        }

        if let Some(f_path) = file {
            return Ok(Self::File {
                path: PathBuf::from(f_path),
                attribute: parse_attr_for_installable(installable_str_opt),
            });
        }

        if let Some(e_str) = expr {
            return Ok(Self::Expression {
                expression: e_str,
                attribute: parse_attr_for_installable(installable_str_opt),
            });
        }

        if let Some(i_str) = installable_str_opt {
            let mut elems = i_str.splitn(2, '#');
            let reference = elems.next().unwrap_or(".").to_owned(); // Default to "." if empty before #
            let attr_part = elems.next().map(|s| s.to_string());
            return Ok(Self::Flake {
                reference,
                attribute: parse_attr_for_installable(attr_part),
            });
        }

        // ... (rest of your env var fallback logic, ensuring parse_attr_for_installable is used) ...
        // For brevity, not reproducing all env var logic here.
        // Ensure each place that extracts an attribute string uses the new robust parser.

        Err(clap::Error::new(clap::error::ErrorKind::MissingRequiredArgument)) // Or your existing error
    }
    // ... update_from_arg_matches ...
}

// Keep your existing parse_attribute function for now for fallback or until fully migrated
pub fn parse_attribute<S>(s: S) -> Vec<String>
where S: AsRef<str>, { /* ... your existing simple splitter ... */ }

pub fn join_attribute<I>(attribute: I) -> String // Ensure this correctly quotes segments
where I: IntoIterator, I::Item: AsRef<str> { /* ... your existing improved join ... */ }


// (Your existing tests, add new ones for chumsky parser as described in plan)
#[cfg(test)]
mod tests_installable {
    use super::*;
    // ... your existing tests for Installable ...

    #[test]
    fn test_parse_attribute_robust_chumsky() {
        assert_eq!(parse_attribute_robust("foo.bar"), Ok(vec!["foo".to_string(), "bar".to_string()]));
        assert_eq!(parse_attribute_robust("foo.\"bar.baz\""), Ok(vec!["foo".to_string(), "bar.baz".to_string()]));
        assert_eq!(parse_attribute_robust("\"foo.bar\".baz"), Ok(vec!["foo.bar".to_string(), "baz".to_string()]));
        assert_eq!(parse_attribute_robust("foo"), Ok(vec!["foo".to_string()]));
        assert_eq!(parse_attribute_robust("\"foo\""), Ok(vec!["foo".to_string()]));
        assert_eq!(parse_attribute_robust(""), Ok(vec![]));
        // Test error cases if your parser is configured to error on them, e.g. "foo."
        // assert!(parse_attribute_robust("foo.").is_err());
    }
}

You'll need to carefully integrate parse_attribute_robust into Installable::from_arg_matches_mut and handle potential Err from chumsky (e.g., by logging and falling back to the old parser during transition, or by converting to clap::Error). The example above shows one way to handle the fallback.

4. New: src/workflow_types.rs

Rust

// src/workflow_types.rs
use crate::installable::Installable;
use std::path::PathBuf;
use std::ffi::OsString; // For extra_build_args

// This struct centralizes common arguments for any rebuild-like operation.
// It's derived from CLI parsing and environment variables.
#[derive(Debug, Clone)]
pub struct CommonRebuildArgs {
    pub installable: Installable,
    pub no_preflight: bool,
    pub strict_lint: bool,
    pub medium_checks: bool, // Renamed from 'medium' for clarity
    pub full_checks: bool,   // Renamed from 'full' for clarity
    pub dry_run: bool,       // Renamed from 'dry'
    pub ask_confirmation: bool, // Renamed from 'ask'
    pub no_nom: bool,
    pub out_link: Option<PathBuf>,
    pub clean_after: bool,   // Renamed from 'clean'
    // Ensure extra_build_args is Vec<String> or Vec<OsString> as needed by nix build
    pub extra_build_args: Vec<OsString>,
}
5. New: src/context.rs (Initial Stub)

Rust

// src/context.rs
use crate::workflow_types::CommonRebuildArgs;
use crate::interface::UpdateArgs; // Assuming UpdateArgs is still in interface.rs
// use crate::nix_interface::NixInterface; // Will be uncommented later
// use crate::error_handler::ErrorHandler; // Will be uncommented later

// Holds the operational context for a given ng command run.
// Passed around to workflow steps and strategies.
#[derive(Debug)] // Add Clone if needed, but refs might be better
pub struct OperationContext<'a> {
    pub common_args: &'a CommonRebuildArgs,
    pub update_args: &'a UpdateArgs, // For flake update logic
    pub verbose_count: u8,
    // pub nix_interface: NixInterface<'a>, // Placeholder for Part 3
    // pub error_handler: ErrorHandler<'a>, // Placeholder for Part 3
    // Potentially:
    // pub config: &'a NgConfig, // Loaded ng configuration file
}

impl<'a> OperationContext<'a> {
    pub fn new(
        common_args: &'a CommonRebuildArgs,
        update_args: &'a UpdateArgs,
        verbose_count: u8,
        // nix_interface: NixInterface<'a>,
        // error_handler: ErrorHandler<'a>,
    ) -> Self {
        Self {
            common_args,
            update_args,
            verbose_count,
            // nix_interface,
            // error_handler,
        }
    }
}
6. New: src/workflow_strategy.rs

Rust

// src/workflow_strategy.rs
use crate::installable::Installable;
use crate::Result; // from color_eyre
use crate::context::OperationContext;
use crate::workflow_types::CommonRebuildArgs; // If it lives here
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivationMode {
    Switch, // Activate and make default (if applicable)
    Boot,   // Prepare for next boot (e.g., NixOS, Darwin)
    Test,   // Activate but don't make default (e.g., NixOS, Darwin)
    Build,  // Just build, no activation (replaces is_just_build_variant)
}

pub trait PlatformRebuildStrategy {
    // Platform-specific arguments, typically the clap::Args struct for the platform's rebuild command
    // e.g., crate::interface::OsRebuildArgs
    type PlatformArgs: std::fmt::Debug;

    fn name(&self) -> &str; // e.g., "NixOS", "HomeManager", "Darwin"

    // Initial platform-specific checks (e.g., root user for NixOS).
    // Called before any shared pre-flight checks.
    fn pre_rebuild_hook(&self, op_ctx: &OperationContext, platform_args: &Self::PlatformArgs) -> Result<()>;

    // Determines the final Nix Installable target for the build operation.
    // This incorporates hostname, user-specified attributes, specialisations, etc.
    fn get_toplevel_installable(&self, op_ctx: &OperationContext, platform_args: &Self::PlatformArgs) -> Result<Installable>;

    // Path to the current system profile for diffing against the new build.
    // Returns None if not applicable or not found.
    fn get_current_profile_path(&self, op_ctx: &OperationContext, platform_args: &Self::PlatformArgs) -> Option<PathBuf>;

    // Performs the platform-specific activation.
    // `built_profile_path` is the /nix/store path of the newly built configuration.
    fn activate_configuration(
        &self,
        op_ctx: &OperationContext,
        platform_args: &Self::PlatformArgs,
        built_profile_path: &Path,
        activation_mode: &ActivationMode,
    ) -> Result<()>;

    // Final platform-specific actions after activation/cleanup.
    fn post_rebuild_hook(&self, op_ctx: &OperationContext, platform_args: &Self::PlatformArgs) -> Result<()>;

    // Optional: For platform-specific pre-flight checks that don't fit the shared model.
    // fn platform_specific_pre_flight_checks(&self, op_ctx: &OperationContext, platform_args: &Self::PlatformArgs) -> Result<()>;
}
7. New: src/workflow_executor.rs (Skeletal)

Rust

// src/workflow_executor.rs
use crate::workflow_strategy::{PlatformRebuildStrategy, ActivationMode};
use crate::workflow_types::CommonRebuildArgs;
use crate::context::OperationContext;
use crate::installable::Installable;
use crate::util::{self, MaybeTempPath}; // For manage_out_path
use crate::Result; // from color_eyre
use std::path::{Path, PathBuf};
use tracing::{info, warn, error, debug};
use color_eyre::eyre::{bail, eyre, WrapErr};

// Stubs for functions to be implemented/refactored later or in other modules
// mod pre_flight { /* ... */ pub fn run_shared_pre_flight_checks(...) -> Result<()> { Ok(()) } } // Placeholder
// mod nix_interface_stub { pub fn build_configuration(...) -> Result<PathBuf> { Ok(PathBuf::from("/tmp/dummy")) } } // Placeholder
// mod update_logic { pub fn update_flake_inputs(...) -> Result<()> { Ok(()) } } // Placeholder
// mod cleanup_logic { pub fn perform_manual_cleanup(...) -> Result<()> { Ok(()) } } // Placeholder
// mod diff_logic { pub fn show_diff(...) -> Result<()> { Ok(()) } } // Placeholder

pub fn execute_rebuild_workflow<S: PlatformRebuildStrategy>(
    platform_strategy: &S,
    op_ctx: &OperationContext,
    platform_args: &S::PlatformArgs, // The specific args like OsRebuildArgs
    activation_mode: ActivationMode,
) -> Result<()> {
    let workflow_span = tracing::info_span!("execute_rebuild_workflow", platform = platform_strategy.name(), mode = ?activation_mode);
    let _enter = workflow_span.enter();

    info!("🚀 Starting {} rebuild workflow ({:?})...", platform_strategy.name(), activation_mode);

    // 1. Platform-specific pre-rebuild hook
    platform_strategy.pre_rebuild_hook(op_ctx, platform_args)
        .wrap_err_with(|| format!("{} pre-rebuild hook failed", platform_strategy.name()))?;
    debug!("Pre-rebuild hook for {} completed.", platform_strategy.name());

    // 2. Shared Pre-flight checks (Git, Parse, Lint, Eval, DryRun) - STUBBED for Part 1 code focus
    if !op_ctx.common_args.no_preflight {
        info!("(Stub) Would run shared pre-flight checks now...");
        // In Part 2/3, this will call:
        // pre_flight::run_shared_pre_flight_checks(op_ctx, platform_strategy, platform_args, &get_checks_for_mode(op_ctx))?;
        debug!("Shared pre-flight checks for {} completed.", platform_strategy.name());
    } else {
        info!("[⏭️ Pre-flight] All checks skipped due to --no-preflight.");
    }

    // 3. Optional Flake Update - STUBBED
    if op_ctx.update_args.update || op_ctx.update_args.update_input.is_some() {
        info!("(Stub) Would update flake inputs now...");
        // In Part 2/3, this will call:
        // crate::update::update(&op_ctx.common_args.installable, op_ctx.update_args.update_input.clone())
        //    .wrap_err("Failed to update flake inputs")?;
        debug!("Flake input update for {} completed.", platform_strategy.name());
    }

    // 4. Get Toplevel Derivation
    let toplevel_installable = platform_strategy.get_toplevel_installable(op_ctx, platform_args)
        .wrap_err_with(|| format!("Failed to determine toplevel installable for {}", platform_strategy.name()))?;
    debug!("Resolved toplevel installable for {}: {:?}", platform_strategy.name(), toplevel_installable);

    // 5. Build Configuration - STUBBED (will use NixInterface)
    let out_path_manager = util::manage_out_path(op_ctx.common_args.out_link.as_ref())?;
    let built_profile_path: PathBuf; // Actual path to the /nix/store derivation

    info!("(Stub) Building configuration for {:?}...", &toplevel_installable);
    // In Part 2/3, this will be:
    // built_profile_path = op_ctx.nix_interface.build_configuration(
    // &toplevel_installable,
    // out_path_manager.get_path(),
    // &op_ctx.common_args.extra_build_args,
    // op_ctx.common_args.no_nom,
    // op_ctx.verbose_count,
    // )?;
    // For now, a dummy operation for testing flow:
    let dummy_target = out_path_manager.get_path();
    if !op_ctx.common_args.dry_run || activation_mode != ActivationMode::Build { // only create dummy if not pure build dry run
        std::fs::write(dummy_target, format!("content for {:?}", toplevel_installable))
            .wrap_err_with(|| format!("Failed to write dummy build file at {:?}", dummy_target))?;
        // In a real scenario, Nix build creates the out-link as a symlink.
        // We need to simulate that if out_link is not None. If out_link is None,
        // the result path is usually inside the temp dir.
        // This dummy logic needs to be realistic enough for tests of `activate_configuration`.
        // Let's assume `built_profile_path` is the direct result for now.
        // For a real build, `out_path_manager.get_path()` is the *link name*, and `built_profile_path` is what it points to.
        // For this stub, let's assume `built_profile_path` is the `out_path_manager.get_path()` itself.
        // This will need to be the actual /nix/store path after real build.
        built_profile_path = dummy_target.to_path_buf(); // This isn't quite right for a real build.
                                                         // A real build cmd: nix build --out-link <link_name> <installable>
                                                         // Then built_profile_path = fs::read_link(<link_name>)?
        info!("(Stub) Configuration 'built' to: {}", built_profile_path.display());
    } else {
        info!("(Stub) Dry run build for {:?}, no output created.", &toplevel_installable);
        // For dry run build, we might not have a path, or a conceptual one.
        // This makes testing activation tricky. Let's still assign a dummy.
        built_profile_path = PathBuf::from("/tmp/dummy-dry-run-build-result-not-actually-active");
    }


    // 6. Show Diff - STUBBED
    if activation_mode != ActivationMode::Build && !op_ctx.common_args.dry_run {
        if let Some(current_profile) = platform_strategy.get_current_profile_path(op_ctx, platform_args) {
            if current_profile.exists() {
                info!("(Stub) Would show diff between {} and {}", current_profile.display(), built_profile_path.display());
                // In Part 2/3:
                // diff_logic::show_diff(&current_profile, &built_profile_path, op_ctx.verbose_count)?;
            } else {
                info!("Current profile {} does not exist, skipping diff.", current_profile.display());
            }
        }
    }

    // 7. Optional Confirmation
    if op_ctx.common_args.ask_confirmation && activation_mode != ActivationMode::Build && !op_ctx.common_args.dry_run {
        info!("Confirmation for applying new configuration would be requested now.");
        if !dialoguer::Confirm::new()
            .with_prompt(format!("Apply the new {} configuration?", platform_strategy.name()))
            .default(false)
            .interact()?
        {
            bail!("User rejected the new configuration for {}.", platform_strategy.name());
        }
        info!("User confirmed action for {}.", platform_strategy.name());
    }

    // 8. Activate (if not dry run and not just a build variant)
    if activation_mode != ActivationMode::Build && !op_ctx.common_args.dry_run {
        info!("Attempting activation for {} (mode: {:?})...", platform_strategy.name(), activation_mode);
        platform_strategy.activate_configuration(op_ctx, platform_args, &built_profile_path, &activation_mode)
            .wrap_err_with(|| format!("Failed to activate {} configuration", platform_strategy.name()))?;
        info!("Activation for {} completed.", platform_strategy.name());
    } else if op_ctx.common_args.dry_run {
        info!("Dry-run: Skipping activation for {}.", platform_strategy.name());
    } else { // Build mode
        info!("Build-only mode: Result at {}", built_profile_path.display());
    }

    // 9. Optional Cleanup (triggered by --clean flag) - STUBBED
    if op_ctx.common_args.clean_after && activation_mode != ActivationMode::Build && !op_ctx.common_args.dry_run {
        info!("(Stub) Would perform manual cleanup requested by --clean flag...");
        // In Part 2/3:
        // cleanup_logic::perform_manual_cleanup(op_ctx, platform_strategy)?;
        debug!("Manual cleanup for {} completed.", platform_strategy.name());
    }

    // 10. Automatic Lazy Cleanup (if configured) - STUBBED for Part 3
    // if should_auto_clean(...) { perform_auto_clean(...); }

    // 11. Platform-specific post-rebuild hook
    platform_strategy.post_rebuild_hook(op_ctx, platform_args)
        .wrap_err_with(|| format!("{} post-rebuild hook failed", platform_strategy.name()))?;
    debug!("Post-rebuild hook for {} completed.", platform_strategy.name());

    info!("🏆 {} rebuild workflow ({:?}) finished successfully!", platform_strategy.name(), activation_mode);
    Ok(())
}

Test: Conceptual, as it needs strategies. Compile check.
Rationale: Central workflow orchestrator.
8. Initial: src/nixos_strategy.rs (or integrated into src/nixos.rs)

Rust

// src/nixos_strategy.rs (or move into src/nixos.rs)
use crate::workflow_strategy::{PlatformRebuildStrategy, ActivationMode};
use crate::context::OperationContext;
use crate::installable::Installable;
use crate::interface::OsRebuildArgs; // Your existing clap struct for 'nh os switch/boot/...'
use crate::util::{self, UtilCommandError};
use crate::Result; // from color_eyre
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand; // Alias to avoid conflict if Command struct exists
use tracing::{info, warn, debug};
use color_eyre::eyre::{bail, eyre, Context, Report};

pub struct NixosPlatformStrategy;

impl PlatformRebuildStrategy for NixosPlatformStrategy {
    type PlatformArgs = OsRebuildArgs;

    fn name(&self) -> &str { "NixOS" }

    fn pre_rebuild_hook(&self, _op_ctx: &OperationContext, platform_args: &Self::PlatformArgs) -> Result<()> {
        debug!("NixOSStrategy: pre_rebuild_hook");
        if !platform_args.bypass_root_check && nix::unistd::Uid::effective().is_root() {
            bail!("Do not run `ng os` as root. Sudo will be used internally if needed based on the operation.");
        }
        Ok(())
    }

    fn get_toplevel_installable(&self, op_ctx: &OperationContext, platform_args: &Self::PlatformArgs) -> Result<Installable> {
        debug!("NixOSStrategy: get_toplevel_installable");
        let hostname = platform_args.hostname.clone()
            .or_else(|| util::get_hostname().ok()) // Try to get system hostname
            .ok_or_else(|| eyre!("Hostname could not be determined and was not provided via --hostname. Needed for NixOS configuration attribute path."))?;

        let mut final_installable = op_ctx.common_args.installable.clone();
        match &mut final_installable {
            Installable::Flake { reference, attribute } => {
                if attribute.is_empty() { // Only default if user hasn't specified an attribute
                    attribute.push("nixosConfigurations".to_string());
                    attribute.push(hostname);
                }
                // The convention for nixos-rebuild is that the attribute path points to the
                // NixOS *system derivation* itself, which usually means something like:
                // nixosConfigurations.<hostname>.config.system.build.toplevel
                // However, your current `toplevel_for` in `nixos.rs` appends this.
                // Let's stick to appending it here if attribute is up to hostname.
                if attribute.last().map_or(false, |a| a == &hostname) {
                     attribute.extend(vec!["config".to_string(), "system".to_string(), "build".to_string(), "toplevel".to_string()]);
                }
                // TODO: Handle platform_args.specialisation and platform_args.no_specialisation here
                // to adjust the attribute path if a specialisation is targeted.
                // e.g., ...nixosConfigurations.myhost.specialisations.myspec.config.system.build.toplevel
            }
            Installable::File { attribute, .. } | Installable::Expression { attribute, .. } => {
                // For file/expr, if attr is empty, it implies the whole file/expr is the system derivation.
                // If attr is provided, it should point to the system derivation.
                // No automatic "toplevel" appending here unless it's a common pattern for your non-flake usage.
            }
            Installable::Store { .. } => { /* Store path is already a toplevel */ }
        }
        debug!("NixOS toplevel: {:?}", final_installable);
        Ok(final_installable)
    }

    fn get_current_profile_path(&self, _op_ctx: &OperationContext, _platform_args: &Self::PlatformArgs) -> Option<PathBuf> {
        Some(PathBuf::from("/run/current-system"))
    }

    fn activate_configuration(
        &self,
        op_ctx: &OperationContext,
        platform_args: &Self::PlatformArgs,
        built_profile_path: &Path, // This IS the /nix/store path of the system derivation
        activation_mode: &ActivationMode,
    ) -> Result<()> {
        debug!("NixOSStrategy: activate_configuration (mode: {:?}, profile: {})", activation_mode, built_profile_path.display());

        if op_ctx.common_args.dry_run {
            info!("Dry-run: Would activate NixOS ({:?}) with profile: {}", activation_mode, built_profile_path.display());
            return Ok(());
        }

        let action_str = match activation_mode {
            ActivationMode::Switch => "switch",
            ActivationMode::Boot => "boot",
            ActivationMode::Test => "test",
            ActivationMode::Build => return Ok(()), // Should be caught by workflow_executor
        };

        let switch_script_path = built_profile_path.join("bin/switch-to-configuration");
        if !switch_script_path.exists() {
            bail!("Activation script 'bin/switch-to-configuration' not found in built profile: {}", built_profile_path.display());
        }

        let mut cmd = StdCommand::new(if platform_args.bypass_root_check {
            // If bypassing root check, assume we are already root or script handles it
            switch_script_path.as_os_str().to_os_string()
        } else {
            "sudo".into() // Otherwise, use sudo
        });

        if !platform_args.bypass_root_check {
            cmd.arg(switch_script_path);
        }
        cmd.arg(action_str);

        info!("Executing NixOS activation: {:?}", cmd);
        util::run_cmd_inherit_stdio(&mut cmd)
            .map_err(|e| {
                let context_msg = format!("NixOS activation with action '{}' failed.", action_str);
                // TODO: Pass full UtilCommandError to ErrorHandler in Part 3 for richer detail
                match e {
                    UtilCommandError::InheritedNonZeroStatus { command_str, status_code } => {
                        eyre!("{}. Command: '{}', Status: {}", context_msg, command_str, status_code)
                    }
                    _ => eyre!("{}: {}", context_msg, e),
                }
            })
            .map(|_| ())?;

        info!("NixOS configuration ({:?}) activated successfully.", activation_mode);
        Ok(())
    }

    fn post_rebuild_hook(&self, _op_ctx: &OperationContext, _platform_args: &Self::PlatformArgs) -> Result<()> {
        debug!("NixOSStrategy: post_rebuild_hook");
        // Example: could print info about rebooting if kernel changed, etc.
        Ok(())
    }
}
Test (Unit Tests - src/nixos_test.rs or new nixos_strategy_test.rs):
Test pre_rebuild_hook for root check logic.
Test get_toplevel_installable with various Installable inputs, hostname, specialisation flags. Assert correct final Installable.
Test activate_configuration by asserting the command string it would build for switch, boot, test (mock out util::run_cmd_inherit_stdio or check Command before execution).
9. Modified: src/interface.rs

Rust

// src/interface.rs
// ...
use crate::workflow_types::CommonRebuildArgs as CoreCommonRebuildArgs; // Alias new one

// Keep your existing CommonArgs for CLI definition, but it will map to CoreCommonRebuildArgs
#[derive(Debug, Args, Clone)]
pub struct CommonArgs { // This is what Clap parses
    #[arg(long, global = true)]
    pub no_preflight: bool,
    #[arg(long, global = true)]
    pub strict_lint: bool,
    #[arg(long, global = true)]
    pub medium: bool, // Keep 'medium' and 'full' for CLI
    #[arg(long, global = true)]
    pub full: bool,
    #[arg(long, short = 'n', global = true)]
    pub dry: bool,
    #[arg(long, short, global = true)]
    pub ask: bool,
    #[arg(long, global = true)]
    pub no_nom: bool,
    #[arg(long, short, global = true)]
    pub out_link: Option<PathBuf>,
    #[arg(long, global = true)]
    pub clean: bool,
}

// Your existing CommonRebuildArgs from clap definition
#[derive(Debug, Args)]
pub struct CommonRebuildArgs { // This is what Clap parses
    #[command(flatten)]
    pub common: CommonArgs, // Uses the above clap-specific CommonArgs
    #[command(flatten)]
    pub installable: Installable,
    // Any other common args for rebuild that are not in CommonArgs yet.
    // For example, if extra_args was here before:
    // #[arg(last = true)]
    // pub extra_args: Vec<String>, // Or OsString
}


#[derive(Debug, Args)]
pub struct OsRebuildArgs { // This remains your clap definition
    #[command(flatten)]
    pub common: CommonRebuildArgs, // Uses clap-specific CommonRebuildArgs
    #[command(flatten)]
    pub update_args: UpdateArgs,
    #[arg(long, short = 'H', global = true)]
    pub hostname: Option<String>,
    #[arg(long, short)]
    pub specialisation: Option<String>,
    #[arg(long, short = 'S')]
    pub no_specialisation: bool,
    #[arg(last = true)]
    pub extra_args: Vec<std::ffi::OsString>, // Make this OsString
    #[arg(short = 'R', long, env = "NH_BYPASS_ROOT_CHECK")]
    pub bypass_root_check: bool,
}
// ... rest of interface.rs ...
The key is that OsRebuildArgs (and later HomeRebuildArgs, DarwinRebuildArgs) are what clap parses. Then, when constructing OperationContext, you map values from these clap-parsed structs into the new workflow_types::CommonRebuildArgs.

10. Modified: src/nixos.rs (OsArgs::run for Switch)

Rust

// src/nixos.rs
// ... (imports) ...
use crate::workflow_executor::execute_rebuild_workflow;
use crate::nixos_strategy::NixosPlatformStrategy; // Assuming it's in its own file
use crate::workflow_strategy::ActivationMode;
use crate::context::OperationContext;
use crate::workflow_types; // For CoreCommonRebuildArgs

// ... (keep OsSubcommand, toplevel_for (or move its logic to strategy), OsReplArgs, OsGenerationsArgs for now) ...
// ... (The old OsRebuildArgs::rebuild method will be phased out or its internal logic moved to the strategy)

impl interface::OsArgs {
    pub fn run(self, verbose_count: u8) -> Result<()> {
        std::env::set_var("NH_CURRENT_COMMAND", "os"); // Good to keep for env var logic
        match self.subcommand {
            OsSubcommand::Switch(cli_args) => {
                // Map from clap's interface::CommonRebuildArgs/CommonArgs to workflow_types::CommonRebuildArgs
                let core_common_args = workflow_types::CommonRebuildArgs {
                    installable: cli_args.common.installable.clone(),
                    no_preflight: cli_args.common.common.no_preflight,
                    strict_lint: cli_args.common.common.strict_lint,
                    medium_checks: cli_args.common.common.medium,
                    full_checks: cli_args.common.common.full,
                    dry_run: cli_args.common.common.dry,
                    ask_confirmation: cli_args.common.common.ask,
                    no_nom: cli_args.common.common.no_nom,
                    out_link: cli_args.common.common.out_link.clone(),
                    clean_after: cli_args.common.common.clean,
                    extra_build_args: cli_args.extra_args.clone(),
                };

                let op_ctx = OperationContext::new(
                    &core_common_args,
                    &cli_args.update_args,
                    verbose_count,
                    // nix_interface and error_handler will be added here in later phases
                );
                let strategy = NixosPlatformStrategy;
                execute_rebuild_workflow(
                    &strategy,
                    &op_ctx,
                    &cli_args, // Pass the full OsRebuildArgs as PlatformArgs
                    ActivationMode::Switch,
                )
            }
            OsSubcommand::Boot(args) => {
                warn!("'ng os boot' not yet fully refactored to new workflow. Using old logic or placeholder.");
                // TODO: Refactor similar to Switch, but with ActivationMode::Boot
                // For now, you might call a simplified version of old logic or just error
                // args.rebuild(OsRebuildVariant::Boot, verbose_count) // Old call
                Err(eyre!("'boot' subcommand not fully refactored yet."))
            }
            OsSubcommand::Test(args) => {
                warn!("'ng os test' not yet fully refactored to new workflow.");
                // TODO: Refactor similar to Switch, but with ActivationMode::Test
                Err(eyre!("'test' subcommand not fully refactored yet."))
            }
            OsSubcommand::Build(args) => {
                 warn!("'ng os build' not yet fully refactored to new workflow.");
                // TODO: Refactor similar to Switch, but with ActivationMode::Build
                // This one won't call activate_configuration.
                // Your current build implementation can be a guide.
                if args.common.common.ask || args.common.common.dry {
                     warn!("`--ask` and `--dry` have no effect for `nh os build` with old logic path");
                }
                // args.rebuild(OsRebuildVariant::Build, verbose_count) // Old call
                Err(eyre!("'build' subcommand not fully refactored yet."))
            }
            OsSubcommand::Repl(args) => args.run(verbose_count), // To be refactored in Phase 10
            OsSubcommand::Info(args) => args.info(),             // To be refactored in Phase 10
        }
    }
}

// Remove or comment out the old OsRebuildArgs::rebuild method as its logic is moved
// to the strategy and workflow_executor.
// Or, keep it for the subcommands not yet refactored, and make the refactored ones
// (like Switch) NOT call it.

// Keep run_parallel_parse_check for now. It will be refactored into a PreFlightCheck
// in Part 2 of the plan (Moment 6.3).
// fn run_parallel_parse_check(&self, verbose_count: u8) -> Result<(), String> { ... }
Test (Integration assert_cmd):
Run ng os switch --flake .#yourTestSystemName (using a simple, valid test flake).
Assert: The command completes successfully (if activation script logic is correct in strategy). Logs from execute_rebuild_workflow and NixosPlatformStrategy should appear. A dummy file at the out_link (or temp path) should be created by the stubbed build step.
Modify your test NixOS configuration to cause a build failure (e.g., undefined variable).
Assert: ng os switch fails. The error reported should come through color_eyre. (More detailed error reporting via ErrorHandler will come when the build step is fully integrated with NixInterface and ErrorHandler).
Test ng os switch --dry-run.
Assert: Logs should indicate "Dry-run: Skipping activation...". No activation should occur.
Test ng os switch --ask.
Assert: It prompts. Test 'no' (should bail) and 'yes' (should proceed).
11. Modified src/lib.rs & src/main.rs

Declare new modules if you created new files (e.g., workflow_types.rs, context.rs, workflow_strategy.rs, workflow_executor.rs, nixos_strategy.rs).
Rust

// src/lib.rs
// ...
pub mod workflow_types;
pub mod context;
pub mod workflow_strategy;
pub mod workflow_executor;
pub mod nixos_strategy; // if in its own file
// pub mod nix_interface; // Stub for later
// pub mod nix_analyzer; // Stub for later
// ...
main.rs probably doesn't need direct changes for these internal modules.

# Part 2
## Part 2- Plan


Show thinking
Okay, let's proceed with Part 2 of 3 of the detailed, incremental moment plan for refactoring ng. Part 1 focused on standardizing command execution, error types, and Installable parsing. Part 2 will build the core PlatformRebuildStrategy and execute_rebuild_workflow, migrate ng os switch to it, and start integrating pre-flight checks, all with extremely incremental testing.

Recap of Key Goals for Part 2:

Establish the generic rebuild workflow (execute_rebuild_workflow and PlatformRebuildStrategy).
Refactor ng os switch (for NixOS) as the first consumer of this new workflow.
Incrementally integrate existing pre-flight checks (Git, Parse, Lint) into the workflow.
Continue standardizing command execution using util::run_cmd.
Lay down stubs/placeholders for the NixInterface and ensure the enhanced ErrorHandler is used by the new workflow.
Maintain an extreme focus on incremental testing.
Part 2: Implementing Core Workflow and Migrating ng os switch - Step-by-Step Moment Plan

Phase 3: Defining the Core Workflow Abstractions (Your Point 1)

Moment 3.1: Define CommonRebuildArgs and OperationContext

Action (Code - e.g., src/workflow_args.rs or within src/interface.rs if it makes sense for CLI parsing, then a separate OperationContext in src/workflow.rs):
Define CommonRebuildArgs struct: This should consolidate common arguments from your existing interface::CommonArgs, interface::CommonRebuildArgs (like dry, ask, no_preflight, strict_lint, medium, full, no_nom, out_link, clean, and the installable::Installable).
Define OperationContext struct: This will be created after initial CLI parsing and will carry runtime context.
Rust

// e.g., in src/workflow_types.rs or similar
use crate::installable::Installable;
use std::path::PathBuf;

#[derive(Debug, Clone)] // Make it cloneable for strategies if they need to own parts
pub struct CommonRebuildArgs {
    pub installable: Installable,
    pub no_preflight: bool,
    pub strict_lint: bool,
    pub medium_checks: bool, // Renamed from medium for clarity
    pub full_checks: bool,   // Renamed from full for clarity
    pub dry_run: bool,       // Renamed from dry
    pub ask_confirmation: bool, // Renamed from ask
    pub no_nom: bool,
    pub out_link: Option<PathBuf>,
    pub clean_after: bool,   // Renamed from clean
    pub extra_build_args: Vec<String>, // Assuming this is common
}

// Potentially in src/context.rs or workflow.rs
use crate::nix_interface::NixInterface; // Placeholder for now
use crate::error_handler::ErrorHandler; // Placeholder for now

pub struct OperationContext<'a> {
    pub common_args: &'a CommonRebuildArgs,
    pub update_args: &'a crate::interface::UpdateArgs, // From your existing structure
    pub verbose_count: u8,
    // pub nix_interface: NixInterface<'a>, // To be added in later phase/part
    // pub error_handler: ErrorHandler<'a>, // To be added
    // Potentially other shared resources like a common TempDir manager if not handled by out_link logic
}
Test: N/A (struct definitions). Compile check.
Rationale: Centralizes common data structures used by the workflow.
Moment 3.2: Define PlatformRebuildStrategy Trait

Action (Code - e.g., src/workflow_strategy.rs): Define the trait as per your Point 1, including type Args.
Rust

// e.g., in src/workflow_strategy.rs
use crate::installable::Installable;
use crate::Result; // from color_eyre
use std::path::{Path, PathBuf};
use super::workflow_types::CommonRebuildArgs; // Or wherever CommonRebuildArgs is
use super::context::OperationContext; // Or wherever OperationContext is

pub enum ActivationMode { // To specify what 'activate' means
    Switch, // Activate and make default
    Boot,   // Prepare for next boot, don't activate running system
    Test,   // Activate running system, don't make default
    None,   // Just build
}

pub trait PlatformRebuildStrategy {
    type PlatformArgs; // Platform-specific CLI arguments

    fn name(&self) -> &str; // e.g., "NixOS", "Home Manager"

    // Called at the very beginning, for platform-specific initial checks (e.g., root user)
    fn pre_rebuild_hook(&self, op_ctx: &OperationContext, platform_args: &Self::PlatformArgs) -> Result<()>;

    // Determines the final Nix installable string/path (e.g., flake#attr.to.system)
    fn get_toplevel_installable(&self, op_ctx: &OperationContext, platform_args: &Self::PlatformArgs) -> Result<Installable>;

    // Gets the path to the current system/profile for diffing
    fn get_current_profile_path(&self, op_ctx: &OperationContext, platform_args: &Self::PlatformArgs) -> Option<PathBuf>;

    // Performs the actual activation
    fn activate_configuration(
        &self,
        op_ctx: &OperationContext,
        platform_args: &Self::PlatformArgs,
        built_profile_path: &Path,
        activation_mode: &ActivationMode,
    ) -> Result<()>;

    // Called at the very end, for any platform-specific cleanup or final messages
    fn post_rebuild_hook(&self, op_ctx: &OperationContext, platform_args: &Self::PlatformArgs) -> Result<()>;

    // Optional: if platform needs very different pre-flight checks than shared ones
    // fn platform_specific_pre_flight_checks(&self, op_ctx: &OperationContext, platform_args: &Self::PlatformArgs) -> Result<()>;
}
Test: N/A (trait definition). Compile check.
Rationale: Defines the contract for platform-specific logic.
Moment 3.3: Skeletal execute_rebuild_workflow Function

Action (Code - e.g., src/workflow_executor.rs): Create the execute_rebuild_workflow function. Initially, it will be very simple, perhaps only calling a few hooks and logging.
Rust

// e.g., in src/workflow_executor.rs
use super::workflow_strategy::{PlatformRebuildStrategy, ActivationMode};
use super::workflow_types::CommonRebuildArgs;
use super::context::OperationContext; // Assuming you create this
use crate::Result;
use tracing::{info, error}; // Assuming tracing is set up

pub fn execute_rebuild_workflow<S: PlatformRebuildStrategy>(
    platform_strategy: &S,
    op_ctx: &OperationContext, // Pass the whole context
    platform_args: &S::PlatformArgs,
    activation_mode: ActivationMode, // e.g., Switch, Boot, Test, Build
) -> Result<()> {
    info!("🚀 Starting {} rebuild workflow...", platform_strategy.name());

    platform_strategy.pre_rebuild_hook(op_ctx, platform_args)?;
    info!("Pre-rebuild hook for {} completed.", platform_strategy.name());

    // ---- STUBS FOR NOW - To be filled incrementally ----
    // 2. Shared Pre-flight checks (Git, Parse, Lint, Eval, DryRun)
    // if !op_ctx.common_args.no_preflight { run_shared_pre_flight_checks(op_ctx, platform_args)?; }

    // 3. Optional Flake Update
    // if op_ctx.update_args.update || op_ctx.update_args.update_input.is_some() { update_flake_inputs(op_ctx)?; }

    // 4. Get Toplevel Derivation
    let toplevel_installable = platform_strategy.get_toplevel_installable(op_ctx, platform_args)?;
    info!("Resolved toplevel installable for {}: {:?}", platform_strategy.name(), toplevel_installable);

    // 5. Build Configuration
    // let built_profile_path = build_configuration(op_ctx, &toplevel_installable)?;
    // For now, let's assume a dummy path
    let dummy_built_path = std::path::PathBuf::from("/tmp/dummy-build-result");
    std::fs::create_dir_all(&dummy_built_path.parent().unwrap())?; // Ensure parent exists for dummy
    std::fs::write(&dummy_built_path, "dummy content")?;


    // 6. Show Diff
    // if !op_ctx.common_args.dry_run && !matches!(activation_mode, ActivationMode::None) { show_platform_diff(platform_strategy, op_ctx, platform_args, &dummy_built_path)?; }


    // 7. Optional Confirmation
    // if op_ctx.common_args.ask_confirmation && !op_ctx.common_args.dry_run && !matches!(activation_mode, ActivationMode::None) {
    //     if !request_user_confirmation("Apply the new configuration?")? {
    //         bail!("User rejected the new configuration.");
    //     }
    // }

    // 8. Activate
    if !op_ctx.common_args.dry_run && !matches!(activation_mode, ActivationMode::None) {
         info!("Attempting activation for {} (mode: {:?})...", platform_strategy.name(), activation_mode);
         platform_strategy.activate_configuration(op_ctx, platform_args, &dummy_built_path, &activation_mode)?;
         info!("Activation for {} completed.", platform_strategy.name());
    } else if op_ctx.common_args.dry_run {
         info!("Dry-run: Skipping activation for {}.", platform_strategy.name());
    } else {
         info!("Build-only mode: Skipping activation for {}.", platform_strategy.name());
    }


    // 9. Optional Cleanup (Manual --clean flag)
    // if op_ctx.common_args.clean_after && !op_ctx.common_args.dry_run && !matches!(activation_mode, ActivationMode::None) {
    //     perform_manual_cleanup(op_ctx)?;
    // }

    // 10. Automatic Lazy Cleanup (if configured) - to be added in later phase
    // run_automatic_cleanup_if_configured(op_ctx, platform_strategy, true /* activation_success */)?;

    platform_strategy.post_rebuild_hook(op_ctx, platform_args)?;
    info!("Post-rebuild hook for {} completed.", platform_strategy.name());

    info!("🏆 {} rebuild workflow finished successfully!", platform_strategy.name());
    Ok(())
}
Test (Conceptual): Can't test this directly yet without a strategy. Compile check.
Rationale: Establishes the central orchestrator. Stubs allow incremental implementation.
Phase 4: Initial PlatformRebuildStrategy Implementation (NixOS)

Moment 4.1: Create NixosPlatformStrategy Struct
Action (Code - src/nixos.rs or a new src/nixos_strategy.rs):
Rust

// In src/nixos.rs or src/nixos_strategy.rs
use crate::workflow_strategy::{PlatformRebuildStrategy, ActivationMode};
use crate::workflow_types::CommonRebuildArgs; // If defined elsewhere
use crate::context::OperationContext; // If defined elsewhere
use crate::installable::Installable;
use crate::interface::OsRebuildArgs; // Your existing CLI args for NixOS
use crate::util; // For run_cmd
use crate::Result;
use std::path::{Path, PathBuf};
use tracing::info;
use color_eyre::eyre::{bail, Context};


pub struct NixosPlatformStrategy;

impl PlatformRebuildStrategy for NixosPlatformStrategy {
    type PlatformArgs = OsRebuildArgs; // Use your existing struct

    fn name(&self) -> &str { "NixOS" }

    fn pre_rebuild_hook(&self, op_ctx: &OperationContext, platform_args: &Self::PlatformArgs) -> Result<()> {
        info!("NixOS pre-rebuild hook executing...");
        if !platform_args.bypass_root_check && nix::unistd::Uid::effective().is_root() {
            bail!("Do not run `ng os` as root. Sudo will be used internally as needed.");
        }
        // Any other NixOS specific initial checks
        Ok(())
    }

    fn get_toplevel_installable(&self, op_ctx: &OperationContext, platform_args: &Self::PlatformArgs) -> Result<Installable> {
        info!("NixOS determining toplevel installable...");
        // Adapt existing logic from nixos.rs to determine hostname and final installable
        // For now, let's assume platform_args.common.installable is mostly complete or
        // use logic from your current `toplevel_for` specific to NixOS.
        let hostname = platform_args.hostname.clone().ok_or_else(crate::util::get_hostname)
            .or_else(|e| {
                tracing::warn!("Failed to get hostname automatically: {}. Using 'default_host' or similar. You might need to specify --hostname.", e);
                Ok::<String, color_eyre::eyre::Report>("default_host".to_string()) // Provide a fallback or make it mandatory
            })?;

        let mut final_installable = op_ctx.common_args.installable.clone();
        if let Installable::Flake { attribute, .. } = &mut final_installable {
            if attribute.is_empty() { // Only if user didn't provide #attr
                attribute.push("nixosConfigurations".to_string());
                attribute.push(hostname);
            }
            // The old `toplevel_for` appended ["config", "system", "build", "toplevel"]
            // This should be part of what the Nix expression for nixosConfigurations.<host> resolves to.
            // For the build command, it usually expects the system derivation directly.
            // Let's assume the user provides installable up to the system derivation e.g. nixosConfigurations.myhost
            // or nixosConfigurations.myhost.config.system.build.toplevel if that's what nixos-rebuild expects
            // For now, let's assume the attribute path should point to the system derivation.
            // If specialisation is used, it might be different.
            if let Some(spec) = &platform_args.specialisation {
                 if !platform_args.no_specialisation {
                    // The attribute path might need to include the specialisation.
                    // e.g. nixosConfigurations.myhost.specialisations.mySpecialisation.config.system.build.toplevel
                    // This part requires careful thought on how specialisations are structured in the flake.
                    // For now, keeping it simpler, assuming specialisation is handled by the nix code itself
                    // or applied to the built result path.
                    tracing::info!("Applying specialisation (logic TBD based on flake structure): {}", spec);
                 }
            }
        }
        Ok(final_installable)
    }

    fn get_current_profile_path(&self, _op_ctx: &OperationContext, _platform_args: &Self::PlatformArgs) -> Option<PathBuf> {
        Some(PathBuf::from("/run/current-system")) // Standard NixOS current system
    }

    fn activate_configuration(
        &self,
        op_ctx: &OperationContext,
        platform_args: &Self::PlatformArgs,
        built_profile_path: &Path, // This is the /nix/store path from the build
        activation_mode: &ActivationMode,
    ) -> Result<()> {
        info!("NixOS activating configuration (mode: {:?}) from {}...", activation_mode, built_profile_path.display());

        let action_str = match activation_mode {
            ActivationMode::Switch => "switch",
            ActivationMode::Boot => "boot",
            ActivationMode::Test => "test",
            ActivationMode::None => return Ok(()), // Should not happen if workflow logic is correct
        };

        // Path to switch-to-configuration might be in the built_profile_path itself
        let switch_script = built_profile_path.join("bin/switch-to-configuration");
        if !switch_script.exists() {
            bail!("switch-to-configuration script not found in build result: {:?}", switch_script);
        }

        let mut cmd = std::process::Command::new(switch_script);
        cmd.arg(action_str);

        // Handle elevation
        let needs_sudo = !platform_args.bypass_root_check; // sudo if not bypassing root check (i.e. we are not already root)
        if needs_sudo {
            let mut sudo_cmd = std::process::Command::new("sudo");
            sudo_cmd.arg(cmd.get_program());
            for arg in cmd.get_args() {
                sudo_cmd.arg(arg);
            }
            cmd = sudo_cmd;
        }
        info!("Executing activation: {:?}", cmd);
        util::run_cmd_inherit_stdio(&mut cmd)
            .map_err(|e| eyre!("NixOS activation failed (action: {}): {}", action_str, e))
            .map(|_| ())
    }

    fn post_rebuild_hook(&self, _op_ctx: &OperationContext, _platform_args: &Self::PlatformArgs) -> Result<()> {
        info!("NixOS post-rebuild hook executing.");
        // e.g., print specific success messages for NixOS
        Ok(())
    }
}
Test (Unit Test):
Create NixosPlatformStrategy.
Test name() returns "NixOS".
Test pre_rebuild_hook (mock OperationContext, verify it bails if run as root without bypass flag).
Test get_toplevel_installable with mock OperationContext and OsRebuildArgs (various Installable inputs, hostname, specialisation flags). Assert the output Installable is correct. (This is a key test).
Test get_current_profile_path returns the correct constant.
Test activate_configuration (this is harder to unit test as it shells out). You can mock util::run_cmd_inherit_stdio if you introduce an interface for it, or simply test that it constructs the Command correctly for different ActivationModes.
Rationale: Implements the first concrete strategy.
Phase 5: Migrating ng os switch (Incrementally)

Moment 5.1: Adapt interface::OsRebuildArgs and CommonRebuildArgs

Action (Code - src/interface.rs, src/workflow_types.rs):
Ensure interface::OsRebuildArgs contains all necessary NixOS-specific options (like hostname, specialisation, no_specialisation, bypass_root_check).
Ensure workflow_types::CommonRebuildArgs (from Moment 3.1) correctly captures all shared flags from interface::CommonArgs that were previously in interface::OsRebuildArgs.common.
The CommonRebuildArgs in OsRebuildArgs (i.e., pub common: CommonRebuildArgs) should now be of the new central workflow_types::CommonRebuildArgs type.
Test: Compile check.
Rationale: Align CLI argument structures with the new workflow types.
Moment 5.2: Modify nixos.rs::OsArgs::run for Switch to call execute_rebuild_workflow

Action (Code - src/nixos.rs):
Rust

// In src/nixos.rs
// ...
impl interface::OsArgs {
    pub fn run(self, verbose_count: u8) -> Result<()> {
        // ...
        match self.subcommand {
            OsSubcommand::Switch(args) => { // Focus on this one first
                let common_rebuild_args = crate::workflow_types::CommonRebuildArgs {
                    installable: args.common.installable.clone(), // Assuming CommonRebuildArgs in OsRebuildArgs is the old one
                    no_preflight: args.common.common.no_preflight,
                    strict_lint: args.common.common.strict_lint,
                    medium_checks: args.common.common.medium,
                    full_checks: args.common.common.full,
                    dry_run: args.common.common.dry,
                    ask_confirmation: args.common.common.ask,
                    no_nom: args.common.common.no_nom,
                    out_link: args.common.common.out_link.clone(),
                    clean_after: args.common.common.clean,
                    extra_build_args: args.extra_args.clone(), // Or from args.common.extra_build_args if it moves
                };
                let op_ctx = crate::context::OperationContext {
                    common_args: &common_rebuild_args,
                    update_args: &args.update_args,
                    verbose_count,
                    // nix_interface: ..., // Not yet
                    // error_handler: ..., // Not yet
                };
                let strategy = NixosPlatformStrategy;
                crate::workflow_executor::execute_rebuild_workflow(
                    &strategy,
                    &op_ctx,
                    &args, // Pass the full OsRebuildArgs as PlatformArgs
                    crate::workflow_strategy::ActivationMode::Switch,
                )
            }
            OsSubcommand::Boot(args) => args.rebuild(Boot, verbose_count), // Will be refactored later
            OsSubcommand::Test(args) => args.rebuild(Test, verbose_count),  // Will be refactored later
            OsSubcommand::Build(args) => { /* ... */ args.rebuild(Build, verbose_count); } // Will be refactored later
            OsSubcommand::Repl(args) => args.run(verbose_count),
            OsSubcommand::Info(args) => args.info(),
        }
    }
}
// The old OsRebuildArgs::rebuild method can be slowly dismantled or its logic moved
// into NixosPlatformStrategy methods.
Test (Integration Test - assert_cmd):
Create a minimal, working flake.nix and test/configuration.nix for a NixOS system.
Run ng os switch --flake .#testSystemName (or however your test installable is specified).
Initial Assertion: Verify it runs through the skeletal execute_rebuild_workflow and the NixosPlatformStrategy hooks are logged (pre, get_toplevel, activate, post). It might "fail" at activation if the dummy build path is used, or succeed if activate_configuration is just logging for now. The key is that the flow is invoked.
If get_toplevel_installable in NixosPlatformStrategy is correctly implemented, it should resolve the target.
If activate_configuration in NixosPlatformStrategy calls the actual (old) activation logic (now via util::run_cmd), the test should fully pass as before the refactor.
Rationale: First end-to-end test of the new workflow with a real command.
Moment 5.3: Implement Real Build Step in execute_rebuild_workflow

Action (Code - src/workflow_executor.rs): Replace the dummy build path logic with a call to a new function, e.g., build_nix_configuration. This function will initially use your existing commands::Build struct, but ensure its .run() method now uses util::run_cmd (from Phase 1.4).
Rust

// In src/workflow_executor.rs
// ...
fn build_nix_configuration(
    op_ctx: &OperationContext,
    installable: &Installable,
    out_link_manager: &impl crate::util::MaybeTempPath, // From your util.rs
) -> Result<PathBuf> { // Returns the actual built profile path
    info!("Building Nix configuration for {:?}...", installable);
    let pb_build = crate::progress::start_stage_spinner("Build", "Building configuration...");

    // This should eventually use NixInterface
    // For now, adapt your existing commands::Build
    let build_cmd_struct = crate::commands::Build::new(installable.clone()) // Assuming Build can take Installable
        .extra_arg("--out-link")
        .extra_arg(out_link_manager.get_path())
        .extra_args(op_ctx.common_args.extra_build_args.iter().map(|s| s.as_str()))
        .message("Building configuration (via workflow)") // Differentiate log message
        .nom(!op_ctx.common_args.no_nom);

    match build_cmd_struct.run() { // Assuming this run now uses util::run_cmd and returns Result
        Ok(_) => {
            let built_path = std::fs::read_link(out_link_manager.get_path())
                .wrap_err_with(|| format!("Failed to read symlink at out-link: {:?}", out_link_manager.get_path()))?;
            crate::progress::finish_stage_spinner_success(&pb_build, "Build", &format!("Configuration built: {}", built_path.display()));
            Ok(built_path)
        }
        Err(e) => {
            crate::progress::finish_spinner_fail(&pb_build); // Ensure spinner is handled
            // Error reporting should ideally be handled by the caller or a global handler if
            // build_cmd_struct.run() returns a specific error type that can be caught.
            // For now, assume color_eyre::Report is propagated.
            // This is where ErrorHandler will be critical.
            // Your nixos.rs already has good error reporting for build failures;
            // that logic will eventually move into ErrorHandler and be triggered here.
            error!("Build failed: {}. See details above if NOM was used, or run with -v.", e);
            Err(e)
        }
    }
}

// In execute_rebuild_workflow:
// ...
// 5. Build Configuration
let out_path_manager = crate::util::manage_out_path(op_ctx.common_args.out_link.as_ref())?; // Your existing helper
let built_profile_path = build_nix_configuration(op_ctx, &toplevel_installable, out_path_manager.as_ref())?;
// ...
// Replace dummy_built_path with built_profile_path in subsequent steps
platform_strategy.activate_configuration(op_ctx, platform_args, &built_profile_path, &activation_mode)?;
// ...
Test (Integration Test): Re-run ng os switch test. Now it should perform a real build.
Verify that if the build fails (e.g., syntax error in test/configuration.nix), the error is reported (even if not yet through the fully enhanced ErrorHandler).
Verify successful build produces the correct out-link.
Rationale: Integrates the actual build process into the workflow.
Moment 5.4: Implement Real Diff Step in execute_rebuild_workflow (Your Point 1, Step 6)

Action (Code - src/workflow_executor.rs):
Rust

// In src/workflow_executor.rs
// ...
fn show_platform_diff<S: PlatformRebuildStrategy>(
    platform_strategy: &S,
    op_ctx: &OperationContext,
    platform_args: &S::PlatformArgs,
    new_profile_path: &Path,
) -> Result<()> {
    if let Some(current_profile) = platform_strategy.get_current_profile_path(op_ctx, platform_args) {
        if current_profile.exists() { // Important check
            info!("Comparing changes with nvd...");
            let pb_diff = crate::progress::start_stage_spinner("Diff", "Comparing configurations...");
            let mut cmd = std::process::Command::new("nvd"); // Or use commands::Command builder
            cmd.arg("diff")
               .arg(&current_profile)
               .arg(new_profile_path);

            match crate::util::run_cmd_inherit_stdio(&mut cmd) {
                Ok(_) => crate::progress::finish_stage_spinner_success(&pb_diff, "Diff", "Comparison displayed."),
                Err(e) => {
                    crate::progress::finish_spinner_fail(&pb_diff);
                    // Your existing error reporting for diff failure in nixos.rs is good
                    warn!("Failed to run nvd diff: {}. This is non-critical.", e);
                    // error_handler::report_failure("Diff", ..., Some(e.to_string()), recommendations)
                }
            }
        } else {
            info!("Current profile {} does not exist, skipping diff.", current_profile.display());
        }
    } else {
        info!("No current profile path provided by strategy for {}, skipping diff.", platform_strategy.name());
    }
    Ok(())
}

// In execute_rebuild_workflow, uncomment and call:
if !op_ctx.common_args.dry_run && !matches!(activation_mode, ActivationMode::None) { // And not your is_just_build_variant
    show_platform_diff(platform_strategy, op_ctx, platform_args, &built_profile_path)?;
}
// ...
Test (Integration Test):
Run ng os switch. Make a small change to test/configuration.nix (e.g., add a package). Run ng os switch again. Verify nvd diff output is shown.
Test with a fresh system (no /run/current-system) – diff should be skipped gracefully.
Rationale: Integrates the diffing step.
Moment 5.5: Implement Confirmation & Cleanup Steps (Your Point 1, Steps 7 & 9)

Action (Code - src/workflow_executor.rs): Implement the --ask confirmation logic using dialoguer and the --clean logic (which would call crate::clean::CleanMode::All(CleanArgs{...}).run(verbose_count) or a more targeted cleanup function).
Rust

// In src/workflow_executor.rs
// ...
// 7. Optional Confirmation
if op_ctx.common_args.ask_confirmation && !op_ctx.common_args.dry_run && !matches!(activation_mode, ActivationMode::None) {
    info!("User confirmation required to apply the new configuration.");
    if !dialoguer::Confirm::new().with_prompt("Apply the new configuration?").default(false).interact()? {
        bail!("User rejected the new configuration.");
    }
    info!("User confirmed.");
}

// ...
// 9. Optional Cleanup (Manual --clean flag)
if op_ctx.common_args.clean_after && !op_ctx.common_args.dry_run && !matches!(activation_mode, ActivationMode::None) {
    info!("Performing cleanup as requested by --clean flag...");
    // This needs to be adapted to call your clean logic.
    // Example: create CleanArgs based on some defaults or op_ctx.
    let clean_args = crate::interface::CleanArgs { // Assuming CleanArgs is accessible
        keep: 3, // Default for --clean, or could be configurable
        keep_since: humantime::parse_duration("7d")?.into(), // Default
        dry: op_ctx.common_args.dry_run, // Should be false here if we reach this
        ask: false, // --clean should not re-ask
        nogc: false, // Defaults for --clean
        nooptimise: false,
        nogcroots: false, // Or true, depending on desired --clean behavior
    };
    // Assuming you have a way to run a "full clean" or a "profile specific clean"
    // For now, let's conceptualize a general clean.
    // This needs careful mapping to your existing clean::CleanMode::All or a new function.
    // Temporarily, let's just log it.
    // crate::clean::CleanMode::All(clean_args).run(op_ctx.verbose_count)?;
    info!("Cleanup step placeholder for --clean.");
    // Your existing `perform_cleanup` can be adapted here.
    // It might be better to call the nix-store gc directly via NixInterface eventually.
    crate::nixos::perform_cleanup(op_ctx.verbose_count)?; // If this is your current cleanup call
    info!("Cleanup after rebuild completed.");
}
// ...
Test (Integration Test):
Test ng os switch --ask. Respond 'no', ensure it bails. Respond 'yes', ensure it proceeds.
Test ng os switch --clean. Verify cleanup actions are logged or (if fully implemented) occur.
Rationale: Completes the main conditional steps of the generic workflow.
Phase 6: Integrating Pre-Flight Checks Incrementally

Moment 6.1: Define PreFlightCheck Trait and run_shared_pre_flight_checks

Action (Code - src/workflow_executor.rs or new src/pre_flight.rs):
Rust

// In src/pre_flight.rs
use crate::Result;
use crate::context::OperationContext;
use crate::workflow_strategy::PlatformRebuildStrategy;

#[derive(Debug, Clone, Copy)]
pub enum CheckStatusReport {
    Passed,
    Warnings,
    FailedCritical, // A failure that should halt the process
    FailedNonCritical, // A failure that might allow proceeding with warning
}

pub trait PreFlightCheck {
    fn name(&self) -> &str;
    // Takes op_ctx and platform_args to allow checks to be platform-aware if needed
    fn run<S: PlatformRebuildStrategy>(&self, op_ctx: &OperationContext, platform_args: &S::PlatformArgs) -> Result<CheckStatusReport>;
}

// In src/workflow_executor.rs or pre_flight.rs
pub fn run_shared_pre_flight_checks<S: PlatformRebuildStrategy>(
    op_ctx: &OperationContext,
    platform_args: &S::PlatformArgs,
    checks: &[Box<dyn PreFlightCheck>], // Pass in the checks to run
) -> Result<()> {
    if op_ctx.common_args.no_preflight {
        info!("[⏭️ Pre-flight] All checks skipped due to --no-preflight.");
        return Ok(());
    }
    info!("🚀 Running shared pre-flight checks...");
    for check in checks {
        info!("Executing pre-flight check: {}...", check.name());
        let pb = crate::progress::start_stage_spinner("Pre-flight", &format!("Running {} check", check.name()));
        match check.run(op_ctx, platform_args) {
            Ok(CheckStatusReport::Passed) => crate::progress::finish_stage_spinner_success(&pb, check.name(), "Check passed."),
            Ok(CheckStatusReport::Warnings) => crate::progress::finish_stage_spinner_success(&pb, check.name(), "Check passed with warnings."), // Or a different spinner finish
            Ok(CheckStatusReport::FailedNonCritical) => {
                crate::progress::finish_spinner_fail(&pb); // Or a warning finish
                warn!("Pre-flight check {} failed non-critically. Check warnings above.", check.name());
            }
            Ok(CheckStatusReport::FailedCritical) => {
                crate::progress::finish_spinner_fail(&pb);
                // Error should have been reported by the check itself via ErrorHandler
                bail!("Critical pre-flight check {} failed. Aborting.", check.name());
            }
            Err(e) => {
                crate::progress::finish_spinner_fail(&pb);
                // Report error using ErrorHandler. This is a failure in the check runner itself.
                // error_handler::report_failure("Pre-flight", &format!("Check {} execution error", check.name()), Some(e.to_string()), vec![]);
                error!("Error during pre-flight check {}: {}", check.name(), e); // Simplified for now
                if op_ctx.common_args.strict_lint { // Or a new flag for strict preflight
                     bail!("Critical error in pre-flight check {} failed. Aborting.", check.name());
                } else {
                     warn!("Pre-flight check {} encountered an error. Continuing...", check.name());
                }
            }
        }
    }
    info!("✅ All pre-flight checks completed.");
    Ok(())
}
Test: N/A (definitions). Compile check.
Rationale: Modular structure for pre-flight checks.
Moment 6.2: Implement and Integrate Git Check

Action (Code - src/check_git.rs and src/workflow_executor.rs):
Make run_git_check_warning_only (or a new function GitPreFlightCheck::run) implement PreFlightCheck. It should return Ok(CheckStatusReport::Warnings) if there are untracked files (as it's warning-only) or Ok(CheckStatusReport::Passed).
In execute_rebuild_workflow, instantiate GitPreFlightCheck and pass it to run_shared_pre_flight_checks.
Test (Integration Test):
ng os switch in a clean Git repo.
ng os switch in a Git repo with untracked .nix files (verify warning).
ng os switch --no-preflight (verify Git check is skipped).
Rationale: First pre-flight check integrated.
Moment 6.3: Implement and Integrate Parse Check

Action (Code - src/nixos.rs (or where run_parallel_parse_check is), src/workflow_executor.rs):
Adapt your existing run_parallel_parse_check (from nixos.rs, home.rs, etc. - this logic should be moved to a common pre_flight_checks module) to implement PreFlightCheck.
It uses nix-instantiate --parse. If errors occur, it should use ErrorHandler::report_failure and return Ok(CheckStatusReport::FailedCritical).
Instantiate and add it to the checks list in execute_rebuild_workflow.
Test (Integration Test):
ng os switch with syntactically correct Nix files.
ng os switch with a .nix file having a syntax error. Verify ng bails and ErrorHandler reports it (using current stderr parsing for now).
Rationale: Integrates basic Nix syntax validation. This is the hook point for future nil-syntax based parsing.
Moment 6.4: Implement and Integrate Lint Check

Action (Code - src/lint.rs, src/workflow_executor.rs):
Adapt lint::run_lint_checks to implement PreFlightCheck.
It should return Ok(CheckStatusReport::FailedCritical) if strict_lint is on and critical lint issues are found. Otherwise Ok(CheckStatusReport::Warnings) or Ok(CheckStatusReport::Passed).
Instantiate and add to checks list.
Test (Integration Test):
ng os switch with clean Nix files.
ng os switch with files that would trigger lint warnings/errors (if you have mock linters or can set up files for alejandra/statix to find issues).
Test with --strict-lint.
Rationale: Integrates linting into the standard flow.
Phase 7: Initial Stubs for NixInterface and ErrorHandler Focus

Moment 7.1: Create NixInterface Module Stub

Action (Code - src/nix_interface.rs):
Rust

// src/nix_interface.rs
use crate::installable::Installable;
use crate::Result;
use std::path::{Path, PathBuf};
use crate::util; // For UtilCommandError

#[derive(Debug)]
pub struct NixInterface<'a> { // Lifetime might be for OperationContext if it holds refs
    verbose_count: u8,
    // Potential future fields: nix_command_path, extra_global_nix_args
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a> NixInterface<'a> {
    pub fn new(verbose_count: u8) -> Self {
        Self { verbose_count, _marker: std::marker::PhantomData }
    }

    // Example: what build_nix_configuration in workflow_executor might eventually call
    pub fn build_configuration(
        &self,
        installable: &Installable,
        out_link: &Path,
        extra_build_args: &[String],
        no_nom: bool,
    ) -> Result<PathBuf> {
        // This would encapsulate the logic from commands::Build
        // For now, it can directly call a refactored commands::Build::run_and_get_path
        // or reimplement the nix build call using util::run_cmd
        tracing::info!("NixInterface: Building {:?} with out-link {}", installable, out_link.display());
        // ... actual nix build call using util::run_cmd ...
        // ... parse output / check for errors ...
        // For now, placeholder:
        if installable.to_args().join(" ").contains("fail_build") { // Test hook
             Err(color_eyre::eyre::eyre!("Simulated build failure via NixInterface"))
        } else {
             let built_path = std::fs::read_link(out_link)
                .wrap_err_with(|| format!("NixInterface: Failed to read symlink at out-link: {:?}", out_link))?;
             Ok(built_path)
        }
    }

    // Add other methods as needed: eval_json, get_path_info, run_gc, etc.
}
Test: Unit test basic instantiation. No complex logic yet.
Rationale: Defines the place where all nix CLI interactions will be centralized. The build_nix_configuration function in workflow_executor.rs should be updated to call op_ctx.nix_interface.build_configuration(...) once NixInterface is added to OperationContext.
Moment 7.2: Ensure ErrorHandler is Being Used

Action (Review/Code): Review execute_rebuild_workflow and the pre-flight check implementations. Any Result::Err that should be user-facing and halt execution should ideally be processed by ErrorHandler::report_failure.
In workflow_executor.rs, if build_nix_configuration (or later nix_interface.build_configuration) returns an Err, this error should be caught and passed to ErrorHandler.
Rust

// Conceptual snippet in execute_rebuild_workflow
// let built_profile_path = match op_ctx.nix_interface.build_configuration(...) {
//     Ok(path) => path,
//     Err(e) => {
//         crate::error_handler::report_failure("Build", "Configuration build failed", Some(e.to_string()), vec![]);
//         bail!("Build failed, see details above.");
//     }
// };
Test: Ensure build failures (e.g., by making test/configuration.nix invalid) trigger ErrorHandler::report_failure and the output matches its formatting (even if recommendations are basic for now).
Rationale: Prepares for enhancing ErrorHandler with AST/CST info in Part 3.
This concludes Part 2. ng now has a generic, extensible workflow for its rebuild commands. ng os switch is the first adopter. Basic pre-flight checks are integrated. The foundation for NixInterface is laid, and ErrorHandler is positioned to be the central diagnostic hub. Each step was designed to be small and individually testable.

## Part 2- Code
Okay, let's dive into Part 2 of 3 for generating the refactored ng codebase.
Building on the foundations from Part 1 (standardized command execution, Installable parsing with chumsky, initial workflow structures), this part will focus on:

Fleshing out the execute_rebuild_workflow with real build, diff, confirmation, flake update, and manual cleanup steps.
Defining and incrementally integrating the pre-flight checks (Git, current Nix-based Parse, Lint) into this workflow.
Ensuring that errors from these new steps are channeled appropriately (even if full ErrorHandler enhancement is in Part 3).
Making the build_nix_configuration step within the workflow use the (refactored) commands::Build logic.
As before, this is a conceptual representation. You'll need to handle all use statements and perform thorough testing.

Part 2 of 3: Fleshing out Workflow and Integrating Pre-flight Checks

Phase 5 (Continued): Migrating ng os switch & Detailing execute_rebuild_workflow

We'll continue enhancing execute_rebuild_workflow.rs and ensure NixosPlatformStrategy provides the necessary details for these steps.

1. Modified: src/workflow_executor.rs - Implementing Core Steps

This file will see the most changes as we replace stubs with actual logic.

Rust

// src/workflow_executor.rs
use crate::workflow_strategy::{PlatformRebuildStrategy, ActivationMode};
// use crate::workflow_types::CommonRebuildArgs; // Already included if in same mod/crate
use crate::context::OperationContext;
use crate::installable::Installable;
use crate::util::{self, MaybeTempPath};
use crate::Result; // from color_eyre
use std::path::{Path, PathBuf};
use tracing::{info, warn, error, debug, instrument}; // Add instrument
use color_eyre::eyre::{bail, eyre, WrapErr};
use crate::commands::Command as NgCommand; // Your existing command builder from commands.rs
                                          // Assuming commands::Build is also in commands.rs

// New module for pre-flight checks
mod pre_flight;
use pre_flight::{run_shared_pre_flight_checks, get_core_pre_flight_checks, CheckStatusReport};


// Function to perform the actual Nix build
#[instrument(skip(op_ctx, out_link_manager), fields(installable = ?installable.to_args()))]
fn build_nix_configuration(
    op_ctx: &OperationContext,
    installable: &Installable,
    out_link_manager: &dyn MaybeTempPath, // Your existing trait
) -> Result<PathBuf> { // Returns the actual built /nix/store path
    info!("Building Nix configuration...");
    let pb_build = crate::progress::start_stage_spinner("Build", "Building configuration...");

    // This uses your existing commands::Build, which in Part 1 plan
    // should have been refactored to use util::run_cmd internally.
    let build_cmd_struct = crate::commands::Build::new(installable.clone())
        .extra_arg("--out-link")
        .extra_arg(out_link_manager.get_path())
        // Convert Vec<OsString> to Vec<&str> or ensure commands::Build::extra_args takes OsString
        .extra_args(op_ctx.common_args.extra_build_args.iter().map(|s| s.as_os_str()))
        .message("Building configuration (via workflow)")
        .nom(!op_ctx.common_args.no_nom);

    match build_cmd_struct.run() { // This run must return Result<(), YourErrorType>
        Ok(_) => {
            // If --out-link is used, nix build creates a symlink there. We need to read it.
            let built_path = std::fs::read_link(out_link_manager.get_path())
                .wrap_err_with(|| format!("Build succeeded, but failed to read the out-link symlink at: {:?}", out_link_manager.get_path()))?;
            crate::progress::finish_stage_spinner_success(&pb_build, "Build", &format!("Configuration built: {}", built_path.display()));
            Ok(built_path)
        }
        Err(e) => {
            crate::progress::finish_spinner_fail(&pb_build);
            // Error reporting will be enhanced in Phase 8 with ErrorHandler
            error!("Build command failed: {}. Check detailed Nix output above (if NOM was used) or run with more verbosity (-v).", e);
            // We expect `e` to be color_eyre::Report if commands::Build::run was fully refactored
            Err(eyre!("Build step failed").wrap_err(e))
        }
    }
}

// Function to show diff using nvd
#[instrument(skip(platform_strategy, op_ctx, platform_args, new_profile_path))]
fn show_platform_diff<S: PlatformRebuildStrategy>(
    platform_strategy: &S,
    op_ctx: &OperationContext,
    platform_args: &S::PlatformArgs,
    new_profile_path: &Path,
) -> Result<()> {
    if let Some(current_profile_path) = platform_strategy.get_current_profile_path(op_ctx, platform_args) {
        if current_profile_path.exists() {
            info!("Comparing changes with nvd (current: {}, new: {})...", current_profile_path.display(), new_profile_path.display());
            let pb_diff = crate::progress::start_stage_spinner("Diff", "Comparing configurations...");

            // Using util::run_cmd_inherit_stdio for nvd
            let mut nvd_cmd = std::process::Command::new("nvd"); // Assumes nvd is in PATH
            nvd_cmd.arg("diff")
                   .arg(&current_profile_path)
                   .arg(new_profile_path);

            match util::run_cmd_inherit_stdio(&mut nvd_cmd) {
                Ok(_) => crate::progress::finish_stage_spinner_success(&pb_diff, "Diff", "Comparison displayed."),
                Err(e) => {
                    crate::progress::finish_spinner_fail(&pb_diff); // Use a warning variant if available
                    warn!("Failed to run 'nvd diff': {}. This is non-critical. Ensure 'nvd' is installed and profiles are valid.", e);
                    // In Part 3, this could use ErrorHandler for a more structured warning.
                }
            }
        } else {
            info!("Current profile {} does not exist, skipping diff.", current_profile_path.display());
        }
    } else {
        info!("No current profile path provided by strategy for {}, skipping diff.", platform_strategy.name());
    }
    Ok(())
}


#[instrument(skip_all, fields(platform = %platform_strategy.name(), mode = ?activation_mode))]
pub fn execute_rebuild_workflow<S: PlatformRebuildStrategy>(
    platform_strategy: &S,
    op_ctx: &OperationContext,
    platform_args: &S::PlatformArgs,
    activation_mode: ActivationMode,
) -> Result<()> {
    info!("🚀 Starting {} rebuild workflow ({:?})...", platform_strategy.name(), activation_mode);

    // 1. Platform-specific pre-rebuild hook
    platform_strategy.pre_rebuild_hook(op_ctx, platform_args)
        .wrap_err_with(|| format!("{} pre-rebuild hook failed", platform_strategy.name()))?;
    debug!("Pre-rebuild hook for {} completed.", platform_strategy.name());

    // 2. Shared Pre-flight checks
    if !op_ctx.common_args.no_preflight {
        // Get the core checks (Git, Parse, Lint for now)
        let checks_to_run = get_core_pre_flight_checks(op_ctx); // Pass op_ctx for config
        run_shared_pre_flight_checks(op_ctx, platform_strategy, platform_args, &checks_to_run)?;
        debug!("Shared pre-flight checks for {} completed.", platform_strategy.name());
    } else {
        info!("[⏭️ Pre-flight] All checks skipped due to --no-preflight.");
    }

    // 3. Optional Flake Update
    if op_ctx.update_args.update || op_ctx.update_args.update_input.is_some() {
        info!("Updating flake inputs as requested...");
        let pb_update = crate::progress::start_stage_spinner("Update", "Updating flake inputs...");
        crate::update::update(&op_ctx.common_args.installable, op_ctx.update_args.update_input.clone())
            .wrap_err("Failed to update flake inputs")
            .map_err(|e| {
                crate::progress::finish_spinner_fail(&pb_update);
                // TODO: Use ErrorHandler in Part 3
                error!("Flake update failed: {}", e);
                e
            })?;
        crate::progress::finish_stage_spinner_success(&pb_update, "Update", "Flake inputs updated.");
        debug!("Flake input update for {} completed.", platform_strategy.name());
    }

    // 4. Get Toplevel Derivation
    let toplevel_installable = platform_strategy.get_toplevel_installable(op_ctx, platform_args)
        .wrap_err_with(|| format!("Failed to determine toplevel installable for {}", platform_strategy.name()))?;
    debug!("Resolved toplevel installable for {}: {:?}", platform_strategy.name(), toplevel_installable.to_args());

    // 5. Build Configuration
    // Create out_path_manager here, as it might be a TempDir whose lifetime needs to span until after activation for some strategies
    let out_path_manager = util::manage_out_path(op_ctx.common_args.out_link.as_ref())?;
    let built_profile_store_path: PathBuf;

    if op_ctx.common_args.dry_run && activation_mode == ActivationMode::Build {
        info!("Dry-run build-only mode: Simulating build for {:?}. Output would be at {}.",
            &toplevel_installable.to_args(),
            out_path_manager.get_path().display()
        );
        // For a pure dry-run build, we don't actually build or have a real result path.
        // Subsequent steps like diff/activate will be skipped anyway.
        // We can assign a conceptual path or handle it.
        built_profile_store_path = PathBuf::from("/dry_run_build_no_actual_path"); // Placeholder
    } else {
        built_profile_store_path = build_nix_configuration(op_ctx, &toplevel_installable, out_path_manager.as_ref())?;
    }


    // 6. Show Diff (if not a dry run for activation/test/switch, and not just a build command)
    if activation_mode != ActivationMode::Build && !op_ctx.common_args.dry_run {
        show_platform_diff(platform_strategy, op_ctx, platform_args, &built_profile_store_path)?;
    } else if op_ctx.common_args.dry_run && activation_mode != ActivationMode::Build {
        info!("Dry-run: Skipping diff display.");
    }


    // 7. Optional Confirmation
    if op_ctx.common_args.ask_confirmation && activation_mode != ActivationMode::Build && !op_ctx.common_args.dry_run {
        info!("User confirmation required to apply the new configuration for {}.", platform_strategy.name());
        if !dialoguer::Confirm::new()
            .with_prompt(format!("Apply the new {} configuration?", platform_strategy.name()))
            .default(false)
            .interact()
            .wrap_err("Failed to get user confirmation")?
        {
            bail!("User rejected the new configuration for {}.", platform_strategy.name());
        }
        info!("User confirmed action for {}.", platform_strategy.name());
    }

    // 8. Activate
    if activation_mode != ActivationMode::Build && !op_ctx.common_args.dry_run {
        info!("Attempting activation for {} (mode: {:?})...", platform_strategy.name(), activation_mode);
        let pb_activate = crate::progress::start_stage_spinner("Activate", &format!("Activating {} configuration", platform_strategy.name()));
        platform_strategy.activate_configuration(op_ctx, platform_args, &built_profile_store_path, &activation_mode)
            .wrap_err_with(|| format!("Failed to activate {} configuration", platform_strategy.name()))
            .map_err(|e| {
                crate::progress::finish_spinner_fail(&pb_activate);
                // TODO: Use ErrorHandler in Part 3 for rich error display from activation
                error!("Activation failed for {}: {}", platform_strategy.name(), e);
                e
            })?;
        crate::progress::finish_stage_spinner_success(&pb_activate,"Activate", &format!("{} configuration activated.", platform_strategy.name()));
    } else if op_ctx.common_args.dry_run && activation_mode != ActivationMode::Build {
        info!("Dry-run: Skipping activation for {}.", platform_strategy.name());
    } else if activation_mode == ActivationMode::Build {
        // If it was a real build (not dry run), print the output path.
        if !op_ctx.common_args.dry_run {
            info!("Build-only mode complete. Result at out-link: {} (points to: {})", out_path_manager.get_path().display(), built_profile_store_path.display());
        }
    }

    // 9. Optional Cleanup (triggered by --clean flag)
    if op_ctx.common_args.clean_after && activation_mode != ActivationMode::Build && !op_ctx.common_args.dry_run {
        info!("Performing cleanup as requested by --clean flag for {}...", platform_strategy.name());
        let pb_clean = crate::progress::start_stage_spinner("Clean", "Performing post-rebuild cleanup...");
        // This should call a refactored version of your clean logic.
        // For now, using the example from nixos.rs which calls nix store gc.
        // This needs to be made platform-aware or profile-aware.
        // crate::nixos::perform_cleanup(op_ctx.verbose_count) // Old call from nixos.rs
        match crate::clean::perform_basic_cleanup(op_ctx.verbose_count, op_ctx.common_args.dry_run) {
            Ok(_) => crate::progress::finish_stage_spinner_success(&pb_clean, "Clean", "Cleanup completed."),
            Err(e) => {
                crate::progress::finish_spinner_fail(&pb_clean);
                warn!("Manual cleanup step failed (non-critical): {}", e);
            }
        }
    }

    // 10. Automatic Lazy Cleanup (if configured) - To be implemented in Phase 11 (Code Gen Part 3)
    // if activation_mode != ActivationMode::Build && !op_ctx.common_args.dry_run && should_auto_clean(...) {
    //     perform_auto_clean(op_ctx, platform_strategy)?;
    // }

    // 11. Platform-specific post-rebuild hook
    platform_strategy.post_rebuild_hook(op_ctx, platform_args)
        .wrap_err_with(|| format!("{} post-rebuild hook failed", platform_strategy.name()))?;
    debug!("Post-rebuild hook for {} completed.", platform_strategy.name());

    info!("🏆 {} rebuild workflow ({:?}) finished successfully!", platform_strategy.name(), activation_mode);
    Ok(())
}
2. New: src/pre_flight.rs - Defining and Orchestrating Pre-flight Checks

Rust

// src/pre_flight.rs
use crate::Result;
use crate::context::OperationContext;
use crate::workflow_strategy::PlatformRebuildStrategy;
use crate::error_handler; // Assuming report_failure lives here
use tracing::{info, warn, debug, error};
use color_eyre::eyre::{bail, eyre};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatusReport {
    Passed,
    PassedWithWarnings, // A check passed but issued warnings
    FailedCritical,     // A failure that should halt the process
    // FailedNonCritical, // If some checks can fail but allow continuation (not used by core checks yet)
}

pub trait PreFlightCheck: std::fmt::Debug + Send + Sync { // Send + Sync for potential parallelism
    fn name(&self) -> &str;
    // op_ctx provides common_args, verbose_count. platform_args if check is platform-specific.
    fn run<S: PlatformRebuildStrategy>(
        &self,
        op_ctx: &OperationContext,
        platform_strategy: &S, // For context if needed by check
        platform_args: &S::PlatformArgs,
    ) -> Result<CheckStatusReport>;
}

// --- Git PreFlightCheck ---
#[derive(Debug)]
pub struct GitPreFlightCheck;
impl PreFlightCheck for GitPreFlightCheck {
    fn name(&self) -> &str { "Git Status" }
    fn run<S: PlatformRebuildStrategy>(
        &self,
        _op_ctx: &OperationContext,
        _platform_strategy: &S,
        _platform_args: &S::PlatformArgs,
    ) -> Result<CheckStatusReport> {
        debug!("Running GitPreFlightCheck...");
        // Calls your existing logic from check_git.rs
        // run_git_check_warning_only currently prints warnings itself.
        // It should ideally return a list of warnings or a status.
        match crate::check_git::run_git_check_warning_only() {
            Ok(had_warnings) => { // Modify run_git_check_warning_only to return bool
                if had_warnings {
                    Ok(CheckStatusReport::PassedWithWarnings)
                } else {
                    Ok(CheckStatusReport::Passed)
                }
            }
            Err(e) => {
                warn!("Git check itself failed to execute: {}", e);
                // Decide if this is critical. For now, non-critical warning.
                Ok(CheckStatusReport::PassedWithWarnings)
            }
        }
    }
}

// --- Parse PreFlightCheck (using nix-instantiate for now) ---
#[derive(Debug)]
pub struct NixParsePreFlightCheck;
impl PreFlightCheck for NixParsePreFlightCheck {
    fn name(&self) -> &str { "Nix Syntax Parse" }
    fn run<S: PlatformRebuildStrategy>(
        &self,
        op_ctx: &OperationContext,
        platform_strategy: &S,
        platform_args: &S::PlatformArgs,
    ) -> Result<CheckStatusReport> {
        debug!("Running NixParsePreFlightCheck...");
        // This needs to adapt the logic from e.g. nixos.rs::run_parallel_parse_check
        // That function currently finds all .nix files and calls `nix-instantiate --parse`
        // It should be moved to a common utility, perhaps in this module.
        // For now, let's assume such a function exists and is called.
        // It should use error_handler::report_failure for errors.

        // Simplified conceptual call:
        // This needs to be specific to the platform's file finding logic if necessary.
        // Currently, run_parallel_parse_check is in nixos.rs, home.rs, etc.
        // Let's assume a placeholder for a generic one.
        let parse_check_result = match platform_strategy.name() {
            "NixOS" => crate::nixos::run_parallel_parse_check_for_platform(op_ctx.verbose_count),
            "HomeManager" => crate::home::run_parallel_parse_check_for_platform(op_ctx.verbose_count),
            "Darwin" => crate::darwin::run_parallel_parse_check_for_platform(op_ctx.verbose_count),
            _ => {
                warn!("Parse check not implemented for platform: {}", platform_strategy.name());
                return Ok(CheckStatusReport::Passed); // Or skipped
            }
        };

        match parse_check_result {
            Ok(_) => Ok(CheckStatusReport::Passed),
            Err(details) => {
                error_handler::report_failure( // Use your existing handler
                    "Parse Check",
                    "Syntax errors found in Nix files",
                    Some(details), // details from run_parallel_parse_check
                    Vec::new() // ErrorHandler will generate recommendations in Part 3
                );
                Ok(CheckStatusReport::FailedCritical)
            }
        }
    }
}
// TODO: In Part 3, NixParsePreFlightCheck will use nil-syntax.

// --- Lint PreFlightCheck ---
#[derive(Debug)]
pub struct LintPreFlightCheck;
impl PreFlightCheck for LintPreFlightCheck {
    fn name(&self) -> &str { "Lint & Format" }
    fn run<S: PlatformRebuildStrategy>(
        &self,
        op_ctx: &OperationContext,
        _platform_strategy: &S,
        _platform_args: &S::PlatformArgs,
    ) -> Result<CheckStatusReport> {
        debug!("Running LintPreFlightCheck...");
        let use_strict_lint = op_ctx.common_args.strict_lint || op_ctx.common_args.full_checks || op_ctx.common_args.medium_checks;
        match crate::lint::run_lint_checks(use_strict_lint, op_ctx.verbose_count) {
            Ok(lint_summary) => {
                match lint_summary.outcome {
                    Some(crate::lint::LintOutcome::Passed) => Ok(CheckStatusReport::Passed),
                    Some(crate::lint::LintOutcome::Warnings) => Ok(CheckStatusReport::PassedWithWarnings),
                    Some(crate::lint::LintOutcome::CriticalFailure(msg)) => {
                        // run_lint_checks likely already prints details.
                        // ErrorHandler might be used by run_lint_checks itself.
                        error!("Critical lint failure: {}", msg); // Ensure this is user-visible
                        Ok(CheckStatusReport::FailedCritical)
                    }
                    None => Ok(CheckStatusReport::Passed), // Or warnings if files were formatted
                }
            }
            Err(e) => {
                error_handler::report_failure("Lint", "Failed to run linters", Some(e.to_string()), vec![]);
                if use_strict_lint {
                    Ok(CheckStatusReport::FailedCritical)
                } else {
                    warn!("Linter execution failed: {}. Continuing as non-strict.", e);
                    Ok(CheckStatusReport::PassedWithWarnings) // Or just Passed if we don't want to halt
                }
            }
        }
    }
}


// Function to get the standard set of pre-flight checks
pub fn get_core_pre_flight_checks(_op_ctx: &OperationContext) -> Vec<Box<dyn PreFlightCheck>> {
    // In the future, op_ctx.config could disable/enable checks
    vec![
        Box::new(GitPreFlightCheck),
        Box::new(NixParsePreFlightCheck), // Uses nix-instantiate for now
        Box::new(LintPreFlightCheck),
        // TODO in Part 3: Add SemanticPreFlightCheck (using nil-ide)
        // TODO: Add EvalPreFlightCheck, DryRunBuildPreFlightCheck based on medium/full flags
    ]
}


#[instrument(skip_all, fields(platform = %platform_strategy.name()))]
pub fn run_shared_pre_flight_checks<S: PlatformRebuildStrategy>(
    op_ctx: &OperationContext,
    platform_strategy: &S,
    platform_args: &S::PlatformArgs,
    checks_to_run: &[Box<dyn PreFlightCheck>],
) -> Result<()> {
    if op_ctx.common_args.no_preflight {
        info!("[⏭️ Pre-flight] All checks skipped due to --no-preflight.");
        return Ok(());
    }
    info!("🚀 Running shared pre-flight checks for {}...", platform_strategy.name());
    let mut overall_status = CheckStatusReport::Passed;

    for check in checks_to_run {
        debug!("Executing pre-flight check: {}...", check.name());
        let pb = crate::progress::start_stage_spinner("Pre-flight", &format!("Running {} check", check.name()));
        match check.run(op_ctx, platform_strategy, platform_args) {
            Ok(CheckStatusReport::Passed) => {
                crate::progress::finish_stage_spinner_success(&pb, check.name(), "Check passed.");
            }
            Ok(CheckStatusReport::PassedWithWarnings) => {
                crate::progress::finish_stage_spinner_success(&pb, check.name(), "Check passed with warnings.");
                if overall_status == CheckStatusReport::Passed { // Don't override a failure
                    overall_status = CheckStatusReport::PassedWithWarnings;
                }
            }
            Ok(CheckStatusReport::FailedCritical) => {
                crate::progress::finish_spinner_fail(&pb);
                // Error message should have been printed by the check itself using ErrorHandler
                bail!("Critical pre-flight check '{}' failed. Aborting.", check.name());
            }
            Err(e) => { // Error in the check runner itself
                crate::progress::finish_spinner_fail(&pb);
                error_handler::report_failure(
                    "Pre-flight System Error",
                    &format!("Check '{}' failed to execute", check.name()),
                    Some(e.to_string()),
                    vec!["This indicates an issue with ng itself or its environment.".to_string()]
                );
                // Decide if any error in a check runner is critical
                bail!("Error executing pre-flight check '{}'. Aborting.", check.name());
            }
        }
    }

    match overall_status {
        CheckStatusReport::Passed => info!("✅ All pre-flight checks passed for {}.", platform_strategy.name()),
        CheckStatusReport::PassedWithWarnings => info!("⚠️ Pre-flight checks for {} completed with warnings.", platform_strategy.name()),
        CheckStatusReport::FailedCritical => {} // Should have bailed already
    }
    Ok(())
}

You will also need to move/refactor the run_parallel_parse_check logic from nixos.rs, home.rs, darwin.rs into a common place, perhaps also in pre_flight.rs, so NixParsePreFlightCheck can call it.

3. Modified: src/nixos.rs (and home.rs, darwin.rs similarly for their parse checks)

Remove the direct calls to run_git_check_warning_only, run_parallel_parse_check, lint::run_lint_checks from OsRebuildArgs::rebuild (and equivalents in home.rs, darwin.rs). These are now handled by execute_rebuild_workflow via run_shared_pre_flight_checks.
You'll need pub fn run_parallel_parse_check_for_platform(verbose_count: u8) -> Result<(), String> (or similar name) in each of nixos.rs, home.rs, darwin.rs that NixParsePreFlightCheck can call, or a single generic one in pre_flight.rs if the file finding logic is identical.
4. Modified: src/update.rs (Ensure it uses new command execution)
This should already be using crate::commands::Command which, if refactored in Part 1, uses util::run_cmd. Double check.

5. Modified: src/clean.rs (for manual --clean step)

Rust

// src/clean.rs
// ... (keep existing Generation, GenerationsTagged, ProfilesTagged, etc.)
// ... (keep profiles_in_dir, cleanable_generations, remove_path_nofail)

// New function for basic cleanup, callable by workflow
pub fn perform_basic_cleanup(verbose_count: u8, dry_run: bool) -> Result<()> {
    info!("Performing basic Nix store garbage collection...");
    let mut gc_cmd = std::process::Command::new("nix-store"); // Should eventually use NixInterface
    gc_cmd.arg("--gc");
    if dry_run {
        info!("Dry-run: Would execute: {:?}", gc_cmd);
        return Ok(());
    }
    util::add_verbosity_flags(&mut gc_cmd, verbose_count); // Your existing helper
    match util::run_cmd(&mut gc_cmd) { // Using run_cmd to capture output/error
        Ok(output) => {
            debug!("nix-store --gc stdout: {}", String::from_utf8_lossy(&output.stdout));
            debug!("nix-store --gc stderr: {}", String::from_utf8_lossy(&output.stderr));
            info!("Nix store garbage collection completed.");
            Ok(())
        }
        Err(e) => {
            // Use ErrorHandler in Part 3
            error!("nix-store --gc failed: {}", e);
            Err(eyre!("Nix store garbage collection failed during cleanup").wrap_err(e))
        }
    }
}


impl crate::interface::CleanMode {
    pub fn run(&self, verbose_count: u8) -> Result<()> {
        // ... your existing complex clean logic ...
        // Make sure all calls to `nix-store --gc` and `nix-store --optimise`
        // (either direct or via `commands::Command`) are using the
        // new `util::run_cmd` or `util::run_cmd_inherit_stdio`.
        // For example:
        // Command::new("sudo").args(["nix-store", "--gc"]).dry(args.dry).run()?;
        // should become:
        // let mut cmd = StdCommand::new("sudo"); cmd.args(["nix-store", "--gc"]);
        // if !args.dry { util::run_cmd_inherit_stdio(&mut cmd)?; } else { info!("Dry-run: sudo nix-store --gc");}

        // This function is mostly okay but ensure external commands use new util.
        // The logic for identifying profiles and generations is FS-based.

        // ...
        if !args.nogc {
            let pb_gc = crate::progress::start_stage_spinner("GC", "Performing garbage collection...");
            let mut gc_nix_cmd = StdCommand::new("nix-store");
            gc_nix_cmd.arg("--gc");
            // For sudo, it's tricky. Your Command builder handles sudo.
            // You'd need to replicate that or make util::run_cmd sudo-aware.
            // For now, assuming 'elevate' flag in commands::Command handles it.
            let cmd_builder = NgCommand::new("nix-store").arg("--gc").elevate(true).dry(args.dry);
            // This cmd_builder.run() must have been refactored in Part 1 to use util::run_cmd
            match cmd_builder.run() {
                 Ok(_) => crate::progress::finish_stage_spinner_success(&pb_gc, "GC", "Garbage collection completed."),
                 Err(e) => {
                    crate::progress::finish_spinner_fail(&pb_gc);
                    return Err(eyre!("GC failed").wrap_err(e));
                 }
            }
        }
        // Similar for --optimise
        // ...
        Ok(())
    }
}
6. Testing for Part 2:

execute_rebuild_workflow tests:
Test ng os switch (as it's the primary command being refactored through it).
Verify flake updates happen with --update.
Verify diff step with nvd (mock nvd or ensure it's in test env path).
Verify --ask confirmation works.
Verify --clean flag triggers perform_basic_cleanup (check logs).
PreFlightCheck tests:
Git: Test ng os switch in a clean repo, a repo with untracked .nix files (should warn), and with --no-preflight (should skip).
Parse: Test with syntactically correct .nix files. Test with a file containing a syntax error (should bail, and ErrorHandler should report it – even if basic report for now).
Lint: Test with clean files. If possible, set up files that alejandra or statix would change/warn about. Test with --strict-lint (should bail on critical lint error).
Ensure dry_run correctly skips build actualization and activation steps.
This completes the main items for Part 2 of the code generation. The execute_rebuild_workflow is now significantly more functional and incorporates the initial set of pre-flight checks using the new trait system. ng os switch is the main testbed for this.

Key things still to be done in Part 3 of code generation:

Deep nil-syntax integration into NixParsePreFlightCheck (replacing nix-instantiate --parse).
Implementing SemanticPreFlightCheck using nil-ide (RootDatabase, diagnostics).
Fully implementing NixInterface (potentially with nil-nix-interop).
Enhancing ErrorHandler to consume structured errors from nil-syntax and nil-ide.
Refactoring ng home, ng darwin, and other commands (clean fully, repl, info) to use the new workflow/NixInterface.
Implementing the automatic lazy cleaning feature and config file.
Comprehensive test suite expansion.
This incremental approach, while detailed, helps manage complexity and ensures each part is verifiable.

# Part 3
## Part 3- Plan

Okay, this is where ng truly transforms by deeply integrating the Nix language intelligence from oxalica/nil. Part 3 of 3 will focus on this integration for vastly improved error handling and diagnostics, fully implementing the NixInterface, refactoring the remaining commands, implementing automatic lazy cleaning, and ensuring comprehensive testing.

Recap of Goals for Part 3:

Deep oxalica/nil Integration: Leverage nil-syntax for AST/CST parsing and nil-ide for semantic diagnostics.
Enhanced ErrorHandler: Make it consume and present structured error information from oxalica/nil.
Full NixInterface: Centralize all nix CLI interactions, ideally using nil-nix-interop.
Refactor Remaining Commands: Bring home, darwin, clean, info, etc., into the new architecture.
Implement Automatic Lazy Cleaning.
Finalize Configuration & Comprehensive Testing.
Part 3: Deep Nix Intelligence, Command Refactoring, and Final Polish - Step-by-Step Moment Plan

Phase 8: Deep oxalica/nil Integration for Advanced Analysis & Error Handling

Moment 8.1: Add nil-syntax and nil-ide Dependencies

Action (Code - Cargo.toml): Add the necessary oxalica/nil crates. You'll likely need nil-syntax and parts of nil-ide (like nil-ide::base, nil-ide::def, nil-ide::diagnostics). Check oxalica/nil's own Cargo.toml for how they structure these if they are published as separate crates, or you might vendor/path-dependency them if building from source.
Test: cargo check to ensure dependencies resolve.
Rationale: Makes the core parsing and analysis libraries available.
Moment 8.2: Create NixAnalysisContext (or similar)

Action (Code - e.g., src/nix_analyzer.rs):
Rust

// src/nix_analyzer.rs
use nil_syntax::{SourceFile, AstNode, TextRange, SyntaxError};
use nil_ide::{
    base::{SourceDatabase, SourceDatabaseExt, FileId, Change, FileSource},
    def::DefDatabase, // For name resolution, etc.
    diagnostics::{self, Diagnostic, DiagnosticKind as NilDiagnosticKind}, // Renamed to avoid clash
    RootDatabase, // The central query database from nil-ide
    config::Config as NilConfig, // nil's config
};
use std::{sync::Arc, collections::HashMap, path::{Path, PathBuf}};
use crate::Result; // Your project's Result

#[derive(Debug)]
pub struct NgDiagnostic { // Your ng-specific diagnostic structure
    pub file_id_for_db: FileId, // Keep nil's FileId for context
    pub range: TextRange,
    pub message: String,
    pub severity: NgSeverity, // e.g., Error, Warning
    // Potentially add source (Syntax, Semantic, Liveness)
}

#[derive(Debug, Clone, Copy)]
pub enum NgSeverity { Error, Warning, Info, Hint }

pub struct NixAnalysisContext {
    db: RootDatabase,
    file_map: HashMap<PathBuf, FileId>, // Map your paths to nil's FileId
    next_file_id: u32,
}

impl NixAnalysisContext {
    pub fn new() -> Self {
        Self {
            db: RootDatabase::default(),
            file_map: HashMap::new(),
            next_file_id: 0,
        }
    }

    fn get_or_assign_file_id(&mut self, path: &Path) -> FileId {
        if let Some(id) = self.file_map.get(path) {
            return *id;
        }
        let file_id = FileId(self.next_file_id);
        self.next_file_id += 1;
        self.file_map.insert(path.to_path_buf(), file_id);
        // Initially set file source to be the path itself for nil's VFS
        self.db.set_file_source(file_id, FileSource::Local(path.to_path_buf()));
        file_id
    }

    pub fn parse_file_with_syntax(&mut self, path: &Path, content: Arc<String>) -> (FileId, Arc<SourceFile>, Vec<SyntaxError>) {
        let file_id = self.get_or_assign_file_id(path);
        let mut change = Change::new();
        change.change_file(file_id, Some(content));
        self.db.apply_change(change);
        let source_file = self.db.parse(file_id);
        let errors = source_file.errors().to_vec(); // Clone errors
        (file_id, source_file, errors)
    }

    pub fn get_semantic_diagnostics(&self, file_id: FileId, nil_config: &NilConfig) -> Vec<Diagnostic> {
        // This query will trigger parsing, name resolution, HIR lowering, etc., as needed by nil's DB.
        diagnostics::diagnostics(&self.db, file_id, nil_config)
    }

    // Helper to convert nil's Diagnostic to ng's NgDiagnostic
    pub fn convert_nil_diagnostic(d: &Diagnostic, file_id_for_db: FileId) -> NgDiagnostic {
        NgDiagnostic {
            file_id_for_db,
            range: d.range,
            message: d.message.clone(),
            severity: match d.severity {
                nil_ide::Severity::Error => NgSeverity::Error,
                nil_ide::Severity::Warning => NgSeverity::Warning,
                nil_ide::Severity::Info => NgSeverity::Info,
                nil_ide::Severity::Hint => NgSeverity::Hint,
            }
        }
    }
}
Test: Unit test NixAnalysisContext::new(), get_or_assign_file_id.
Rationale: Encapsulates nil-ide's RootDatabase and provides a clean interface for ng to parse files and get diagnostics.
Moment 8.3: Upgrade "Parse Pre-flight Check" with nil-syntax

Action (Code - in your PreFlightCheck implementation for Parsing, e.g., src/pre_flight.rs):
Modify the ParsePreFlightCheck::run method.
For each .nix file:
Read its content.
Use NixAnalysisContext::parse_file_with_syntax() to parse it.
If syntax_errors (from file.errors()) is not empty:
Convert each nil_syntax::Error into your NgDiagnostic format (or a temporary internal error struct). These errors usually contain a TextRange and a message.
Report these structured errors using ErrorHandler::report_failure (which will need to accept these new structured errors). The report should show the filename, range, and message.
Return CheckStatusReport::FailedCritical.
Test (Integration Test - assert_cmd + insta):
Create several .nix files with various syntax errors (missing semicolons, unmatched brackets, invalid tokens).
Run ng os switch (or any command that uses the workflow).
Verify: The "Parse Check" fails.
Snapshot Test (insta): Snapshot the output of ErrorHandler::report_failure. It should now be much more precise, highlighting the exact error location from nil-syntax. For example:
Error in file: /path/to/error.nix
Syntax Error at 5:10-5:11: Unexpected token ';', expected <expression>
Context:
3 | let
4 |   a = 1
5 |   b = ; # <--- Error here
6 | in a + b
Rationale: Replaces nix-instantiate --parse with superior, integrated parsing for syntax errors.
Moment 8.4: Implement "Semantic Pre-flight Check" using nil-ide

Action (Code - new SemanticPreFlightCheck in src/pre_flight.rs):
Create SemanticPreFlightCheck implementing PreFlightCheck.
Its run method will:
Iterate through .nix files relevant to the current operation (this might require some logic to determine which files are part of the current config's closure, or initially check all in CWD).
For each file, ensure it's parsed via NixAnalysisContext::parse_file_with_syntax(). If syntax errors exist, this check might wait or report that semantic analysis is partial.
Call NixAnalysisContext::get_semantic_diagnostics().
Convert nil_ide::diagnostics::Diagnostic into your NgDiagnostic format.
Report these using ErrorHandler::report_failure (for errors) or warn! (for warnings/hints).
Return CheckStatusReport::FailedCritical if there are errors, CheckStatusReport::Warnings if only warnings/hints, otherwise CheckStatusReport::Passed.
Add this check to the checks list in execute_rebuild_workflow.
Test (Integration Test - assert_cmd + insta):
Create .nix files with:
Undefined variables.
Unused let bindings (which nil_ide::diagnostics::liveness should find).
Run ng os switch.
Verify: The "Semantic Check" reports these issues.
Snapshot Test (insta): Snapshot ErrorHandler's output. Example:
Warning in file: /path/to/semantic.nix
Unused variable `unused_var` at 2:5-2:15
Context:
1 | let
2 |   unused_var = "I am not used";
3 |   used_var = "I am used";
4 | in used_var

Error in file: /path/to/semantic.nix
Undefined variable `non_existent` at 7:5-7:17
Context:
6 | let
7 |   a = non_existent + 1;
8 | in a
Rationale: Adds powerful static analysis beyond basic syntax, catching common errors early.
Moment 8.5: Enhance ErrorHandler to Consume Structured Diagnostics

Action (Code - src/error_handler.rs):
Modify report_failure (and related functions like generate_syntax_error_recommendations) to primarily work with NgDiagnostic (or the raw nil_syntax::Error and nil_ide::diagnostics::Diagnostic).
Use the TextRange to show code snippets with precise error highlighting (your existing enhance_syntax_error_output can be adapted for this, taking a TextRange and file content).
generate_syntax_error_recommendations can now look at the type of nil_syntax::Error or nil_ide::DiagnosticKind to give more specific advice than just regexing stderr. E.g., for NilDiagnosticKind::UndefinedVariable { name }, it could suggest checking for typos or adding it to inputs or args.
Test (Unit Test for ErrorHandler + insta):
Create mock NgDiagnostic objects representing various syntax and semantic errors.
Call report_failure and snapshot its output.
Test generate_syntax_error_recommendations with these structured inputs.
Rationale: Fully realizes the benefit of AST/CST parsing for user-facing error messages.
Phase 9: Full NixInterface Implementation & Usage

Moment 9.1: Implement Core NixInterface Methods

Action (Code - src/nix_interface.rs):
Flesh out methods like:
build_configuration(...) -> Result<PathBuf>: This should now fully encapsulate the logic from commands::Build::run(), including nom handling. It will construct the nix build command and execute it via util::run_cmd (or run_piped_commands if nom is used). It should parse JSON output if --json is supported by nix build for build results, or rely on the out-link.
evaluate_json(installable: &Installable, eval_flags: Option<EvalFlags>) -> Result<serde_json::Value>: Uses util::run_cmd to call nix eval --json <installable_args> [eval_flags] ... and parses the JSON output. (Consider using nil-nix-interop::eval functions here if they fit).
get_store_path_info(store_path: &Path, flags: Option<PathInfoFlags>) -> Result<NixPathInfo>: Calls nix path-info --json <path> and parses.
run_gc(dry_run: bool) and run_store_optimise(dry_run: bool): Call nix-store --gc and nix-store --optimise via util::run_cmd.
These methods should return structured Results, with Ok values being parsed data and Err values being UtilCommandError (which ErrorHandler can then process, possibly by fetching Nix logs if it's a build failure).
Test (Unit Tests for NixInterface methods):
Mock util::run_cmd to return predefined Ok(Output) or Err(UtilCommandError).
Test that NixInterface methods construct the correct nix CLI arguments.
Test that they correctly parse JSON output from (mocked) nix commands.
Test error propagation.
Rationale: Centralizes all nix CLI interaction, making the rest of ng independent of direct nix CLI calls.
Moment 9.2: Integrate NixInterface into execute_rebuild_workflow and OperationContext

Action (Code - src/workflow_executor.rs, src/context.rs):
Add a nix_interface: NixInterface field to OperationContext.
execute_rebuild_workflow's "Build Configuration" step should now call op_ctx.nix_interface.build_configuration(...).
Any other places that were calling nix directly (e.g., cleanup step in nixos.rs for nix store gc) should be refactored to use op_ctx.nix_interface.
Test (Integration Test): Re-run ng os switch tests. Verify builds, GCs (if part of --clean) still work, now via NixInterface. Build failures should still be handled by ErrorHandler.
Rationale: NixInterface is now the live gatekeeper for Nix operations.
Phase 10: Refactor Remaining Commands (Home Manager, Darwin, Clean, etc.)

Moment 10.1: Implement HomeManagerPlatformStrategy

Action (Code - src/home.rs or src/home_strategy.rs):
Create HomeManagerPlatformStrategy struct.
Implement PlatformRebuildStrategy trait for it.
get_toplevel_installable: Adapt logic from home.rs::toplevel_for.
get_current_profile_path: Determine path to current home-manager profile (e.g., ~/.local/state/nix/profiles/home-manager or /nix/var/nix/profiles/per-user/$USER/home-manager).
activate_configuration: Run the activate script from the built home-manager profile (via NixInterface if it's just running an executable, or util::run_cmd).
Test (Unit Tests for HomeManagerPlatformStrategy).
Rationale: Strategy for Home Manager.
Moment 10.2: Refactor ng home switch/build/repl

Action (Code - src/home.rs, src/interface.rs):
Adapt HomeRebuildArgs and HomeReplArgs for consistency with CommonRebuildArgs.
Modify HomeArgs::run to call execute_rebuild_workflow with HomeManagerPlatformStrategy for switch/build.
Refactor repl to use NixInterface for calling nix repl.
Test (Integration Test - assert_cmd):
Create a minimal working flake.nix with a homeConfigurations.testuser (use a fixed username for testing).
Test ng home switch --flake .#testuser (or equivalent). Verify success and basic error paths.
Test ng home build.
Test ng home repl.
Rationale: Home Manager commands now use the new architecture.
Moment 10.3: Implement DarwinPlatformStrategy and Refactor ng darwin commands

Action: Similar to Home Manager: define DarwinPlatformStrategy, adapt DarwinRebuildArgs/DarwinReplArgs, modify DarwinArgs::run to use the workflow and NixInterface.
Test (Integration Test): Requires a Darwin environment or careful mocking. Focus on ensuring the command flow and argument construction are correct if full execution isn't feasible in all test environments. The existing Darwin test in your CI (Test_Darwin in build.yaml) is a good end-to-end check.
Rationale: Darwin commands aligned.
Moment 10.4: Refactor ng clean

Action (Code - src/clean.rs):
The core logic of identifying profiles and generations to clean (profiles_in_dir, cleanable_generations) is largely FS-based and can remain.
Calls to nix-store --gc and nix-store --optimise should go through NixInterface::run_gc() and NixInterface::run_store_optimise().
Ensure it uses the standardized util::run_cmd for any other helper commands if needed.
Test (Unit/Integration): Test the logic for identifying generations. Mock FS for some tests. Test that the correct NixInterface methods are called for GC/optimise (mock NixInterface or check logs if it logs the commands it runs). Test --dry-run and --ask.
Rationale: ng clean aligns with new Nix interaction patterns.
Moment 10.5: Review ng os info, ng search, ng completions

Action (Code - respective modules):
ng os info (generations.rs): Mostly FS ops. If it ever calls nix path-info, use NixInterface.
ng search: Seems self-contained with reqwest. Likely minimal changes unless it needs to resolve local flake paths or Nix store paths for some reason.
ng completions: Generated by clap. No changes expected from this refactor.
Test: Ensure existing tests for these commands pass.
Rationale: Ensure all commands are touched by the refactor where relevant.
Phase 11: Implement Automatic Lazy Cleaning & Configuration File

Moment 11.1: Define ng Configuration Structure and Loading

Action (Code - e.g., src/config.rs):
Define structs for NgConfig, AutoCleanSettings, etc. (e.g., using serde for deserialization).
Implement logic to load config from a standard location (e.g., ~/.config/ng/config.toml) at ng startup. If no file, use defaults. Store the loaded config in OperationContext or a global static (using once_cell or similar, if appropriate for your app structure).
Test (Unit Test): Test config loading with a sample TOML file, test default values.
Rationale: Persistent configuration for ng.
Moment 11.2: Implement Auto-Clean Logic in execute_rebuild_workflow

Action (Code - src/workflow_executor.rs, src/clean.rs):
In execute_rebuild_workflow, after successful activation and post-hooks:
Check NgConfig.auto_clean.enabled and if the current command type matches auto_clean.on_success_for.
If yes, call a refactored version of the cleaning logic from clean.rs. This cleaning function should take keep_count, keep_days from NgConfig.auto_clean and the specific profile path(s) to clean (e.g., from PlatformRebuildStrategy::get_main_profile_path_for_cleanup()).
It should also respect NgConfig.auto_clean.run_gc and run_optimise.
Ensure this auto-clean does not run if the explicit --clean flag was passed for the rebuild command (to avoid double work or conflicting settings).
Test (Integration Test):
Enable auto-clean in a test config file.
Run ng os switch multiple times to create several generations.
Verify that after a successful switch, old generations (for /nix/var/nix/profiles/system) are automatically removed according to the auto-clean config.
Test that GC/optimise run (or don't) based on config.
Test that if --clean is given, auto-clean for that run is skipped.
Rationale: Implements the desired lazy auto-cleaning feature.
Phase 12: Final Polish and Comprehensive Testing Review

Moment 12.1: Code Review & Refinement

Action: Review all new and refactored code for clarity, consistency, error handling, and adherence to Rust best practices.
Add comments, ensure documentation is updated.
Run cargo clippy and cargo fmt.
Moment 12.2: Test Coverage Analysis & Augmentation (Your Point 8)

Action: Use tools like cargo-tarpaulin or grcov to assess test coverage.
Identify and write tests for any critical paths or error conditions not yet covered by the incremental tests.
Pay special attention to:
All branches in execute_rebuild_workflow.
All PlatformRebuildStrategy method implementations.
ErrorHandler with diverse inputs (structured nil errors, UtilCommandError, Nix CLI stderr).
NixInterface methods and their error parsing.
Auto-clean logic under various configurations.
Moment 12.3: Documentation Updates

Action: Update README.md, CLI help texts (clap attributes), and any other user documentation to reflect the new behaviors, improved error messages, configuration file, and auto-clean feature.
Moment 12.4: Performance Sanity Checks

Action: While full benchmarking was suggested earlier, do some manual timing of common operations (e.g., ng os switch on a small config) to ensure the refactoring hasn't introduced major performance regressions. The new parsing/analysis steps will add some overhead, but it should be acceptable for the UX benefits.
If significant slowdowns are noticed, profile and investigate.
This concludes the 3-part plan. It's a massive undertaking, but by following these extremely incremental steps and testing rigorously at each "moment," you can systematically transform ng into a much more powerful, robust, and user-friendly tool with significantly improved Nix error handling and logging, all while building on your existing codebase and well-thought-out initial suggestions.

## Part 3- Code

Phase 8 (Continued): Deep oxalica/nil Integration (The Core of Part 3)

Moment 8.1 (Revisit): Ensure Cargo.toml has oxalica/nil crate dependencies.

Action: You'll need to add nil-syntax, nil-ide (or its specific sub-crates like nil-ide-base, nil-ide-def, nil-ide-diagnostics), and potentially nil-nix-interop. The exact names will depend on how they are published or if you are using path dependencies to a local checkout of oxalica/nil.
Ini, TOML

# Cargo.toml
[dependencies]
# ... other ng dependencies ...
nil-syntax = { version = "0.1.0" } # Replace with actual version or path
nil-ide = { version = "0.1.0" }    # Replace with actual version or path
# Or specific sub-crates if preferred:
# nil-ide-base = "..."
# nil-ide-def = "..."
# nil-ide-diagnostics = "..."
# nil-nix-interop = { version = "0.1.0", optional = true } # If used by NixInterface
# ...
Test: cargo check.
Rationale: Makes the libraries available.
Moment 8.2 (Revisit & Flesh out): src/nix_analyzer.rs - NixAnalysisContext

Action (Code): Ensure NixAnalysisContext from Part 2's plan is fully defined. It manages nil_ide::RootDatabase and provides methods to parse files and get diagnostics.
Rust

// src/nix_analyzer.rs (ensure it's complete from Part 2 plan)
// Key methods:
// pub fn new() -> Self
// pub fn parse_file_with_syntax(&mut self, path: &Path, content: Arc<String>) -> (FileId, Arc<SourceFile>, Vec<SyntaxError>)
// pub fn get_semantic_diagnostics(&self, file_id: FileId, nil_config: &NilConfig) -> Vec<NilIdeDiagnostic> // aliased Diagnostic
// pub fn convert_nil_diagnostic_to_ng(d: &NilIdeDiagnostic, file_id_for_db: FileId, file_path: &Path) -> NgDiagnostic
// pub fn convert_nil_syntax_error_to_ng(e: &SyntaxError, file_id_for_db: FileId, file_path: &Path) -> NgDiagnostic
// pub fn get_file_content(&self, file_id: FileId) -> Option<Arc<String>>
Your NgDiagnostic struct should now also store the original file_path: PathBuf for user messages.
Test (Unit Tests): Test file registration, parsing a simple valid Nix string, parsing a string with a known syntax error and checking the Vec<SyntaxError>, getting content.
Rationale: Central hub for ng's own Nix code analysis.
Moment 8.3 (Upgrade): src/pre_flight.rs - NixParsePreFlightCheck with nil-syntax

Action (Code): Modify NixParsePreFlightCheck::run.
It should use a (potentially shared or newly created) NixAnalysisContext instance.
For each .nix file to check:
Read content.
Call analyzer.parse_file_with_syntax(path, Arc::new(content)).
If the returned Vec<SyntaxError> is not empty, convert these errors to NgDiagnostic and pass them to a new method in ErrorHandler (e.g., ErrorHandler::report_structured_syntax_errors).
Return CheckStatusReport::FailedCritical if any syntax errors.
Rust

// src/pre_flight.rs - inside NixParsePreFlightCheck::run
// ... (obtain or create NixAnalysisContext instance, perhaps from op_ctx if added there)
// For simplicity, assuming analyzer is available or created per run here.
// let mut analyzer = crate::nix_analyzer::NixAnalysisContext::new();
// let all_nix_files = find_all_nix_files_for_project(); // You need a helper for this

// for path_to_nix_file in all_nix_files {
//     let content = match std::fs::read_to_string(&path_to_nix_file) {
//         Ok(c) => Arc::new(c),
//         Err(e) => {
//             warn!("Failed to read Nix file {} for parsing: {}", path_to_nix_file.display(), e);
//             continue;
//         }
//     };
//     let (file_id, _source_file, syntax_errors) = analyzer.parse_file_with_syntax(&path_to_nix_file, content);
//     if !syntax_errors.is_empty() {
//         let ng_diagnostics: Vec<_> = syntax_errors.iter()
//             .map(|se| analyzer.convert_nil_syntax_error_to_ng(se, file_id, &path_to_nix_file))
//             .collect();
//         crate::error_handler::report_ng_diagnostics("Syntax Check", &ng_diagnostics, &analyzer);
//         had_critical_errors = true; // Manage this flag
//     }
// }
// if had_critical_errors { return Ok(CheckStatusReport::FailedCritical) }
Test (Integration assert_cmd + insta):
Run ng os switch with a Nix file containing a clear syntax error (e.g., let a = 1; in a b c without operator).
Verify the "Parse Check" fails and the ErrorHandler output (snapshotted with insta) shows the precise error location and message derived from nil-syntax.
Rationale: Replaces nix-instantiate --parse with much richer, integrated syntax error reporting.
Moment 8.4 (Implement): src/pre_flight.rs - SemanticPreFlightCheck with nil-ide

Action (Code): Create SemanticPreFlightCheck.
Its run method uses NixAnalysisContext. For each (syntactically valid) .nix file:
Call analyzer.get_semantic_diagnostics(file_id, &NilConfig::default()).
Convert returned nil_ide::diagnostics::Diagnostic to NgDiagnostic.
Report these using ErrorHandler::report_ng_diagnostics.
Determine CheckStatusReport based on severity (e.g., Error -> FailedCritical, Warning -> PassedWithWarnings).
Add this to get_core_pre_flight_checks in pre_flight.rs, likely after the syntax parse check.
Test (Integration assert_cmd + insta):
Create Nix files with:
Undefined variables (a = b + 1; where b is not defined).
Unused let bindings (let unused = 1; in "foo").
Run ng os switch. Verify "Semantic Check" reports these issues correctly via ErrorHandler. Snapshot the output.
Test with --strict-lint (or a new --strict-semantic) ensuring critical semantic errors halt the process.
Rationale: Adds powerful static analysis for common logical errors in Nix code.
Moment 8.5 (Upgrade): src/error_handler.rs - Consume Structured Diagnostics

Action (Code):
Create report_ng_diagnostics(stage: &str, diagnostics: &[NgDiagnostic], analyzer: &NixAnalysisContext).
This function iterates through NgDiagnostics. For each:
Prints stage, severity, file path, and the diagnostic message.
Uses diagnostic.range and analyzer.get_file_content(diagnostic.file_id_for_db) to fetch the relevant lines of source code.
Prints the code snippet with the TextRange highlighted (e.g., using carets ^ or different coloring). Your existing enhance_syntax_error_output can be heavily adapted for this.
generate_syntax_error_recommendations (and a new generate_semantic_error_recommendations) should take an NgDiagnostic (or the original nil diagnostic type) to provide more targeted advice based on DiagnosticKind.
E.g., for NilDiagnosticKind::UndefinedVariable { name }, suggest checking for typos or adding to let bindings or function arguments.
For NilDiagnosticKind::UnusedBinding, suggest removing it or prefixing with _.
Test (Unit Tests for ErrorHandler with insta):
Craft various NgDiagnostic instances (for syntax errors, undefined vars, unused lets).
Call report_ng_diagnostics and snapshot the formatted output. Verify code snippets and highlighting.
Test recommendation generation for different diagnostic kinds.
Rationale: This is the payoff for nil-syntax/ide integration – vastly superior user-facing error messages.
Phase 9 (Continued): Full NixInterface Implementation

Moment 9.1 (Revisit & Flesh Out): src/nix_interface.rs

Action (Code):
If nil-nix-interop is mature and covers common needs (eval --json, build --json (or build and get out-link reliably), path-info --json, store gc/optimise), NixInterface methods should primarily wrap calls to nil-nix-interop functions. This is the ideal "Unix philosophy" approach internally.
If nil-nix-interop is not yet a full fit for some commands or error reporting details ng needs:
NixInterface methods will construct std::process::Command for nix ... calls.
Crucially, they must use appropriate flags like --json for output where available.
They will use util::run_cmd to execute and get Output.
They will then parse the stdout (if JSON) or stderr (if errors) into structured Result types (e.g., Result<serde_json::Value, NixInterfaceError>, Result<NixPathInfo, NixInterfaceError>).
NixInterfaceError (defined in Part 1 thinking) should have variants like NixCliFailed(UtilCommandError), OutputJsonParseFailed(serde_json::Error, String), NixEvalErrorDetailed { /* from nix json error output */ }.
Ensure build_configuration in NixInterface is robust. If nom is used, it still needs to pipe nix build --log-format internal-json -v to nom --json, and NixInterface should manage this pipeline (using util::run_piped_commands from your plan). The final result is still the store path.
Test (Unit Tests for NixInterface):
For each method, mock util::run_cmd (or nil-nix-interop calls).
Provide mock std::process::Output with sample JSON success output from nix and test successful parsing.
Provide mock std::process::Output with sample JSON error output from nix (some nix commands can output errors as JSON) and test error parsing.
Provide mock std::process::Output with non-zero status and plain stderr, test conversion to NixInterfaceError.
Rationale: All Nix CLI interactions are now robustly handled and abstracted.
Moment 9.2 (Revisit): OperationContext and execute_rebuild_workflow use NixInterface

Action (Code):
Add nix_interface: NixInterface to OperationContext.
The "Build Configuration" step in execute_rebuild_workflow must now call op_ctx.nix_interface.build_configuration(...).
Calls to nix-store --gc (in manual --clean or auto-clean) should use op_ctx.nix_interface.run_gc().
Test (Integration Test): Re-run ng os switch tests. Verify successful builds and build failures are handled correctly through the NixInterface and ErrorHandler. Snapshot error outputs.
Rationale: The workflow now correctly uses the abstracted Nix interaction layer.
Phase 10 (Continued): Refactor Remaining Commands

Moment 10.1 & 10.2 (Revisit): HomeManagerPlatformStrategy & ng home commands

Action (Code):
Complete HomeManagerPlatformStrategy.
get_toplevel_installable: Use op_ctx.nix_interface.evaluate_json (if needed to check existence of username@hostname vs username attributes) and existing logic from home.rs::toplevel_for to construct the Installable for homeConfigurations.<...>.activationPackage.
activate_configuration: Will execute the activate script from the built home-manager profile. This is a direct script execution, likely via util::run_cmd_inherit_stdio.
Refactor src/home.rs::HomeArgs::run for switch and build to call execute_rebuild_workflow with this strategy.
Refactor ng home repl to determine its target installable and call op_ctx.nix_interface.run_repl(...) (a new method in NixInterface).
Test (Integration assert_cmd): Basic ng home switch, ng home build, ng home repl tests using a minimal Home Manager flake. Test error handling for build failures.
Moment 10.3 (Revisit): DarwinPlatformStrategy & ng darwin commands

Action: Similar to Home Manager. The activate_configuration for Darwin will involve running darwin-rebuild <action> or the specific activation scripts like /run/current-system/activate and potentially /run/current-system/activate-user. These calls go through NixInterface (if they are nix subcommands) or util::run_cmd (if they are direct scripts, possibly with sudo).
Test: Similar integration tests, ideally in a Darwin CI environment.
Moment 10.4 (Revisit): ng clean

Action (Code - src/clean.rs):
Ensure the main CleanMode::run logic for identifying profiles and generations is sound.
All calls to nix-store --gc and nix-store --optimise (which require sudo) MUST go through op_ctx.nix_interface.run_gc(op_ctx.common_args.dry_run) and op_ctx.nix_interface.run_store_optimise(op_ctx.common_args.dry_run). The NixInterface methods will handle sudo if necessary (or ng passes an elevate: bool hint).
Test (Integration): Test ng clean all --dry-run, ng clean user --dry-run. Mocking the FS for generation listing might be needed for precise unit tests. Assert that NixInterface methods for GC/optimise are called (e.g., via logs or mock NixInterface in unit tests).
Moment 10.5 (Revisit): ng os info, ng search

Action:
ng os info (generations.rs): If it needs to resolve the canonical store path of /run/current-system or other profile links, or get metadata about them using nix path-info, it should use op_ctx.nix_interface.get_store_path_info().
ng search: Likely remains unchanged unless it needs to evaluate local flakes for some reason.
Test: Ensure existing tests pass.
Phase 11 (Continued): Implement Automatic Lazy Cleaning & Configuration File

Moment 11.1 (Revisit): ng Configuration File

Action (Code - new src/config.rs):
Rust

// src/config.rs
use serde::Deserialize;
use std::path::PathBuf;
use color_eyre::eyre::{Result, WrapErr};

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct NgConfig {
    pub auto_clean: Option<AutoCleanSettings>,
    // pub default_verbosity: Option<u8>,
    // pub default_flake_ref: Option<String>,
}

#[derive(Debug, Deserialize, Clone)] // Clone for use in OperationContext
#[serde(rename_all = "kebab-case")]
pub struct AutoCleanSettings {
    pub enabled: bool,
    #[serde(default = "default_auto_clean_commands")]
    pub on_success_for: Vec<String>, // "switch", "boot", "test"
    #[serde(default = "default_auto_clean_keep_count")]
    pub keep_count: u32,
    #[serde(default = "default_auto_clean_keep_days")]
    pub keep_days: u64, // Store as days, convert to duration later
    #[serde(default)]
    pub run_gc: bool,
    #[serde(default)]
    pub run_optimise: bool,
    #[serde(default = "default_auto_clean_target_profile")]
    pub target: AutoCleanTarget,
}
// Defaults for serde
fn default_auto_clean_commands() -> Vec<String> { vec!["switch".to_string(), "boot".to_string()] }
fn default_auto_clean_keep_count() -> u32 { 3 }
fn default_auto_clean_keep_days() -> u64 { 14 }
fn default_auto_clean_target_profile() -> AutoCleanTarget { AutoCleanTarget::CurrentPlatformProfile }


#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AutoCleanTarget {
    CurrentPlatformProfile, // Cleans system for OS, home for Home, etc.
    // AllUserProfiles, // Potentially too aggressive for auto
    // AllSystemProfiles,
}


impl NgConfig {
    pub fn load() -> Result<Self> {
        // Determine config file path (e.g., ~/.config/ng/config.toml or XDG_CONFIG_HOME)
        let config_path_opt = dirs::config_dir().map(|p| p.join("ng/config.toml"));
        if let Some(config_path) = config_path_opt {
            if config_path.exists() {
                tracing::debug!("Loading ng config from: {}", config_path.display());
                let content = std::fs::read_to_string(&config_path)
                    .wrap_err_with(|| format!("Failed to read ng config file at {}", config_path.display()))?;
                let config: NgConfig = toml::from_str(&content)
                    .wrap_err_with(|| format!("Failed to parse ng config file at {}", config_path.display()))?;
                return Ok(config);
            } else {
                tracing::debug!("ng config file not found at {}, using defaults.", config_path.display());
            }
        }
        Ok(NgConfig::default())
    }
}
Load this in main.rs and make it accessible, perhaps by adding config: Arc<NgConfig> to OperationContext.
Test (Unit): Test loading a sample TOML, test default values.
Moment 11.2 (Revisit): Auto-Clean Logic in execute_rebuild_workflow

Action (Code - src/workflow_executor.rs):
Rust

// In execute_rebuild_workflow, after successful post_rebuild_hook
if activation_mode != ActivationMode::Build && !op_ctx.common_args.dry_run && !op_ctx.common_args.clean_after { // Don't auto-clean if --clean was specified
    if let Some(auto_clean_settings) = op_ctx.config.auto_clean.as_ref() { // Assuming op_ctx.config
        if auto_clean_settings.enabled &&
           auto_clean_settings.on_success_for.contains(&activation_mode_to_string(&activation_mode)) {
            info!("🚀 Performing automatic lazy cleanup for {}...", platform_strategy.name());
            let pb_auto_clean = crate::progress::start_stage_spinner("AutoClean", "Performing automatic cleanup...");
            match crate::clean::perform_automatic_cleanup(op_ctx, platform_strategy, platform_args, auto_clean_settings) {
                Ok(_) => crate::progress::finish_stage_spinner_success(&pb_auto_clean, "AutoClean", "Automatic cleanup completed."),
                Err(e) => {
                    crate::progress::finish_spinner_fail(&pb_auto_clean);
                    warn!("Automatic cleanup failed (non-critical): {}", e);
                }
            }
        }
    }
}
// helper
fn activation_mode_to_string(mode: &ActivationMode) -> String {
    format!("{:?}", mode).to_lowercase()
}
Action (Code - src/clean.rs):
Create perform_automatic_cleanup<S: PlatformRebuildStrategy>(op_ctx: &OperationContext, strategy: &S, platform_args: &S::PlatformArgs, settings: &AutoCleanSettings) -> Result<().
This function will:
Determine the primary profile path to clean using strategy.get_main_profile_path_for_cleanup() (new method in trait).
Call your existing cleanable_generations and remove_path_nofail for that specific profile path, using settings.keep_count and settings.keep_days.
If settings.run_gc or settings.run_optimise are true, call op_ctx.nix_interface.run_gc(false) etc.
Test (Integration): Configure auto-clean in a test config. Run ng os switch. Verify old generations for the system profile are cleaned according to settings.
Phase 12 (Revisit): Final Polish and Comprehensive Testing Review

Moment 12.1-12.4: As outlined in the plan (code review, test coverage, docs, performance sanity checks). This is where you ensure everything ties together.
Crucial Test: End-to-end tests for the "happy path" of ng os switch, ng home switch, ng darwin switch showing all pre-flight checks (syntax, semantic) passing, build, diff, confirm, activate, and auto-clean all working together.
Crucial Test: Test complex error scenarios. E.g., a Nix file with both a syntax error reported by nil-syntax AND a semantic error in another part that nil-ide would find. Ensure ErrorHandler presents this clearly. Test what happens if a Nix build fails and NixInterface tries to parse its error output.
This concludes the conceptual code generation for Part 3. ng should now be a significantly more intelligent and robust tool. The integration of oxalica/nil's parsing and analysis capabilities will make its error handling and diagnostics for Nix code issues state-of-the-art. The NixInterface centralizes Nix interactions, and all major commands follow a consistent workflow.

Remember, this is a blueprint. The actual implementation will involve significant effort in coding, testing, and refining these interactions, especially the error flow and the setup of nil-ide's RootDatabase. Good luck!
