import assert from 'node:assert/strict';
import { Given, When, Then } from '@cucumber/cucumber';
import { SapphireWorld } from '../support/world';
import { closeWindowSurfaces } from './workspace-hub.steps';

When('the user opens the chat for {string}', async function (this: SapphireWorld, name: string) {
  await this.bridge.agentHub({ action: 'open', name });
  await this.bridge.waitFor('the chat renderer', () => this.bridge.agentChat(), s => s.phase === 'running');
});
When('the user drafts the agent prompt {string}', async function (this: SapphireWorld, text: string) {
  await this.bridge.chatAction('draft', text);
});
When('the user sends the agent prompt {string}', async function (this: SapphireWorld, text: string) {
  await this.bridge.chatAction('draft', text);
  await this.bridge.waitFor('the agent ready to send', () => this.bridge.agentChat(), s => s.canSend);
  await this.bridge.chatAction('send');
});
Then('the agent transcript contains {string}', async function (this: SapphireWorld, text: string) {
  await this.bridge.waitFor(`transcript containing ${text}`, () => this.bridge.agentChat(), s => s.dom.text.includes(text));
});
Then('the agent transcript has a {string} item', async function (this: SapphireWorld, kind: string) {
  await this.bridge.waitFor(`transcript kind ${kind}`, () => this.bridge.agentChat(), s => s.dom.rows.some(row => row.kind.includes(kind)));
});
Then('the agent transcript has no executable content', async function (this: SapphireWorld) {
  const { dom } = await this.bridge.agentChat();
  assert.equal(dom.scripts, 0);
  assert.equal(dom.unsafe, 0);
});
Then('the agent prompt draft is {string}', async function (this: SapphireWorld, draft: string) {
  await this.bridge.waitFor('the saved draft', () => this.bridge.agentChat(), s => s.draft === draft);
});
When('the user closes and reopens the agent chat {string}', async function (this: SapphireWorld, name: string) {
  const state = await this.bridge.state();
  await this.bridge.dispatch({ command: 'closeTab', id: state.activeTab! });
  await this.bridge.waitFor('agent tab closed', () => this.bridge.state(), s => !s.tabs.some(t => t.title === name));
  await this.bridge.press('s', ['control']);
  await this.bridge.waitFor('Agent Hub loaded', () => this.bridge.state(), s => s.agentHub?.loaded === true);
  await this.bridge.agentHub({ action: 'open', name });
  await this.bridge.waitFor('reopened chat', () => this.bridge.agentChat(), s => s.phase === 'running');
});
Then('the agent chat shows a permission card', async function (this: SapphireWorld) {
  await this.bridge.waitFor('permission card rendered', () => this.bridge.agentChat(), s => s.questions === 1 && s.dom.cards === 1);
});
Then('the agent chat has no permission card', async function (this: SapphireWorld) {
  await this.bridge.waitFor('permission card removed', () => this.bridge.agentChat(), s => s.questions === 0 && s.dom.cards === 0);
});
When('the user allows the agent permission with Command+Y', async function (this: SapphireWorld) {
  await this.bridge.press('y', ['command']);
});
When('the user requests Pause in the agent chat', async function (this: SapphireWorld) { await this.bridge.chatAction('pause'); });
When('the user resumes the agent chat', async function (this: SapphireWorld) { await this.bridge.chatAction('retry'); });
Then('the agent chat is paused', async function (this: SapphireWorld) {
  await this.bridge.waitFor('agent paused after its turn', () => this.bridge.agentChat(), s => s.phase === 'paused');
});

Then('the agent {string} should be pausing', async function (this: SapphireWorld, name: string) {
  await this.bridge.waitFor('pending pause status', () => this.bridge.state(), s => s.tabs.some(tab => tab.title === name && tab.agentStatus === 'pausing'));
});

const topRows = new WeakMap<SapphireWorld, {sequence: string; offset: number}>();

Given('the agent chat has {int} completed test turns', async function (this: SapphireWorld, turns: number) {
  for (let index = 0; index < turns; index++) {
    await this.bridge.chatAction('draft', `History turn ${index}`);
    await this.bridge.waitFor('ready for history turn', () => this.bridge.agentChat(), s => s.canSend);
    await this.bridge.chatAction('send');
    await this.bridge.waitFor('history turn complete', () => this.bridge.agentChat(), s => !s.running && s.draft === '');
  }
  const chat = await this.bridge.agentChat();
  assert.equal(chat.dom.rows.length, 200);
  assert.notEqual(chat.dom.rows[0].sequence, '1');
});
When('the user scrolls up for older agent history', async function (this: SapphireWorld) {
  await this.bridge.chatAction('scrollTop');
  const chat = await this.bridge.agentChat();
  topRows.set(this, chat.dom.rows.find(row => row.sequence === chat.dom.topSequence)!);
});
Then('earlier agent history loads without moving the visible row', async function (this: SapphireWorld) {
  const chat = await this.bridge.waitFor('first history page loaded', () => this.bridge.agentChat(), s => s.dom.rows[0]?.sequence === '1');
  const saved = topRows.get(this)!;
  const row = chat.dom.rows.find(row => row.sequence === saved.sequence)!;
  assert.ok(Math.abs(row.offset - saved.offset) <= 1, `anchor moved from ${saved.offset} to ${row.offset}`);
});
When('the user scrolls to the first agent history item', async function (this: SapphireWorld) {
  await this.bridge.chatAction('scrollTop');
  await this.bridge.waitFor('first row anchor persisted', () => this.bridge.agentChat(), s => s.anchor === '1');
});
Then('the first agent history item is restored at the top', async function (this: SapphireWorld) {
  await this.bridge.waitFor('restored transcript scroll anchor', () => this.bridge.agentChat(), s => s.dom.topSequence === '1' && s.dom.rows[0]?.sequence === '1');
});

Then('the agent tool has a Judgess badge', async function (this: SapphireWorld) {
  await this.bridge.waitFor('Judgess verdict on the tool', () => this.bridge.agentChat(), s => s.dom.verdict.includes('Judgess'));
});
Then('the agent thinking and tool details are collapsed', async function (this: SapphireWorld) {
  const { dom } = await this.bridge.agentChat();
  assert.equal(dom.thinkingOpen, false);
  assert.equal(dom.toolOpen, false);
});
When('the user expands the agent thinking and tool details', async function (this: SapphireWorld) {
  await this.bridge.chatAction('toggleThinking');
  await this.bridge.chatAction('toggleTool');
});
Then('the expanded agent details show their content', async function (this: SapphireWorld) {
  await this.bridge.waitFor('expanded typed details', () => this.bridge.agentChat(), s => s.dom.thinkingOpen && s.dom.toolOpen && s.dom.text.includes('A collapsed thought') && s.dom.text.includes('Visual tool result') && s.dom.text.includes('pwd'));
});

When('the user returns to the agent prompt', async function (this: SapphireWorld) {
  await closeWindowSurfaces(this);
});
When('the user sends the agent prompt with plain Enter', async function (this: SapphireWorld) {
  await this.bridge.waitFor('Enter-sends preference loaded', () => this.bridge.agentChat(), s => s.enterSends && s.canSend);
  await this.bridge.press('Return');
});
When('the user answers the agent permission with {string}', async function (this: SapphireWorld, shortcut: string) {
  await this.bridge.press(shortcut === 'Command+N' ? 'n' : 'y', shortcut === 'Command+N' ? ['command'] : ['command', 'shift']);
});
When('the user presses Escape and Control+C in the agent prompt', async function (this: SapphireWorld) {
  await this.bridge.press('Escape');
  await this.bridge.press('c', ['control']);
});
Then('the agent turn is still running', async function (this: SapphireWorld) {
  assert.equal((await this.bridge.agentChat()).running, true);
});
When('the user stops the agent turn', async function (this: SapphireWorld) { await this.bridge.chatAction('stop'); });

Then('the agent context bar has a known window and threshold {int}', async function (this: SapphireWorld, threshold: number) {
  await this.bridge.waitFor('typed context metrics', () => this.bridge.agentChat(), s => s.context !== null && BigInt(s.context.window) > 0n && BigInt(s.context.used) > 0n && s.context.threshold === threshold);
});
