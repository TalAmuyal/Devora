import { describe, it, expect } from 'vitest';
import { initLayerStack, getLayerStack } from '../stack';
import { LayerStack } from '../LayerStack';

function deps() {
  return {
    pageHost: document.createElement('div'),
    modalHost: document.body,
    resolveBaseFocus: () => null,
  };
}

describe('layer stack accessor', () => {
  it('initLayerStack creates a LayerStack and getLayerStack returns the same instance', () => {
    const created = initLayerStack(deps());
    expect(created).toBeInstanceOf(LayerStack);
    expect(getLayerStack()).toBe(created);
  });

  it('initLayerStack replaces the singleton on a fresh init', () => {
    const first = initLayerStack(deps());
    const second = initLayerStack(deps());
    expect(second).not.toBe(first);
    expect(getLayerStack()).toBe(second);
  });
});
