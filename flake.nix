{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";
  };

  outputs =
    {
      self,
      nixpkgs,
    }:
    let
      forAllSystems =
        function:
        nixpkgs.lib.genAttrs [
          "x86_64-linux"
          "aarch64-linux"
          # experimental
          "x86_64-darwin"
          "aarch64-darwin"
        ] (system: function nixpkgs.legacyPackages.${system});

      rev = self.shortRev or self.dirtyShortRev or "dirty";
    in
    {
      overlays.default = final: prev: { nh = final.callPackage ./package.nix { inherit rev; }; };

      packages = forAllSystems (pkgs: rec {
        nh = pkgs.callPackage ./package.nix { inherit rev; };
        default = nh;
      });

      devShells = forAllSystems (pkgs: {
        default = import ./shell.nix { inherit pkgs; };
      });

      formatter = forAllSystems (pkgs: pkgs.nixfmt-rfc-style);
      
      # Add a simple test NixOS configuration
      nixosConfigurations = {
        test = nixpkgs.lib.nixosSystem {
          system = "x86_64-linux";
          modules = [
            ({ pkgs, ... }: {
              system.stateVersion = "24.11";
              boot.loader.systemd-boot.enable = true;
              boot.loader.efi.canTouchEfiVariables = true;
              networking.hostName = "test";
              
              # Add a root filesystem configuration
              fileSystems."/" = {
                device = "/dev/disk/by-label/nixos";
                fsType = "ext4";
              };
            })
          ];
        };
      };
    };
}
