import { invokeLogOnly } from '../invoke';
import { marked } from 'marked';

export class WebContentOverlay {

  static async createMarkdownContent(filePath: string, title: string): Promise<HTMLElement> {
    const container = document.createElement('div');
    container.className = 'web-content';

    // Header bar with title and close hint
    const header = document.createElement('div');
    header.className = 'web-content-header';
    const titleEl = document.createElement('span');
    titleEl.textContent = title;
    header.appendChild(titleEl);
    container.appendChild(header);

    // Content area
    const body = document.createElement('div');
    body.className = 'web-content-body';

    try {
      // The failure is rendered inline below, so a banner would be redundant
      const markdown = await invokeLogOnly<string>('read_text_file', { path: filePath });
      body.innerHTML = await marked(markdown);
    } catch (e) {
      body.textContent = `Failed to load: ${e}`;
    }

    container.appendChild(body);
    return container;
  }

  static createUrlContent(url: string, title: string): HTMLElement {
    const container = document.createElement('div');
    container.className = 'web-content';

    const header = document.createElement('div');
    header.className = 'web-content-header';
    const titleEl = document.createElement('span');
    titleEl.textContent = title;
    header.appendChild(titleEl);
    container.appendChild(header);

    const iframe = document.createElement('iframe');
    iframe.className = 'web-content-iframe';
    iframe.src = url;
    container.appendChild(iframe);

    return container;
  }
}
