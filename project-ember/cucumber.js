export default {
  default: {
    paths: ['tests/features/', '../acceptance/features/'],
    tags: 'not @sapphire-only',
    import: ['tests/steps/*.ts', 'tests/support/*.ts'],
    format: ['progress-bar'],
    publishQuiet: true,
  },
};
