{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";
    crane = {
      url = "github:Xe Gravel/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
      };

  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
       rust-overlay,
    }:
    let
      inherit (nixpkgs) system;
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ rust-overlay.overlay ];
      };
      rustToolchain = pkgs.rust-bin.stable.latest.default;
      craneLib = crane.mkLib pkgs; # IMPORTANT CHANGE

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
      overlays.default = final: prev: { ng = final.callPackage ./package.nix { inherit rev craneLib; }; };

      packages = forAllSystems (pkgs: rec {
        ng = pkgs.callPackage ./package.nix { inherit rev craneLib; };
        default = ng;
      });

      devShells = forAllSystems (pkgs: {
        default = import ./shell.nix { inherit pkgs rustToolchain; };
      });

      formatter = forAllSystems (pkgs: pkgs.nixfmt-rfc-style);
    };
}
