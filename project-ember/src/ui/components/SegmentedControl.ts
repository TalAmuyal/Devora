/** Mutually exclusive toggle button bar. DOM: `div.segmented-control > button.segmented-control-btn`. */

export interface SegmentedControlItem<K extends string = string> {
  key: K;
  label: string;
}

export interface SegmentedControlOptions<K extends string = string> {
  items: SegmentedControlItem<K>[];
  activeKey: K;
  onSelect: (key: K) => void;
}

export function createSegmentedControl<K extends string>(
  options: SegmentedControlOptions<K>,
): HTMLElement {
  const bar = document.createElement('div');
  bar.className = 'segmented-control';

  for (const item of options.items) {
    const btn = document.createElement('button');
    btn.className = 'segmented-control-btn';
    if (item.key === options.activeKey) {
      btn.classList.add('segmented-control-active');
    }
    btn.textContent = item.label;
    btn.addEventListener('click', () => options.onSelect(item.key));
    bar.appendChild(btn);
  }

  return bar;
}
