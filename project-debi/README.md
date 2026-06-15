# Devora

Debi is Devora's CLI for workspace management and utilities.
In the past, `debi` also had the responsibility of being the app's TUI (before the Ember build), but now it is primarily a CLI tool.
The TUI code still exists in the repo as "Devora OG" (original), and will be removed once all of its features are ported to Ember.

## TUI

### Tech Stack

- [Bubble Tea](https://github.com/charmbracelet/bubbletea) for building the TUI (deprecated)
- [Lip Gloss](https://github.com/charmbracelet/lipgloss) for styling the TUI (deprecated)

### Kitty Setup

The Kitty integration below applies to the legacy Kitty-based build (Devora OG); the published Ember build does not use Kitty.
Devora OG uses Kitty's remote control protocol to manage terminal sessions.

## Running the App

Because `mise` is auto-running `go mod tidy` for you under the hood, you can run the app directly via:
```bash
mise start
```

## Building

```bash
mise build
```

This produces the `debi` binary.

## Testing

```bash
mise test
```

## Documentation

See [docs/README.md](docs/README.md) for a guide to the project documentation.
