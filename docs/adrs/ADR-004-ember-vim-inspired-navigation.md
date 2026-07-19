# ADR-004: Vim/Neovim-Inspired Keyboard Navigation

## Status

Accepted

## Date

2026-07-19

## Context

Devora is a keyboard-friendly IDE that sprinkled a bit of Vim key-bindings, as they are very useful.
Because the convention is implicit, motions are accreted per surface and inconsistently:
- List `j`/`k` is hand-rolled in the Workspace Hub and Settings Hub but shared through `createListCursor` (`src/ui/components/listCursor.ts`) in the Command Palette and Repo List.
- The read-only scrollable surfaces — the Workspace Hub cheatsheet, the User Guide, and the Health Hub — have **no** keyboard scrolling at all (mouse/trackpad only).
- There is no shared primitive for scrolling a viewport by key, and no half-page (`Ctrl+D`/`Ctrl+U`) or jump-to-edge (`gg`/`G`) support anywhere.

Adding scrolling to the cheatsheet in isolation will grow the pile.
The underlying need is a stated model for keyboard navigation and a small set of reusable primitives that every surface adopts, so motions stay consistent and discoverable as the app grows.

## Decision

Adopt **Vim/Neovim as the explicit model** for Devora's keyboard navigation, and express it through shared primitives rather than per-surface reimplementation.

This should provide reusable primitives that support standard motions (`j`/`k`, `Ctrl+D`/`Ctrl+U`, `gg`/`G`) with extensibility for future motions (or other functionalities) without per-surface reimplementation.

## Consequences

- One place defines what `j`/`k`/`Ctrl+D`/`Ctrl+U`/`gg`/`G` do; the cheatsheet, User Guide, and other surfaces get identical, documented scrolling, and future scrollable surfaces adopt it in one line.
- Native `PageDown`/`Home`/`End`/`Space` scrolling is preserved: each surface scrolls its existing scroll container (the cheatsheet scrolls its focused layer wrapper).
- The Vim-inspired convention is now explicit, so `q`-dismiss, `j`/`k`, and the new scroll motions read as one coherent scheme instead of incidental choices.
