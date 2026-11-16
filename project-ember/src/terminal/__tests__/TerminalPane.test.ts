import { describe, it, expect, vi, afterEach } from 'vitest';
import type { Terminal } from '@xterm/xterm';
import { TerminalPane } from '../TerminalPane';

/**
 * The terminal Ember makes, with its real options (no scrollback), on the real xterm.js.
 * Narrowing it (a preview pane opening beside it) rewraps its lines; with the cursor on the bottom row, xterm 6.0.0 used to
 * leave that row with no line behind it, dropping everything written after (`scripts/patch-xterm.ts`).
 */

function write(terminal: Terminal, data: string): Promise<void> {
  return new Promise((resolve) => terminal.write(data, resolve));
}

function screenRows(terminal: Terminal): string[] {
  const buffer = terminal.buffer.active;
  return Array.from({ length: terminal.rows }, (_, row) => buffer.getLine(buffer.baseY + row)?.translateToString(true) ?? '(no line)');
}

function makeTerminal(): Terminal {
  // happy-dom has no WebGL, so the pane falls back to the DOM renderer and says so.
  const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
  const container = document.createElement('div');
  document.body.appendChild(container);
  new TerminalPane(container);
  expect(warn).toHaveBeenCalledWith(expect.stringContaining('WebGL addon failed to load'), expect.anything());
  warn.mockRestore();
  return (container as unknown as { __xtermTerminal: Terminal }).__xtermTerminal;
}

describe('TerminalPane', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('keeps showing output written after narrowing pushes the cursor to the bottom row', async () => {
    const terminal = makeTerminal();
    terminal.resize(72, 40);
    for (let i = 0; i < 8; i++) await write(terminal, `line ${i} ${'x'.repeat(50)} END\r\n`);

    terminal.resize(11, 40);
    await write(terminal, 'AFTER');

    const rows = screenRows(terminal);
    const after = rows.findIndex((row) => row.startsWith('AFTER'));
    expect(after).toBe(terminal.rows - 1);
    expect(rows[after - 1]).toMatch(/END$/);
  });
});
