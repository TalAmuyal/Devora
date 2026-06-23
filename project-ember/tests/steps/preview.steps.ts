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

When(
  'a Markdown file with a Mermaid diagram is previewed in the active session',
  async function (this: EmberWorld) {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'devora-preview-'));
    const file = path.join(dir, 'diagram.md');
    fs.writeFileSync(file, '# Diagram\n\n```mermaid\nflowchart LR\n  A[Client] --> B[Server]\n```\n');
    (this.previewFiles ??= []).push(file);
    await sendPreview(this, file, false);
  },
);

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

Then(
  'the preview pane should render a Mermaid diagram containing {string}',
  async function (this: EmberWorld, label: string) {
    // A rendered (not error) diagram contains the node label; an SVG appearing at all proves mermaid runs under the production CSP (no unsafe-eval) and its lazy diagram chunk loaded.
    await this.driver.pollFor(
      `return (() => {
        const svg = document.querySelector('.preview-pane .mermaid-diagram svg');
        return svg && (svg.textContent || '').includes(${JSON.stringify(label)}) ? ${JSON.stringify(label)} : null;
      })()`,
      label,
      10_000,
    );
  },
);

Then(
  'the Mermaid diagram nodes should use a themed fill color',
  async function (this: EmberWorld) {
    // The prior step guarantees the diagram has rendered. Mermaid colors its nodes via a runtime inline <style>; if the production CSP blocked that style, the node would fall back to the SVG default (black/none).
    // A real fill therefore proves mermaid's inline styles applied under CSP.
    const fill = await this.driver.eval(`
      return (() => {
        const svg = document.querySelector('.preview-pane .mermaid-diagram svg');
        if (!svg) return 'no-svg';
        const shape = svg.querySelector('.node rect, rect.basic, .node polygon, .node path, .node circle');
        if (!shape) return 'no-shape';
        return getComputedStyle(shape).fill || 'empty';
      })()
    `);
    const unstyled = ['no-svg', 'no-shape', 'empty', 'none', 'rgb(0, 0, 0)'];
    if (unstyled.includes(fill)) {
      throw new Error(`Mermaid node fill not themed (got "${fill}") — inline styles may be CSP-blocked`);
    }
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
