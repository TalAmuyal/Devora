/** A client for the app's test bridge (`Sources/AppShell/TestBridge.swift`). */

export interface TabState {
  id: number;
  title: string;
  kind: 'shell' | 'workspace' | 'creatingWorkspace' | 'creationFailed';
  /** `none`, `starting`, `running` or `failed: <reason>`. */
  terminal: string;
  /** Whether the tab's terminal is its window's first responder. */
  focused: boolean;
  /** The names of the layers on the tab's stack, bottom to top; `terminal` once its terminal runs. */
  layers: string[];
}

export interface AppState {
  version: string;
  profiles: { name: string; path: string }[];
  selectedProfile: string | null;
  workspaces: { id: string; title: string; path: string; active: boolean }[];
  /** The error banners on screen, oldest first; `count` is how many times the message was shown while its banner was up. */
  errorBanners: { id: number; message: string; count: number }[];
  /** This run's log file. */
  logPath: string;
  /** Why the app state couldn't be opened at launch; the app waits for an answer (`startFresh`, or `quit`) meanwhile. */
  startupProblem:
    | { kind: 'stateInUse'; path: string }
    | { kind: 'stateUnreadable'; path: string; reason: string }
    | null;
  activeTab: number | null;
  tabs: TabState[];
  /** The terminals' font size, in points. */
  fontSize: number;
  /** The names of the layers on the window stack, bottom to top. */
  windowLayers: string[];
}

export type Modifier = 'shift' | 'control' | 'option' | 'command';

export type Command =
  | { command: 'load' }
  | { command: 'openShell' }
  | { command: 'selectProfile'; path: string }
  | { command: 'openWorkspace'; id: string }
  | { command: 'activateTab'; id: number }
  | { command: 'closeTab'; id: number }
  | { command: 'startFresh' }
  | { command: 'dismissError'; id: number };

export class BridgeError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

export class Bridge {
  constructor(private readonly port: number) {}

  state(): Promise<AppState> {
    return this.request('GET', '/test/state');
  }

  /** Dispatches a store command and waits for it to finish; state changes reach `state()` on the next frame. */
  async dispatch(command: Command): Promise<void> {
    await this.request('POST', '/test/dispatch', command);
  }

  /** Asks the app to quit, as the app menu's Quit does; the app may exit before it answers, so callers wait for the process to exit. */
  async quit(): Promise<void> {
    try {
      await this.request('POST', '/test/quit');
    } catch (error) {
      if (error instanceof BridgeError) throw error;
    }
  }

  /** The scrollback and screen of a tab's terminal (the active tab's by default). */
  async terminalText(tab?: number): Promise<string> {
    const query = tab === undefined ? '' : `?tab=${tab}`;
    return (await this.request('GET', `/test/terminal/text${query}`)).text;
  }

  /** Types a line into a tab's terminal (the active tab's by default) and presses Return. */
  async typeLine(line: string, tab?: number): Promise<void> {
    await this.request('POST', '/test/terminal/type', { line, tab });
  }

  /**
   * Presses a key: the app makes the key-down AppKit would and sends it through its key router, then, unless the router consumes it, to the window's first responder.
   * `key` is a letter, a digit, `Minus`, `Equal`, `Left`, `Right`, `Escape` or `Return`.
   */
  async press(key: string, modifiers: Modifier[] = []): Promise<void> {
    await this.request('POST', '/test/key', { key, modifiers });
  }

  /** The errors shown since the last scrape, which it clears, as Ember's `__scrapeErrors`. */
  async scrapeErrors(): Promise<string[]> {
    return (await this.request('POST', '/test/errors/scrape')).errors;
  }

  /** Opens a stand-in window surface: a page over the whole window, with nothing to focus, that `q` or Escape closes. */
  async openTestSurface(): Promise<void> {
    await this.request('POST', '/test/surface');
  }

  /** Polls `read` until `done` accepts its value; a `BridgeError` (e.g. a terminal that isn't running yet) counts as not done. */
  async waitFor<T>(what: string, read: () => Promise<T>, done: (value: T) => boolean, timeoutMs = 10_000): Promise<T> {
    const start = Date.now();
    let last: T | BridgeError | undefined;
    for (;;) {
      try {
        last = await read();
        if (done(last)) return last;
      } catch (error) {
        if (!(error instanceof BridgeError)) throw error;
        last = error;
      }
      if (Date.now() - start >= timeoutMs) {
        const seen = last instanceof BridgeError ? last.message : JSON.stringify(last);
        throw new Error(`Timed out after ${timeoutMs}ms waiting for ${what}; last saw: ${seen}`);
      }
      await new Promise((r) => setTimeout(r, 100));
    }
  }

  private async request(method: string, path: string, body?: unknown): Promise<any> {
    const response = await fetch(`http://127.0.0.1:${this.port}${path}`, {
      method,
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    const json = await response.json();
    if (!response.ok) throw new BridgeError(response.status, `${method} ${path}: ${json.error}`);
    return json;
  }
}
