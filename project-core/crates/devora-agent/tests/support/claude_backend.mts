/** Offline model transport shared with Ember, using safe text-only cassette events. */
import * as readline from 'node:readline';
import { FakeClaudeServer } from '../../../../../project-ember/tests/support/fake-claude-server.ts';

if (process.env.RECORD_MODE === '1') throw new Error('Agent integration tests require offline replay');
const server = new FakeClaudeServer();
await server.loadCassette('Claude_Code_hook_receives_session_context');
const peer = server as any;
const originalInteraction = structuredClone(peer.cassette.interactions[0]);
let inputTokens: number | undefined;
let toolCommand: string | undefined;
let errorAt: number | undefined;
let sequence = 0;
let requests: unknown[] = [];

function interaction() {
  const dropped = new Set<number>();
  const events: string[] = [];
  const id = ++sequence;
  for (const wire of originalInteraction.response.sse_events as string[]) {
    const line = wire.split('\n').find((line: string) => line.startsWith('data: '));
    if (!line) continue;
    const message = JSON.parse(line.slice(6));
    if (message.type === 'content_block_start' && message.content_block.type === 'tool_use') {
      dropped.add(message.index);
    }
    if (dropped.has(message.index)) continue;
    if (message.type === 'message_start') {
      message.message.id += '_' + id;
      if (inputTokens !== undefined) {
        message.message.usage.input_tokens = inputTokens;
        message.message.usage.cache_read_input_tokens = 0;
        message.message.usage.cache_creation_input_tokens = 0;
        message.message.usage.cache_creation = { ephemeral_1h_input_tokens: 0, ephemeral_5m_input_tokens: 0 };
      }
    }
    if (message.type === 'message_delta') {
      message.delta.stop_reason = 'end_turn';
      if (inputTokens !== undefined) {
        message.usage.input_tokens = inputTokens;
        message.usage.cache_read_input_tokens = 0;
        message.usage.cache_creation_input_tokens = 0;
        message.usage.cache_creation = { ephemeral_1h_input_tokens: 0, ephemeral_5m_input_tokens: 0 };
      }
    }
    events.push('event: ' + message.type + '\ndata: ' + JSON.stringify(message) + '\n\n');
  }
  return {
    ...originalInteraction,
    request_summary: 'text-only projection of Ember cassette; tool blocks removed',
    response: { ...originalInteraction.response, sse_events: events },
  };
}

function toolInteraction(command: string) {
  const response = interaction();
  const startWire = response.response.sse_events.find((wire: string) => wire.startsWith('event: message_start\n'))!;
  const event = (type: string, fields: object) => 'event: ' + type + '\ndata: ' + JSON.stringify({ type, ...fields }) + '\n\n';
  const tool = { type: 'tool_use', id: 'toolu_devora_' + sequence, name: 'Bash', input: {} };
  return {
    ...response,
    response: { ...response.response, sse_events: [
      startWire,
      event('content_block_start', { index: 0, content_block: tool }),
      event('content_block_delta', { index: 0, delta: { type: 'input_json_delta', partial_json: JSON.stringify({ command }) } }),
      event('content_block_stop', { index: 0 }),
      event('message_delta', { delta: { stop_reason: 'tool_use', stop_sequence: null }, usage: { output_tokens: 20 } }),
      event('message_stop', {}),
    ] },
  };
}

function reset() {
  requests = [];
  peer.interactionIndex = 0;
  const interactions = [
    ...(toolCommand === undefined ? [] : [toolInteraction(toolCommand)]),
    ...Array.from({ length: 50 }, interaction),
  ];
  if (errorAt !== undefined) interactions.splice(errorAt, 0, {
    request_summary: 'synthetic prompt-too-long response',
    response: {
      status: 400,
      headers: { 'content-type': 'application/json' },
      sse_events: [JSON.stringify({ type: 'error', error: {
        type: 'invalid_request_error', message: 'prompt is too long: 1000000 tokens > 200000 maximum',
      } })],
    },
  });
  peer.cassette = { scenario: 'agent-offline', interactions };
}
reset();
const handle = peer.handleRequest.bind(server);
peer.handleRequest = (request: any, response: any) => {
  if (request.url?.startsWith('/v1/messages')) {
    const chunks: Buffer[] = [];
    request.on('data', (chunk: Buffer) => chunks.push(chunk));
    request.on('end', () => requests.push({
      body: JSON.parse(Buffer.concat(chunks).toString('utf8')),
      auth: request.headers.authorization?.startsWith('Bearer ') ? 'oauth' : 'api-key',
      callerKey: request.headers['x-api-key'] === 'offline-test-key',
      callerOAuth: request.headers.authorization === 'Bearer offline-oauth-token',
      settingsKey: request.headers['x-api-key'] === 'settings-api-key',
      settingsOAuth: request.headers.authorization === 'Bearer settings-oauth-token',
    }));
  }
  handle(request, response);
};
await server.start();
process.stdout.write(JSON.stringify({ type: 'ready', port: server.getPort() }) + '\n');
const lines = readline.createInterface({ input: process.stdin });
for await (const line of lines) {
  const command = JSON.parse(line);
  if (command.action === 'configure') {
    inputTokens = command.inputTokens;
    toolCommand = command.toolCommand;
    errorAt = command.errorAt;
    reset();
    process.stdout.write(JSON.stringify({ type: 'configured' }) + '\n');
  } else if (command.action === 'snapshot') {
    process.stdout.write(JSON.stringify({ type: 'snapshot', requests }) + '\n');
  } else if (command.action === 'stop') {
    await server.stop();
    break;
  } else {
    throw new Error('Unknown backend command');
  }
}

lines.close();
process.stdin.destroy();
