const NEED_TIMEOUT_MS = 5_000;
const NOT_READY = '__NOT_READY__:';
const NEED_PRELUDE = `const need = (value, what) => {
  if (value === null || value === undefined) throw new Error(${JSON.stringify(NOT_READY)} + what);
  return value;
};
`;

export class AppDriver {
  private testHarnessPort: number;
  private ipcPort: number;

  constructor(testHarnessPort: number, ipcPort: number) {
    this.testHarnessPort = testHarnessPort;
    this.ipcPort = ipcPort;
  }

  /**
   * Run `js` as an async function body in the app's webview and return its result.
   *
   * The script can call `need(value, what)` to require something that may not be rendered yet: it returns `value` when present, otherwise the whole script is re-run (every 100ms) until it is, or until `timeoutMs` passes.
   * Call `need` before any side effect, since a re-run repeats everything before it.
   */
  async eval(js: string, timeoutMs = NEED_TIMEOUT_MS): Promise<any> {
    const start = Date.now();
    for (;;) {
      try {
        return await this.evalOnce(NEED_PRELUDE + js);
      } catch (error) {
        const missing = String(error).split(NOT_READY)[1];
        if (missing === undefined) throw error;
        if (Date.now() - start >= timeoutMs) {
          throw new Error(`Timed out after ${timeoutMs}ms waiting for ${missing}`);
        }
        await new Promise((r) => setTimeout(r, 100));
      }
    }
  }

  private async evalOnce(js: string): Promise<any> {
    const id = crypto.randomUUID();
    const response = await fetch(`http://127.0.0.1:${this.testHarnessPort}/test/eval`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ id, js }),
    });
    if (!response.ok) {
      throw new Error(`eval request failed: ${response.status} ${response.statusText}`);
    }
    const body = await response.json();
    if (body.error) {
      throw new Error(body.error);
    }
    return body.result;
  }

  async pollFor(js: string, expected: any, timeoutMs: number): Promise<any> {
    const expectedJson = JSON.stringify(expected);
    const start = Date.now();
    while (Date.now() - start < timeoutMs) {
      const result = await this.eval(js);
      if (JSON.stringify(result) === expectedJson) return result;
      await new Promise((r) => setTimeout(r, 100));
    }
    throw new Error(
      `pollFor timed out after ${timeoutMs}ms: expected ${expectedJson}`,
    );
  }

  async ipcPost(path: string, body: any): Promise<any> {
    const response = await fetch(`http://127.0.0.1:${this.ipcPort}${path}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
    if (!response.ok) {
      throw new Error(`ipcPost ${path} failed: ${response.status} ${response.statusText}`);
    }
    return response.json();
  }

  ipcPostFireAndForget(path: string, body: any): void {
    fetch(`http://127.0.0.1:${this.ipcPort}${path}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    }).catch(() => {});
  }
}
