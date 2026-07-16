# MD Viewer

A small, native, read-only Markdown viewer for Linux, written in Rust. It opens
and renders Markdown without presenting an editor, preview split, browser
engine, web server, or background service.

## Features

- freely resizable native window;
- CommonMark and common GitHub-flavoured elements;
- headings, emphasis, lists, task lists, tables, block quotes and code blocks;
- responsive full-width tables with content-weighted columns, cell wrapping,
  Markdown alignment, and consistently top-aligned rows;
- full-width lists with hanging indents for wrapped item text;
- local PNG, JPEG, GIF, WebP and SVG images;
- opening by command-line argument, file dialog, drag-and-drop, or desktop file
  association;
- a compact interface that keeps all controls available without editor chrome;
- light and dark themes;
- a comfortably readable 140% default document font;
- document-only zoom controls that leave the toolbar at a stable size;
- clickable task-list checkboxes for the current viewing session, without
  writing changes to the file;
- reload without reopening the window;
- clear in-window errors for missing, non-Markdown, non-UTF-8, or oversized
  files;
- no editing and no mutation of the opened file.

Remote images are deliberately not fetched automatically. Links open only when
the user clicks them. Raw HTML is not executed in a browser engine.

## Keyboard shortcuts

| Action | Shortcut |
|---|---|
| Open | `Ctrl+O` |
| Reload | `Ctrl+R` or `F5` |
| Zoom in | `Ctrl++` or `Ctrl+=` |
| Zoom out | `Ctrl+-` |
| Reset zoom | `Ctrl+0` |

Task-list checkboxes can be toggled while reading. These changes exist only in
the open window; `Reload` restores the file's actual state from disk.

## Build prerequisites

Install a Rust 1.92-or-newer toolchain through
[rustup](https://rustup.rs/). On Ubuntu 24.04, install the native X11, Wayland,
and OpenGL development packages with:

```bash
sudo apt install \
  build-essential pkg-config \
  libx11-dev libx11-xcb-dev libxcursor-dev libxi-dev libxrandr-dev \
  libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libgl1-mesa-dev
```

## Run from source

```bash
# Development build.
cargo run -- examples/showcase.md

# Responsive table and long-list stress fixture.
cargo run -- examples/layout-stress.md

# Tests and optimized release build.
cargo test
cargo build --release

# Open a file with the release binary.
target/release/md-viewer README.md
```

The accepted extensions are `.md`, `.markdown`, `.mdown`, and `.mkd`, matched
case-insensitively. Documents must be UTF-8 and no larger than 64 MiB.

## User-local installation

No root access is needed after building:

```bash
# Install binary, icon and desktop launcher, and make it the Markdown default.
./scripts/install-user.sh

# Install without changing the current default application.
./scripts/install-user.sh --no-default
```

Installed paths:

```text
~/.local/bin/md-viewer
~/.local/share/applications/md-viewer.desktop
~/.local/share/icons/hicolor/scalable/apps/md-viewer.svg
```

After installation, Markdown files can be opened from the file manager. The
command-line form remains available:

```bash
md-viewer path/to/file.md
```

## Uninstall

```bash
./scripts/uninstall-user.sh
```

If another program should become the Markdown default afterwards, select it in
the file manager's “Open With” dialog or use `xdg-mime default`.

## Architecture and dependency choices

- `eframe`/`egui` provides the native resizable window and GPU-accelerated UI.
- The smaller OpenGL (`glow`) renderer is used instead of the larger default
  `wgpu` stack.
- `egui_commonmark` handles CommonMark/GFM rendering.
- A small project-local `egui_commonmark` adaptation keeps list markers and
  scaled item text on the same row, lets checkboxes replace unordered task
  bullets, and renders responsive tables. Its upstream MIT/Apache-2.0 license
  files are retained in `vendor/`.
- `rfd` uses the desktop portal for the native file chooser.
- `image` decodes selected local raster formats; SVG support comes through
  `egui_extras`.

There is no Electron, WebKit, database, network listener, telemetry, or update
service.

## Development checks

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

The pure argument, file-validation, UTF-8, BOM, size-display, and extension
logic is covered by unit tests. GUI behavior is additionally checked with local
smoke tests against `examples/showcase.md` and `examples/layout-stress.md`.
