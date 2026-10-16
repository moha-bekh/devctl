# devctl for NixOS: `pkgs.callPackage ./nix/package.nix { }`.
{ lib, rustPlatform, addDriverRunpath }:

rustPlatform.buildRustPackage {
  pname = "devctl";
  version = "0.1.0";
  src = lib.cleanSource ../.;
  cargoLock.lockFile = ../Cargo.lock;

  # NVML (libnvidia-ml.so) is loaded at run time from the driver's directory.
  nativeBuildInputs = [ addDriverRunpath ];
  postFixup = "addDriverRunpath $out/bin/devctl";

  meta.mainProgram = "devctl";
}
