export class AppDriver {
  private testHarnessPort: number;
  private ipcPort: number;

  constructor(testHarnessPort: number, ipcPort: number) {
    this.testHarnessPort = testHarnessPort;
    this.ipcPort = ipcPort;
  }

  async eval(js: string): Promise<any> {
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
