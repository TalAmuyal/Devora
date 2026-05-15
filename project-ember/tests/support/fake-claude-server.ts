import * as http from 'node:http';
import * as https from 'node:https';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';
import * as zlib from 'node:zlib';

interface CassetteInteraction {
  request_summary: string;
  response: {
    status: number;
    headers: Record<string, string>;
    sse_events: string[];
  };
}

interface Cassette {
  scenario: string;
  recorded_at: string;
  interactions: CassetteInteraction[];
}

const THIS_DIR = path.dirname(fileURLToPath(import.meta.url));
const CASSETTES_DIR = path.join(THIS_DIR, 'fixtures', 'cassettes');
const SSE_EVENT_DELAY_MS = 50;
export const STRUCTURED_LOG_BASE_DIR = '/tmp/devora-ember-test';

function readRequestBody(req: http.IncomingMessage): Promise<string> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    req.on('data', (chunk: Buffer) => chunks.push(chunk));
    req.on('end', () => resolve(Buffer.concat(chunks).toString('utf-8')));
    req.on('error', reject);
  });
}

function createLogPath(): string {
  const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
  return `/tmp/devora-ember-fake-claude-${timestamp}.log`;
}

function createScenarioLogPath(scenarioName: string): string {
  const scenarioDir = path.join(STRUCTURED_LOG_BASE_DIR, scenarioName);
  fs.mkdirSync(scenarioDir, { recursive: true });
  const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
  return path.join(scenarioDir, `${timestamp}.json`);
}

export class FakeClaudeServer {
  private server: http.Server | null = null;
  private port: number | null = null;
  private cassette: Cassette | null = null;
  private interactionIndex: number = 0;
  private recordedInteractions: CassetteInteraction[] = [];
  private currentScenarioName: string | null = null;
  private logStream: fs.WriteStream;
  private scenarioLogStream: fs.WriteStream | null = null;
  private recordMode: boolean;
  private pendingProxies: Set<Promise<void>> = new Set();

  constructor() {
    this.logStream = fs.createWriteStream(createLogPath(), { flags: 'a' });
    this.recordMode = process.env.RECORD_MODE === '1';
  }

  private log(message: string): void {
    const line = `[${new Date().toISOString()}] ${message}\n`;
    this.logStream.write(line);
  }

  private logStructured(entry: object): void {
    const line = JSON.stringify(entry) + '\n';
    this.logStream.write(line);
    if (this.scenarioLogStream) {
      this.scenarioLogStream.write(line);
    }
  }

  async start(): Promise<void> {
    const server = http.createServer((req, res) => this.handleRequest(req, res));
    this.server = server;

    await new Promise<void>((resolve) => {
      server.listen(0, '127.0.0.1', () => {
        const addr = server.address();
        if (addr && typeof addr === 'object') {
          this.port = addr.port;
        }
        this.log(`Server started on port ${this.port} (record_mode=${this.recordMode})`);
        resolve();
      });
    });
  }

  getPort(): number {
    if (this.port === null) {
      throw new Error('FakeClaudeServer has not been started');
    }
    return this.port;
  }

  async loadCassette(name: string): Promise<void> {
    this.currentScenarioName = name;
    this.interactionIndex = 0;
    this.recordedInteractions = [];

    if (this.scenarioLogStream) {
      this.scenarioLogStream.end();
    }
    this.scenarioLogStream = fs.createWriteStream(createScenarioLogPath(name), { flags: 'a' });

    if (this.recordMode) {
      this.log(`Recording mode: scenario "${name}"`);
      this.cassette = null;
      return;
    }

    const filePath = path.join(CASSETTES_DIR, `${name}.json.gz`);
    this.log(`Loading cassette: ${filePath}`);

    const compressed = fs.readFileSync(filePath);
    const json = zlib.gunzipSync(compressed).toString('utf-8');
    this.cassette = JSON.parse(json) as Cassette;
    this.log(`Loaded cassette with ${this.cassette.interactions.length} interactions`);
  }

  reset(): void {
    this.interactionIndex = 0;
    this.cassette = null;
    this.recordedInteractions = [];
    this.currentScenarioName = null;
    if (this.scenarioLogStream) {
      this.scenarioLogStream.end();
      this.scenarioLogStream = null;
    }
    this.log('State reset');
  }

  async saveCassette(): Promise<void> {
    if (this.pendingProxies.size > 0) {
      this.log(`Waiting for ${this.pendingProxies.size} pending proxy response(s)...`);
      await Promise.all(this.pendingProxies);
    }

    if (!this.recordMode || this.recordedInteractions.length === 0 || !this.currentScenarioName) {
      return;
    }

    const cassette: Cassette = {
      scenario: this.currentScenarioName,
      recorded_at: new Date().toISOString(),
      interactions: this.recordedInteractions,
    };

    fs.mkdirSync(CASSETTES_DIR, { recursive: true });
    const filePath = path.join(CASSETTES_DIR, `${this.currentScenarioName}.json.gz`);
    const json = JSON.stringify(cassette, null, 2);
    const compressed = zlib.gzipSync(Buffer.from(json, 'utf-8'));
    fs.writeFileSync(filePath, compressed);
    this.log(`Saved cassette to ${filePath} (${this.recordedInteractions.length} interactions)`);
  }

  async stop(): Promise<void> {
    if (!this.server) return;

    await new Promise<void>((resolve, reject) => {
      this.server!.close((err) => {
        if (err) reject(err);
        else resolve();
      });
    });

    this.log('Server stopped');
    this.logStream.end();
    if (this.scenarioLogStream) {
      this.scenarioLogStream.end();
      this.scenarioLogStream = null;
    }
    this.server = null;
    this.port = null;
  }

  private handleRequest(req: http.IncomingMessage, res: http.ServerResponse): void {
    const rawUrl = req.url ?? '';
    const method = req.method ?? '';
    const pathname = rawUrl.split('?')[0];
    this.log(`${method} ${rawUrl}`);

    if (method === 'HEAD') {
      res.writeHead(200);
      res.end();
    } else if (method === 'POST' && pathname === '/test/scenario') {
      this.handleTestScenario(req, res);
    } else if (method === 'POST' && pathname === '/test/reset') {
      this.handleTestReset(res);
    } else if (method === 'POST' && pathname === '/v1/messages') {
      this.handleMessages(req, res).catch((err) => {
        this.log(`Error in handleMessages: ${err.message}`);
        if (!res.headersSent) {
          res.writeHead(500, { 'Content-Type': 'application/json' });
          res.end(JSON.stringify({ error: err.message }));
        }
      });
    } else {
      res.writeHead(404, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ error: `Unknown route: ${method} ${rawUrl}` }));
    }
  }

  private async handleTestScenario(req: http.IncomingMessage, res: http.ServerResponse): Promise<void> {
    try {
      const body = await readRequestBody(req);
      const { name } = JSON.parse(body) as { name: string };
      await this.loadCassette(name);
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ ok: true }));
    } catch (err: any) {
      this.log(`Error loading cassette: ${err.message}`);
      res.writeHead(500, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ error: err.message }));
    }
  }

  private handleTestReset(res: http.ServerResponse): void {
    this.reset();
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ ok: true }));
  }

  private async handleMessages(req: http.IncomingMessage, res: http.ServerResponse): Promise<void> {
    if (this.recordMode) {
      const proxyDone = this.handleRecordProxy(req, res);
      this.pendingProxies.add(proxyDone);
      proxyDone.finally(() => this.pendingProxies.delete(proxyDone));
      return;
    }

    const body = await readRequestBody(req);
    let parsedBody: { model?: string; messages?: Array<{ role?: string; content?: string }> } = {};
    try {
      parsedBody = JSON.parse(body);
    } catch {
      // body not parseable, continue with empty parsed body
    }

    const firstUserMessage = parsedBody.messages?.find((m) => m.role === 'user');
    const firstUserContent = typeof firstUserMessage?.content === 'string'
      ? firstUserMessage.content.slice(0, 200)
      : undefined;

    if (!this.cassette) {
      this.log('No cassette loaded');
      res.writeHead(500, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ error: 'No cassette loaded. Call POST /test/scenario first.' }));
      return;
    }

    const cassetteExhausted = this.interactionIndex >= this.cassette.interactions.length;

    this.logStructured({
      type: 'request',
      timestamp: new Date().toISOString(),
      interactionIndex: this.interactionIndex,
      cassette: {
        totalInteractions: this.cassette.interactions.length,
        exhausted: cassetteExhausted,
      },
      request: {
        model: parsedBody.model ?? null,
        messageCount: parsedBody.messages?.length ?? 0,
        firstUserMessage: firstUserContent ?? null,
      },
    });

    if (cassetteExhausted) {
      this.log(`Cassette exhausted at request #${this.interactionIndex + 1} — returning stop response`);
      res.writeHead(200, {
        'Content-Type': 'text/event-stream',
        'Cache-Control': 'no-cache',
        'Connection': 'keep-alive',
      });
      const stopId = `msg_stop_${Date.now()}`;
      res.write(`event: message_start\ndata: {"type":"message_start","message":{"id":"${stopId}","type":"message","role":"assistant","content":[],"model":"fake-stop","stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":1,"output_tokens":1}}}\n\n`);
      res.write(`event: content_block_start\ndata: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}\n\n`);
      res.write(`event: content_block_stop\ndata: {"type":"content_block_stop","index":0}\n\n`);
      res.write(`event: message_delta\ndata: {"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":1}}\n\n`);
      res.write(`event: message_stop\ndata: {"type":"message_stop"}\n\n`);
      res.end();

      this.logStructured({
        type: 'response',
        timestamp: new Date().toISOString(),
        interactionIndex: this.interactionIndex,
        sseEventCount: 5,
        fromCassette: false,
      });
      return;
    }

    const interaction = this.cassette.interactions[this.interactionIndex];
    this.interactionIndex++;
    this.log(`Replaying interaction ${this.interactionIndex}/${this.cassette.interactions.length}: ${interaction.request_summary}`);

    const headers: Record<string, string> = {
      'Content-Type': 'text/event-stream',
      'Cache-Control': 'no-cache',
      'Connection': 'keep-alive',
      ...interaction.response.headers,
    };
    res.writeHead(interaction.response.status, headers);

    this.streamSseEvents(res, interaction.response.sse_events);

    this.logStructured({
      type: 'response',
      timestamp: new Date().toISOString(),
      interactionIndex: this.interactionIndex - 1,
      sseEventCount: interaction.response.sse_events.length,
      fromCassette: true,
    });
  }

  private streamSseEvents(res: http.ServerResponse, events: string[]): void {
    let i = 0;

    const writeNext = (): void => {
      if (i >= events.length) {
        res.end();
        return;
      }
      res.write(events[i]);
      i++;
      if (SSE_EVENT_DELAY_MS > 0) {
        setTimeout(writeNext, SSE_EVENT_DELAY_MS);
      } else {
        setImmediate(writeNext);
      }
    };

    writeNext();
  }

  private async handleRecordProxy(req: http.IncomingMessage, res: http.ServerResponse): Promise<void> {
    const requestBody = await readRequestBody(req);
    this.log(`Proxying request to real Anthropic API: ${req.url}`);

    let requestSummary: string;
    try {
      const parsed = JSON.parse(requestBody);
      requestSummary = `${parsed.model ?? 'unknown-model'} (${(parsed.messages?.length ?? 0)} messages)`;
    } catch {
      requestSummary = 'unparseable request';
    }

    const incomingAuth = req.headers['authorization'];
    const apiKey = process.env.ANTHROPIC_API_KEY;

    const headers: Record<string, string> = {
      'content-type': req.headers['content-type'] ?? 'application/json',
    };
    for (const name of ['anthropic-version', 'anthropic-beta', 'anthropic-dangerous-direct-browser-access']) {
      if (req.headers[name]) headers[name] = req.headers[name] as string;
    }

    if (incomingAuth) {
      headers['authorization'] = incomingAuth as string;
    } else if (apiKey) {
      headers['x-api-key'] = apiKey;
    } else {
      const msg = 'No auth available — log in to Claude Code via OAuth or set ANTHROPIC_API_KEY';
      this.log(msg);
      res.writeHead(500, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ error: msg }));
      return;
    }

    await new Promise<void>((resolveProxy, rejectProxy) => {
    const proxyReq = https.request(
      {
        hostname: 'api.anthropic.com',
        port: 443,
        path: req.url ?? '/v1/messages',
        method: 'POST',
        headers,
      },
      (proxyRes) => {
        const status = proxyRes.statusCode ?? 500;
        const responseHeaders: Record<string, string> = {};
        for (const [key, value] of Object.entries(proxyRes.headers)) {
          if (value) responseHeaders[key] = Array.isArray(value) ? value.join(', ') : value;
        }

        res.writeHead(status, proxyRes.headers);

        const chunks: Buffer[] = [];
        proxyRes.on('data', (chunk: Buffer) => {
          chunks.push(chunk);
          res.write(chunk);
        });

        proxyRes.on('end', () => {
          res.end();

          const rawSseBody = Buffer.concat(chunks).toString('utf-8');
          const sseEvents = parseSseEvents(rawSseBody);
          this.recordedInteractions.push({
            request_summary: requestSummary,
            response: {
              status,
              headers: responseHeaders,
              sse_events: sseEvents,
            },
          });
          this.log(`Recorded interaction: ${requestSummary} (${sseEvents.length} SSE events)`);
          resolveProxy();
        });
      },
    );

    proxyReq.on('error', (err) => {
      this.log(`Proxy error: ${err.message}`);
      res.writeHead(502, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ error: `Proxy error: ${err.message}` }));
      rejectProxy(err);
    });

    proxyReq.write(requestBody);
    proxyReq.end();
    });
  }
}

function parseSseEvents(raw: string): string[] {
  const normalized = raw.replace(/\r\n/g, '\n').replace(/\r/g, '\n');
  const events: string[] = [];
  let current = '';

  for (const line of normalized.split('\n')) {
    current += line + '\n';
    if (line === '') {
      if (current.trim().length > 0) {
        events.push(current);
      }
      current = '';
    }
  }

  if (current.trim().length > 0) {
    events.push(current);
  }

  return events;
}
