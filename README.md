# LDraw

LDraw is a Linux-first, native whiteboard app built in Rust.

It focuses on a fast infinite canvas workflow with compact UI controls, vector shapes, freehand tools, and local file export.

## Features

- Infinite canvas with smooth pan and zoom.
- Compact floating UI (tool rail, style panel, minimap).
- Tools: Select, Hand, Laser, Pen, Pencil, Highlighter, Eraser, Line, Arrow, Rectangle, Ellipse, Diamond, Triangle, Polygon, Star, Text, Image frame.
- Selection and transforms: marquee select, move, resize, rotate, duplicate, delete.
- Save/load `.ldrw`, autosave, export SVG and PNG.

## Linux Prerequisites

1) Install Rust:

```bash
curl https://sh.rustup.rs -sSf | sh
source "$HOME/.cargo/env"
```

2) Install native deps (pick your distro):

Ubuntu/Debian:

```bash
sudo apt update
sudo apt install -y build-essential pkg-config make \
  libx11-dev libxrandr-dev libxi-dev libxcursor-dev libxkbcommon-dev \
  libwayland-dev libgl1-mesa-dev desktop-file-utils
```

Fedora:

```bash
sudo dnf install -y gcc gcc-c++ make pkg-config desktop-file-utils \
  libX11-devel libXrandr-devel libXi-devel libXcursor-devel libxkbcommon-devel \
  wayland-devel mesa-libGL-devel
```

Arch:

```bash
sudo pacman -S --needed base-devel pkgconf desktop-file-utils \
  libx11 libxrandr libxi libxcursor libxkbcommon wayland mesa
```

## Quick Start

Run in development:

```bash
git clone <your-fork-or-repo-url> ldraw
cd ldraw
make run
```

## System-Wide Install (Minimal)

Install binary + desktop entry + app icon:

```bash
sudo make install
```

Then launch from:

- your app launcher/menu (`LDraw`), or
- terminal with:

```bash
ldraw
```

Uninstall system-wide:

```bash
sudo make uninstall
```

## Build Release Only

```bash
make release
./target/release/ldraw
```

## Controls

- `V` Select
- `H` Hand / Pan
- `K` Laser
- `P` Pen
- `N` Pencil
- `Y` Highlighter
- `E` Eraser
- `L` Line
- `A` Arrow
- `R` Rectangle
- `O` Ellipse
- `D` Diamond
- `T` Triangle
- `G` Polygon
- `S` Star
- `X` Text
- `I` Image frame
- `Ctrl+Z` Undo
- `Ctrl+Shift+Z` Redo
- `Ctrl+S` Save
- `Ctrl+O` Open
- `Ctrl+N` New
- `Ctrl+D` Duplicate
- `Delete` Delete selection

## Contributing

See `CONTRIBUTING.md` for setup, workflow, and PR guidelines.

## Notes

- Image tool currently places an image frame with path label (bitmap rendering is planned).
- Tablet pressure uses pointer/touch-force paths when available; otherwise stroke pressure falls back.
