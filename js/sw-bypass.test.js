import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

// The runtime Service Worker (sw.js) must NOT intercept /tools/* — those are
// lightweight standalone pages that must be served as static assets, never
// booted through the wafer/solobase runtime. solobase injects this bypass from
// solobase.toml [assets].extra_bypass_prefix into sw.js via __EXTRA_BYPASS__.
// Requires `solobase build` to have produced pkg/sw.js (CI builds before tests).
const swPath = fileURLToPath(new URL('../pkg/sw.js', import.meta.url));

test('sw.js bypasses /tools/ so tool pages stay static', () => {
  assert.ok(existsSync(swPath), 'pkg/sw.js missing — run `solobase build` first');
  const src = readFileSync(swPath, 'utf8');
  assert.match(
    src,
    /startsWith\(['"]\/tools\/['"]\)/,
    'sw.js is missing the /tools/ bypass — check extra_bypass_prefix in solobase.toml',
  );
});

test('sw.js bypasses /ffmpeg-engine.js and /ffmpeg.js so the chat ffmpeg bridge loads statically', () => {
  assert.ok(existsSync(swPath), 'pkg/sw.js missing — run `solobase build` first');
  const src = readFileSync(swPath, 'utf8');
  assert.match(
    src,
    /startsWith\(['"]\/ffmpeg-engine\.js['"]\)/,
    'sw.js is missing the /ffmpeg-engine.js bypass — check extra_bypass_prefix in solobase.toml',
  );
  assert.match(
    src,
    /startsWith\(['"]\/ffmpeg\.js['"]\)/,
    'sw.js is missing the /ffmpeg.js bypass — check extra_bypass_prefix in solobase.toml',
  );
});
