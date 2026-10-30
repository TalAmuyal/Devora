# Third-Party Licenses

Devora bundles and depends on the following third-party software and assets.

## Go Dependencies

### charm.land/lipgloss/v2

- **License:** MIT
- **Source:** https://github.com/charmbracelet/lipgloss

### github.com/BurntSushi/toml

- **License:** MIT
- **Source:** https://github.com/BurntSushi/toml

### github.com/goccy/go-yaml

- **License:** MIT
- **Source:** https://github.com/goccy/go-yaml

### golang.org/x/sync

- **License:** BSD-3-Clause
- **Source:** https://github.com/golang/sync

## Assets

### Catppuccin palette

- **License:** MIT
- **Source:** https://github.com/catppuccin/catppuccin
- **Note:** Source of the Mocha hex values used in `project-debi/internal/style/styles.go`.

### Catppuccin for Kitty (Pocco81)

- **License:** MIT
- **Source:** https://github.com/catppuccin/kitty
- **Note:** The Macchiato variant in `themes/catppuccin-macchiato.conf` is taken from this port.

## Devora Sapphire

### Ghostty

- **License:** MIT
- **Source:** https://github.com/ghostty-org/ghostty
- **Note:** libghostty is built from source at a pinned commit and linked into Devora Sapphire; `project-sapphire/Sources/TerminalKit/TerminalView.swift` adapts the keyboard and IME handling of Ghostty's macOS app.
