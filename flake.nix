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
          craneLib = crane.mkLib pkgs;
          projectSrc = craneLib.cleanCargoSource (craneLib.path ./.);
          commonSystemDeps = with pkgs; [
            openssl pkg-config gtk3 glib cairo pango gdk-pixbuf
          ];
          cargoArtifacts = craneLib.buildDepsOnly {
            src = projectSrc;
            cargoVendorDir = pkgs.lib.mkForce null;
            nativeBuildInputs = commonSystemDeps;
          };
          commonCrateArgs = {
            inherit cargoArtifacts projectSrc;
            strictDeps = true;
            nativeBuildInputs = commonSystemDeps;
            buildInputs = commonSystemDeps;
          };
          ng-cli = craneLib.buildPackage (commonCrateArgs // { pname = "ng"; });
          ng-gui = craneLib.buildPackage (commonCrateArgs // { pname = "ng"; cargoExtraArgs = "--bin ng-gui"; });
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
      packages = nixpkgs.lib.mapAttrs (sn: so: so.packages) perSystemOutputs;
      devShells = nixpkgs.lib.mapAttrs (sn: so: so.devShells) perSystemOutputs;
      formatter = nixpkgs.lib.mapAttrs (sn: so: so.formatter) perSystemOutputs;
    };
}
