This repo keeps its agent instructions in `CLAUDE.md` files; read them as if they were `AGENTS.md` files.

- Start with the root `CLAUDE.md`, then read every `CLAUDE.md` on the path from the repo root to each file you work on (for example `project-ember/CLAUDE.md` and `project-ember/src/ui/layers/CLAUDE.md` before changing `project-ember/src/ui/layers/LayerStack.ts`).
- A line like `@README.md` in a `CLAUDE.md` imports that file, relative to the `CLAUDE.md`; read it as part of the instructions.
- Edit the `CLAUDE.md` files, not this one, when instructions change.
