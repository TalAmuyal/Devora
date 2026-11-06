import { markdownToHtml, renderMermaidDiagrams } from './index';
import './chat.css';

type Anchor = { sequence: string; offset: number } | null;
type Item = {
  sequence: string; kind: string; state: string; text: string;
  name?: string; input?: string; result?: string; verdict?: string; verdictDetail?: string;
};
type Question = { generation: string; id: string; tool: string; input: string; reason: string; rules: string; always: boolean; sequence?: string };
type Chat = { items: Item[]; questions: Question[]; anchor: Anchor; hasOlder: boolean };
type Host = (message: unknown) => void;

/** Morph sanitized Markdown without replacing unchanged nodes or text selections. */
function updateChildren(target: Element, source: Element) {
  const children = [...source.childNodes];
  children.forEach((child, index) => {
    const old = target.childNodes[index];
    if (!old) { target.append(child.cloneNode(true)); return; }
    if (old.nodeType !== child.nodeType || old.nodeName !== child.nodeName) {
      old.replaceWith(child.cloneNode(true));
    } else if (old instanceof Element && child instanceof Element) {
      for (const attribute of [...old.attributes]) {
        if (!child.hasAttribute(attribute.name)) old.removeAttribute(attribute.name);
      }
      for (const attribute of [...child.attributes]) old.setAttribute(attribute.name, attribute.value);
      updateChildren(old, child);
    } else if (old.textContent !== child.textContent) {
      const text = child.textContent ?? '';
      if (old instanceof Text && text.startsWith(old.data)) old.appendData(text.slice(old.length));
      else old.textContent = text;
    }
  });
  while (target.childNodes.length > children.length) target.lastChild!.remove();
}

function text(tag: string, value: string, className?: string) {
  const node = document.createElement(tag);
  node.textContent = value;
  if (className) node.className = className;
  return node;
}

/** One local renderer instance per tab. Only host-created controls send actions. */
export function chatRenderer(content: HTMLElement, post: Host) {
  const rows = new Map<string, { node: HTMLElement; value: string }>();
  const cards = new Map<string, { node: HTMLElement; value: string }>();
  let initialized = false;
  let hasOlder = false;
  let timer: ReturnType<typeof setTimeout>;
  let historyPending = false;

  function anchor(): Anchor {
    if (content.scrollHeight - content.scrollTop - content.clientHeight < 24) return null;
    const row = [...content.querySelectorAll<HTMLElement>(':scope > [data-sequence]')]
      .find(row => row.offsetTop + row.offsetHeight > content.scrollTop);
    return row ? { sequence: row.dataset.sequence!, offset: Math.round(row.offsetTop - content.scrollTop) } : null;
  }
  function scroll(saved: Anchor) {
    if (!saved) { content.scrollTop = content.scrollHeight; return; }
    const row = rows.get(saved.sequence)?.node;
    if (row) content.scrollTop = row.offsetTop - saved.offset;
  }
  content.addEventListener('scroll', () => {
    if (!initialized) return;
    clearTimeout(timer);
    timer = setTimeout(() => post({ scroll: anchor() }), 250);
    if (content.scrollTop < 80 && hasOlder && !historyPending) {
      historyPending = true;
      post({ older: true, scroll: anchor() });
    }
  });

  return async (chat: Chat) => {
    const initialTop = content.scrollTop;
    const saved = initialized ? anchor() : chat.anchor;
    content.classList.add('agent-transcript');
    hasOlder = chat.hasOlder;
    const live = new Set(chat.items.map(item => item.sequence));
    for (const [sequence, row] of rows) {
      if (!live.has(sequence)) { row.node.remove(); rows.delete(sequence); }
    }
    let previous: Element | null = null;
    for (const item of chat.items) {
      let row = rows.get(item.sequence);
      if (!row) {
        const node = document.createElement('article');
        node.dataset.sequence = item.sequence;
        node.className = `chat-item chat-${item.kind}`;
        node.title = `#${item.sequence}`;
        row = { node, value: '' };
        rows.set(item.sequence, row);
      }
      const { node } = row;
      if (previous ? previous.nextElementSibling !== node : content.firstElementChild !== node) {
        if (previous) previous.after(node); else content.prepend(node);
      }
      previous = node;
      const value = JSON.stringify(item);
      if (row.value === value) continue;
      if (!node.firstChild) {
        node.append(text('span', '', 'chat-dot'));
        if (item.kind === 'tool' || item.kind === 'thinking') {
          const detail = document.createElement('details');
          detail.append(document.createElement('summary'), text('div', '', 'chat-body'));
          node.append(detail);
        } else node.append(text('div', '', 'chat-body'));
      }
      const body = node.querySelector('.chat-body')!;
      node.dataset.state = item.state;
      if (item.kind === 'tool') {
        const summary = node.querySelector('summary')!;
        summary.textContent = `${item.name} `;
        if (item.verdict) {
          const badge = text('span', `Judgess ${item.verdict}`, 'chat-verdict');
          badge.title = `#${item.sequence} · ${item.verdictDetail ?? item.verdict}`;
          badge.dataset.verdict = item.verdict;
          summary.append(badge);
        }
        body.replaceChildren(text('pre', item.input ?? ''), text('pre', item.result ?? 'Awaiting result…'));
      } else {
        if (item.kind === 'thinking') node.querySelector('summary')!.textContent = 'Thinking';
        if (item.kind === 'assistant' || item.kind === 'user' || item.kind === 'thinking') {
          const source = document.createElement('div');
          source.innerHTML = await markdownToHtml(item.text);
          updateChildren(body, source);
          if (item.state !== 'streaming') await renderMermaidDiagrams(body as HTMLElement);
        } else body.replaceChildren(text('p', item.text));
      }
      row.value = value;
    }
    const active = new Set(chat.questions.map(question => question.id));
    for (const [id, card] of cards) {
      if (!active.has(id)) { card.node.remove(); cards.delete(id); }
    }
    for (const question of chat.questions) {
      let card = cards.get(question.id);
      const value = JSON.stringify(question);
      if (!card) {
        card = { node: document.createElement('aside'), value: '' };
        card.node.className = 'chat-permission';
        card.node.setAttribute('aria-label', 'Permission request');
        cards.set(question.id, card);
      }
      if (card.value !== value) {
        card.node.replaceChildren(text('strong', question.tool), text('pre', question.input), text('p', question.reason), text('pre', question.rules));
        for (const [label, action] of [['Allow ⌘Y', 'allow'], ['Deny ⌘N', 'deny'], ['Always allow ⌘⇧Y', 'always']]) {
          if (action === 'always' && !question.always) continue;
          const button = text('button', label) as HTMLButtonElement;
          button.addEventListener('click', () => post({ permission: { id: question.id, action, generation: question.generation } }));
          card.node.append(button);
        }
        card.value = value;
      }
      const owner = question.sequence && rows.get(question.sequence)?.node;
      if (owner) owner.append(card.node); else content.append(card.node);
    }
    scroll(content.scrollTop !== initialTop ? anchor() : saved);
    initialized = true;
    historyPending = false;
  };
}
