/**
 * Orchestrates non-blocking task creation.
 * Clicking "Create" in the Workspace Hub hands off here: a new session tab opens immediately with a creation-progress surface on its own layer stack, the backend `create_workspace` command streams progress over a channel, and on completion the surface is removed and the terminal connects in the workspace.
 * Cancelling (button or Esc/q) tears the tab and its surface down; the backend cleans up the partial/reused workspace.
 */

import { invoke, invokeLogOnly, Channel } from '../invoke';
import { showError } from '../errors';
import { SessionManager } from '../session/SessionManager';
import { pageLayer } from '../ui/layers/presets';
import {
  createTaskCreationProgress,
  TaskCreationProgressHandle,
} from '../ui/components/TaskCreationProgress';
import { DismissDecision, LayerHandle } from '../ui/layers/types';
import { CreationEvent } from './types';

interface InFlightCreation {
  progress: TaskCreationProgressHandle;
  /** The progress surface on the session's own stack; removed when creation finishes, fails-and-closes, or is cancelled. */
  layer: LayerHandle;
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
  /** Resolve the per-profile terminal app command (mirrors opening an existing workspace). */
  resolveAppCommand: (profilePath: string | null) => Promise<string | undefined>;
  /** Re-render the tab bar after tabs/overlays change. */
  onChange: () => void;
}

export class TaskCreationController {
  private creations = new Map<number, InFlightCreation>();

  constructor(private deps: TaskCreationControllerDeps) {}

  /**
   * Begin creating a task: open the pending tab + progress surface and drive the backend channel.
   * `sourceWorkspacePath` is set when duplicating a session, so the backend pins each shared repo to the source worktree's commit and copies its CLAUDE.md; pass `null` for a plain new task.
   */
  async start(
    taskName: string,
    repoPaths: string[],
    profilePath: string,
    sourceWorkspacePath: string | null,
  ): Promise<void> {
    const repoNames = repoPaths.map((p) => p.split('/').pop() ?? p);
    const session = this.deps.sessionManager.createPendingSession(taskName, profilePath);
    const progress = createTaskCreationProgress(`Creating: ${taskName}`);

    // The progress surface owns its own dismissal (Esc/q cancels or closes); the controller tears it down once that resolves.
    const layer = session.layers.push(
      pageLayer({
        name: 'task-creation-progress',
        element: progress.element,
        onUserDismissRequest: () => this.handleDismiss(session.id),
      }),
    );

    const creation: InFlightCreation = {
      progress,
      layer,
      repoNames,
      profilePath,
      creationId: null,
      cancelRequested: false,
      failed: false,
    };
    this.creations.set(session.id, creation);

    progress.onCancel(() => this.cancel(session.id));
    progress.onClose(() => this.close(session.id));
    this.deps.onChange();

    const onEvent = new Channel<CreationEvent>();
    onEvent.onmessage = (event) => this.handleEvent(session.id, event);

    try {
      const creationId = await invoke<number>('create_workspace', {
        profilePath,
        repoPaths,
        taskName,
        sourceWorkspacePath,
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
    const session = this.deps.sessionManager.getSession(sessionId);

    this.creations.delete(sessionId);
    session?.layers.remove(creation.layer);

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

  /** Remove the surface and close the tab (used on cancellation completion and after a failure). */
  private close(sessionId: number): void {
    const creation = this.creations.get(sessionId);
    this.creations.delete(sessionId);
    if (creation) {
      this.deps.sessionManager.getSession(sessionId)?.layers.remove(creation.layer);
    }
    this.deps.sessionManager.closeSession(sessionId);
    this.deps.onChange();
  }

  /**
   * Esc/q dismissal of a creation surface: cancel while running, close after a failure.
   * Returns `handled` so the stack leaves the surface in place — teardown happens here (close), or later when the backend confirms the cancel.
   */
  handleDismiss(sessionId: number): DismissDecision {
    const creation = this.creations.get(sessionId);
    if (creation) {
      if (creation.failed) {
        this.close(sessionId);
      } else {
        this.cancel(sessionId);
      }
    }
    return 'handled';
  }
}
