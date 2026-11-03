import { When, Then } from '@cucumber/cucumber';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { SapphireWorld } from '../support/world';

const debi = path.resolve(import.meta.dirname, '../../build/test-tools/debi');
const quote = (value: string) => "'" + value.replaceAll("'", "'\"'\"'") + "'";
function file(world: SapphireWorld, contents: string): string {
  const name = path.join(world.scenarioDir, `preview-${world.previewFiles.length}.md`);
  fs.writeFileSync(name, contents);
  world.previewFiles.push(name);
  return name;
}
async function preview(world: SapphireWorld, name: string, unknown = false, tab?: number): Promise<void> {
  await world.bridge.waitFor('a running terminal', () => world.bridge.state(), s => s.tabs.some(t => t.id === (tab ?? s.activeTab) && t.terminal === 'running'));
  const marker = `PREVIEW_SENT_${Date.now()}`;
  await world.bridge.typeLine(`${unknown ? 'DEVORA_PTY_ID=999999 ' : ''}${quote(debi)} preview ${quote(name)} && printf '\\n${marker}\\n'`, tab);
  await world.bridge.waitFor('debi preview to succeed', () => world.bridge.terminalText(tab), text => text.includes(marker + '\n') || text.includes(marker + '\r'));
}
async function active(world: SapphireWorld) {
  const state = await world.bridge.state();
  return state.tabs.find(t => t.id === state.activeTab)!;
}
When('a Markdown file is previewed in the active session', async function (this: SapphireWorld) { await preview(this, file(this, '# First Doc\n')); });
When('another Markdown file is previewed in the active session', async function (this: SapphireWorld) { await preview(this, file(this, '# Second Doc\n')); });
When('the same Markdown file is previewed again in the active session', async function (this: SapphireWorld) { await preview(this, this.previewFiles.at(-1)!); });
When('a Markdown file with a Mermaid diagram is previewed in the active session', async function (this: SapphireWorld) {
  await preview(this, file(this, '# Diagram\n\n```mermaid\nflowchart LR\n A[Client] --> B[Server]\n```\n'));
});
When('a Markdown file is previewed for an unknown session', async function (this: SapphireWorld) { await preview(this, file(this, '# Orphan\n'), true); });
When('the preview pane close button is clicked', async function (this: SapphireWorld) { await this.bridge.closePreview((await active(this)).previews[0].id); });
Then('the {word} session should have {int} preview pane(s)', async function (this: SapphireWorld, ordinal: string, count: number) {
  await this.bridge.waitFor(`${count} previews`, async () => { const state = await this.bridge.state(); return ordinal === 'first' ? state.tabs[0] : state.tabs.find(t => t.id === state.activeTab)!; }, tab => tab.previews.length === count);
});
Then('the preview pane should render the Markdown heading {string}', async function (this: SapphireWorld, heading: string) {
  await this.bridge.waitFor(`rendered heading ${heading}`, async () => this.bridge.preview((await active(this)).previews[0].id), value => value.heading === heading);
});
Then('the preview pane should render a Mermaid diagram containing {string}', async function (this: SapphireWorld, label: string) {
  await this.bridge.waitFor('rendered Mermaid SVG', async () => this.bridge.preview((await active(this)).previews[0].id), value => value.mermaid.includes(label));
});
Then('the Mermaid diagram nodes should use a themed fill color', async function (this: SapphireWorld) {
  const value = await this.bridge.preview((await active(this)).previews[0].id);
  assert.ok(value.fill && !['none', 'rgb(0, 0, 0)'].includes(value.fill), value.fill);
});

When('the preview file is changed to heading {string}', function (this: SapphireWorld, title: string) {
  fs.writeFileSync(this.previewFiles.at(-1)!, `# ${title}\n`);
});
When('the preview file is deleted', function (this: SapphireWorld) { fs.unlinkSync(this.previewFiles.at(-1)!); });
When('the last preview file is sent to the first session', async function (this: SapphireWorld) {
  const tab = (await this.bridge.state()).tabs[0];
  await preview(this, this.previewFiles.at(-1)!, false, tab.id);
});
Then('the preview pane should show a file error', async function (this: SapphireWorld) {
  await this.bridge.waitFor('an in-pane file error', () => active(this), tab => !!tab.previews[0]?.error);
});
