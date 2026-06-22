import { invokeLogOnly } from '../invoke';
import { marked } from 'marked';

export class WebContentOverlay {

  /** A themed `.web-content-body` element showing a load failure inline (a banner would be redundant). */
  private static createErrorBody(e: unknown): HTMLElement {
    const body = document.createElement('div');
    body.className = 'web-content-body';
    body.textContent = `Failed to load: ${e}`;
    return body;
  }

  /** Render a Markdown file into a themed `.web-content-body` element. */
  static async createMarkdownBody(filePath: string): Promise<HTMLElement> {
    try {
      const markdown = await invokeLogOnly<string>('read_text_file', { path: filePath });
      const body = document.createElement('div');
      body.className = 'web-content-body';
      body.innerHTML = await marked(markdown);
      return body;
    } catch (e) {
      return this.createErrorBody(e);
    }
  }

  /**
   * Render an HTML file into a sandboxed `<iframe>` element.
   * The sandbox (no `allow-scripts`, no `allow-same-origin`) isolates the file's document: no script execution, no access to the host app, and no style bleed.
   */
  static async createHtmlBody(filePath: string): Promise<HTMLElement> {
    try {
      const html = await invokeLogOnly<string>('read_text_file', { path: filePath });
      const iframe = document.createElement('iframe');
      iframe.className = 'web-content-iframe';
      iframe.setAttribute('sandbox', '');
      iframe.srcdoc = html;
      return iframe;
    } catch (e) {
      return this.createErrorBody(e);
    }
  }

  private static buildShell(title: string): HTMLElement {
    const container = document.createElement('div');
    container.className = 'web-content';

    const header = document.createElement('div');
    header.className = 'web-content-header';
    const titleEl = document.createElement('span');
    titleEl.textContent = title;
    header.appendChild(titleEl);
    container.appendChild(header);

    return container;
  }

  static async createMarkdownContent(filePath: string, title: string): Promise<HTMLElement> {
    const container = this.buildShell(title);
    container.appendChild(await this.createMarkdownBody(filePath));
    return container;
  }

  static createUrlContent(url: string, title: string): HTMLElement {
    const container = this.buildShell(title);
    const iframe = document.createElement('iframe');
    iframe.className = 'web-content-iframe';
    iframe.src = url;
    container.appendChild(iframe);
    return container;
  }
}
