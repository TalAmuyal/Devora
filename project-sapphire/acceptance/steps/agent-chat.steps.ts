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
When("the user {word} the agent's system messages", async function (this: SapphireWorld, action: string) {
  assert.ok(action === 'shows' || action === 'hides', `"shows" or "hides", not "${action}"`);
  await this.bridge.showSystemMessages(action === 'shows');
  await this.bridge.waitFor(`system messages ${action === 'shows' ? 'shown' : 'hidden'}`, () => this.bridge.agentChat(), s => s.showsSystem === (action === 'shows'));
});
Then('the agent transcript does not contain {string}', async function (this: SapphireWorld, text: string) {
  const chat = await this.bridge.agentChat();
  assert.ok(!chat.dom.text.includes(text), `the transcript shows "${text}"`);
});
Then('the agent transcript contains {string}', async function (this: SapphireWorld, text: string) {
  await this.bridge.waitFor(`transcript containing ${text}`, () => this.bridge.agentChat(), s => s.dom.text.includes(text));
});
// The restart notice's row, whose text a resent one extends with "(resent)".
const restartNotice = 'Devora restarted: asked the agent to re-check its waits';
Then('the agent has been told once that Devora restarted', async function (this: SapphireWorld) {
  const chat = await this.bridge.waitFor('the restart notice, answered', () => this.bridge.agentChat(),
    s => s.dom.text.includes(restartNotice) && !s.running);
  assert.equal(chat.dom.text.split(restartNotice).length - 1, 1, 'exactly one restart notice');
});
Then('the agent has not been told that Devora restarted', async function (this: SapphireWorld) {
  const chat = await this.bridge.agentChat();
  assert.ok(!chat.dom.text.includes(restartNotice), 'no restart notice yet');
});
Then('the agent transcript has a {string} item', async function (this: SapphireWorld, kind: string) {
  await this.bridge.waitFor(`transcript kind ${kind}`, () => this.bridge.agentChat(), s => s.dom.rows.some(row => row.kind.includes(kind)));
});
// The native transcript runs nothing: markup an agent or a user writes is drawn as its text.
Then('the agent transcript shows {string} as text', async function (this: SapphireWorld, markup: string) {
  await this.bridge.waitFor(`transcript showing ${markup}`, () => this.bridge.agentChat(), s => s.dom.text.includes(markup));
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
Then("the agent chat's turn status is {string}", async function (this: SapphireWorld, step: string) {
  await this.bridge.waitFor(`the turn status "${step}"`, () => this.bridge.agentChat(), s => s.turnStatus?.step === step);
});
Then('the agent chat shows no turn status', async function (this: SapphireWorld) {
  await this.bridge.waitFor('no turn status', () => this.bridge.agentChat(), s => s.turnStatus === null);
});
// Today's rows show only the time, so none names the year.
Then("every agent transcript row shows today's time", async function (this: SapphireWorld) {
  const year = String(new Date().getFullYear());
  await this.bridge.waitFor("every row showing today's time", () => this.bridge.agentChat(),
    s => s.dom.rows.length > 0 && s.dom.rows.every(row => row.time !== null && row.time !== '' && !row.time.includes(year)));
});
Then('the agent transcript has no {string} row', async function (this: SapphireWorld, text: string) {
  const chat = await this.bridge.agentChat();
  assert.ok(!chat.dom.rows.some(row => row.text === text), `a "${text}" row is shown: ${JSON.stringify(chat.dom.rows.map(row => row.text))}`);
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

// Ninety turns, one after another, take about 23 s under the stress runs' load: past the default 30 s budget now and
// then, which is a hang guard, not a measure of this step. Its own budget is the harness's longest.
Given('the agent chat has {int} completed test turns', { timeout: 120_000 }, async function (this: SapphireWorld, turns: number) {
  for (let index = 0; index < turns; index++) {
    await this.bridge.chatAction('draft', `History turn ${index}`);
    await this.bridge.waitFor('ready for history turn', () => this.bridge.agentChat(), s => s.canSend);
    await this.bridge.chatAction('send');
    await this.bridge.waitFor('history turn complete', () => this.bridge.agentChat(), s => !s.running && s.draft === '');
  }
  // The chat keeps its 200-item tail; the rows are fewer, since a turn's "Turn completed" isn't shown.
  const chat = await this.bridge.agentChat();
  assert.equal(chat.items, 200);
  assert.notEqual(chat.dom.rows[0].sequence, '1');
});
When('the user scrolls up for older agent history', async function (this: SapphireWorld) {
  await this.bridge.chatAction('scrollTop');
  const chat = await this.bridge.agentChat();
  topRows.set(this, chat.dom.rows.find(row => row.sequence === chat.dom.topSequence)!);
});
// The history's first shown row: its first prompt, since the session's notes before it are system messages, hidden.
const firstPrompt = 'History turn 0';
Then('earlier agent history loads without moving the visible row', async function (this: SapphireWorld) {
  const chat = await this.bridge.waitFor('first history page loaded', () => this.bridge.agentChat(), s => s.dom.rows[0]?.text === firstPrompt);
  const saved = topRows.get(this)!;
  const row = chat.dom.rows.find(row => row.sequence === saved.sequence)!;
  assert.ok(Math.abs(row.offset - saved.offset) <= 1, `anchor moved from ${saved.offset} to ${row.offset}`);
});
const firstRows = new WeakMap<SapphireWorld, string>();
When('the user scrolls to the first agent history item', async function (this: SapphireWorld) {
  await this.bridge.chatAction('scrollTop');
  const first = (await this.bridge.agentChat()).dom.rows.find(row => row.text === firstPrompt)!.sequence;
  firstRows.set(this, first);
  await this.bridge.waitFor('first row anchor persisted', () => this.bridge.agentChat(), s => s.anchor === first);
});
Then('the first agent history item is restored at the top', async function (this: SapphireWorld) {
  const first = firstRows.get(this)!;
  await this.bridge.waitFor('restored transcript scroll anchor', () => this.bridge.agentChat(), s => s.dom.topSequence === first && s.dom.rows[0]?.sequence === first);
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
When('the user sets the agent compaction threshold to {int}', async function (this: SapphireWorld, percent: number) {
  await this.bridge.setCompactThreshold(percent);
});
When('the user resets the agent compaction threshold', async function (this: SapphireWorld) {
  await this.bridge.setCompactThreshold(null);
});
When('the user compacts the agent now', async function (this: SapphireWorld) {
  await this.bridge.chatAction('compactNow');
});
Then('the agent compacts at {int} for this agent, over the profile threshold {int}', async function (this: SapphireWorld, threshold: number, profile: number) {
  const chat = await this.bridge.waitFor('the agent threshold', () => this.bridge.agentChat(), s => s.context?.threshold === threshold);
  assert.equal(chat.context?.defaultThreshold, profile);
  assert.equal(chat.contextPopover.thresholdNote, `This agent only · profile default is ${profile}%`);
});
Then('the agent compaction has finished', async function (this: SapphireWorld) {
  await this.bridge.waitFor('the compaction to finish', () => this.bridge.agentChat(), s => !s.running && s.context?.compaction === 'idle' && s.dom.text.includes('Devora compacts the conversation.'));
});
Then('the context popover refuses Compact Now with {string}', async function (this: SapphireWorld, note: string) {
  const chat = await this.bridge.agentChat();
  assert.equal(chat.contextPopover.compactEnabled, false);
  assert.equal(chat.contextPopover.thresholdEnabled, false);
  assert.equal(chat.contextPopover.thresholdNote, note);
});
Then('the agent transcript draws its Mermaid diagram', async function (this: SapphireWorld) {
  await this.bridge.waitFor('Mermaid diagram drawn', () => this.bridge.agentChat(), s => s.dom.diagrams.drawn === 1 && s.dom.diagrams.failed === 0);
});
Then('the agent transcript shows the Mermaid source {string} with a warning', async function (this: SapphireWorld, source: string) {
  await this.bridge.waitFor('Mermaid diagram failed', () => this.bridge.agentChat(), s => s.dom.diagrams.failed === 1 && s.dom.text.includes(source));
});
Then('the agent model widget shows {string}, as the model it is set to', async function (this: SapphireWorld, label: string) {
  await this.bridge.waitFor(`the model widget showing ${label}`, () => this.bridge.agentChat(),
    s => s.modelWidget.label === label && !s.modelWidget.mismatch && !s.modelWidget.grayed);
});
Then('the agent model widget shows {string}, not the model it is set to', async function (this: SapphireWorld, label: string) {
  const chat = await this.bridge.waitFor(`the model widget showing ${label} as a mismatch`, () => this.bridge.agentChat(),
    s => s.modelWidget.label === label && s.modelWidget.mismatch);
  assert.match(chat.modelWidget.glyphTooltip ?? '', /set to opus/);
});
Then('the agent transcript says once {string}', async function (this: SapphireWorld, text: string) {
  const chat = await this.bridge.waitFor(`transcript containing ${text}`, () => this.bridge.agentChat(),
    s => s.dom.text.includes(text) && !s.running);
  assert.equal(chat.dom.text.split(text).length - 1, 1, `exactly one "${text}"`);
});
