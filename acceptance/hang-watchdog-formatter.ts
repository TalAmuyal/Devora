import { hangWatchdog } from './hang-watchdog';

/**
 * A Cucumber formatter that reports the run's progress to the hang watchdog (hang-watchdog.ts): the run's start, each
 * scenario's start and end, and each step's and hook's, by name. Its own output is each scenario as it starts, with the
 * time, so how far a run got shows even when it's stopped.
 * Use: `--format ../acceptance/hang-watchdog-formatter.ts:<file>`.
 */

// The parts of Cucumber's messages it reads (@cucumber/messages).
interface GherkinScenario { id: string; keyword: string }
interface Envelope {
  gherkinDocument?: {
    uri?: string;
    feature?: { name: string; children: { scenario?: GherkinScenario; rule?: { children: { scenario?: GherkinScenario }[] } }[] };
  };
  pickle?: { id: string; name: string; uri: string; astNodeIds: string[]; steps: { id: string; text: string }[] };
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
    // The clock starts with the formatter, so a hang before the first scenario (parsing the features, a BeforeAll hook) is
    // caught too. Only loading the support code comes before it: Cucumber loads it first.
    watchdog.activity('Cucumber started its formatters (it parses the features next, then starts the run)');
    const pickles = new Map<string, NonNullable<Envelope['pickle']>>();
    const hooks = new Map<string, NonNullable<Envelope['hook']>>();
    const testCases = new Map<string, NonNullable<Envelope['testCase']>>();
    const started = new Map<string, string>();
    // The flake report's identity of each test case (scripts/ci/test-outcomes/cucumber.sh): the feature's name, and the
    // scenario's, an outline's row numbered among the rows that run.
    const features = new Map<string, string>();
    const outlines = new Set<string>();
    const rows = new Map<string, number>();
    const identities = new Map<string, { suite: string; name: string }>();

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
      if (envelope.gherkinDocument) {
        const { uri, feature } = envelope.gherkinDocument;
        if (uri && feature) {
          features.set(uri, feature.name);
          const scenarios = feature.children.flatMap((child) => child.rule ? child.rule.children.map((c) => c.scenario) : [child.scenario]);
          for (const s of scenarios) if (s?.keyword === 'Scenario Outline') outlines.add(s.id);
        }
      } else if (envelope.pickle) pickles.set(envelope.pickle.id, envelope.pickle);
      else if (envelope.hook) hooks.set(envelope.hook.id, envelope.hook);
      else if (envelope.testCase) {
        testCases.set(envelope.testCase.id, envelope.testCase);
        const pickle = pickles.get(envelope.testCase.pickleId)!;
        let name = pickle.name;
        if (outlines.has(pickle.astNodeIds[0])) {
          const key = `${pickle.uri}\n${pickle.name}`;
          const row = (rows.get(key) ?? 0) + 1;
          rows.set(key, row);
          name += ` #${row}`;
        }
        identities.set(envelope.testCase.id, { suite: features.get(pickle.uri) ?? pickle.uri, name });
      }
      else if (envelope.testRunStarted) watchdog.activity('the run started (its BeforeAll hooks run next, then the first scenario)');
      else if (envelope.testCaseStarted) {
        const { id, testCaseId } = envelope.testCaseStarted;
        started.set(id, testCaseId);
        watchdog.activity(`${scenario(id)} started`, identities.get(testCaseId));
        write(`${new Date().toISOString()}  ${scenario(id)}\n`);
      } else if (envelope.testStepStarted) {
        const { testCaseStartedId, testStepId } = envelope.testStepStarted;
        watchdog.activity(`${step(testCaseStartedId, testStepId)} of ${scenario(testCaseStartedId)} started`);
      } else if (envelope.testStepFinished) {
        const { testCaseStartedId, testStepId } = envelope.testStepFinished;
        watchdog.activity(`${step(testCaseStartedId, testStepId)} of ${scenario(testCaseStartedId)} finished`);
      } else if (envelope.testCaseFinished) {
        watchdog.activity(`${scenario(envelope.testCaseFinished.testCaseStartedId)} finished`, null);
      } else if (envelope.testRunFinished) {
        watchdog.activity('the last scenario finished, and the run should have exited');
      }
    });
  },
};
