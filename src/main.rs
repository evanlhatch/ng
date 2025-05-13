use clap::Parser;
use color_eyre::eyre::Result;
use ng::interface::Main;

fn main() -> Result<()> {
    color_eyre::install()?;
    
    // Parse command line arguments using the interface::Main struct
    let cli = Main::parse();
    
    // Run the command with the specified verbosity level
    cli.command.run(cli.verbose)
}
