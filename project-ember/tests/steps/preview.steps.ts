import { When, Then } from '@cucumber/cucumber';
import { EmberWorld } from '../support/world';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

async function activePtyId(world: EmberWorld): Promise<number> {
  return world.driver.eval(
    'return window.__test.sessionManager.getActiveSession().getPtyId()',
  );
}

function makeTempFile(world: EmberWorld, ext: string, heading: string): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'devora-preview-'));
  const file = path.join(dir, `doc${ext}`);
  const contents = ext === '.html' ? `<h1>${heading}</h1>` : `# ${heading}\n`;
  fs.writeFileSync(file, contents);
  (world.previewFiles ??= []).push(file);
  return file;
}

async function sendPreview(world: EmberWorld, filePath: string, stack: boolean): Promise<void> {
  const ptyId = await activePtyId(world);
  world.driver.ipcPostFireAndForget('/preview/open', { ptyId, path: filePath, stack });
  await new Promise((r) => setTimeout(r, 200));
}

When('a Markdown file is previewed in the active session', async function (this: EmberWorld) {
  await sendPreview(this, makeTempFile(this, '.md', 'First Doc'), false);
});

When('another Markdown file is previewed in the active session', async function (this: EmberWorld) {
  await sendPreview(this, makeTempFile(this, '.md', 'Second Doc'), false);
});

When(
  'another Markdown file is previewed in the active session with --stack',
  async function (this: EmberWorld) {
    await sendPreview(this, makeTempFile(this, '.md', 'Stacked Doc'), true);
  },
);

When(
  'the same Markdown file is previewed again in the active session',
  async function (this: EmberWorld) {
    const files = this.previewFiles!;
    await sendPreview(this, files[files.length - 1], false);
  },
);

When('an HTML file is previewed in the active session', async function (this: EmberWorld) {
  await sendPreview(this, makeTempFile(this, '.html', 'Hello HTML'), false);
});

When('a Markdown file is previewed for an unknown session', async function (this: EmberWorld) {
  const file = makeTempFile(this, '.md', 'Orphan');
  this.driver.ipcPostFireAndForget('/preview/open', { ptyId: 999999, path: file, stack: false });
  await new Promise((r) => setTimeout(r, 200));
});

When('the preview pane close button is clicked', async function (this: EmberWorld) {
  await this.driver.eval(`
    const btn = document.querySelector('.preview-pane .tab-close');
    if (!btn) throw new Error('no preview close button found');
    btn.click();
  `);
});

Then(
  'the active session should have {int} preview pane(s)',
  async function (this: EmberWorld, count: number) {
    await this.driver.pollFor(
      'return window.__test.sessionManager.getActiveSession().getPreviewCount()',
      count,
      10_000,
    );
  },
);

Then(
  'the preview pane should render the Markdown heading {string}',
  async function (this: EmberWorld, heading: string) {
    await this.driver.pollFor(
      `return (() => {
        const h = document.querySelector('.preview-pane .web-content-body h1');
        return h ? h.textContent : null;
      })()`,
      heading,
      10_000,
    );
  },
);

Then('the preview pane should contain a sandboxed iframe', async function (this: EmberWorld) {
  await this.driver.pollFor(
    `return (() => {
      const f = document.querySelector('.preview-pane iframe.web-content-iframe');
      return f ? f.getAttribute('sandbox') : null;
    })()`,
    '',
    10_000,
  );
});
