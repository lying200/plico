{ pkgs, ... }:
{
  languages.rust = {
    enable = true;
    channel = "stable";
    targets = [ "x86_64-pc-windows-msvc" ];
  };
  packages = with pkgs; [
    git pkg-config clang lld llvmPackages.llvm cmake ninja python3 curl cargo-xwin
    openssl zlib fontconfig freetype libxkbcommon wayland
    libGL vulkan-loader vulkan-tools
    xorg.libX11 xorg.libXcursor xorg.libXi xorg.libXrandr
    xorg.libxcb
  ];
  env.LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (with pkgs; [
    libGL vulkan-loader fontconfig freetype libxkbcommon wayland
    xorg.libX11 xorg.libXcursor xorg.libXi xorg.libXrandr xorg.libxcb
  ]);
  env.LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
}
