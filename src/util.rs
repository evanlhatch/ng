extern crate semver;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::str;

use color_eyre::{eyre, Result};
use semver::Version;
use tempfile::TempDir;

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
    let current = Version::parse(current)?;
    let target = Version::parse(target)?;

    Ok(current.cmp(&target))
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
    let output = Command::new("nix").arg("--version").output()?;

    let output_str = str::from_utf8(&output.stdout)?;
    let version_str = output_str
        .lines()
        .next()
        .ok_or_else(|| eyre::eyre!("No version string found"))?;

    // Extract the version substring using a regular expression
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
///
/// This function adds `-v` flags to the command for each level of verbosity.
/// The maximum number of flags is capped at 7 to prevent excessive verbosity.
///
/// # Arguments
///
/// * `cmd` - A mutable reference to a Command to add flags to.
/// * `verbosity_level` - The verbosity level (number of `-v` flags to add).
///
/// # Example
///
/// ```
/// use std::process::Command;
/// use nh::util::add_verbosity_flags;
///
/// let mut cmd = Command::new("nix");
/// add_verbosity_flags(&mut cmd, 3);
/// // cmd now has args: ["-v", "-v", "-v"]
/// ```
pub fn add_verbosity_flags(cmd: &mut Command, verbosity_level: u8) {
    let effective_level = std::cmp::min(verbosity_level, 7); // Cap at 7 to be reasonable
    for _ in 0..effective_level {
        cmd.arg("-v");
    }
}

/// Runs a command and captures its output.
///
/// This function executes a command and captures its stdout and stderr.
/// It returns an error if the command fails to execute or returns a non-zero status.
///
/// # Arguments
///
/// * `command` - A mutable reference to a Command to execute.
///
/// # Returns
///
/// * `Result<std::process::Output>` - The command's output or an error.
pub fn run_cmd(command: &mut Command) -> Result<std::process::Output> {
    use color_eyre::eyre::WrapErr;
    use tracing::{debug, warn};
    
    debug!("Executing command: {:?}", command);
    
    let output = command
        .output()
        .wrap_err_with(|| format!("Failed to execute command: {:?}", command))?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        warn!(
            "Command failed: {:?} - Exit Code: {:?}",
            command,
            output.status.code()
        );
        debug!("Stderr: {}", stderr);
        debug!("Stdout: {}", stdout);
        
        return Err(color_eyre::eyre::eyre!(
            "Command exited with non-zero status: {:?}",
            output.status.code()
        ));
    }
    
    Ok(output)
}

/// Runs a command with inherited stdio.
///
/// This function executes a command with stdin, stdout, and stderr inherited from the parent process.
/// It returns an error if the command fails to execute or returns a non-zero status.
///
/// # Arguments
///
/// * `command` - A mutable reference to a Command to execute.
///
/// # Returns
///
/// * `Result<std::process::ExitStatus>` - The command's exit status or an error.
pub fn run_cmd_inherit_stdio(command: &mut Command) -> Result<std::process::ExitStatus> {
    use color_eyre::eyre::WrapErr;
    use std::process::Stdio;
    use tracing::{debug, warn};
    
    debug!("Executing command with inherited stdio: {:?}", command);
    
    let status = command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .wrap_err_with(|| {
            format!("Failed to execute command with inherited stdio: {:?}", command)
        })?;
    
    if !status.success() {
        warn!(
            "Command failed: {:?} - Exit Code: {:?}",
            command,
            status.code()
        );
        
        return Err(color_eyre::eyre::eyre!(
            "Command exited with non-zero status: {:?}",
            status.code()
        ));
    }
    
    Ok(status)
}

/// Checks if stdout is connected to a terminal.
///
/// This function uses the atty crate to determine if stdout is a TTY.
///
/// # Returns
///
/// * `bool` - True if stdout is a TTY, false otherwise.
pub fn is_stdout_tty() -> bool {
    atty::is(atty::Stream::Stdout)
}
