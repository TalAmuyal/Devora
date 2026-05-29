/** Horizontal bar of `<kbd>key</kbd> description` hints with an optional trailing element. DOM: `div.keyboard-hint-bar`. */

export interface KeyboardHint {
  keys: string;
  description: string;
}

export interface KeyboardHintBarOptions {
  hints: KeyboardHint[];
  trailing?: HTMLElement;
}

export function createKeyboardHintBar(options: KeyboardHintBarOptions): HTMLElement {
  const bar = document.createElement('div');
  bar.className = 'keyboard-hint-bar';

  for (const hint of options.hints) {
    const item = document.createElement('span');
    item.className = 'keyboard-hint-item';
    item.innerHTML = `<kbd>${hint.keys}</kbd> ${hint.description}`;
    bar.appendChild(item);
  }

  if (options.trailing) {
    bar.appendChild(options.trailing);
  }

  return bar;
}
