import type { LayerStack } from './LayerStack';

export interface LayerRouterDeps {
  /** The app-covering stack: hubs, the Command Palette, etc. */
  windowStack: LayerStack;
  /** The active tab's stack, resolved live so it can never go stale; null when no session is open. */
  resolveTabStack: () => LayerStack | null;
}

/**
 * The single owner of keyboard routing for the whole app (ADR-003).
 *
 * One `window`-capture keydown listener runs three stages in order:
 * app-wide shortcuts → the window stack → the active tab's stack.
 *
 * Shortcuts run *first*, so each one carries its own precondition (see `../KeyboardShortcuts.ts`).
 * That is what lets font sizing and `F1` stay live under every surface.
 *
 * A stack that reports `blocked` ends routing without consuming the event, so the key still reaches the focused element — that is how a terminal surface receives typing while remaining an opaque barrier.
 */
export class LayerRouter {
  private readonly deps: LayerRouterDeps;
  private keyObserver: ((e: KeyboardEvent) => void) | null = null;
  private shortcutHandler: ((e: KeyboardEvent) => boolean) | null = null;

  constructor(deps: LayerRouterDeps) {
    this.deps = deps;
  }

  /** Register THE window-capture keydown listener. Call once, before any other keydown handler. */
  install(): void {
    window.addEventListener('keydown', this.boundHandleKeydown, true);
  }

  uninstall(): void {
    window.removeEventListener('keydown', this.boundHandleKeydown, true);
  }

  /** A non-consuming observer that sees every keydown before anything else acts on it (Shift-Shift tracking). */
  setKeyObserver(fn: (e: KeyboardEvent) => void): void {
    this.keyObserver = fn;
  }

  /** The app-wide shortcut policy, tried before any layer. Returning `true` consumes. */
  setShortcutHandler(fn: (e: KeyboardEvent) => boolean): void {
    this.shortcutHandler = fn;
  }

  private readonly boundHandleKeydown = (e: KeyboardEvent): void => this.handleKeydown(e);

  private handleKeydown(e: KeyboardEvent): void {
    this.keyObserver?.(e);

    if (this.shortcutHandler?.(e) === true) {
      this.consume(e);
      return;
    }

    const inWindow = this.deps.windowStack.route(e);
    if (inWindow === 'consumed') {
      this.consume(e);
      return;
    }
    if (inWindow === 'blocked') return;

    if (this.deps.resolveTabStack()?.route(e) === 'consumed') {
      this.consume(e);
    }
  }

  private consume(e: KeyboardEvent): void {
    e.preventDefault();
    // stopImmediatePropagation (not stopPropagation) so no sibling window-capture listener can also act on a consumed key.
    e.stopImmediatePropagation();
  }
}
