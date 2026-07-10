import { test } from 'node:test';
import assert from 'node:assert/strict';
import { buildManifestSets, classifyToolsPath } from './tools-guard.js';

const manifests = buildManifestSets({
  tools: [{ slug: 'calculator' }, { slug: 'word-count' }],
  hubs: [{ slug: 'audio' }, { slug: 'audio-convert' }],
  pairs: [{ path: 'audio-convert/mp3-to-wav' }],
});

test('non-tools paths are never guarded', () => {
  assert.equal(classifyToolsPath('/', manifests), 'pass');
  assert.equal(classifyToolsPath('/chat', manifests), 'pass');
  assert.equal(classifyToolsPath('/toolsy/thing/', manifests), 'pass');
});

test('the tools hub itself passes', () => {
  assert.equal(classifyToolsPath('/tools/', manifests), 'pass');
  assert.equal(classifyToolsPath('/tools/index.html', manifests), 'pass');
});

test('known tool slugs pass at any depth', () => {
  assert.equal(classifyToolsPath('/tools/calculator/', manifests), 'pass');
  assert.equal(classifyToolsPath('/tools/calculator', manifests), 'pass');
  assert.equal(classifyToolsPath('/tools/calculator/tool.json', manifests), 'pass');
  assert.equal(classifyToolsPath('/tools/calculator/index.md', manifests), 'pass');
});

test('known category hubs pass', () => {
  assert.equal(classifyToolsPath('/tools/audio/', manifests), 'pass');
});

test('known pair pages under a hub pass', () => {
  assert.equal(classifyToolsPath('/tools/audio-convert/mp3-to-wav/', manifests), 'pass');
  assert.equal(
    classifyToolsPath('/tools/audio-convert/mp3-to-wav/index.md', manifests),
    'pass'
  );
});

test('unknown pair pages under a known hub 404', () => {
  assert.equal(
    classifyToolsPath('/tools/audio-convert/flac-to-midi/', manifests),
    'not-found'
  );
});

test('unknown slugs 404', () => {
  assert.equal(classifyToolsPath('/tools/definitely-not-a-tool-xyz/', manifests), 'not-found');
  assert.equal(classifyToolsPath('/tools/word-counter/', manifests), 'not-found');
});

test('manifest files and dotted files under /tools/ pass', () => {
  assert.equal(classifyToolsPath('/tools/_index.json', manifests), 'pass');
  assert.equal(classifyToolsPath('/tools/_hubs.json', manifests), 'pass');
  assert.equal(classifyToolsPath('/tools/_pairs.json', manifests), 'pass');
});

test('percent-encoded segments are decoded before matching', () => {
  assert.equal(classifyToolsPath('/tools/word%2Dcount/', manifests), 'pass');
  assert.equal(classifyToolsPath('/tools/nope%20nope/', manifests), 'not-found');
});

test('empty manifests fail open (deploy-ordering safety)', () => {
  const empty = buildManifestSets({ tools: [], hubs: [], pairs: [] });
  assert.equal(classifyToolsPath('/tools/anything-at-all/', empty), 'pass');
});
