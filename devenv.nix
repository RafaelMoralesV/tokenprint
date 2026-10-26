{ pkgs, lib, ... }:

let
  dlopenLibraries = with pkgs; [
    libGL
    vulkan-loader

    # To use the x11 feature
    xorg.libX11
    xorg.libXcursor
    xorg.libXi
    xorg.libXrandr

    # To use the wayland feature
    libxkbcommon
    wayland
  ];

  ldpath = lib.makeLibraryPath dlopenLibraries;
in
{
  # https://devenv.sh/packages/
  packages = with pkgs; [
    git
    bacon
    openssl
    zenity
    clang
    pre-commit
  ];

  env = {
    LD_LIBRARY_PATH = ldpath;
    LIBCLANG_PATH = lib.makeLibraryPath [ pkgs.llvmPackages.libclang ];
  };

  enterShell = ''
    echo "
    ⭐ Rust GUI Dev Shell
    ---------------------
    • bacon:  interactive watcher
    • clippy:  quick clippy check
    • test:   run cargo tests
    "
  '';

  # https://devenv.sh/languages/
  languages.rust = {
    enable = true;
    mold.enable = true;
  };

  scripts.check.exec = "bacon check";
  scripts.test.exec = "bacon test";
  scripts.clippy.exec = "bacon clippy";

  scripts.docs.exec = "cargo doc --open";

  # https://devenv.sh/tests/
  enterTest = ''
    echo "Running tests"
    cargo test
  '';
}
