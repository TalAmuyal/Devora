import { describe, it, expect } from 'vitest';
import { initWindowLayerStack, getWindowLayerStack } from '../stack';
import { LayerStack } from '../LayerStack';

function deps() {
  return { host: document.createElement('div') };
}

describe('window layer stack accessor', () => {
  it('initWindowLayerStack creates a LayerStack and getWindowLayerStack returns the same instance', () => {
    const created = initWindowLayerStack(deps());
    expect(created).toBeInstanceOf(LayerStack);
    expect(getWindowLayerStack()).toBe(created);
  });

  it('initWindowLayerStack replaces the singleton on a fresh init', () => {
    const first = initWindowLayerStack(deps());
    const second = initWindowLayerStack(deps());
    expect(second).not.toBe(first);
    expect(getWindowLayerStack()).toBe(second);
  });
});
