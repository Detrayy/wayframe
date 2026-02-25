# WayFrame

WayFrame is a small nested Wayland compositor that wraps a client app inside a GTK4/libadwaita host window and enforces server-side decorations (SSD).

It exists for apps that behave poorly on GNOME when `xdg-decoration` negotiation does not match what users want.

## What It Does

- Starts its own Wayland socket.
- Launches a target app on that socket.
- Implements `xdg-shell`, `xdg-decoration`, `wl_shm`, and `linux-dmabuf` handling.
- Forces SSD mode for the client.
- Presents the client content in a host GTK window.
- Forwards input (pointer, scroll, keyboard) to the nested client.
- Syncs host actions (move/maximize/fullscreen/minimize/close) with client requests.
- Uses dmabuf texture import when available, with SHM fallback.

## Current Status

This is an active prototype focused on behavior and compatibility:

- Works with many Wayland apps, including Electron-based apps.
- Has resize throttling/debounce to reduce configure spam under heavy resize.
- Uses app identity metadata (title/app_id) and launch-command seeding for better shell/dock integration.

## Build

Requirements (typical):

- Rust stable + Cargo
- GTK4 + libadwaita development packages
- Wayland development packages

Then:

```bash
cargo build
```

## Run

```bash
cargo run -- <app> [args...]
```

Examples:

```bash
cargo run -- spotify
cargo run -- code --ozone-platform=wayland
cargo run -- alacritty
```

## Notes

- WayFrame is a wrapper compositor, not a full desktop compositor.
- Some client behavior still depends on toolkit implementation details.
- If a client does not provide metadata early, WayFrame seeds identity from the launch command and updates when real metadata arrives.

## Known Issues

- Resize behavior is still heuristic-driven; some apps can feel jumpy or delayed depending on debounce settings.
- Damage-aware partial updates are not implemented yet; large surfaces can still be expensive.
- Some app/toolkit combinations may report metadata late, causing brief fallback identity before correction.
- SSD/CSD edge cases vary by app (especially Electron/Chromium variants).

## Roadmap (Current)

1. Stabilize resize policy with configurable presets (`snappy`, `balanced`, `stable`) and per-app overrides.
2. Add damage-region-aware updates to reduce work during animation/resize.
3. Improve decoration policy handling for apps with custom titlebars and mixed CSD/SSD behavior.
4. Add protocol/identity diagnostics mode (`--debug-protocol`) for faster compatibility debugging.
5. Split larger modules further and add focused integration tests around input, metadata, and resize paths.
