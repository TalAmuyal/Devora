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
let toolBackground = false;
let errorAt: number | undefined;
let holdStream = false;
let sequence = 0;
let requests: unknown[] = [];

function interaction(tokens = inputTokens) {
  const dropped = new Set<number>();
  const events: string[] = [];
  const id = ++sequence;
  for (const wire of originalInteraction.response.sse_events as string[]) {
    const line = wire.split('\n').find((line: string) => line.startsWith('data: '));
    if (!line) continue;
    const message = JSON.parse(line.slice(6));
    if (message.type === 'content_block_start' && (message.content_block.type === 'tool_use' || (holdStream && message.content_block.type === 'thinking'))) {
      dropped.add(message.index);
    }
    if (dropped.has(message.index)) continue;
    if (holdStream && message.index !== undefined) message.index = 0;
    if (message.type === 'message_start') {
      message.message.id += '_' + id;
      if (tokens !== undefined) {
        message.message.usage.input_tokens = tokens;
        message.message.usage.cache_read_input_tokens = 0;
        message.message.usage.cache_creation_input_tokens = 0;
        message.message.usage.cache_creation = { ephemeral_1h_input_tokens: 0, ephemeral_5m_input_tokens: 0 };
      }
    }
    if (message.type === 'message_delta') {
      message.delta.stop_reason = 'end_turn';
      if (tokens !== undefined) {
        message.usage.input_tokens = tokens;
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

function toolUse(name: string, input: object, tokens = inputTokens) {
  const response = interaction(tokens);
  const startWire = response.response.sse_events.find((wire: string) => wire.startsWith('event: message_start\n'))!;
  const event = (type: string, fields: object) => 'event: ' + type + '\ndata: ' + JSON.stringify({ type, ...fields }) + '\n\n';
  const tool = { type: 'tool_use', id: 'toolu_devora_' + sequence, name, input: {} };
  return {
    ...response,
    response: { ...response.response, sse_events: [
      startWire,
      event('content_block_start', { index: 0, content_block: tool }),
      event('content_block_delta', { index: 0, delta: { type: 'input_json_delta', partial_json: JSON.stringify(input) } }),
      event('content_block_stop', { index: 0 }),
      event('message_delta', { delta: { stop_reason: 'tool_use', stop_sequence: null }, usage: { output_tokens: 20 } }),
      event('message_stop', {}),
    ] },
  };
}

function toolInteraction(command: string) {
  return toolUse('Bash', toolBackground ? { command, run_in_background: true, description: 'background' } : { command });
}

// A background subagent started by the main turn after the shell's, and what
// it runs; and a tool call answering the CLI's own task-notification turn after
// `delayMs`, as a model would after thinking. Requests for these are routed by
// their content, outside the main sequence.
let agentPrompt: string | undefined;
let subagentCommand: string | undefined;
// The subagent's own context, reported on its replies; and whether it runs in the background (the default) or
// within the turn that starts it.
let subagentInputTokens: number | undefined;
let agentBackground = true;
let notificationTool: { command: string; delayMs: number } | undefined;

function userText(message: any): string {
  return typeof message?.content === 'string' ? message.content : JSON.stringify(message?.content ?? '');
}

function reset() {
  requests = [];
  peer.interactionIndex = 0;
  const interactions = [
    ...(toolCommand === undefined ? [] : [toolInteraction(toolCommand)]),
    ...(agentPrompt === undefined ? [] : [interaction(), toolUse('Agent', {
      description: 'a CI wait', prompt: agentPrompt, subagent_type: 'general-purpose', run_in_background: agentBackground,
    })]),
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
const { Readable } = await import('node:stream');
const send = (response: any, events: string[]) => {
  response.writeHead(200, { 'content-type': 'text/event-stream' });
  for (const wire of events) response.write(wire);
  response.end();
};
const route = (request: any, response: any) => {
  const chunks: Buffer[] = [];
  request.on('data', (chunk: Buffer) => chunks.push(chunk));
  request.on('end', () => {
    const raw = Buffer.concat(chunks);
    const body = JSON.parse(raw.toString('utf8'));
    const users = body.messages.filter((message: any) => message.role === 'user');
    const first = userText(users[0]);
    const last = userText(users[users.length - 1]);
    if (agentPrompt !== undefined && subagentCommand !== undefined && first.includes(agentPrompt)) {
      send(response, last.includes('tool_result')
        ? interaction(subagentInputTokens ?? inputTokens).response.sse_events
        : toolUse('Bash', { command: subagentCommand, description: 'the CI wait' }, subagentInputTokens ?? inputTokens).response.sse_events);
      return;
    }
    if (notificationTool !== undefined && last.includes('<task-notification>')) {
      const tool = toolUse('Bash', { command: notificationTool.command, description: 'react' }).response.sse_events;
      setTimeout(() => { if (!response.destroyed) send(response, tool); }, notificationTool.delayMs);
      return;
    }
    const replay: any = Readable.from([raw]);
    replay.url = request.url;
    replay.method = request.method;
    replay.headers = request.headers;
    handle(replay, response);
  });
};
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
  if (holdStream && request.url?.startsWith('/v1/messages')) {
    // Send a partial text response in one batch, without the cassette driver's
    // inter-event pacing. The native stream stays open until the CLI is stopped.
    request.on('end', () => {
      response.writeHead(200, { 'content-type': 'text/event-stream' });
      const events = interaction().response.sse_events;
      for (const wire of events) {
        response.write(wire);
        if (wire.includes('"type":"text_delta"')) break;
      }
    });
    request.resume();
    return;
  }
  if (request.url?.startsWith('/v1/messages') && (agentPrompt !== undefined || notificationTool !== undefined)) {
    route(request, response);
    return;
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
    toolBackground = command.toolBackground ?? false;
    errorAt = command.errorAt;
    holdStream = command.holdStream ?? false;
    agentPrompt = command.agentPrompt;
    subagentCommand = command.subagentCommand;
    subagentInputTokens = command.subagentInputTokens;
    agentBackground = command.agentBackground ?? true;
    notificationTool = command.notificationTool;
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
