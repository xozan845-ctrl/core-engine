import { defineSuite } from '../../suite.mjs';

export default defineSuite({
  id: 'intentional-failure',
  area: 'engine',
  mode: 'pure',
  run: ({ test, assert }) => {
    test('la fixture centinela falla de forma deliberada', () => {
      assert.fail('fallo deliberado para probar el exit code del runner');
    });
  },
});
