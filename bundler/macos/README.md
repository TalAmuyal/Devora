## MacOS App Structure

```
Devora/                           (DMG root)
├─ Applications                   (symlink to /Applications)
├─ CHANGELOG.md
├─ USER_GUIDE.md
├─ cc-simple-statusline
└─ Devora.app/
   └─ Contents/
      ├─ Info.plist
      ├─ MacOS/
      │  └─ bootstrap.sh
      └─ Resources/
         ├─ cc-plugins/
         │  ├─ crit/
         │  │  ├─ .claude-plugin/
         │  │  │  └─ plugin.json
         │  │  ├─ hooks/
         │  │  │  └─ hooks.json
         │  │  └─ skills/
         │  ├─ detached-flow/
         │  │  ├─ bin/
         │  │  │  ├─ check-pr
         │  │  │  ├─ close-pr
         │  │  │  └─ submit-pr
         │  │  └─ skills/
         │  │     └─ submit-pr/
         │  │        └─ SKILL.md
         │  ├─ judge/
         │  │  ├─ hooks/
         │  │  │  └─ hooks.json
         │  │  └─ main.py
         │  └─ team-work/
         │     └─ skills/
         │        └─ team-work/
         │           └─ SKILL.md
         ├─ app.icns
         ├─ CHANGELOG.md
         ├─ USER_GUIDE.md
         ├─ VERSION
         ├─ bundled-apps/
         │  ├─ ccc
         │  ├─ crit                   (wrapper, forwards to original-crit with overlay behavior)
         │  ├─ debi
         │  ├─ glimpse-tty           (launcher shim, forwards to ../glimpse-tty/glimpse-tty)
         │  ├─ glow
         │  └─ original-crit
         ├─ kitty-configs/
         │  ├─ current-theme.conf
         │  ├─ glow-theme.json
         │  └─ kitty.conf
         ├─ crit-license.txt
         ├─ kitty-license.txt
         ├─ uv-license.txt
         ├─ glimpse-tty-license.txt
         ├─ kitty.app/
         │  └─ ...
         ├─ glimpse-tty/             (distribution: launcher + Electron + node_modules)
         │  └─ ...
         └─ uv
```

## Dev Mode

The `--dev` flag builds a "Dev-Devora" variant that can coexist with the production app.
It uses a different bundle identifier (`com.devora-org.devora-dev`), different app/window titles, a separate kitty socket path, and a distinct `macos_titlebar_color` (`#F5A97F`, Catppuccin Macchiato peach) so the dev window is visually distinguishable from the release build.

Output goes to `bin/macOS/Dev-Devora/Dev-Devora.app` (instead of `bin/macOS/Devora/Devora.app`).
