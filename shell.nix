{
  pkgs ? import <nixpkgs> { },
}:
with pkgs;
mkShell {
  strictDeps = true;

  nativeBuildInputs = [
    cargo
    rustc

    rust-analyzer-unwrapped
    (rustfmt.override { asNightly = true; })
    clippy
    nvd
    nix-output-monitor
    taplo
    yaml-language-server
  ];

  buildInputs = lib.optionals stdenv.isDarwin [
    darwin.apple_sdk.frameworks.SystemConfiguration
    libiconv
  ];

  env = {
    NG_NOM = "1";
    NG_LOG = "ng=trace"; # Assuming nh=trace should be ng=trace
    RUST_SRC_PATH = "${rustPlatform.rustLibSrc}";
  };

  shellHook = ''
    # Add target/debug to PATH if it exists, for local ng testing
    if [ -d "${toString ./target/debug}" ]; then
      export PATH="${toString ./target/debug}:$PATH"
      echo "ng (dev version from ./target/debug) is now in your PATH for this shell."
    else
      echo "Run 'cargo build' to make the local 'ng' dev binary available in PATH."
    fi
    echo "Run 'cargo build' to compile 'ng'."
  '';
}
