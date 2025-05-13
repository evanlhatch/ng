# ng/justfile

# Build and install/upgrade the 'ng' CLI to your USER Nix profile.
# This makes 'ng' available system-wide if ~/.nix-profile/bin is in your PATH.
install-user:
    @echo "Building and installing/upgrading 'ng' CLI to your USER Nix profile..."
    # The 'nix profile install' command will also trigger a build if needed.
    # Explicitly building first can give clearer build logs if something goes wrong.
    nix build .#ng
    nix profile install .#ng  # Installs/upgrades the 'ng' package output from this flake
    @echo ""
    @echo "'ng' CLI has been installed/updated to your user profile."
    @echo "Make sure '~/.nix-profile/bin' is in your shell's PATH."
    @echo "You may need to open a new terminal or re-source your shell configuration"
    @echo "(e.g., source ~/.bashrc, source ~/.zshrc) for the change to take effect."
    @echo "Verify with: which ng"
    @echo "And: ng --version"

# Common alias for the above
install: install-user

# Uninstall 'ng' from your user profile (for completeness)
# This uses the flake reference. For more precise uninstallation,
# use `nix profile list` to find the index or attribute path and then `nix profile remove <value>`.
uninstall-user:
    @echo "Attempting to uninstall 'ng' (installed via .#ng) from your USER Nix profile..."
    nix profile remove .#ng
    @echo "'ng' CLI uninstall attempted. Use 'nix profile list' and 'nix profile remove <index/attr>' for specifics."

# Build the project using Nix
build-nix:
    @echo "Building 'ng' using Nix..."
    nix build .#ng
    @echo "Build completed. The result is available in ./result"

# Test the Nix build
test-nix: build-nix
    @echo "Testing the Nix build..."
    ./result/bin/ng --version

