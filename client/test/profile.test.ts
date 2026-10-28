import { test } from 'node:test';
import assert from 'node:assert/strict';
import { profileValues } from '../src/profile.ts';

test('rejects incomplete daylight-saving-day profiles', () => {
  assert.throws(() => profileValues(Array(24).fill('10'), 25, 'NOMINT'), /25 hourly/);
});
test('rejects blank, fractional, negative and unsafe hourly quantities', () => {
  for (const value of ['', '2.1', '-1', '9007199254740992']) {
    assert.throws(() => profileValues([value], 1, 'NOMINT'), /whole/);
  }
});
test('programme profile uses a complete set of zero hours', () => {
  assert.deepEqual(profileValues(Array(23).fill('22'), 23, 'PRODOC'), Array(23).fill(0));
});
test('preserves complete hourly nomination edits', () => {
  assert.deepEqual(profileValues(['0', '15', '90'], 3, 'NOMINT'), [0, 15, 90]);
});
