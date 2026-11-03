import './standalone.css';
import './content.css';
import { markdownToHtml, renderMermaidDiagrams } from './index';

const content = document.querySelector<HTMLElement>('#content')!;
Object.assign(window, {
  devoraScroll(action: string) {
    const moves: Record<string, number> = { up: -0.1, down: 0.1, halfUp: -0.5, halfDown: 0.5 };
    if (action === 'top') content.scrollTo({ top: 0 });
    else if (action === 'bottom') content.scrollTo({ top: content.scrollHeight });
    else if (action in moves) content.scrollBy({ top: content.clientHeight * moves[action] });
  },
  async devoraRender(markdown: string, theme: Record<string, string>) {
    for (const [name, value] of Object.entries(theme)) {
      if (name.startsWith('--')) document.documentElement.style.setProperty(name, value);
    }
    content.innerHTML = await markdownToHtml(markdown);
    await renderMermaidDiagrams(content);
  },
});

// The native host renders only after this module has finished initializing.
const host = (window as Window & { webkit?: { messageHandlers?: { devoraRenderer?: { postMessage: (message: unknown) => void } } } }).webkit;
host?.messageHandlers?.devoraRenderer?.postMessage({ ready: true });
