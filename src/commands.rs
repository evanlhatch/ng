use std::ffi::{OsStr, OsString};
use std::process::Command as StdCommand;
use std::process::Output as StdProcessOutput; // For run_capture_output
use std::path::{Path, PathBuf}; // ADDED PathBuf
 // For ExitStatus::from_raw on Unix

use color_eyre::{
    eyre::{eyre}, 
    Result,
    owo_colors::OwoColorize,
};
use thiserror::Error;
use tracing::{debug, info};

#[cfg(test)]
pub(crate) mod test_support {
    use std::cell::RefCell;
    use crate::Result; // For mock results
    use color_eyre::eyre::{eyre, Report};
    use tracing::warn;
    use std::process::Output as StdProcessOutput; // Use StdProcessOutput for the mock queue

    thread_local! {
        static TEST_MODE_ENABLED: RefCell<bool> = RefCell::new(false);
        static RECORDED_COMMANDS: RefCell<Vec<String>> = RefCell::new(Vec::new());
        static MOCK_RUN_RESULTS_QUEUE: RefCell<Vec<Result<()>>> = RefCell::new(Vec::new());
        static MOCK_CAPTURE_STDOUT: RefCell<Option<Result<Option<String>>>> = RefCell::new(None);
        static MOCK_CAPTURE_ERROR: RefCell<Option<Result<Option<String>>>> = RefCell::new(None);
        // Corrected to use std::process::Output
        static MOCK_PROCESS_OUTPUT_QUEUE: RefCell<Vec<Result<StdProcessOutput, Report>>> = RefCell::new(Vec::new());
    }

    pub fn enable_test_mode() {
        TEST_MODE_ENABLED.with(|flag| *flag.borrow_mut() = true);
        RECORDED_COMMANDS.with(|cmds| cmds.borrow_mut().clear());
        MOCK_RUN_RESULTS_QUEUE.with(|q| q.borrow_mut().clear());
        MOCK_CAPTURE_STDOUT.with(|mock| *mock.borrow_mut() = None);
        MOCK_CAPTURE_ERROR.with(|mock| *mock.borrow_mut() = None);
        MOCK_PROCESS_OUTPUT_QUEUE.with(|q| q.borrow_mut().clear()); // Updated to new queue
        eprintln!("[DEBUG TEST_SUPPORT] Test mode enabled and mocks cleared.");
    }

    pub fn disable_test_mode() {
        RECORDED_COMMANDS.with(|cmds| {
            eprintln!("[DEBUG TEST_SUPPORT] disable_test_mode CALLED. Recorded commands BEFORE clear: {:?}", cmds.borrow());
        });
        TEST_MODE_ENABLED.with(|flag| *flag.borrow_mut() = false);
        RECORDED_COMMANDS.with(|cmds| cmds.borrow_mut().clear());
        MOCK_RUN_RESULTS_QUEUE.with(|q| q.borrow_mut().clear());
        MOCK_CAPTURE_STDOUT.with(|mock| *mock.borrow_mut() = None);
        MOCK_CAPTURE_ERROR.with(|mock| *mock.borrow_mut() = None);
        MOCK_PROCESS_OUTPUT_QUEUE.with(|q| q.borrow_mut().clear()); // Updated to new queue
        eprintln!("[DEBUG TEST_SUPPORT] Test mode explicitly disabled now.");
    }

    pub fn is_test_mode_enabled() -> bool {
        TEST_MODE_ENABLED.with(|flag| *flag.borrow())
    }

    pub fn record_command(cmd_str: &str) {
        RECORDED_COMMANDS.with(|cmds| cmds.borrow_mut().push(cmd_str.to_string()));
    }

    pub fn get_recorded_commands() -> Vec<String> {
        RECORDED_COMMANDS.with(|cmds| cmds.borrow().clone())
    }

    /// Sets a mock result for the next `Command::run_capture_output()` call in test mode.
    /// The oldest added result is consumed first (FIFO).
    pub fn set_mock_process_output(result: Result<StdProcessOutput, Report>) { // Corrected type
        MOCK_PROCESS_OUTPUT_QUEUE.with(|q| q.borrow_mut().push(result));
    }

    /// Gets a mock result for `Command::run_capture_output()` if in test mode and queue is not empty.
    /// If queue is empty, warns and returns None.
    pub(crate) fn get_mock_process_output() -> Option<Result<StdProcessOutput, Report>> { // Corrected type
        if !is_test_mode_enabled() {
            return None;
        }
        MOCK_PROCESS_OUTPUT_QUEUE.with(|q| {
            let mut queue = q.borrow_mut();
            if queue.is_empty() {
                warn!("Mock process output queue was empty, test might not behave as expected if Command::run_capture_output is called.");
                None
            } else {
                Some(queue.remove(0))
            }
        })
    }

    // Pushes to the back of the queue
    pub fn set_mock_run_result(result: Result<()>) {
        MOCK_RUN_RESULTS_QUEUE.with(|q| q.borrow_mut().push(result));
    }

    // Removes from the front of the queue (FIFO)
    pub(crate) fn get_mock_run_result() -> Result<()> {
        MOCK_RUN_RESULTS_QUEUE.with(|q| {
            let mut queue = q.borrow_mut();
            if queue.is_empty() {
                warn!("Mock run results queue was empty, defaulting to Ok(()). Test behavior might be unexpected.");
                Ok(())
            } else {
                queue.remove(0)
            }
        })
    }

    // For run_capture's stdout
    pub fn set_mock_capture_stdout(stdout: String) {
        MOCK_CAPTURE_STDOUT.with(|cell| *cell.borrow_mut() = Some(Ok(Some(stdout))));
    }
    
    // For run_capture returning an error
    pub fn set_mock_capture_error(error_message: String) {
         MOCK_CAPTURE_ERROR.with(|cell| *cell.borrow_mut() = Some(Err(eyre!(error_message))));
    }

    // Gets combined result for run_capture
    pub(crate) fn get_mock_capture_result() -> Result<Option<String>> {
        if let Some(error_res) = MOCK_CAPTURE_ERROR.with(|cell| cell.borrow_mut().take()) {
            return error_res;
        }
        MOCK_CAPTURE_STDOUT.with(|cell| {
            cell.borrow_mut().take().unwrap_or_else(|| {
                warn!("Mock capture stdout was not set or already consumed, defaulting to Ok(None).");
                Ok(None) // Default to Ok(None) if no specific mock stdout was set
            })
        })
    }
    
    // --- Old mock functions for single Option-based MOCK_PROCESS_OUTPUT removed --- 
    // pub fn set_mock_process_output(output_result: Result<StdProcessOutput>) { ... } was here
    // pub(crate) fn get_mock_process_output_result() -> Result<StdProcessOutput> { ... } was here
}

#[cfg(test)]
mod tests {
    use super::*; // To bring Command and test_support into scope
    // use crate::Result; // Unused in this test module directly
    use color_eyre::eyre::eyre; // For eyre! macro

    #[test]
    fn test_command_run_with_mock_error() {
        test_support::enable_test_mode();
        let expected_err_msg = "mocked run error";
        test_support::set_mock_run_result(Err(eyre!(expected_err_msg)));

        let cmd = Command::new("test_cmd");
        let result = cmd.run();

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains(expected_err_msg));
        let recorded_commands = test_support::get_recorded_commands();
        assert_eq!(recorded_commands.len(), 1, "Expected exactly one command to be recorded in this test");
        let cmd_to_check = recorded_commands.first().expect("No command was recorded").clone();
        assert_eq!(cmd_to_check, "test_cmd");

        test_support::disable_test_mode();
    }

    #[test]
    fn test_command_run_capture_with_mock_error() {
        test_support::enable_test_mode();
        let expected_err_msg = "mocked capture error";
        test_support::set_mock_capture_error(expected_err_msg.to_string());

        let cmd = Command::new("capture_cmd").arg("some_arg");
        let result = cmd.run_capture();

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains(expected_err_msg));
        let recorded_commands = test_support::get_recorded_commands();
        assert_eq!(recorded_commands.len(), 1, "Expected exactly one command to be recorded in this test");
        let cmd_to_check = recorded_commands.first().expect("No command was recorded").clone();
        assert_eq!(cmd_to_check, "capture_cmd some_arg");

        test_support::disable_test_mode();
    }
    
    #[test]
    fn test_command_dry_run_does_not_execute_but_logs_and_uses_to_command_string() {
        test_support::enable_test_mode(); // Even in test mode, dry(true) should take precedence for logging.
        
        // Set mock results that would indicate execution if dry_run wasn't respected by Command itself.
        test_support::set_mock_run_result(Err(eyre!("SHOULD_NOT_HAPPEN_RUN")));
        test_support::set_mock_capture_stdout("SHOULD_NOT_HAPPEN_CAPTURE".to_string());

        let cmd_run = Command::new("run_cmd_dry").arg("arg1").dry(true);
        // We need to capture logs to verify the info! message, which is complex here.
        // For now, trust that .dry(true) on Command makes its run/run_capture a no-op beyond logging.
        // The key is that it should return Ok(()) or Ok(None) as per its own dry_run logic,
        // *not* the mock results if the Command.dry flag is honored first.
        assert!(cmd_run.run().is_ok(), "Expected dry run of cmd.run() to be Ok(())");
        // Assert that no command was recorded by the mock framework because .dry(true) should prevent execution path
        // Or rather, assert that the *log* shows the dry run. This test needs log capture.
        // For now, we'll assert it didn't try to use the mock error for run()

        let cmd_capture = Command::new("capture_cmd_dry").arg("arg_c").dry(true);
        let capture_res = cmd_capture.run_capture();
        assert!(capture_res.is_ok(), "Expected dry run of cmd.run_capture() to be Ok");
        assert!(capture_res.unwrap().is_none(), "Expected dry run of cmd.run_capture() to be None");

        // Clean up mocks even if not used by the .dry(true) path
        test_support::disable_test_mode(); // Also resets mocks
    }
}


use crate::installable::Installable;
use crate::util::{self}; // UtilCommandError was unused

#[derive(Debug)]
pub struct Command {
    dry: bool,
    message: Option<String>,
    command: OsString,
    args: Vec<OsString>,
    elevate: bool,
    current_working_dir: Option<PathBuf>, // ADDED
}

impl Command {
    pub fn new<S: AsRef<OsStr>>(command: S) -> Self {
        Self {
            dry: false,
            message: None,
            command: command.as_ref().to_os_string(),
            args: vec![],
            elevate: false,
            current_working_dir: None, // ADDED
        }
    }

    pub fn get_command_name(&self) -> &std::ffi::OsStr {
        &self.command
    }

    pub fn elevate(mut self, elevate: bool) -> Self {
        self.elevate = elevate;
        self
    }

    pub fn dry(mut self, dry: bool) -> Self {
        self.dry = dry;
        self
    }

    /// Sets the current working directory for the command.
    pub fn current_dir<P: AsRef<Path>>(mut self, dir: P) -> Self {
        self.current_working_dir = Some(dir.as_ref().to_path_buf());
        self
    }

    pub fn arg<S: AsRef<OsStr>>(mut self, arg: S) -> Self {
        self.args.push(arg.as_ref().to_os_string());
        self
    }

    pub fn args<I>(mut self, args: I) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<OsStr>,
    {
        for elem in args {
            self.args.push(elem.as_ref().to_os_string());
        }
        self
    }

    pub fn add_verbosity_flags(mut self, verbosity_level: u8) -> Self {
        let effective_level = std::cmp::min(verbosity_level, 7); // Cap at 7
        for _ in 0..effective_level {
            self.args.push(OsString::from("-v"));
        }
        self
    }

    pub fn message<S: AsRef<str>>(mut self, message: S) -> Self {
        self.message = Some(message.as_ref().to_string());
        self
    }

    pub fn to_command_string(&self) -> String {
        let mut s = self.command.to_string_lossy().into_owned();
        for arg in &self.args {
            s.push_str(" ");
            s.push_str(&arg.to_string_lossy());
        }
        // Note: This doesn't show `sudo` if self.elevate is true, as sudo is handled in run().
        s
    }

    pub fn run(&self) -> Result<()> {
        if let Some(m) = &self.message {
            info!("{}", m);
        }

        let cmd_str = self.to_command_string(); // Generate the string once

        if self.dry {
            info!("DRY-RUN (not executing): {}", cmd_str.bright_cyan());
            #[cfg(test)]
            if test_support::is_test_mode_enabled() {
                test_support::record_command(&cmd_str);
            }
            return Ok(());
        }

        #[cfg(test)]
        if test_support::is_test_mode_enabled() {
            test_support::record_command(&cmd_str);
            return test_support::get_mock_run_result();
        }

        // Actual execution logic follows here from the original function...
        // Check 3: Actual execution
        let mut cmd_to_execute = if self.elevate { 
            let mut sudo_cmd = StdCommand::new("sudo");
            if cfg!(target_os = "macos") {
                let mut check_cmd = StdCommand::new("sudo");
                check_cmd.arg("--help");
                match util::run_cmd(&mut check_cmd) {
                    Ok(output) => {
                        let output_str = String::from_utf8_lossy(&output.stdout);
                        if output_str.contains("--preserve-env") {
                            sudo_cmd.args(["--set-home", "--preserve-env=PATH", "env"]);
                        } else {
                            sudo_cmd.arg("--set-home");
                        }
                    },
                    Err(_) => {
                        sudo_cmd.arg("--set-home");
                    }
                }
            }
            if let Some(cwd) = &self.current_working_dir {
                sudo_cmd.current_dir(cwd);
            }
            sudo_cmd.arg(&self.command);
            sudo_cmd.args(&self.args);
            sudo_cmd
        } else {
            let mut std_cmd = StdCommand::new(&self.command);
            if let Some(cwd) = &self.current_working_dir {
                std_cmd.current_dir(cwd);
            }
            std_cmd.args(&self.args);
            std_cmd
        };

        debug!("Executing command: {:?}", cmd_to_execute);
        
        match util::run_cmd_inherit_stdio(&mut cmd_to_execute) {
            Ok(_) => Ok(()),
            Err(e) => {
                if let Some(m) = &self.message {
                    Err(eyre!("{}: {}", m, e))
                } else {
                    Err(e.into())
                }
            }
        }
    }

    pub fn run_capture(&self) -> Result<Option<String>> {
        if let Some(m) = &self.message {
            info!("{}", m);
        }

        let cmd_str = self.to_command_string(); // Generate the string once

        if self.dry {
            info!("DRY-RUN (not executing, would capture): {}", cmd_str.bright_cyan());
            #[cfg(test)]
            if test_support::is_test_mode_enabled() {
                test_support::record_command(&cmd_str);
            }
            return Ok(None); // Consistent with original dry run return
        }

        #[cfg(test)]
        if test_support::is_test_mode_enabled() {
            test_support::record_command(&cmd_str);
            return test_support::get_mock_capture_result();
        }

        // Actual execution logic follows...
        let mut cmd = StdCommand::new(&self.command);
        cmd.args(&self.args);

        if let Some(cwd) = &self.current_working_dir { // ADDED
            cmd.current_dir(cwd);                    // ADDED
        }                                              // ADDED
        
        debug!("Executing command: {:?}", cmd);
        
        match util::run_cmd(&mut cmd) {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                Ok(Some(stdout))
            },
            Err(e) => {
                if let Some(m) = &self.message {
                    Err(eyre!("{}: {}", m, e))
                } else {
                    Err(e.into())
                }
            }
        }
    }

    /// Runs the command and captures its full std::process::Output (stdout, stderr, status).
    pub fn run_capture_output(&self) -> Result<std::process::Output> {
        if let Some(m) = &self.message {
            info!("{}", m);
        }

        let cmd_str = self.to_command_string(); // Generate the string once

        if self.dry {
            info!("DRY-RUN (not executing, would capture output): {}", cmd_str.bright_cyan());
            #[cfg(test)]
            if test_support::is_test_mode_enabled() {
                test_support::record_command(&cmd_str);
            }
            // Return a dummy Output for dry runs (original logic preserved).
            #[cfg(unix)]
            let status = std::os::unix::process::ExitStatusExt::from_raw(0);
            #[cfg(not(unix))]
            let status = { std::process::Command::new("cmd").arg("/C").arg("exit 0").status().unwrap() };
            
            return Ok(StdProcessOutput {
                status, 
                stdout: Vec::new(),
                stderr: Vec::new(),
            });
        }

        #[cfg(test)]
        if test_support::is_test_mode_enabled() {
            use tracing::warn; // Moved import here
            test_support::record_command(&cmd_str);
            // Use the new mock getter which returns Option<Result<std::process::Output, Report>>
            if let Some(mock_result) = test_support::get_mock_process_output() {
                // If a mock is provided, return it directly.
                return mock_result;
            } else {
                // No specific mock in queue, default mock success for safety in tests.
                warn!("No specific mock for run_capture_output, returning default empty success. Test: {}", cmd_str);
                #[cfg(unix)]
                let status = std::os::unix::process::ExitStatusExt::from_raw(0);
                #[cfg(not(unix))]
                let status = { std::process::Command::new("cmd").arg("/C").arg("exit 0").status().unwrap() };
                return Ok(StdProcessOutput {
                    status, 
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                });
            }
        }

        // Actual execution
        let mut cmd_to_run = StdCommand::new(&self.command);
        cmd_to_run.args(&self.args);

        if let Some(cwd) = &self.current_working_dir { // ADDED
            cmd_to_run.current_dir(cwd);             // ADDED
        }                                              // ADDED
        
        if self.elevate {
            // util::run_cmd does not handle sudo. This path needs a sudo-aware counterpart of util::run_cmd
            // or this method should not support elevate=true.
            return Err(eyre!("run_capture_output with elevate=true is not supported as util::run_cmd does not handle sudo. Refactor required."));
        }

        debug!("Executing command for full output capture: {:?}", cmd_to_run);
        util::run_cmd(&mut cmd_to_run).map_err(Into::into) // util::run_cmd returns Result<std::process::Output, UtilCommandError>
    }
}

#[derive(Debug)]
pub struct Build {
    message: Option<String>,
    installable: Installable,
    extra_args: Vec<OsString>,
    nom: bool,
}

impl Build {
    pub fn new(installable: Installable) -> Self {
        Self {
            message: None,
            installable,
            extra_args: vec![],
            nom: false,
        }
    }

    pub fn message<S: AsRef<str>>(mut self, message: S) -> Self {
        self.message = Some(message.as_ref().to_string());
        self
    }

    pub fn extra_arg<S: AsRef<OsStr>>(mut self, arg: S) -> Self {
        self.extra_args.push(arg.as_ref().to_os_string());
        self
    }

    pub fn nom(mut self, yes: bool) -> Self {
        self.nom = yes;
        self
    }

    pub fn extra_args<I>(mut self, args: I) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<OsStr>,
    {
        for elem in args {
            self.extra_args.push(elem.as_ref().to_os_string());
        }
        self
    }

    pub fn run(&self) -> Result<()> {
        if let Some(m) = &self.message {
            info!("{}", m);
        }

        let installable_args = self.installable.to_args();

        if self.nom {
            // Use run_piped_commands for nom integration
            let mut nix_cmd = StdCommand::new("nix");
            nix_cmd.arg("build")
                .args(&installable_args)
                .args(&["--log-format", "internal-json", "--verbose"])
                .args(&self.extra_args);

            let mut nom_cmd = StdCommand::new("nom");
            nom_cmd.args(&["--json"]);

            debug!("Executing piped command: {:?} | {:?}", nix_cmd, nom_cmd);
            
            match util::run_piped_commands(nix_cmd, nom_cmd) {
                Ok(_) => Ok(()),
                Err(e) => Err(e.into())
            }
        } else {
            // Use regular command execution
            let mut cmd = StdCommand::new("nix");
            cmd.arg("build")
                .args(&installable_args)
                .args(&self.extra_args);

            debug!("Executing command: {:?}", cmd);
            
            match util::run_cmd_inherit_stdio(&mut cmd) {
                Ok(_) => Ok(()),
                Err(e) => Err(e.into())
            }
        }
    }
}

#[derive(Debug, Error)]
#[error("Command execution failed: {0}")]
pub struct ExitError(String);
