extern crate semver;

use std::path::{Path, PathBuf};
// use std::process::Command; // Removed, using std::process::Command directly
use std::str;

use color_eyre::{eyre, Result};
use semver::Version;
use tempfile::TempDir;
use tracing::{debug, warn};
use thiserror::Error;
use walkdir::WalkDir;

#[derive(Debug, Error)]
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
    #[error("Command '{command_str}' (inheriting stdio) exited with status {status_code}.")]
    InheritedNonZeroStatus {
        command_str: String,
        status_code: String,
    },
}

/// Compares two semantic versions and returns their order.
///
/// This function takes two version strings, parses them into `semver::Version` objects, and compares them.
/// It returns an `Ordering` indicating whether the current version is less than, equal to, or
/// greater than the target version.
///
/// # Arguments
///
/// * `current` - A string slice representing the current version.
/// * `target` - A string slice representing the target version to compare against.
///
/// # Returns
///
/// * `Result<std::cmp::Ordering>` - The comparison result.
pub fn compare_semver(current: &str, target: &str) -> Result<std::cmp::Ordering> {
    let current_ver = Version::parse(current)?;
    let target_ver = Version::parse(target)?;
    Ok(current_ver.cmp(&target_ver))
}

/// Retrieves the installed Nix version as a string.
///
/// This function executes the `nix --version` command, parses the output to extract the version string,
/// and returns it. If the version string cannot be found or parsed, it returns an error.
///
/// # Returns
///
/// * `Result<String>` - The Nix version string or an error if the version cannot be retrieved.
pub fn get_nix_version() -> Result<String> {
    let output = std::process::Command::new("nix").arg("--version").output()?;
    let output_str = str::from_utf8(&output.stdout)?;
    let version_str = output_str
        .lines()
        .next()
        .ok_or_else(|| eyre::eyre!("No version string found"))?;
    let re = regex::Regex::new(r"\d+\.\d+\.\d+")?;
    if let Some(captures) = re.captures(version_str) {
        let version = captures
            .get(0)
            .ok_or_else(|| eyre::eyre!("No version match found"))?
            .as_str();
        return Ok(version.to_string());
    }
    Err(eyre::eyre!("Failed to extract version"))
}

pub trait MaybeTempPath: std::fmt::Debug {
    fn get_path(&self) -> &Path;
}

impl MaybeTempPath for PathBuf {
    fn get_path(&self) -> &Path {
        self.as_ref()
    }
}

impl MaybeTempPath for (PathBuf, TempDir) {
    fn get_path(&self) -> &Path {
        self.0.as_ref()
    }
}

pub fn get_hostname() -> Result<String> {
    #[cfg(not(target_os = "macos"))]
    {
        use color_eyre::eyre::Context;
        Ok(hostname::get()
            .context("Failed to get hostname")?
            .to_str()
            .unwrap()
            .to_string())
    }
    #[cfg(target_os = "macos")]
    {
        use color_eyre::eyre::bail;
        use system_configuration::{
            core_foundation::{base::TCFType, string::CFString},
            sys::dynamic_store_copy_specific::SCDynamicStoreCopyLocalHostName,
        };
        let ptr = unsafe { SCDynamicStoreCopyLocalHostName(std::ptr::null()) };
        if ptr.is_null() {
            bail!("Failed to get hostname");
        }
        let name = unsafe { CFString::wrap_under_get_rule(ptr) };
        Ok(name.to_string())
    }
}

/// Adds verbosity flags to a command based on the verbosity level.
pub fn add_verbosity_flags(cmd: &mut std::process::Command, verbosity_level: u8) {
    let effective_level = std::cmp::min(verbosity_level, 7);
    for _ in 0..effective_level {
        cmd.arg("-v");
    }
}

/// Runs a command and captures its output.
pub fn run_cmd(command: &mut std::process::Command) -> Result<std::process::Output, UtilCommandError> {
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
        debug!("Stderr: {}", stderr);
        debug!("Stdout: {}", stdout);
        return Err(UtilCommandError::NonZeroStatus {
            command_str,
            status_code: output.status.code().map_or_else(|| "unknown".to_string(), |c| c.to_string()),
            stdout,
            stderr,
        });
    }
    Ok(output)
}

/// Runs a command with inherited stdio.
pub fn run_cmd_inherit_stdio(command: &mut std::process::Command) -> Result<std::process::ExitStatus, UtilCommandError> {
    use std::process::Stdio;
    let command_str = format!("{:?}", command);
    debug!("Executing command with inherited stdio: {:?}", command_str);
    let status = command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
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
        return Err(UtilCommandError::InheritedNonZeroStatus {
            command_str,
            status_code: status.code().map_or_else(|| "unknown".to_string(), |c| c.to_string()),
        });
    }
    Ok(status)
}

/// Checks if stdout is connected to a terminal.
pub fn is_stdout_tty() -> bool {
    atty::is(atty::Stream::Stdout)
}

/// Manages the output path for Nix builds.
pub fn manage_out_path(out_link_opt: Option<&PathBuf>) -> Result<Box<dyn MaybeTempPath>> {
    use color_eyre::eyre::WrapErr;
    match out_link_opt {
        Some(p) => {
            if let Some(parent_dir) = p.parent() {
                std::fs::create_dir_all(parent_dir)
                    .wrap_err_with(|| format!("Failed to create parent directory for out-link: {:?}", parent_dir))?;
            }
            Ok(Box::new(p.clone()))
        }
        None => {
            let temp_dir = TempDir::new_in(std::env::temp_dir())
                .wrap_err("Failed to create temporary directory for build output")?;
            let path = temp_dir.path().join("result");
            Ok(Box::new((path, temp_dir)))
        }
    }
}

/// Runs two commands in a pipeline.
pub fn run_piped_commands(
    mut cmd1_builder: std::process::Command,
    mut cmd2_builder: std::process::Command,
) -> Result<std::process::Output, UtilCommandError> {
    use std::process::Stdio;
    let cmd1_str = format!("{:?}", cmd1_builder);
    let cmd2_str = format!("{:?}", cmd2_builder);
    debug!("Executing piped command: {:?} | {:?}", cmd1_str, cmd2_str);
    let mut child1 = cmd1_builder
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| UtilCommandError::SpawnFailed {
            command_str: cmd1_str.clone(),
            io_error: e
        })?;
    let child1_stdout = child1.stdout.take().ok_or_else(|| UtilCommandError::SpawnFailed {
        command_str: cmd1_str.clone(),
        io_error: std::io::Error::new(std::io::ErrorKind::Other, "cmd1 stdout missing"),
    })?;
    let child2 = cmd2_builder
        .stdin(child1_stdout)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| UtilCommandError::SpawnFailed {
            command_str: cmd2_str.clone(),
            io_error: e
        })?;
    let output2 = child2.wait_with_output().map_err(|e| UtilCommandError::SpawnFailed {
        command_str: format!("wait_with_output for {:?}", cmd2_builder),
        io_error: e,
    })?;
    let _ = child1.wait(); // Wait for child1 to avoid zombies
    if !output2.status.success() {
        return Err(UtilCommandError::NonZeroStatus {
            command_str: format!("{:?} | {:?}", cmd1_str, cmd2_str),
            status_code: output2.status.code().map_or_else(|| "unknown".to_string(), |c| c.to_string()),
            stdout: String::from_utf8_lossy(&output2.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output2.stderr).into_owned(),
        });
    }
    Ok(output2)
}

/// Checks if a command exists in the PATH.
pub fn command_exists(cmd_name: &str) -> bool {
    let cmd_version = crate::commands::Command::new(cmd_name).arg("--version");
    match cmd_version.run() {
        Ok(_) => true,
        Err(_) => {
            let cmd_help = crate::commands::Command::new(cmd_name).arg("--help");
            cmd_help.run().is_ok()
        }
    }
}

/// Checks if a path is hidden (starts with a dot).
pub fn is_hidden_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

/// Checks if a directory entry is hidden (for walkdir)
pub fn is_hidden_walkdir(entry: &walkdir::DirEntry) -> bool {
    entry.file_name()
        .to_str()
        .map_or(false, |s| s.starts_with('.'))
    //  || entry.path().components().any(|c| { // REMOVED: Check for hidden parent components
    //      c.as_os_str().to_str().map_or(false, |s| s.starts_with('.'))
    //  })
}

/// Finds all .nix files in the given base directory and subdirectories using walkdir.
pub fn find_nix_files_walkdir(base_path: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    eprintln!("[DEBUG TEST util] find_nix_files_walkdir: V2 scanning root: {:?}", base_path);
    for entry_result in WalkDir::new(base_path)
        .follow_links(true)
        .into_iter()
        // .filter_entry(|e| !is_hidden_walkdir(e)) // Filter is applied manually below
    {
        match entry_result {
            Ok(entry) => {
                let is_hidden = is_hidden_walkdir(&entry);
                let is_file = entry.file_type().is_file();
                let is_nix = entry.path().extension().map_or(false, |ext| ext == "nix");
                eprintln!("[DEBUG TEST util] V2 WalkDir entry: path: {:?}, type: {:?}, is_hidden: {}, is_file: {}, is_nix: {}", 
                    entry.path(), 
                    entry.file_type(), 
                    is_hidden,
                    is_file,
                    is_nix);
                if !is_hidden && is_file && is_nix {
                    eprintln!("[DEBUG TEST util] V2 Adding file: {:?}", entry.path());
                    files.push(entry.path().to_path_buf());
                }
            }
            Err(err) => {
                eprintln!("[DEBUG TEST util] V2 WalkDir error: {:?}", err);
            }
        }
    }
    eprintln!("[DEBUG TEST util] V2 find_nix_files_walkdir: found {} files in {:?}.", files.len(), base_path);
    Ok(files)
}
