/**
 * Orchestrates non-blocking task creation.
 * Clicking "Create" in the Workspace Hub hands off here: a new session tab opens immediately with a creation-progress panel overlay, the backend `create_workspace` command streams progress over a channel, and on completion the overlay is dismissed and the terminal connects in the workspace.
 * Cancelling (button or Esc/q) tears the tab and overlay down; the backend cleans up the partial/reused workspace.
 */

import { invoke, invokeLogOnly, Channel } from '../invoke';
import { showError } from '../errors';
import { SessionManager } from '../session/SessionManager';
import { OverlayManager } from '../ui/OverlayManager';
import {
  createTaskCreationProgress,
  TaskCreationProgressHandle,
} from '../ui/components/TaskCreationProgress';

type CreationEvent =
  | { type: 'step'; label: string }
  | { type: 'log'; line: string }
  | { type: 'done'; workspace: { path: string; name: string } }
  | { type: 'failed'; message: string }
  | { type: 'cancelled' };

interface InFlightCreation {
  progress: TaskCreationProgressHandle;
  repoNames: string[];
  profilePath: string;
  /** Backend creation id, set once `create_workspace` returns; null until then. */
  creationId: number | null;
  /** Set if the user cancels before the backend id is known, so we can cancel once it arrives. */
  cancelRequested: boolean;
  /** Set once the creation has failed — Esc/q and the footer action then close rather than cancel. */
  failed: boolean;
}

export interface TaskCreationControllerDeps {
  sessionManager: SessionManager;
  overlayManager: OverlayManager;
  mainPanelEl: HTMLElement;
  /** Resolve the per-profile terminal app command (mirrors opening an existing workspace). */
  resolveAppCommand: (profilePath: string | null) => Promise<string | undefined>;
  /** Re-render the tab bar after tabs/overlays change. */
  onChange: () => void;
}

export class TaskCreationController {
  private creations = new Map<number, InFlightCreation>();

  constructor(private deps: TaskCreationControllerDeps) {}

  /** Begin creating a task: open the pending tab + progress overlay and drive the backend channel. */
  async start(taskName: string, repoPaths: string[], profilePath: string): Promise<void> {
    const repoNames = repoPaths.map((p) => p.split('/').pop() ?? p);
    const session = this.deps.sessionManager.createPendingSession(taskName, profilePath);
    const progress = createTaskCreationProgress(taskName);

    const creation: InFlightCreation = {
      progress,
      repoNames,
      profilePath,
      creationId: null,
      cancelRequested: false,
      failed: false,
    };
    this.creations.set(session.id, creation);

    progress.onCancel(() => this.cancel(session.id));
    progress.onClose(() => this.close(session.id));

    this.deps.overlayManager.showPanelOverlay(
      session.id,
      progress.element,
      this.deps.mainPanelEl,
      session.terminalPane,
    );
    this.deps.overlayManager.onSessionActivated(session.id);
    this.deps.onChange();

    const onEvent = new Channel<CreationEvent>();
    onEvent.onmessage = (event) => this.handleEvent(session.id, event);

    try {
      const creationId = await invoke<number>('create_workspace', {
        profilePath,
        repoPaths,
        taskName,
        onEvent,
      });
      const current = this.creations.get(session.id);
      if (!current) return; // already torn down
      current.creationId = creationId;
      if (current.cancelRequested) {
        this.requestBackendCancel(creationId);
      }
    } catch (_) {
      // invoke already surfaced the error; the backend never started, so tear the tab down.
      this.close(session.id);
    }
  }

  private handleEvent(sessionId: number, event: CreationEvent): void {
    const creation = this.creations.get(sessionId);
    if (!creation) return;

    switch (event.type) {
      case 'step':
        creation.progress.setStep(event.label);
        break;
      case 'log':
        creation.progress.appendLog(event.line);
        break;
      case 'done':
        void this.handleDone(sessionId, event.workspace);
        break;
      case 'failed':
        creation.failed = true;
        creation.progress.showError(event.message);
        break;
      case 'cancelled':
        this.close(sessionId);
        break;
    }
  }

  private async handleDone(
    sessionId: number,
    workspace: { path: string; name: string },
  ): Promise<void> {
    const creation = this.creations.get(sessionId);
    if (!creation) return;
    const session = this.deps.sessionManager.getSessions().find((s) => s.id === sessionId);

    // Delete first so dismissing the overlay takes the plain (non-creation) teardown path.
    this.creations.delete(sessionId);
    this.deps.overlayManager.dismissPanelOverlay(sessionId);

    if (!session) return;
    session.setWorkspacePath(workspace.path);

    const cwd =
      creation.repoNames.length === 1
        ? `${workspace.path}/${creation.repoNames[0]}`
        : workspace.path;
    const appCommand = await this.deps.resolveAppCommand(creation.profilePath);
    try {
      await session.connect(cwd, appCommand);
    } catch (e) {
      showError(`Failed to create session: ${e}`);
    }
    this.deps.onChange();
  }

  /** Cancel a running creation (footer button). The backend emits `cancelled`, which closes the tab. */
  private cancel(sessionId: number): void {
    const creation = this.creations.get(sessionId);
    if (!creation) return;
    if (creation.failed) {
      this.close(sessionId);
      return;
    }
    if (creation.creationId === null) {
      creation.cancelRequested = true;
      return;
    }
    this.requestBackendCancel(creation.creationId);
  }

  private requestBackendCancel(creationId: number): void {
    invokeLogOnly('cancel_workspace_creation', { id: creationId }).catch(() => {});
  }

  /** Remove the overlay and close the tab (used on cancellation completion and after a failure). */
  private close(sessionId: number): void {
    this.creations.delete(sessionId);
    this.deps.overlayManager.dismissPanelOverlay(sessionId);
    this.deps.sessionManager.closeSession(sessionId);
    this.deps.onChange();
  }

  /** True while a creation overlay is showing for this session (used to route Esc/q dismissal). */
  isCreating(sessionId: number): boolean {
    return this.creations.has(sessionId);
  }

  /** Esc/q dismissal of a creation overlay: cancel while running, close after a failure. */
  handleDismiss(sessionId: number): void {
    const creation = this.creations.get(sessionId);
    if (!creation) return;
    if (creation.failed) {
      this.close(sessionId);
    } else {
      this.cancel(sessionId);
    }
  }
}
