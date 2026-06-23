import { Marked } from 'marked';
import mermaid from 'mermaid';

/** HTML-escape so raw diagram source survives inside `<pre>` and round-trips via `textContent`. */
function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) =>
    ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!,
  );
}

/**
 * A `marked` instance scoped to file previews: ` ```mermaid ` fences become `<pre class="mermaid">` carrying the raw (escaped) diagram source, while every other language falls back to marked's default code rendering.
 * Kept separate from the global `marked` singleton so unrelated callers are unaffected.
 */
const previewMarked = new Marked();
previewMarked.use({
  renderer: {
    code(token) {
      const lang = (token.lang ?? '').trim().split(/\s+/)[0];
      if (lang === 'mermaid') {
        return `<pre class="mermaid">${escapeHtml(token.text)}</pre>`;
      }
      return false; // fall back to marked's default code renderer
    },
  },
});

/** Render Markdown to an HTML string, tagging ` ```mermaid ` blocks for later diagram rendering. */
export async function markdownToHtml(markdown: string): Promise<string> {
  return previewMarked.parse(markdown);
}

/** Read a CSS custom property off the document root, falling back when it is unset. */
function cssVar(name: string, fallback: string): string {
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return value || fallback;
}

let mermaidInitialized = false;

/** Initialize mermaid once, deriving diagram colors from the app's theme CSS variables. */
function initMermaid(): void {
  if (mermaidInitialized) return;
  mermaid.initialize({
    startOnLoad: false,
    securityLevel: 'strict',
    theme: 'base',
    fontFamily: cssVar('--font-ui', 'sans-serif'),
    themeVariables: {
      darkMode: true,
      background: cssVar('--color-bg', '#24273A'),
      primaryColor: cssVar('--color-surface0', '#363A4F'),
      primaryTextColor: cssVar('--color-text', '#CAD3F5'),
      primaryBorderColor: cssVar('--color-accent', '#C6A0F6'),
      secondaryColor: cssVar('--color-surface1', '#494D64'),
      tertiaryColor: cssVar('--color-surface2', '#5B6078'),
      lineColor: cssVar('--color-subtext0', '#A5ADCB'),
      textColor: cssVar('--color-text', '#CAD3F5'),
      noteBkgColor: cssVar('--color-surface1', '#494D64'),
      noteTextColor: cssVar('--color-text', '#CAD3F5'),
      noteBorderColor: cssVar('--color-accent', '#C6A0F6'),
    },
  });
  mermaidInitialized = true;
}

let diagramCounter = 0;

/**
 * Replace each `<pre class="mermaid">` in `container` with its rendered SVG.
 * Each diagram renders independently: a parse failure leaves that block's source visible
 * (flagged via the `mermaid-error` class) without affecting sibling diagrams.
 */
export async function renderMermaidDiagrams(container: HTMLElement): Promise<void> {
  const blocks = container.querySelectorAll<HTMLPreElement>('pre.mermaid');
  if (blocks.length === 0) return;
  initMermaid();
  for (const block of blocks) {
    const source = block.textContent ?? '';
    // mermaid.render requires a never-reused id (it names its scratch node after it).
    const id = `mermaid-diagram-${diagramCounter++}`;
    try {
      const { svg } = await mermaid.render(id, source);
      const diagram = document.createElement('div');
      diagram.className = 'mermaid-diagram';
      diagram.innerHTML = svg;
      block.replaceWith(diagram);
    } catch {
      block.classList.add('mermaid-error'); // leave the source visible as a fallback
      document.getElementById(id)?.remove(); // drop mermaid's leftover scratch node, if any
    }
  }
}
