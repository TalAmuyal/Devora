import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { createProfileForm, PATH_VALIDATION_DEBOUNCE_MS } from '../ProfileForm';
import { invoke } from '../../invoke';
import { open } from '@tauri-apps/plugin-dialog';

vi.mock('../../invoke', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);
const openMock = vi.mocked(open);

interface ValidationShape {
  kind: 'new' | 'existing_profile' | 'invalid';
  name: string | null;
  error: string | null;
  expandedPath: string;
}

function mockValidation(validation: ValidationShape): void {
  invokeMock.mockImplementation(async (cmd: string) => {
    if (cmd === 'validate_profile_path') return validation;
    throw new Error(`unexpected command ${cmd}`);
  });
}

function nameInput(): HTMLInputElement {
  return document.querySelector('.pm-form-name')!;
}

function pathInput(): HTMLInputElement {
  return document.querySelector('.pm-form-path')!;
}

function submitBtn(): HTMLButtonElement {
  return document.querySelector('.pm-form-submit')!;
}

function statusEl(): HTMLElement {
  return document.querySelector('.pm-form-status')!;
}

function setInput(input: HTMLInputElement, value: string): void {
  input.value = value;
  input.dispatchEvent(new Event('input', { bubbles: true }));
}

/** Type into the path field and let the debounce + validation round-trip settle. */
async function typePathAndValidate(value: string): Promise<void> {
  setInput(pathInput(), value);
  await vi.advanceTimersByTimeAsync(PATH_VALIDATION_DEBOUNCE_MS);
}

describe('createProfileForm', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
    document.body.innerHTML = '';
  });

  it('renders name, path, browse, info box and a disabled submit', () => {
    const form = createProfileForm({ onRegistered: vi.fn() });
    document.body.appendChild(form.element);

    expect(nameInput()).not.toBeNull();
    expect(pathInput()).not.toBeNull();
    expect(document.querySelector('.pm-form-browse')).not.toBeNull();
    expect(document.querySelector('.pm-form-info')?.textContent).toContain('config.json');
    expect(submitBtn().disabled).toBe(true);
    expect(statusEl().style.display).toBe('none');
  });

  it('renders a cancel button only when onCancel is provided', () => {
    const withCancel = createProfileForm({ onRegistered: vi.fn(), onCancel: vi.fn() });
    expect(withCancel.element.querySelector('.pm-form-cancel')).not.toBeNull();

    const withoutCancel = createProfileForm({ onRegistered: vi.fn() });
    expect(withoutCancel.element.querySelector('.pm-form-cancel')).toBeNull();
  });

  it('new path: shows the ok status and enables submit once a name is typed', async () => {
    mockValidation({ kind: 'new', name: null, error: null, expandedPath: '/tmp/p' });
    const form = createProfileForm({ onRegistered: vi.fn() });
    document.body.appendChild(form.element);

    await typePathAndValidate('/tmp/p');

    expect(statusEl().classList.contains('ok')).toBe(true);
    expect(statusEl().textContent).toContain('New profile');
    expect(submitBtn().textContent).toBe('Create Profile');
    expect(submitBtn().disabled).toBe(true); // no name yet

    setInput(nameInput(), 'Work');
    expect(submitBtn().disabled).toBe(false);
  });

  it('existing profile: locks the name to the detected one and relabels submit', async () => {
    mockValidation({
      kind: 'existing_profile',
      name: 'Legacy',
      error: null,
      expandedPath: '/tmp/old',
    });
    const form = createProfileForm({ onRegistered: vi.fn() });
    document.body.appendChild(form.element);

    setInput(nameInput(), 'Typed');
    await typePathAndValidate('/tmp/old');

    expect(statusEl().classList.contains('detect')).toBe(true);
    expect(statusEl().textContent).toContain('"Legacy"');
    expect(nameInput().disabled).toBe(true);
    expect(nameInput().value).toBe('Legacy');
    expect(submitBtn().textContent).toBe('Register Profile');
    expect(submitBtn().disabled).toBe(false);
  });

  it('restores the typed name when leaving the existing-profile state', async () => {
    mockValidation({
      kind: 'existing_profile',
      name: 'Legacy',
      error: null,
      expandedPath: '/tmp/old',
    });
    const form = createProfileForm({ onRegistered: vi.fn() });
    document.body.appendChild(form.element);
    setInput(nameInput(), 'Typed');
    await typePathAndValidate('/tmp/old');

    mockValidation({ kind: 'new', name: null, error: null, expandedPath: '/tmp/fresh' });
    await typePathAndValidate('/tmp/fresh');

    expect(nameInput().disabled).toBe(false);
    expect(nameInput().value).toBe('Typed');
    expect(submitBtn().textContent).toBe('Create Profile');
  });

  it('invalid path: shows the error and disables submit', async () => {
    mockValidation({
      kind: 'invalid',
      name: null,
      error: 'Parent directory does not exist',
      expandedPath: '/nope/p',
    });
    const form = createProfileForm({ onRegistered: vi.fn() });
    document.body.appendChild(form.element);
    setInput(nameInput(), 'Work');

    await typePathAndValidate('/nope/p');

    expect(statusEl().classList.contains('err')).toBe(true);
    expect(statusEl().textContent).toContain('Parent directory does not exist');
    expect(submitBtn().disabled).toBe(true);
  });

  it('hides the status and disables submit when the path is cleared', async () => {
    mockValidation({ kind: 'new', name: null, error: null, expandedPath: '/tmp/p' });
    const form = createProfileForm({ onRegistered: vi.fn() });
    document.body.appendChild(form.element);
    setInput(nameInput(), 'Work');
    await typePathAndValidate('/tmp/p');
    expect(submitBtn().disabled).toBe(false);

    await typePathAndValidate('   ');

    expect(statusEl().style.display).toBe('none');
    expect(submitBtn().disabled).toBe(true);
    expect(invokeMock).toHaveBeenCalledTimes(1); // no validation call for a blank path
  });

  it('disables submit while a validation round-trip is pending', async () => {
    mockValidation({ kind: 'new', name: null, error: null, expandedPath: '/tmp/p' });
    const form = createProfileForm({ onRegistered: vi.fn() });
    document.body.appendChild(form.element);
    setInput(nameInput(), 'Work');
    await typePathAndValidate('/tmp/p');
    expect(submitBtn().disabled).toBe(false);

    setInput(pathInput(), '/tmp/p2');
    // Debounce window still open: the previous result must not keep submit live.
    expect(submitBtn().disabled).toBe(true);
  });

  it('validates an initialPath immediately, without the debounce', async () => {
    mockValidation({ kind: 'new', name: null, error: null, expandedPath: '/home/user/devora' });

    const form = createProfileForm({ initialPath: '~/devora', onRegistered: vi.fn() });
    document.body.appendChild(form.element);
    await vi.advanceTimersByTimeAsync(0);

    expect(pathInput().value).toBe('~/devora');
    expect(invokeMock).toHaveBeenCalledWith('validate_profile_path', { path: '~/devora' });
    expect(statusEl().classList.contains('ok')).toBe(true);
  });

  it('submits via register_profile and reports the result', async () => {
    const onRegistered = vi.fn();
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === 'validate_profile_path') {
        return { kind: 'new', name: null, error: null, expandedPath: '/tmp/p' };
      }
      if (cmd === 'register_profile') {
        return { name: 'Work', path: '/tmp/p' };
      }
      throw new Error(`unexpected command ${cmd}`);
    });
    const form = createProfileForm({ onRegistered });
    document.body.appendChild(form.element);
    setInput(nameInput(), 'Work');
    await typePathAndValidate('/tmp/p');

    submitBtn().click();
    await vi.advanceTimersByTimeAsync(0);

    expect(invokeMock).toHaveBeenCalledWith('register_profile', { path: '/tmp/p', name: 'Work' });
    expect(onRegistered).toHaveBeenCalledWith({ name: 'Work', path: '/tmp/p' });
  });

  it('stays open and re-enables submit when registration fails', async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === 'validate_profile_path') {
        return { kind: 'new', name: null, error: null, expandedPath: '/tmp/p' };
      }
      throw new Error('registration failed');
    });
    const onRegistered = vi.fn();
    const form = createProfileForm({ onRegistered });
    document.body.appendChild(form.element);
    setInput(nameInput(), 'Work');
    await typePathAndValidate('/tmp/p');

    submitBtn().click();
    await vi.advanceTimersByTimeAsync(0);

    expect(onRegistered).not.toHaveBeenCalled();
    expect(submitBtn().disabled).toBe(false);
  });

  it('fills the path from the native folder picker and validates immediately', async () => {
    mockValidation({ kind: 'new', name: null, error: null, expandedPath: '/picked/dir' });
    openMock.mockResolvedValue('/picked/dir');
    const form = createProfileForm({ onRegistered: vi.fn() });
    document.body.appendChild(form.element);

    (document.querySelector('.pm-form-browse') as HTMLButtonElement).click();
    await vi.advanceTimersByTimeAsync(0);

    expect(openMock).toHaveBeenCalledWith({ directory: true });
    expect(pathInput().value).toBe('/picked/dir');
    expect(statusEl().classList.contains('ok')).toBe(true);
  });

  it('ignores a cancelled folder picker', async () => {
    openMock.mockResolvedValue(null);
    const form = createProfileForm({ onRegistered: vi.fn() });
    document.body.appendChild(form.element);

    (document.querySelector('.pm-form-browse') as HTMLButtonElement).click();
    await vi.advanceTimersByTimeAsync(0);

    expect(pathInput().value).toBe('');
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('applies only the latest validation when responses race', async () => {
    const resolvers: Array<(v: ValidationShape) => void> = [];
    invokeMock.mockImplementation(
      () => new Promise((resolve) => resolvers.push(resolve as (v: ValidationShape) => void)),
    );
    const form = createProfileForm({ onRegistered: vi.fn() });
    document.body.appendChild(form.element);

    await typePathAndValidate('/tmp/first');
    await typePathAndValidate('/tmp/second');
    expect(resolvers).toHaveLength(2);

    // Second (latest) request resolves first, then the stale first one lands.
    resolvers[1]({ kind: 'new', name: null, error: null, expandedPath: '/tmp/second' });
    await vi.advanceTimersByTimeAsync(0);
    resolvers[0]({
      kind: 'invalid',
      name: null,
      error: 'stale result',
      expandedPath: '/tmp/first',
    });
    await vi.advanceTimersByTimeAsync(0);

    expect(statusEl().classList.contains('ok')).toBe(true);
    expect(statusEl().textContent).toContain('New profile');
  });
});
