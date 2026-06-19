import { test } from 'node:test';
import assert from 'node:assert/strict';
import { queryPrefill } from './query-prefill.js';

const inputs = [
  { name: 'expr', source: 'field', elementId: 'in-expr' },
  { name: 'mode', source: 'field', elementId: 'in-mode' },
  { name: 'unix', source: 'clock', elementId: 'in-unix' },
  { name: 'image', source: 'file', elementId: 'in-image' },
];

test('prefills only field inputs present in the query', () => {
  const r = queryPrefill(inputs, '?expr=2%2B2&mode=decode&unix=123&nope=x');
  assert.deepEqual(r.fields, [
    { elementId: 'in-expr', value: '2+2' },
    { elementId: 'in-mode', value: 'decode' },
  ]);
  assert.equal(r.url, null); // no ?url=; clock + unknown params ignored
});

test('captures ?url= for media tools', () => {
  const r = queryPrefill(inputs, '?url=https%3A%2F%2Fx.test%2Fcat.jpg&fit=cover');
  assert.equal(r.url, 'https://x.test/cat.jpg');
  assert.deepEqual(r.fields, []); // 'fit' isn't a declared input here
});

test('empty search yields nothing', () => {
  const r = queryPrefill(inputs, '');
  assert.deepEqual(r.fields, []);
  assert.equal(r.url, null);
});
