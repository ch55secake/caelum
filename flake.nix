{
  description = "Caelum development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-26.05-darwin";

  outputs = { nixpkgs, ... }:
    let
      systems = [
        "aarch64-darwin"
        "x86_64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      devShells = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
          linuxLibraries = with pkgs; [
            openssl
          ];
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              cargo
              clippy
              gnumake
              pkg-config
              python3
              rust-analyzer
              rustc
              rustfmt
            ];

            buildInputs = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux linuxLibraries;

            LD_LIBRARY_PATH = pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isLinux
              (pkgs.lib.makeLibraryPath linuxLibraries);
          };
        });
    };
}
