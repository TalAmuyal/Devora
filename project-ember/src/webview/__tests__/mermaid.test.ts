import { describe, it, expect, vi, beforeEach } from 'vitest';

// happy-dom has no SVG layout engine, so the real mermaid cannot render here.
// Mock the library at its boundary and drive its render result, so these tests exercise OUR transform/fallback logic (the marked override, the source round-trip, the per-block error handling) deterministically.
vi.mock('mermaid', () => ({
  default: { initialize: vi.fn(), render: vi.fn() },
}));

import mermaid from 'mermaid';
import { markdownToHtml, renderMermaidDiagrams } from '../mermaid';

const mermaidMock = vi.mocked(mermaid);

function bodyFrom(html: string): HTMLElement {
  const el = document.createElement('div');
  el.innerHTML = html;
  return el;
}

beforeEach(() => {
  mermaidMock.initialize.mockReset();
  mermaidMock.render.mockReset();
});

describe('markdownToHtml', () => {
  it('tags a ```mermaid fence as <pre class="mermaid"> with the source HTML-escaped', async () => {
    const html = await markdownToHtml('```mermaid\nflowchart LR\n  A --> B & <C>\n```\n');
    expect(html).toContain('<pre class="mermaid">');
    expect(html).toContain('A --&gt; B &amp; &lt;C&gt;');
    expect(html).not.toContain('<code'); // not the default code renderer
  });

  it('leaves non-mermaid code blocks as default code rendering', async () => {
    const html = await markdownToHtml('```js\nconst x = 1;\n```\n');
    expect(html).toContain('<code');
    expect(html).not.toContain('class="mermaid"');
  });

  it('renders ordinary markdown normally', async () => {
    expect(await markdownToHtml('# Title\n')).toContain('<h1>Title</h1>');
  });
});

describe('renderMermaidDiagrams', () => {
  it('replaces a mermaid block with its rendered SVG, passing the decoded source', async () => {
    mermaidMock.render.mockResolvedValue({ svg: '<svg><text>Client</text></svg>' } as never);
    const body = bodyFrom(await markdownToHtml('```mermaid\nflowchart LR\n  A[Client] & <x>\n```\n'));

    await renderMermaidDiagrams(body);

    expect(body.querySelector('pre.mermaid')).toBeNull();
    const svg = body.querySelector('.mermaid-diagram svg');
    expect(svg).not.toBeNull();
    expect(svg?.textContent).toContain('Client');
    // The escaped source must round-trip back to the original before reaching mermaid.
    expect(mermaidMock.render).toHaveBeenCalledWith(expect.any(String), expect.stringContaining('A[Client] & <x>'));
  });

  it('falls back to the source for a failed diagram without affecting siblings', async () => {
    mermaidMock.render
      .mockRejectedValueOnce(new Error('parse error'))
      .mockResolvedValueOnce({ svg: '<svg><text>ok</text></svg>' } as never);
    const body = bodyFrom(
      await markdownToHtml('```mermaid\nBROKEN\n```\n\n```mermaid\nflowchart LR\n  A\n```\n'),
    );

    await renderMermaidDiagrams(body);

    const failed = body.querySelector('pre.mermaid.mermaid-error');
    expect(failed).not.toBeNull();
    expect(failed?.textContent).toContain('BROKEN');
    expect(body.querySelector('.mermaid-diagram svg')).not.toBeNull();
  });

  it('does nothing (no render) when there are no mermaid blocks', async () => {
    await renderMermaidDiagrams(bodyFrom(await markdownToHtml('# Just text\n')));
    expect(mermaidMock.render).not.toHaveBeenCalled();
  });
});
