{ pkgs, ... }:

{
  # Define environment variables
  env = {
    NH_NOM = "1";
    NH_LOG = "nh=trace";
    RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
  };

  # Include the same packages as in shell.nix
  packages = with pkgs; [
    cargo
    rustc
    rust-analyzer-unwrapped
    (rustfmt.override { asNightly = true; })
    clippy
    nvd
    nix-output-monitor
    taplo
    yaml-language-server
  ] ++ (if pkgs.stdenv.isDarwin then [
    pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
    pkgs.libiconv
  ] else []);

  enterShell = ''
    echo "Entering nh development environment"
    echo "Run 'cargo check' to verify dependencies"
    echo "Run 'cargo run -- --help' to test the binary"
  '';
}