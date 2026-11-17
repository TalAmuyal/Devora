import { hangWatchdog } from './hang-watchdog';

/**
 * A Cucumber formatter that reports the run's progress to the hang watchdog (hang-watchdog.ts): the run's start, each
 * scenario's start and end, and each step's and hook's, by name. Its own output is each scenario as it starts, with the
 * time, so how far a run got shows even when it's stopped.
 * Use: `--format ../acceptance/hang-watchdog-formatter.ts:<file>`.
 */

// The parts of Cucumber's messages it reads (@cucumber/messages).
interface Envelope {
  pickle?: { id: string; name: string; uri: string; steps: { id: string; text: string }[] };
  hook?: { id: string; name?: string; sourceReference: { uri?: string; location?: { line: number } } };
  testCase?: { id: string; pickleId: string; testSteps: { id: string; pickleStepId?: string; hookId?: string }[] };
  testRunStarted?: object;
  testCaseStarted?: { id: string; testCaseId: string };
  testStepStarted?: { testCaseStartedId: string; testStepId: string };
  testStepFinished?: { testCaseStartedId: string; testStepId: string };
  testCaseFinished?: { testCaseStartedId: string };
  testRunFinished?: object;
}

interface Context {
  on(key: 'message', handler: (envelope: Envelope) => void): void;
  write(text: string): void;
}

export default {
  type: 'formatter',
  formatter({ on, write }: Context) {
    const watchdog = hangWatchdog();
    const pickles = new Map<string, NonNullable<Envelope['pickle']>>();
    const hooks = new Map<string, NonNullable<Envelope['hook']>>();
    const testCases = new Map<string, NonNullable<Envelope['testCase']>>();
    const started = new Map<string, string>();

    const scenario = (testCaseStartedId: string) => {
      const pickle = pickles.get(testCases.get(started.get(testCaseStartedId)!)!.pickleId)!;
      return `scenario "${pickle.name}" (${pickle.uri})`;
    };
    const step = (testCaseStartedId: string, testStepId: string) => {
      const testCase = testCases.get(started.get(testCaseStartedId)!)!;
      const testStep = testCase.testSteps.find((s) => s.id === testStepId)!;
      if (testStep.pickleStepId) {
        const text = pickles.get(testCase.pickleId)!.steps.find((s) => s.id === testStep.pickleStepId)!.text;
        return `step "${text}"`;
      }
      const hook = hooks.get(testStep.hookId!)!;
      const where = `${hook.sourceReference.uri}:${hook.sourceReference.location?.line}`;
      return hook.name ? `hook "${hook.name}" (${where})` : `hook (${where})`;
    };

    on('message', (envelope) => {
      if (envelope.pickle) pickles.set(envelope.pickle.id, envelope.pickle);
      else if (envelope.hook) hooks.set(envelope.hook.id, envelope.hook);
      else if (envelope.testCase) testCases.set(envelope.testCase.id, envelope.testCase);
      else if (envelope.testRunStarted) watchdog.activity('the run started');
      else if (envelope.testCaseStarted) {
        const { id, testCaseId } = envelope.testCaseStarted;
        started.set(id, testCaseId);
        watchdog.activity(`${scenario(id)} started`);
        write(`${new Date().toISOString()}  ${scenario(id)}\n`);
      } else if (envelope.testStepStarted) {
        const { testCaseStartedId, testStepId } = envelope.testStepStarted;
        watchdog.activity(`${step(testCaseStartedId, testStepId)} of ${scenario(testCaseStartedId)} started`);
      } else if (envelope.testStepFinished) {
        const { testCaseStartedId, testStepId } = envelope.testStepFinished;
        watchdog.activity(`${step(testCaseStartedId, testStepId)} of ${scenario(testCaseStartedId)} finished`);
      } else if (envelope.testCaseFinished) {
        watchdog.activity(`${scenario(envelope.testCaseFinished.testCaseStartedId)} finished`);
      } else if (envelope.testRunFinished) {
        watchdog.activity('the last scenario finished, and the run should have exited');
      }
    });
  },
};
