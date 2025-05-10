
#[cfg(test)]
mod error_handler_test;
#[cfg(test)]
mod lint_test;
#[cfg(test)]
mod check_git_test;
#[cfg(test)]
mod progress_test;
#[cfg(test)]
mod util_test;
#[cfg(test)]
mod ui_style_test;

use color_eyre::Result;
// use tracing::debug; // Was used by self_elevate

// Use the library crate `nh` for its modules
// use nh::interface; // Not used directly, Main is fully qualified
// use nh::logging; // Not used directly, setup_logging is fully qualified

const NH_VERSION: &str = env!("CARGO_PKG_VERSION");
const NH_REV: Option<&str> = option_env!("NH_REV");

fn main() -> Result<()> {
    let mut do_warn = false;
    if let Ok(f) = std::env::var("FLAKE") {
        // Set NH_FLAKE if it's not already set
        if std::env::var("NH_FLAKE").is_err() {
            std::env::set_var("NH_FLAKE", f);

            // Only warn if FLAKE is set and we're using it to set NH_FLAKE
            // AND none of the command-specific env vars are set
            if std::env::var("NH_OS_FLAKE").is_err()
                && std::env::var("NH_HOME_FLAKE").is_err()
                && std::env::var("NH_DARWIN_FLAKE").is_err()
            {
                do_warn = true;
            }
        }
    }

    let args = <nh::interface::Main as clap::Parser>::parse();
    let verbose_count = args.verbose; // Extract verbosity count
    
    nh::logging::setup_logging(verbose_count)?; // Pass the count to setup_logging
    tracing::debug!("{args:#?}");
    tracing::debug!(%NH_VERSION, ?NH_REV);

    if do_warn {
        tracing::warn!(
            "nh {NH_VERSION} now uses NH_FLAKE instead of FLAKE, please modify your configuration"
        );
    }

    args.command.run(verbose_count) // Pass the count to run
}
