{
  stdenv,
  lib,
  rustPlatform,
  installShellFiles,
  makeBinaryWrapper,
  darwin,
  nvd,
  use-nom ? true,
  nix-output-monitor ? null,
  rev ? "dirty",
}:
assert use-nom -> nix-output-monitor != null;
let
  runtimeDeps = [ nvd ] ++ lib.optionals use-nom [ nix-output-monitor ];
  cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
in
rustPlatform.buildRustPackage {
  pname = "ng";
  version = "${cargoToml.package.version}-${rev}";

  src = ./.; # Use the entire project directory as source

  strictDeps = true;

  nativeBuildInputs = [
    installShellFiles
    makeBinaryWrapper
  ];

  buildInputs = lib.optionals stdenv.isDarwin [ darwin.apple_sdk.frameworks.SystemConfiguration ];

  # Use the --frozen flag to ensure we use vendored dependencies
  cargoBuildFlags = [
    "--frozen"
  ];

  # Explicitly set CARGO_HOME to ensure cargo can find the config
  CARGO_HOME = "./.cargo";

  preFixup = ''
    mkdir completions
    $out/bin/ng completions bash > completions/ng.bash
    $out/bin/ng completions zsh > completions/ng.zsh
    $out/bin/ng completions fish > completions/ng.fish

    installShellCompletion completions/*
  '';

  postFixup = ''
    wrapProgram $out/bin/ng \\
      --prefix PATH : ${lib.makeBinPath runtimeDeps}
  '';

  # Skip the vendoring phase since we already have the vendor directory
  dontCargoVendor = true;
  
  # Skip the cargo fetch phase
  dontCargoFetch = true;
  
  # Skip the cargo check phase
  dontCargoCheck = true;
  
  # Copy the vendor directory and .cargo config to the build directory
  preBuild = ''
    echo "--- Contents of vendor directory (top level):"
    ls -l vendor || true

    echo "--- Contents of vendor/builtin directory:"
    ls -l vendor/builtin || true # Check if Cargo.toml is listed here

    echo "--- Contents of .cargo directory:"
    ls -l .cargo || true

    echo "--- Attempting to cat vendor/builtin/Cargo.toml:"
    cat vendor/builtin/Cargo.toml || echo "cat vendor/builtin/Cargo.toml FAILED" # Try to display it
    
    echo "--- Attempting to cat .cargo/config.toml:"
    cat .cargo/config.toml || echo "cat .cargo/config.toml FAILED" # Try to display it
  '';

  env = {
    NG_REV = rev;
  };

  meta = {
    description = "Yet another nix cli helper (now ng)";
    homepage = "https://github.com/viperML/nh";
    license = lib.licenses.eupl12;
    mainProgram = "ng";
    maintainers = with lib.maintainers; [
      drupol
      viperML
    ];
  };

}
