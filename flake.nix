{
  description = "Goard — OAR cluster job dashboard (egui/eframe)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };


      # these libs dlopen at runtime. On NixOS these aren't on a
      # standard loader path, so the binary must be told where to find them.
      runtimeLibs = with pkgs; [
        wayland          
        libxkbcommon     
        libGL            
        # X11 fallback 
        libx11
        libxcursor
        libxi
        libxrandr
      ];
    in
    {
      packages.${system}.default = pkgs.rustPlatform.buildRustPackage {
        pname = "rust-dashboard-app";
        version = "1.1.0";
        src = ./.;
        cargoLock.lockFile = ./Cargo.lock;

        nativeBuildInputs = [ pkgs.pkg-config pkgs.makeWrapper ];
        buildInputs = runtimeLibs;

        doCheck = false; # no test suite in this repo

        # Bake the runtime library path into the binary 
        postFixup = ''
          wrapProgram $out/bin/rust-dashboard-app \
            --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath runtimeLibs}
        '';
      };

      apps.${system}.default = {
        type = "app";
        program = "${self.packages.${system}.default}/bin/rust-dashboard-app";
      };

      devShells.${system}.default = pkgs.mkShell {
        nativeBuildInputs = [ pkgs.cargo pkgs.rustc pkgs.pkg-config ];
        buildInputs = runtimeLibs;
        LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath runtimeLibs;
      };
    };
}
