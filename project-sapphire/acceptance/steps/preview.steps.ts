import assert from 'node:assert/strict';
import { After, When, Then } from '@cucumber/cucumber';
import { createServer, type Server } from 'node:http';
import { pathToFileURL } from 'node:url';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { SapphireWorld } from '../support/world';

const debi = path.resolve(import.meta.dirname, '../../build/test-tools/debi');
const quote = (value: string) => "'" + value.replaceAll("'", "'\"'\"'") + "'";
function file(world: SapphireWorld, contents: string, extension = ".md"): string {
  const name = path.join(world.scenarioDir, `preview-${world.previewFiles.length}${extension}`);
  fs.writeFileSync(name, contents);
  world.previewFiles.push(name);
  return name;
}
async function preview(world: SapphireWorld, name: string, unknown = false, tab?: number): Promise<void> {
  await world.bridge.waitFor('a running terminal', () => world.bridge.state(), s => s.tabs.some(t => t.id === (tab ?? s.activeTab) && t.terminal === 'running'));
  const token = Date.now();
  const marker = `PREVIEW_SENT_${token}`;
  // The complete marker must occur only in output, never in the echoed command.
  await world.bridge.typeLine(`${unknown ? 'DEVORA_PTY_ID=999999 ' : ''}${quote(debi)} preview ${quote(name)} && printf '\\nPREVIEW_SENT_%s\\n' ${token}`, tab);
  await world.bridge.waitFor('debi preview to succeed', () => world.bridge.terminalText(tab), text => text.includes(marker + '\n') || text.includes(marker + '\r'));
}
async function active(world: SapphireWorld) {
  const state = await world.bridge.state();
  return state.tabs.find(t => t.id === state.activeTab)!;
}
/** The IPC command can finish before its pane event folds on Swift's next frame. */
async function renderedPreview(world: SapphireWorld) {
  const preview = (await active(world))?.previews[0];
  return preview ? world.bridge.preview(preview.id) : null;
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
Then('the preview pane should render the Markdown/HTML heading {string}', async function (this: SapphireWorld, heading: string) {
  await this.bridge.waitFor(`rendered heading ${heading}`, () => renderedPreview(this), value => value?.heading === heading);
});
Then('the preview pane should render a Mermaid diagram containing {string}', async function (this: SapphireWorld, label: string) {
  await this.bridge.waitFor('rendered Mermaid SVG', () => renderedPreview(this), value => !!value?.mermaid.includes(label));
});
Then('the Mermaid diagram nodes should use a themed fill color', async function (this: SapphireWorld) {
  const value = await this.bridge.preview((await active(this)).previews[0].id);
  assert.ok(value.fill && !['none', 'rgb(0, 0, 0)'].includes(value.fill), value.fill);
});

When('the preview file is changed to heading {string}', function (this: SapphireWorld, title: string) {
  const name = this.previewFiles.at(-1)!;
  fs.writeFileSync(name, name.endsWith('.html') ? `<h1>${title}</h1>` : `# ${title}\n`);
});
When('the preview file is deleted', function (this: SapphireWorld) { fs.unlinkSync(this.previewFiles.at(-1)!); });
When('the last preview file is sent to the first session', async function (this: SapphireWorld) {
  const tab = (await this.bridge.state()).tabs[0];
  await preview(this, this.previewFiles.at(-1)!, false, tab.id);
});
Then('the preview pane should show a file error', async function (this: SapphireWorld) {
  await this.bridge.waitFor('an in-pane file error', () => active(this), tab => !!tab.previews[0]?.error);
});

When('an HTML file is previewed in the active session', async function (this: SapphireWorld) {
  await preview(this, file(this, '<h1>Hello HTML</h1>', '.html'));
});
Then('the preview pane should display sandboxed HTML', async function (this: SapphireWorld) {
  await this.bridge.waitFor('sandboxed HTML to render', () => renderedPreview(this), value => value?.heading === 'Hello HTML' && value.htmlSandbox === true);
});

When('the same preview file is previewed again in the active session', async function (this: SapphireWorld) { await preview(this, this.previewFiles.at(-1)!); });

const htmlProbes = new WeakMap<SapphireWorld, { server: Server; requests: string[] }>();
After(async function (this: SapphireWorld) {
  const probe = htmlProbes.get(this);
  if (probe) await new Promise<void>((resolve, reject) => probe.server.close(error => error ? reject(error) : resolve()));
});

When('an HTML file containing scripts and resource probes is previewed', async function (this: SapphireWorld) {
  const requests: string[] = [];
  const server = createServer((request, response) => {
    requests.push(request.url ?? '');
    response.writeHead(200, { 'Content-Type': 'text/css' }).end('body { --remote-loaded: yes; }');
  });
  await new Promise<void>(resolve => server.listen(0, '127.0.0.1', resolve));
  htmlProbes.set(this, { server, requests });
  const address = server.address();
  if (!address || typeof address === 'string') throw new Error('missing resource probe port');
  const origin = `http://127.0.0.1:${address.port}`;
  const stylesheet = path.join(this.scenarioDir, 'secret.css');
  const image = path.join(this.scenarioDir, 'secret.svg');
  fs.writeFileSync(stylesheet, 'body { --file-loaded: yes; }');
  fs.writeFileSync(image, '<svg xmlns="http://www.w3.org/2000/svg" width="9" height="9"><rect width="9" height="9"/></svg>');
  await preview(this, file(this, `<!doctype html><html><head>
    <meta http-equiv="refresh" content="0;url=${origin}/refresh">
    <link rel="stylesheet" href="${origin}/style.css">
    <link rel="stylesheet" href="${pathToFileURL(stylesheet)}">
    <style>@import url('${origin}/import.css'); body { background-image: url('${origin}/background'); }</style>
    </head><body><h1>Sandbox probe</h1>
    <script src="${origin}/external-script"></script>
    <script>document.body.dataset.scriptRan='yes'; fetch('${origin}/script'); window.open('${origin}/popup-script');</script>
    <img src="${origin}/image" onerror="document.body.dataset.scriptRan='event'">
    <img src="${pathToFileURL(image)}">
    <iframe src="${origin}/frame"></iframe>
    <a id="remote-link" href="${origin}/navigation">Remote link</a>
    <a id="popup-link" href="${origin}/popup" target="_blank">Popup link</a>
    <a id="file-link" href="${pathToFileURL(stylesheet)}">File link</a>
    <a id="renderer-link" href="devora-renderer://app/index.html">App renderer</a>
    <form id="remote-form" action="${origin}/form"><button>Submit</button></form>
    </body></html>`, '.html'));
  await this.bridge.waitFor('the HTML probe document and blocked images to finish', () => renderedPreview(this), value => value?.heading === 'Sandbox probe' && value.images.length === 2 && value.images.every(image => image.complete));
});

When('HTML links popups and forms attempt to leave the preview', async function (this: SapphireWorld) {
  const id = (await active(this)).previews[0].id;
  for (const action of ['link', 'popup', 'file', 'renderer', 'form']) await this.bridge.previewAction(id, action);
});
Then('the HTML sandbox should block scripts resources and navigation', async function (this: SapphireWorld) {
  // Negative network assertions observe a bounded interval after the document and images settled.
  await new Promise(resolve => setTimeout(resolve, 300));
  const value = await renderedPreview(this);
  assert.ok(value?.htmlSandbox);
  assert.equal(value.heading, 'Sandbox probe');
  assert.equal(value.scriptRan, '');
  assert.equal(value.fileStyle, '');
  assert.equal(value.url, 'about:blank');
  assert.ok(value.images.every(image => image.complete && image.width === 0));
  assert.deepEqual(htmlProbes.get(this)!.requests, []);
});
