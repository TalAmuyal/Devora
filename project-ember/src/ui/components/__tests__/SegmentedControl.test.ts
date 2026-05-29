import { describe, it, expect, vi } from 'vitest';
import { createSegmentedControl } from '../SegmentedControl';

describe('createSegmentedControl', () => {
  it('renders correct number of buttons', () => {
    const el = createSegmentedControl({
      items: [
        { key: 'a', label: 'Alpha' },
        { key: 'b', label: 'Beta' },
        { key: 'c', label: 'Charlie' },
      ],
      activeKey: 'a',
      onSelect: () => {},
    });
    const buttons = el.querySelectorAll('button');
    expect(buttons.length).toBe(3);
  });

  it('active button has segmented-control-active class', () => {
    const el = createSegmentedControl({
      items: [
        { key: 'a', label: 'Alpha' },
        { key: 'b', label: 'Beta' },
      ],
      activeKey: 'b',
      onSelect: () => {},
    });
    const buttons = el.querySelectorAll('button');
    expect(buttons[0].classList.contains('segmented-control-active')).toBe(false);
    expect(buttons[1].classList.contains('segmented-control-active')).toBe(true);
  });

  it('clicking a button calls onSelect with the correct key', () => {
    const onSelect = vi.fn();
    const el = createSegmentedControl({
      items: [
        { key: 'x', label: 'X' },
        { key: 'y', label: 'Y' },
      ],
      activeKey: 'x',
      onSelect,
    });
    const buttons = el.querySelectorAll('button');
    buttons[1].click();
    expect(onSelect).toHaveBeenCalledWith('y');
  });

  it('only one button is active at a time', () => {
    const el = createSegmentedControl({
      items: [
        { key: 'a', label: 'A' },
        { key: 'b', label: 'B' },
        { key: 'c', label: 'C' },
      ],
      activeKey: 'b',
      onSelect: () => {},
    });
    const activeButtons = el.querySelectorAll('.segmented-control-active');
    expect(activeButtons.length).toBe(1);
  });

  it('has segmented-control class on the container', () => {
    const el = createSegmentedControl({
      items: [{ key: 'a', label: 'A' }],
      activeKey: 'a',
      onSelect: () => {},
    });
    expect(el.classList.contains('segmented-control')).toBe(true);
  });

  it('buttons have segmented-control-btn class', () => {
    const el = createSegmentedControl({
      items: [{ key: 'a', label: 'A' }],
      activeKey: 'a',
      onSelect: () => {},
    });
    const btn = el.querySelector('button');
    expect(btn?.classList.contains('segmented-control-btn')).toBe(true);
  });
});
