{
  description = "const-array - exploration of const generic arrays";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, fenix, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        fx = fenix.packages.${system};

        # Stable toolchain combined with nightly rustfmt
        toolchain = fx.combine [
          fx.stable.rustc
          fx.stable.cargo
          fx.stable.clippy
          fx.stable.rust-std
          fx.stable.rust-src
          fx.complete.rustfmt
        ];

        # Nightly toolchain for miri (this crate uses unsafe code)
        nightlyToolchain = fx.combine [
          fx.complete.rustc
          fx.complete.cargo
          fx.complete.clippy
          fx.complete.rust-std
          fx.complete.rust-src
          fx.complete.miri
          fx.complete.rustfmt
        ];

        mkDevShell = tc: pkgs.mkShell {
          packages = [ tc pkgs.rust-analyzer pkgs.cargo-mutants ];

          RUST_SRC_PATH = "${tc}/lib/rustlib/src/rust/library";
        };
      in
      {
        devShells.default = mkDevShell toolchain;
        devShells.miri = mkDevShell nightlyToolchain;
      }
    );
}
