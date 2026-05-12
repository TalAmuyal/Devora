## UI Conventions

### Keyboard navigation

- `q` should always close/dismiss overlays, pages, and panels — same as `Escape`. This is a vim-style convention that applies globally across the Ember UI.
- When an input field is focused, `q` is just `q`, and `Escape` should unfocus the input rather than closing the page.
- Overlays and panels should document their keyboard shortcuts in a legend at the bottom of the page.

### Overlay system

- **Tab-covering overlay**: covers entire window including tab bar. Used for the WS management panel, User Guide, cheatsheet.
- **Panel overlay**: covers main panel area only, tab bar visible. Tied to a specific session tab. Used for Crit.
- Both types are dismissible with `q` or `Escape`.
