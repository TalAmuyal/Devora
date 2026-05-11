import { Terminal, type ITheme } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebglAddon } from '@xterm/addon-webgl';
import { Unicode11Addon } from '@xterm/addon-unicode11';
import { WebLinksAddon } from '@xterm/addon-web-links';
import { ClipboardAddon } from '@xterm/addon-clipboard';
import { SearchAddon } from '@xterm/addon-search';
import { invoke, Channel } from '@tauri-apps/api/core';

function getThemeFromCSS(): ITheme {
  const style = getComputedStyle(document.documentElement);
  const get = (prop: string) => style.getPropertyValue(prop).trim();
  return {
    background: get('--color-terminal-bg'),
    foreground: get('--color-terminal-fg'),
    cursor: get('--color-terminal-cursor'),
    selectionBackground: get('--color-terminal-selection-bg'),
    selectionForeground: get('--color-terminal-selection-fg'),
    black: get('--color-ansi-black'),
    red: get('--color-ansi-red'),
    green: get('--color-ansi-green'),
    yellow: get('--color-ansi-yellow'),
    blue: get('--color-ansi-blue'),
    magenta: get('--color-ansi-magenta'),
    cyan: get('--color-ansi-cyan'),
    white: get('--color-ansi-white'),
    brightBlack: get('--color-ansi-bright-black'),
    brightRed: get('--color-ansi-bright-red'),
    brightGreen: get('--color-ansi-bright-green'),
    brightYellow: get('--color-ansi-bright-yellow'),
    brightBlue: get('--color-ansi-bright-blue'),
    brightMagenta: get('--color-ansi-bright-magenta'),
    brightCyan: get('--color-ansi-bright-cyan'),
    brightWhite: get('--color-ansi-bright-white'),
  };
}

function getFontFamilyFromCSS(): string {
  const style = getComputedStyle(document.documentElement);
  return style.getPropertyValue('--font-mono').trim();
}

function getFontSizeFromCSS(): number {
  const raw = getComputedStyle(document.documentElement).fontSize;
  return parseInt(raw, 10) || 15;
}

const RESIZE_DEBOUNCE_MS = 50;

export class TerminalPane {
  private terminal: Terminal;
  private fitAddon: FitAddon;
  private webglAddon: WebglAddon | null = null;
  private _searchAddon: SearchAddon;
  private ptyId: number | null = null;
  private resizeObserver: ResizeObserver | null = null;
  private resizeDebounceTimer: ReturnType<typeof setTimeout> | null = null;
  private onExitCallback: ((ptyId: number) => void) | null = null;

  constructor(container: HTMLElement) {
    this.terminal = new Terminal({
      theme: getThemeFromCSS(),
      fontFamily: getFontFamilyFromCSS(),
      fontSize: getFontSizeFromCSS(),
      scrollback: 0,
      cursorBlink: true,
      allowProposedApi: true,
    });

    this.fitAddon = new FitAddon();
    this.terminal.loadAddon(this.fitAddon);

    this.terminal.open(container);

    // Suppress DECRQM (request mode) sequences — xterm.js v6 has a bug in
    // requestMode that throws ReferenceError during parsing. Register no-op
    // handlers so the built-in crashy handler is never reached.
    this.terminal.parser.registerCsiHandler({ intermediates: '$', final: 'p' }, () => true);
    this.terminal.parser.registerCsiHandler({ prefix: '?', intermediates: '$', final: 'p' }, () => true);

    this.tryLoadWebglAddon();

    const unicode11Addon = new Unicode11Addon();
    this.terminal.loadAddon(unicode11Addon);
    this.terminal.unicode.activeVersion = '11';

    const webLinksAddon = new WebLinksAddon((_event, uri) => {
      window.open(uri, '_blank');
    });
    this.terminal.loadAddon(webLinksAddon);

    const clipboardAddon = new ClipboardAddon();
    this.terminal.loadAddon(clipboardAddon);

    this._searchAddon = new SearchAddon();
    this.terminal.loadAddon(this._searchAddon);

    this.resizeObserver = new ResizeObserver(() => {
      if (this.resizeDebounceTimer !== null) {
        clearTimeout(this.resizeDebounceTimer);
      }
      this.resizeDebounceTimer = setTimeout(() => {
        this.fitAddon.fit();
        this.resizeDebounceTimer = null;
      }, RESIZE_DEBOUNCE_MS);
    });
    this.resizeObserver.observe(container);
  }

  private tryLoadWebglAddon(): void {
    try {
      const webglAddon = new WebglAddon();
      webglAddon.onContextLoss(() => {
        // On WebGL context loss, dispose the addon and fall back to DOM renderer
        webglAddon.dispose();
        this.webglAddon = null;
      });
      this.terminal.loadAddon(webglAddon);
      this.webglAddon = webglAddon;
    } catch (e) {
      console.warn('WebGL addon failed to load, falling back to DOM renderer:', e);
      this.webglAddon = null;
    }
  }

  async connect(cwd?: string, appCommand?: string): Promise<void> {
    this.fitAddon.fit();

    const outputChannel = new Channel<number[]>();
    outputChannel.onmessage = (data: number[]) => {
      try {
        this.terminal.write(new Uint8Array(data));
      } catch (e) {
        console.warn('xterm.js write error (non-fatal):', e);
      }
    };

    const exitChannel = new Channel<number>();
    exitChannel.onmessage = (ptyId: number) => {
      this.onExitCallback?.(ptyId);
    };

    const params: Record<string, unknown> = {
      cols: this.terminal.cols,
      rows: this.terminal.rows,
      onOutput: outputChannel,
      onExit: exitChannel,
    };
    if (cwd) params.cwd = cwd;
    if (appCommand) params.appCommand = appCommand;

    try {
      this.ptyId = await invoke<number>('create_pty', params);
    } catch (e) {
      console.error('Failed to create PTY:', e);
      this.terminal.write(`\r\nFailed to create PTY: ${e}\r\n`);
      return;
    }

    const encoder = new TextEncoder();

    this.terminal.onData((data: string) => {
      invoke('write_pty', {
        id: this.ptyId,
        data: Array.from(encoder.encode(data)),
      }).catch((e: unknown) => console.error('write_pty failed:', e));
    });

    this.terminal.onResize(({ cols, rows }: { cols: number; rows: number }) => {
      if (this.ptyId !== null) {
        invoke('resize_pty', { id: this.ptyId, cols, rows })
          .catch((e: unknown) => console.error('resize_pty failed:', e));
      }
    });

    this.terminal.focus();
  }

  focus(): void {
    this.terminal.focus();
  }

  setFontSize(size: number): void {
    this.terminal.options.fontSize = size;
    this.fitAddon.fit();
  }

  getFontSize(): number {
    return this.terminal.options.fontSize ?? 15;
  }

  getPtyId(): number | null {
    return this.ptyId;
  }

  get searchAddon(): SearchAddon {
    return this._searchAddon;
  }

  onExit(callback: (ptyId: number) => void): void {
    this.onExitCallback = callback;
  }

  onTitleChange(callback: (title: string) => void): void {
    this.terminal.onTitleChange(callback);
  }

  dispose(): void {
    if (this.resizeDebounceTimer !== null) {
      clearTimeout(this.resizeDebounceTimer);
      this.resizeDebounceTimer = null;
    }

    if (this.resizeObserver !== null) {
      this.resizeObserver.disconnect();
      this.resizeObserver = null;
    }

    if (this.ptyId !== null) {
      invoke('close_pty', { id: this.ptyId }).catch((e) => {
        console.warn('Failed to close PTY:', e);
      });
      this.ptyId = null;
    }

    if (this.webglAddon !== null) {
      this.webglAddon.dispose();
      this.webglAddon = null;
    }

    this.terminal.dispose();
  }
}
