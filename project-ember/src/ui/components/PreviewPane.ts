import { WebContentOverlay } from '../../webview/WebContentOverlay';

/**
 * A rendered file preview that sits beside the terminal in a split.
 * DOM: `div.preview-pane > div.web-content > (header[title + ×], body)`.
 * Markdown renders inline (themed); HTML renders in a sandboxed iframe.
 */
export interface PreviewPaneHandle {
  /** The flex child to insert into the session split. */
  readonly el: HTMLElement;
  /** Canonical absolute path of the previewed file. */
  readonly path: string;
  /** Re-read the file and re-render (used when the same path is previewed again). */
  refresh(): void;
}

function basename(path: string): string {
  const parts = path.split('/');
  return parts[parts.length - 1] || path;
}

function isHtmlPath(path: string): boolean {
  return /\.html?$/i.test(path);
}

export function createPreviewPane(opts: { path: string; onClose: () => void }): PreviewPaneHandle {
  const { path, onClose } = opts;

  const el = document.createElement('div');
  el.className = 'preview-pane';

  const container = document.createElement('div');
  container.className = 'web-content';

  const header = document.createElement('div');
  header.className = 'web-content-header';

  const titleEl = document.createElement('span');
  titleEl.className = 'preview-pane-title';
  titleEl.textContent = basename(path);
  titleEl.title = path;
  header.appendChild(titleEl);

  const closeEl = document.createElement('span');
  closeEl.className = 'tab-close';
  closeEl.textContent = '×';
  closeEl.title = 'Close preview';
  closeEl.addEventListener('click', (e) => {
    e.stopPropagation();
    onClose();
  });
  header.appendChild(closeEl);

  container.appendChild(header);

  // Replaced in place on every (re)load so refresh keeps the header/scroll chrome.
  let body: HTMLElement = document.createElement('div');
  body.className = 'web-content-body';
  container.appendChild(body);

  el.appendChild(container);

  // Each (re)load is tagged so a slow render (mermaid diagrams are async) can't overwrite a newer one.
  let loadSeq = 0;
  const loadBody = async (): Promise<void> => {
    const seq = ++loadSeq;
    const next = isHtmlPath(path)
      ? await WebContentOverlay.createHtmlBody(path)
      : await WebContentOverlay.createMarkdownBody(path);
    if (seq !== loadSeq) return; // a newer load superseded this one
    container.replaceChild(next, body);
    body = next;
  };

  void loadBody();

  return {
    el,
    path,
    refresh: () => void loadBody(),
  };
}
