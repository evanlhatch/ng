{
  description = "nix-goon (ng) with Slint GUI, using Crane and Devenv (No Flake-Utils)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    crane.url = "github:ipetkov/crane"; # Let flake.lock determine the rev
    devenv.url = "github:cachix/devenv";
  };

  outputs = { self, nixpkgs, rust-overlay, crane, devenv, ... }@inputsAll:
    let
      supportedSystems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      perSystemOutputs = system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = import nixpkgs { inherit system overlays; };
          rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;
          
          # Create a source filter that includes the vendor directory
          src = pkgs.lib.cleanSourceWith {
            src = ./.;
            filter = path: type:
              (pkgs.lib.hasInfix "/vendor/" path) ||
              (craneLib.filterCargoSources path type);
          };
          
          # Common dependencies
          commonDeps = with pkgs; [
            openssl pkg-config gtk3 glib cairo pango gdk-pixbuf
            # Add nix for the builtin crate's build script
            nix
          ];
          
          # Build dependencies only (this will be cached)
          cargoArtifacts = craneLib.buildDepsOnly {
            inherit src;
            
            # Tell crane to use the vendored dependencies
            cargoExtraArgs = "--offline";
            
            # Provide build inputs
            nativeBuildInputs = commonDeps;
            buildInputs = commonDeps;
          };
          
          # Build the actual package
          ng-cli = craneLib.buildPackage {
            inherit src cargoArtifacts;
            
            # Tell crane to use the vendored dependencies
            cargoExtraArgs = "--offline";
            
            # Provide build inputs
            nativeBuildInputs = commonDeps;
            buildInputs = commonDeps;
          };
          
          # Build the GUI version
          ng-gui = craneLib.buildPackage {
            inherit src cargoArtifacts;
            
            # Tell crane to use the vendored dependencies and build the GUI
            cargoExtraArgs = "--offline --bin ng-gui";
            
            # Provide build inputs
            nativeBuildInputs = commonDeps;
            buildInputs = commonDeps;
          };
        in {
          packages = { default = ng-cli; ng = ng-cli; "ng-cli" = ng-cli; "ng-gui" = ng-gui; };
          devShells = {
            default = devenv.lib.mkShell {
              inherit pkgs;
              inputs = inputsAll;
              modules = [
                ({ pkgsModule, ... }: {
                  languages.rust.enable = true;
                  packages = with pkgsModule; [ rust-analyzer cargo-edit just ] ++ commonSystemDeps;
                  env.CARGO_TERM_COLOR = "always";
                  enterShell = ''
                    echo "Entered ng (Rust + Crane + Devenv) development environment."
                    echo "Rust toolchain: $(rustc --version)"
                    echo "Hint: Run 'cargo build' or 'just install' to install ng globally."
                  '';
                })];};};
          formatter = pkgs.nixfmt-rfc-style;
        };
    in {
      packages = nixpkgs.lib.genAttrs supportedSystems (system:
        let result = perSystemOutputs system; in result.packages
      );
      devShells = nixpkgs.lib.genAttrs supportedSystems (system:
        let result = perSystemOutputs system; in result.devShells
      );
      formatter = nixpkgs.lib.genAttrs supportedSystems (system:
        let result = perSystemOutputs system; in result.formatter
      );
    };
}
