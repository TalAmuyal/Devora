export default {
  default: {
    paths: ['tests/features/'],
    import: ['tests/steps/*.ts', 'tests/support/*.ts'],
    format: ['progress-bar'],
    publishQuiet: true,
  },
};
