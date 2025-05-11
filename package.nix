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
  pname = "ng"; # Renamed from nh
  version = "${cargoToml.package.version}-${rev}";

  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.intersection (lib.fileset.fromSource (lib.sources.cleanSource ./.)) (
      lib.fileset.unions [
        ./src
        ./Cargo.toml
        ./Cargo.lock
        ./vendor # Added for vendored dependencies
        ./.cargo # Added for .cargo/config.toml (from cargo vendor)
      ]
    );
  };

  strictDeps = true;

  nativeBuildInputs = [
    installShellFiles
    makeBinaryWrapper
  ];

  buildInputs = lib.optionals stdenv.isDarwin [ darwin.apple_sdk.frameworks.SystemConfiguration ];

  preFixup = ''
    mkdir completions
    $out/bin/ng completions bash > completions/ng.bash # Renamed from nh
    $out/bin/ng completions zsh > completions/ng.zsh   # Renamed from nh
    $out/bin/ng completions fish > completions/ng.fish # Renamed from nh

    installShellCompletion completions/*
  '';

  postFixup = ''
    wrapProgram $out/bin/ng \\ # Renamed from nh
      --prefix PATH : ${lib.makeBinPath runtimeDeps}
  '';

  cargoLock.lockFile = ./Cargo.lock;

  env = {
    NG_REV = rev; # Renamed from NH_REV
  };

  meta = {
    description = "Yet another nix cli helper (now ng)"; # Updated description
    homepage = "https://github.com/viperML/nh"; # Consider updating if project moves/renames
    license = lib.licenses.eupl12;
    mainProgram = "ng"; # Renamed from nh
    maintainers = with lib.maintainers; [
      drupol
      viperML
    ];
  };
}
