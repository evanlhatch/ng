use clap_complete::generate;
use color_eyre::Result;
use tracing::instrument;

use crate::interface::Main;
use crate::*;

impl interface::CompletionArgs {
    #[instrument(ret, level = "trace")]
    pub fn run(&self, verbose_count: u8) -> Result<()> {
        let mut cmd = <Main as clap::CommandFactory>::command();
        generate(self.shell, &mut cmd, "ng", &mut std::io::stdout());
        Ok(())
    }
}
