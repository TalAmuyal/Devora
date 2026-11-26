# TerminalKit

The only module that imports `GhosttyKit` (libghostty's embedding API): `TerminalRuntime`, the process-wide libghostty app and its config; `TerminalView`, one surface; `TerminalSession`, its model; `TerminalLaunch`, what it runs.

## The config and the colors

libghostty has no setter API, so `TerminalRuntime` writes its config as a file and loads it. The colors are the active variant of the chosen palette (`TerminalRuntime.apply`: light or dark after the app's appearance), in two theme files the config names as a conditional theme (`theme = light:…,dark:…`, `ThemeFiles`); a switch of appearance or palette regenerates the config, and every terminal repaints. Each terminal is told the scheme (`ghostty_surface_set_color_scheme`) when it's made and on every switch, so a program that asked (mode 2031) is told of a switch, and one that asks (DSR 996) gets the current scheme.

**Both theme files hold the active palette, under two names, and the app never gets a scheme (`ghostty_app_set_color_scheme`).**
- libghostty reports the scheme from a surface's config state (`Termio.colorSchemeReportLocked`, `vendor/ghostty/src/termio/Termio.zig`), which a surface's scheme reaches only through a replay of the config's file lines (`Config.changeConditionalState`, `src/config/Config.zig`), and only when the theme is conditional: when its light and dark names differ. Identical contents mean neither scheme can draw other colors than Sapphire chose, so a new terminal in Dark has no light first frame.
- A surface made while the app's scheme differs from the config's (light) is itself made from such a replay (`Surface.init`, `src/Surface.zig`), which drops what the embedding API set on its config directly (`src/apprt/embedded.zig`, `opts.env_vars` and `opts.command`): its shell would start without `DEVORA_PTY_ID` and `DEVORA_IPC_PORT` (`debi preview` then fails) and without its Default app. The app's scheme stays the default, so this never happens; the surface's own scheme is set after its process started, and a replay then can't touch the process (`Termio.changeConfig` never restarts it).
- A surface's scheme replays its config only once Sapphire answers libghostty's `reload_config` (soft) with `ghostty_surface_update_config`, as Ghostty's own app does (`TerminalRuntime.handle`).
- On a switch, in this order: write both theme files (each replaced whole, since a replay reads them at any moment), update the config, then set each terminal's scheme. The files live in `$TMPDIR/sapphire-ghostty-themes-<pid>/`, removed at quit, or at the next launch after a crash.
- A switch can report twice (once for the config change, once for the scheme), as in Ghostty's own app; programs take a repeat as no change.

`terminal-appearance.feature` checks the reports in a terminal made in either appearance, and a new terminal's environment, Default app and directory at launch in Dark and after a switch either way.
