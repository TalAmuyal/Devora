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
         │  ├─ debi
         │  └─ glow
         ├─ kitty-configs/
         │  ├─ current-theme.conf
         │  ├─ glow-theme.json
         │  └─ kitty.conf
         ├─ kitty-license.txt
         ├─ uv-license.txt
         ├─ kitty.app/
         │  └─ ...
         └─ uv
```

## Dev Mode

The `--dev` flag builds a "Dev-Devora" variant that can coexist with the production app.
It uses a different bundle identifier (`com.devora-org.devora-dev`), different app/window titles, a separate kitty socket path, and a distinct `macos_titlebar_color` (`#F5A97F`, Catppuccin Macchiato peach) so the dev window is visually distinguishable from the release build.

Output goes to `bin/macOS/Dev-Devora/Dev-Devora.app` (instead of `bin/macOS/Devora/Devora.app`).
