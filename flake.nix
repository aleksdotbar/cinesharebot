{
  description = "Cinesharebot: native Rust package";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

  outputs = { nixpkgs, ... }:
    let
      pkgs = nixpkgs.legacyPackages.x86_64-linux;
      package = pkgs.callPackage ./package.nix { };
    in
    {
      packages.x86_64-linux = {
        default = package;
        cinesharebot = package;
      };
      devShells.x86_64-linux.default = pkgs.mkShell {
        packages = with pkgs; [ cargo rustc rustfmt clippy ];
      };
    };
}
