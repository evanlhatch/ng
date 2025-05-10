use std::ffi::{OsStr, OsString};
use std::process::Command as StdCommand; // Stdio was unused

use color_eyre::{
    eyre::{eyre}, // bail, Context were unused
    Result,
};
use thiserror::Error;
use tracing::{debug, info};

use crate::installable::Installable;
use crate::util::{self}; // UtilCommandError was unused

#[derive(Debug)]
pub struct Command {
    dry: bool,
    message: Option<String>,
    command: OsString,
    args: Vec<OsString>,
    elevate: bool,
}

impl Command {
    pub fn new<S: AsRef<OsStr>>(command: S) -> Self {
        Self {
            dry: false,
            message: None,
            command: command.as_ref().to_os_string(),
            args: vec![],
            elevate: false,
        }
    }

    pub fn elevate(mut self, elevate: bool) -> Self {
        self.elevate = elevate;
        self
    }

    pub fn dry(mut self, dry: bool) -> Self {
        self.dry = dry;
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

    pub fn run(&self) -> Result<()> {
        if let Some(m) = &self.message {
            info!("{}", m);
        }

        if self.dry {
            info!("Dry-run: Would execute: {} {}", self.command.to_string_lossy(),
                self.args.iter().map(|a| a.to_string_lossy()).collect::<Vec<_>>().join(" "));
            return Ok(());
        }

        let mut cmd = if self.elevate {
            // Handle sudo with proper flags
            let mut sudo_cmd = StdCommand::new("sudo");
            
            if cfg!(target_os = "macos") {
                // Check if sudo has the preserve-env flag
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
                        // If we can't check, use a safe default
                        sudo_cmd.arg("--set-home");
                    }
                }
            }
            
            sudo_cmd.arg(&self.command);
            sudo_cmd.args(&self.args);
            sudo_cmd
        } else {
            let mut cmd = StdCommand::new(&self.command);
            cmd.args(&self.args);
            cmd
        };

        debug!("Executing command: {:?}", cmd);
        
        match util::run_cmd_inherit_stdio(&mut cmd) {
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

        if self.dry {
            info!("Dry-run: Would execute: {} {}", self.command.to_string_lossy(),
                self.args.iter().map(|a| a.to_string_lossy()).collect::<Vec<_>>().join(" "));
            return Ok(None);
        }

        let mut cmd = StdCommand::new(&self.command);
        cmd.args(&self.args);
        
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
