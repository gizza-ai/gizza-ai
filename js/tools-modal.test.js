import { test } from 'node:test';
import assert from 'node:assert/strict';
import { filterTools } from '../site/tools-modal.js';

const LIST = [
  { slug: 'calculator', title: 'Free Online Calculator', description: 'Evaluate math expressions' },
  { slug: 'clock', title: 'Current UTC Time', description: 'Live timestamp' },
];

test('empty query returns the full list', () => {
  assert.deepEqual(filterTools(LIST, ''), LIST);
  assert.deepEqual(filterTools(LIST, '   '), LIST);
});

test('matches on title, case-insensitive', () => {
  const r = filterTools(LIST, 'CALC');
  assert.equal(r.length, 1);
  assert.equal(r[0].slug, 'calculator');
});

test('matches on description', () => {
  const r = filterTools(LIST, 'timestamp');
  assert.equal(r.length, 1);
  assert.equal(r[0].slug, 'clock');
});

test('matches on slug even when title/description do not', () => {
  // clock's title is "Current UTC Time" — only the slug contains "clock".
  const r = filterTools(LIST, 'clock');
  assert.equal(r.length, 1);
  assert.equal(r[0].slug, 'clock');
});

test('no match returns empty array', () => {
  assert.deepEqual(filterTools(LIST, 'zzznope'), []);
});
