# ng - Next Generation Nix Helper

A fork of [nh](https://github.com/viperML/nh) with enhanced pre-flight checks, error reporting, and progress indicators.

## Features

- **Improved Pre-flight Checks**:
  - Git status check to warn about untracked .nix files
  - Parallel syntax checking of .nix files
  - Linting and formatting with auto-fix capabilities

- **Better Error Reporting**:
  - Detailed error messages with context
  - Actionable recommendations
  - Build logs for failed derivations

- **Progress Indicators**:
  - Spinners for long-running operations
  - Clear success/failure indicators
  - Structured output for complex data

- **Verbosity Levels**:
  - Default: Essential information only
  - `-v`: Adds debug information
  - `-vv`: Adds trace information

- **Build Modes**:
  - **Quick Mode** (default): Basic checks only
  - **Medium Mode** (`--medium`): Adds evaluation check
  - **Full Mode** (`--full`): Adds dry-run build

## Installation

```sh
# From GitHub
nix shell github:evanlhatch/ng

# From local clone
git clone https://github.com/evanlhatch/ng.git
cd ng
nix shell .
```

## Usage

```sh
# Basic usage
ng os switch .

# With enhanced checks
ng os switch . --medium

# With full checks
ng os switch . --full

# With verbosity
ng os switch . -vv
```

## Commands

- `os`: NixOS rebuild functionality
- `home`: Home-manager functionality
- `darwin`: Nix-darwin functionality
- `search`: Package search functionality
- `clean`: Enhanced nix cleanup with garbage collection

## Development

```sh
# Clone the repository
git clone https://github.com/evanlhatch/ng.git
cd ng

# Enter development shell
nix develop

# Build the project
cargo build

# Run the project
cargo run -- --help
```

## Building with Nix

This project uses [Crane](https://github.com/ipetkov/crane) to build the Rust project with Nix. The build process is configured to properly handle the local path dependencies in the `vendor` directory.

To build the project:

```sh
# Using nix build
nix build .#ng

# Using just
just build-nix
```

The build result will be available in the `./result` directory.

You can install the built package to your user profile:

```sh
# Using nix profile
nix profile install .#ng

# Using just
just install-user
```

## License

Licensed under the [EUPL-1.2](LICENSE).
