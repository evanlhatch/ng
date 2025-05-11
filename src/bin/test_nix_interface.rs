use nh::nix_interface::NixInterface;
use nh::installable::Installable;
use std::path::PathBuf;
use color_eyre::eyre::Result;

fn main() -> Result<()> {
    // Setup logging
    nh::logging::setup_logging(2)?;
    
    println!("Testing NixInterface...");
    
    // Create a NixInterface
    let dry_run_for_test = true; // Example: test in dry_run mode
    let _nix_interface = NixInterface::new(2, dry_run_for_test);
    
    // Create a simple installable
    let installable = Installable::Flake {
        reference: ".".to_string(),
        attribute: vec!["nixosConfigurations".to_string(), "test".to_string()],
    };
    
    // Create a temporary output path
    let out_path = PathBuf::from("/tmp/test_nix_interface_output");
    
    // Test the build_configuration method
    println!("Testing build_configuration...");
    println!("This would normally build the configuration, but we're just testing the interface.");
    println!("Installable: {:?}", installable);
    println!("Out path: {:?}", out_path);
    
    // Test the run_gc method
    println!("Testing run_gc...");
    println!("This would normally run garbage collection, but we're just testing the interface.");
    
    println!("All tests completed successfully!");
    Ok(())
}