{
  pkgs ? import <nixpkgs> { },
  rev ? "dirty",
  craneLib ? pkgs.crane.lib,
}:

let
  # Get the Rust toolchain
  rustToolchain = pkgs.rust-bin.stable.latest.default;
  craneLibWithToolchain = craneLib.overrideToolchain rustToolchain;

  # Create a source filter that includes the vendor directory
  src = pkgs.lib.cleanSourceWith {
    src = ./.;
    filter =
      path: type:
      (pkgs.lib.hasInfix "/vendor/" path) || (craneLibWithToolchain.filterCargoSources path type);
  };

  # Common dependencies
  commonDeps = with pkgs; [
    openssl
    pkg-config
    gtk3
    glib
    cairo
    pango
    gdk-pixbuf
    # Add nix for the builtin crate's build script
    nix
  ];

  # Build dependencies only (this will be cached)
  cargoArtifacts = craneLibWithToolchain.buildDepsOnly {
    inherit src;

    # Tell crane to use the vendored dependencies
    cargoExtraArgs = "--offline";

    # Provide build inputs
    nativeBuildInputs = commonDeps;
    buildInputs = commonDeps;
  };
in
craneLibWithToolchain.buildPackage {
  inherit src cargoArtifacts;

  # Tell crane to use the vendored dependencies
  cargoExtraArgs = "--offline";

  # Provide build inputs
  nativeBuildInputs = commonDeps;
  buildInputs = commonDeps;

  # Package metadata
  pname = "ng";
  version = "4.0.2";

  meta = with pkgs.lib; {
    description = "Nix Generator (ng) - A command-line tool for Nix project development";
    homepage = "https://github.com/nix-community/ng";
    license = licenses.eupl12;
    platforms = platforms.all;
    maintainers = with maintainers; [ ];
  };
}
