import { AppDriver } from './app-driver';

export const KEY_CR = [0x0d] as const;
export const KEY_ARROW_UP = [0x1b, 0x5b, 0x41] as const;

export async function getTerminalContent(driver: AppDriver): Promise<string> {
  return driver.eval(`
    const session = window.__test.sessionManager.getActiveSession();
    if (!session) return '';
    const term = session.containerEl?.__xtermTerminal;
    if (!term) return '';
    const lines = [];
    for (let i = 0; i < term.buffer.active.length; i++) {
      lines.push(term.buffer.active.getLine(i)?.translateToString(true) ?? '');
    }
    return lines.join('\\n');
  `);
}

export async function writeBytesToTerminal(driver: AppDriver, bytes: readonly number[]): Promise<void> {
  await driver.eval(`
    const session = window.__test.sessionManager.getActiveSession();
    const ptyId = session.getPtyId();
    await window.__TAURI_INTERNALS__.invoke('write_pty', { id: ptyId, data: ${JSON.stringify([...bytes])} });
  `);
}

export async function writeToTerminal(driver: AppDriver, text: string): Promise<void> {
  const bytes = Array.from(new TextEncoder().encode(text + '\n'));
  await writeBytesToTerminal(driver, bytes);
}

export async function waitForTerminalContent(
  driver: AppDriver,
  pattern: string,
  timeoutMs: number,
): Promise<string> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const content = await getTerminalContent(driver);
    if (content.includes(pattern)) return content;
    await new Promise((r) => setTimeout(r, 50));
  }
  throw new Error(
    `Terminal content did not contain "${pattern}" within ${timeoutMs}ms`,
  );
}
