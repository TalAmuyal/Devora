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
  /** The tab's task-creation progress page, while it has one. */
  progress: ProgressState | null;
}

/** A task-creation progress page; `action` is its button: `Cancel` while the creation runs, `Close` once it has failed. */
export interface ProgressState {
  title: string;
  steps: { label: string; state: 'active' | 'done' | 'failed' }[];
  outputLines: number;
  failure: string | null;
  action: 'Cancel' | 'Close';
}

/** The open New Task dialog; `repos` is its list as it shows, filtered. */
export interface NewTaskState {
  heading: 'New Task' | 'Duplicate Task';
  title: string;
  filter: string;
  /** What has the dialog's focus. */
  focus: 'title' | 'filter' | 'cancel' | 'create';
  repos: { name: string; selected: boolean; active: boolean }[];
}

/** A click on what the open New Task dialog draws. */
export type NewTaskAction = { action: 'toggleRepo'; name: string } | { action: 'create' } | { action: 'cancel' };

/** The open Workspace Hub; `items` is the list as it shows, filters applied. */
export interface HubState {
  /** Until the first load is done, the list shows its placeholder. */
  loading: boolean;
  category: 'Active' | 'Inactive' | 'All';
  filter: string;
  /** Whether a text field (the filter) has the keyboard focus. */
  filterFocused: boolean;
  items: { id: string; title: string; dot: 'clean' | 'modified' | 'pending' | 'error' }[];
  focused: string | null;
  /** The focused workspace's detail panel; `repos` is null while its status loads. */
  detail: {
    id: string;
    error: string | null;
    repos: { name: string; branch: string; detached: boolean; modified: number; untracked: number }[] | null;
  } | null;
  toast: string | null;
  /** The cheatsheet's scroll view, while it is open. */
  cheatsheet: { offset: number; contentHeight: number; viewportHeight: number } | null;
  /** The profile dropdown's trigger label: the selected profile's name. */
  profileLabel: string | null;
  /** The open header dropdown (the profiles, or the burger menu), with its rows, separators left out. */
  menu: {
    kind: HubMenu;
    items: { kind: 'option' | 'action'; label: string; detail: string | null; checked: boolean; enabled: boolean }[];
  } | null;
}

export type HubMenu = 'profiles' | 'main';

/** The open confirmation dialog; `focused` is the button Space would press. */
export interface DialogState {
  title: string;
  intro: string;
  items: { name: string; detail: string }[];
  confirmLabel: string;
  focused: 'confirm' | 'cancel';
}

/** A click on what the open hub draws: an action on its focused workspace, a dropdown's trigger, or a row of the open dropdown. */
export type HubAction =
  | { action: 'removeTask' }
  | { action: 'delete' }
  | { action: 'newTask' }
  | { action: 'duplicate' }
  | { action: 'toggleMenu'; menu: HubMenu }
  | { action: 'menuItem'; label: string };

export interface AppState {
  hub: HubState | null;
  dialog: DialogState | null;
  newTask: NewTaskState | null;
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
  /** Whether a text field or text view has the keyboard focus. */
  editingText: boolean;
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
   * `key` is a letter, a digit, `Minus`, `Equal`, `Slash`, `Space`, `Left`, `Right`, `Down`, `Up`, `Escape`, `Tab` or `Return`.
   * Returns whether the router consumed it, so that no view saw it.
   */
  async press(key: string, modifiers: Modifier[] = []): Promise<boolean> {
    return (await this.request('POST', '/test/key', { key, modifiers })).consumed;
  }

  /** Makes a tab's terminal the window's first responder, as a click into it would. */
  async focusTerminal(tab: number): Promise<void> {
    await this.request('POST', '/test/terminal/focus', { tab });
  }

  /** Clicks what the open Workspace Hub draws, as its buttons do; a disabled dropdown row is a 409. */
  async hub(action: HubAction): Promise<void> {
    await this.request('POST', '/test/hub', action);
  }

  /** Clicks what the open New Task dialog draws. */
  async newTask(action: NewTaskAction): Promise<void> {
    await this.request('POST', '/test/new-task', action);
  }

  /** Clicks the button of a tab's progress page (the active tab's by default). */
  async progressAction(tab?: number): Promise<void> {
    await this.request('POST', '/test/progress', { tab });
  }

  /** Clicks a button of the open confirmation dialog. */
  async dialog(button: 'confirm' | 'cancel'): Promise<void> {
    await this.request('POST', '/test/dialog', { button });
  }

  /** The errors shown since the last scrape, which it clears, as Ember's `__scrapeErrors`. */
  async scrapeErrors(): Promise<string[]> {
    return (await this.request('POST', '/test/errors/scrape')).errors;
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
