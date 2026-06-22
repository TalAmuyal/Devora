import { describe, it, expect, vi, beforeEach } from 'vitest';
import { createPreviewPane } from '../PreviewPane';
import { invokeLogOnly } from '../../../invoke';

vi.mock('../../../invoke', () => ({ invokeLogOnly: vi.fn() }));
const invokeMock = vi.mocked(invokeLogOnly);

const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (cmd: string, args?: unknown) => {
    if (cmd !== 'read_text_file') throw new Error(`unexpected command: ${cmd}`);
    const p = (args as { path: string }).path;
    return p.endsWith('.html') ? '<h1>Hello HTML</h1>' : '# Hello MD';
  });
});

describe('createPreviewPane', () => {
  it('builds a .preview-pane with a web-content header showing the basename', () => {
    const { el } = createPreviewPane({ path: '/home/u/docs/README.md', onClose: () => {} });
    expect(el.classList.contains('preview-pane')).toBe(true);
    const title = el.querySelector('.preview-pane-title') as HTMLElement;
    expect(title.textContent).toBe('README.md');
    expect(title.title).toBe('/home/u/docs/README.md');
  });

  it('exposes the canonical path on the handle', () => {
    const handle = createPreviewPane({ path: '/a/b/c.md', onClose: () => {} });
    expect(handle.path).toBe('/a/b/c.md');
  });

  it('calls onClose and stops propagation when the × is clicked', () => {
    const onClose = vi.fn();
    const parentClick = vi.fn();
    const { el } = createPreviewPane({ path: '/x/notes.md', onClose });
    el.addEventListener('click', parentClick);

    const closeEl = el.querySelector('.tab-close') as HTMLElement;
    expect(closeEl.textContent).toBe('×');
    closeEl.dispatchEvent(new MouseEvent('click', { bubbles: true }));

    expect(onClose).toHaveBeenCalledTimes(1);
    expect(parentClick).not.toHaveBeenCalled();
  });

  it('renders Markdown inline into a themed .web-content-body', async () => {
    const { el } = createPreviewPane({ path: '/docs/guide.md', onClose: () => {} });
    await flush();
    const body = el.querySelector('.web-content-body') as HTMLElement;
    expect(body).not.toBeNull();
    expect(body.querySelector('h1')?.textContent).toBe('Hello MD');
  });

  it('renders HTML into a sandboxed iframe with the file contents', async () => {
    const { el } = createPreviewPane({ path: '/docs/page.html', onClose: () => {} });
    await flush();
    const iframe = el.querySelector('iframe.web-content-iframe') as HTMLIFrameElement;
    expect(iframe).not.toBeNull();
    expect(iframe.getAttribute('sandbox')).toBe('');
    expect(iframe.getAttribute('srcdoc')).toContain('Hello HTML');
  });

  it('re-reads the file when refresh() is called', async () => {
    const handle = createPreviewPane({ path: '/docs/guide.md', onClose: () => {} });
    await flush();
    expect(invokeMock).toHaveBeenCalledTimes(1);
    handle.refresh();
    await flush();
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });
});
