import { test } from 'node:test';
import assert from 'node:assert/strict';
import { key } from '../src/api.ts';

test('command keys work on LAN HTTP without crypto.randomUUID', () => {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'crypto');
  const webCrypto = globalThis.crypto;
  Object.defineProperty(globalThis, 'crypto', {
    configurable: true,
    value: { getRandomValues: webCrypto.getRandomValues.bind(webCrypto) },
  });
  try {
    const keys = Array.from({ length: 100 }, () => key());
    for (const value of keys) {
      assert.match(value, /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
    }
    assert.equal(new Set(keys).size, keys.length);
  } finally {
    if (descriptor) Object.defineProperty(globalThis, 'crypto', descriptor);
  }
});
