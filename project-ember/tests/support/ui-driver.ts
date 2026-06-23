import { AppDriver } from './app-driver';

export class UIDriver {
  private driver: AppDriver;

  constructor(driver: AppDriver) {
    this.driver = driver;
  }

  async pressKey(
    key: string,
    options?: { ctrlKey?: boolean; shiftKey?: boolean; code?: string },
  ): Promise<void> {
    const code = options?.code ?? deriveCode(key);
    const ctrlKey = options?.ctrlKey ?? false;
    const shiftKey = options?.shiftKey ?? false;

    await this.driver.eval(`
      // Blur any focused element inside the Workspace Hub, Command Palette, or Settings Hub so their isEditableElementFocused() guard does not swallow navigation keys — any focused input (search or a form field) makes handleKeyDown return early.
      // In the Tauri WKWebView an input can receive focus after render().
      const panel = document.querySelector('.ws-hub, .command-palette, .settings-hub');
      if (panel && panel.contains(document.activeElement)) {
        document.activeElement.blur();
      }

      window.dispatchEvent(new KeyboardEvent('keydown', {
        key: ${JSON.stringify(key)},
        code: ${JSON.stringify(code)},
        ctrlKey: ${ctrlKey},
        shiftKey: ${shiftKey},
        bubbles: true,
        cancelable: true
      }));
    `);
  }

  /*
   * Dispatch a key on window WITHOUT blurring the focused element first (unlike pressKey).
   * This faithfully exercises the real focus state, so a key handler that depends on what currently holds focus (e.g. global q/Escape, which bails when an editable element is focused) is genuinely tested.
   */
  async pressKeyRaw(key: string, code?: string): Promise<void> {
    const resolvedCode = code ?? deriveCode(key);
    await this.driver.eval(`
      window.dispatchEvent(new KeyboardEvent('keydown', {
        key: ${JSON.stringify(key)},
        code: ${JSON.stringify(resolvedCode)},
        bubbles: true,
        cancelable: true,
      }));
    `);
  }

  async pressKeyMultiple(key: string, count: number, delayMs = 50): Promise<void> {
    for (let i = 0; i < count; i++) {
      await this.pressKey(key);
      if (i < count - 1) {
        await new Promise((r) => setTimeout(r, delayMs));
      }
    }
  }

  async click(selector: string): Promise<void> {
    await this.driver.eval(`
      const el = document.querySelector(${JSON.stringify(selector)});
      if (!el) throw new Error('Element not found: ' + ${JSON.stringify(selector)});
      el.click();
    `);
  }

  async typeIntoInput(selector: string, text: string): Promise<void> {
    await this.driver.eval(`
      const input = document.querySelector(${JSON.stringify(selector)});
      if (!input) throw new Error('Input not found: ' + ${JSON.stringify(selector)});
      input.focus();
      input.value = ${JSON.stringify(text)};
      input.dispatchEvent(new Event('input', { bubbles: true }));
    `);
  }

  async getTextContent(selector: string): Promise<string> {
    return await this.driver.eval(`
      const el = document.querySelector(${JSON.stringify(selector)});
      return (el?.textContent ?? '').trim();
    `);
  }

  async getElementCount(selector: string): Promise<number> {
    return await this.driver.eval(
      `return document.querySelectorAll(${JSON.stringify(selector)}).length`,
    );
  }

  async hasElement(selector: string): Promise<boolean> {
    return await this.driver.eval(
      `return document.querySelector(${JSON.stringify(selector)}) !== null`,
    );
  }

  async waitForElement(selector: string, timeoutMs = 5_000): Promise<void> {
    await this.driver.pollFor(
      `return document.querySelector(${JSON.stringify(selector)}) !== null`,
      true,
      timeoutMs,
    );
  }

  async waitForElementCount(
    selector: string,
    expected: number,
    timeoutMs = 5_000,
  ): Promise<void> {
    await this.driver.pollFor(
      `return document.querySelectorAll(${JSON.stringify(selector)}).length`,
      expected,
      timeoutMs,
    );
  }

  async getAttribute(selector: string, attr: string): Promise<string | null> {
    return await this.driver.eval(
      `return document.querySelector(${JSON.stringify(selector)})?.getAttribute(${JSON.stringify(attr)}) ?? null`,
    );
  }

  // Focus the element, then dispatch a keydown on window (mirroring how the
  // browser routes a real keypress on a focused element to global handlers).
  // Returns evt.defaultPrevented so callers can assert whether a handler
  // intercepted the key. Unlike pressKey, focus is NOT blurred first.
  async dispatchKeyToFocused(selector: string, key: string): Promise<boolean> {
    const code = deriveCode(key);
    return await this.driver.eval(`
      const el = document.querySelector(${JSON.stringify(selector)});
      if (!el) throw new Error('Element not found: ' + ${JSON.stringify(selector)});
      el.focus();
      const evt = new KeyboardEvent('keydown', {
        key: ${JSON.stringify(key)},
        code: ${JSON.stringify(code)},
        bubbles: true,
        cancelable: true,
      });
      window.dispatchEvent(evt);
      return evt.defaultPrevented;
    `);
  }
}

function deriveCode(key: string): string {
  if (key.length === 1) {
    const lower = key.toLowerCase();
    if (lower >= 'a' && lower <= 'z') {
      return `Key${lower.toUpperCase()}`;
    }
    if (lower >= '0' && lower <= '9') {
      return `Digit${lower}`;
    }
    const symbolCodes: Record<string, string> = {
      '?': 'Slash',
      '/': 'Slash',
      '\\': 'Backslash',
      '[': 'BracketLeft',
      ']': 'BracketRight',
      ';': 'Semicolon',
      "'": 'Quote',
      ',': 'Comma',
      '.': 'Period',
      '-': 'Minus',
      '=': 'Equal',
      '`': 'Backquote',
    };
    return symbolCodes[key] ?? key;
  }
  return key;
}
