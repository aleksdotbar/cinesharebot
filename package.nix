{ lib, rustPlatform }:
let
  manifest = builtins.fromTOML (builtins.readFile ./Cargo.toml);
in
rustPlatform.buildRustPackage {
  pname = manifest.package.name;
  inherit (manifest.package) version;
  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.unions [ ./Cargo.toml ./Cargo.lock ./src ];
  };
  cargoLock.lockFile = ./Cargo.lock;
  strictDeps = true;
  doCheck = true;
  meta = {
    mainProgram = "cinesharebot";
    platforms = [ "x86_64-linux" ];
  };
}
