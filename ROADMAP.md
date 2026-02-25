# WayFrame Roadmap

## Scope Lock

WayFrame is a **Wayland compatibility layer**, not a full desktop/session compositor.

In scope:

- Wrapping launched apps (`--wrap` style flow).
- SSD/CSD compatibility behavior.
- Input/resize/identity/protocol compatibility for wrapped apps.
- Multi-window handling for wrapped app processes.

Out of scope:

- Replacing Mutter/KWin/Sway as the system compositor.
- Full desktop shell concerns (workspaces, panels, global output/session management).

This roadmap reflects the current codebase state.

## Completed

- Nested Wayland server with dynamic socket launch flow.
- Core globals and handlers: compositor, xdg-shell, xdg-decoration, shm, output, seat.
- Forced SSD decoration negotiation for hosted clients.
- Input forwarding: pointer motion/buttons/scroll and keyboard.
- GTK host window with frame presentation.
- dmabuf texture import path in GTK with SHM fallback.
- Window action bridging: move/maximize/fullscreen/minimize/close.
- Content constraints propagation (min-size hints).
- App identity flow:
  - launch-command seed (title/icon/prgname)
  - runtime xdg metadata updates (title/app_id)

## In Progress

- Resize policy tuning (latency vs smoothness tradeoff under aggressive user resizing).
- Better handling of toolkit-specific decoration edge cases.

## Next

1. Add configurable resize presets and per-app overrides.
2. Implement damage-region-aware updates to reduce full-frame work.
3. Improve pointer/scroll fidelity (smooth scrolling, system-like behavior).
4. Improve drag/resize UX polish (interactive edges, better move/resize handling).
5. Add a decoration policy engine (force SSD / prefer CSD / auto per app).
6. Improve startup/identity integration beyond current best-effort matching.
7. Add protocol diagnostics mode for metadata/decoration/input debugging.
8. Add user config file support (`~/.config/wayframe/config.toml`).
9. Add robust multi-window/toplevel handling.
10. Continue splitting large modules and add focused integration tests.

## Future (Within Scope)

- Evaluate stronger zero-copy paths for more client/toolkit combinations.
- Expand compatibility matrix and document app-specific quirks.
- Add optional compatibility plugins/quirk packs per toolkit family.
