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
            fontconfig
            libxkbcommon
            libX11
            libXcursor
            libXfixes
            libXi
            libXrandr
            libxcb
            vulkan-loader
            wayland
          ];
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              cargo
              clippy
              gnumake
              pkg-config
              (python3.withPackages (p: [ p.asyncssh ]))
              rust-analyzer
              rustc
              rustfmt
            ] ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux (with pkgs; [
              dbus
              desktop-file-utils
              xdg-utils
            ]);

            buildInputs = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux linuxLibraries;

            LD_LIBRARY_PATH = pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isLinux
              (pkgs.lib.makeLibraryPath linuxLibraries);
          };
        });
    };
}
