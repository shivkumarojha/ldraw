# LDraw (Rust Desktop Whiteboard)

Linux-first, native whiteboard inspired by tldraw, with infinite canvas, shape-first toolbar, freehand tools, and fast local save/export.

## What is implemented

- Infinite canvas with smooth pan and zoom.
- Left-side toolbar with direct access to shapes (no hidden shape menu).
- Tools: Select, Hand, Laser, Pen, Pencil, Highlighter, Eraser, Line, Arrow, Rectangle, Ellipse, Diamond, Triangle, Polygon, Star, Text, Image frame.
- Selection and transforms: move, marquee select, resize handles, rotate handle.
- Rich style controls: stroke/fill, opacity, dash, blend mode, shape params.
- Compact floating UI: icon-only tool rail, compact style panel, compact minimap.
- Expanded color palette plus custom color.
- Undo/redo history.
- Layer order actions: bring to front / send to back.
- Lock/unlock selected shapes.
- Snap-to-grid and angle snapping (Shift while rotating or ending line-like strokes).
- File operations:
  - Save/load native `.ldrw` JSON format.
  - Autosave.
  - Export SVG.
  - Export PNG.

## Linux build

```bash
cargo run
```

## Controls

- `V` select
- `H` hand (pan)
- `K` laser
- `P` pen
- `N` pencil
- `Y` highlighter
- `E` eraser
- `L` line
- `A` arrow
- `R` rectangle
- `O` ellipse
- `D` diamond
- `T` triangle
- `G` polygon
- `S` star
- `X` text
- `I` image frame
- `Ctrl+Z` undo
- `Ctrl+Shift+Z` redo
- `Ctrl+S` save
- `Ctrl+O` open
- `Ctrl+N` new
- `Ctrl+D` duplicate
- `Delete` delete selection

## Notes

- Image tool currently places an image frame with the file path label. It does not decode and render bitmap pixels yet.
- Tablet pressure uses touch force events when available from the platform/input stack; otherwise strokes use default pressure.
