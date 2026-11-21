# Linux display backends

GlowstoneCL uses Iced 0.14 and Winit. Linux builds enable both native display
backends: Wayland and X11. This is not an XWayland-only compatibility layer.

## Backend selection

In a Wayland session, Winit can connect directly to the compositor through
WAYLAND_DISPLAY. In an X11 session it uses DISPLAY. XWayland is not required
for the native Wayland backend.

To force native Wayland:

    glowstonecl --backend wayland

To force X11 (including XWayland inside a Wayland desktop):

    glowstonecl --backend x11

The override changes environment variables only inside the GlowstoneCL
process and is applied before Iced creates the Winit event loop.

## Build

The default Linux Cargo features enable both backends:

    cargo run --package glowstonecl

### Arch Linux / CachyOS

    sudo pacman -S --needed wayland libxkbcommon libx11 libxrandr libxinerama libxcursor libxi vulkan-icd-loader

### Fedora

    sudo dnf install wayland-devel libxkbcommon-devel libX11-devel libXrandr-devel libXinerama-devel libXcursor-devel libXi-devel vulkan-loader-devel

### Debian / Ubuntu

    sudo apt install libwayland-dev libxkbcommon-dev libx11-dev libxrandr-dev libxinerama-dev libxcursor-dev libxi-dev libvulkan1

Exact package names can differ between distributions and graphics drivers.
