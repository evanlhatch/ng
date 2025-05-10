// New modules
pub mod check_git;
pub mod error_handler;
pub mod lint;
pub mod progress;
pub mod ui_style;
pub mod tables;
pub mod util;

// Workflow refactoring modules
pub mod workflow_types;
pub mod context;
pub mod workflow_strategy;
pub mod workflow_executor;
pub mod nixos_strategy;
// Phase 3: Implementing nil integration
pub mod nix_analyzer;
pub mod pre_flight;
pub mod nix_interface; // Now with real implementation

#[cfg(test)]
mod workflow_executor_test;
#[cfg(test)]
mod nixos_strategy_test;
#[cfg(test)]
mod installable_test;

// Existing modules
pub mod commands;
pub mod interface;
pub mod logging;
pub mod nixos;
pub mod home;
pub mod darwin;
pub mod clean;
pub mod search;
pub mod update;
pub mod generations;
pub mod installable;
pub mod json;
pub mod completion;

// Re-export color_eyre::Result for convenience
pub use color_eyre::Result;

// Constants and functions from main.rs
pub const NH_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NH_REV: Option<&str> = option_env!("NH_REV");

/// Elevate privileges using sudo
pub fn self_elevate() -> ! {
    use std::os::unix::process::CommandExt;

    let mut cmd = std::process::Command::new("sudo");
    cmd.args(std::env::args());
    tracing::debug!("{:?}", cmd);
    let err = cmd.exec();
    panic!("{}", err);
}