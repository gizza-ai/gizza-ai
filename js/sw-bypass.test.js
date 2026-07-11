import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

// The runtime Service Worker (sw.js) must NOT intercept /tools/* — those are
// lightweight standalone pages that must be served as static assets, never
// booted through the wafer/solobase runtime. solobase injects this bypass from
// solobase.toml [assets].extra_bypass_prefix into sw.js via __EXTRA_BYPASS__.
//
// pkg/sw.js is a `solobase build` artifact, so these are post-build
// integration tests: lanes without a built pkg/ (PR CI, fresh clones) skip
// them, and lanes that build pkg/ set REQUIRE_BUILT_PKG=1 so a missing
// artifact fails loudly instead of skipping.
const swPath = fileURLToPath(new URL('../pkg/sw.js', import.meta.url));
const skip =
  !existsSync(swPath) && !process.env.REQUIRE_BUILT_PKG
    ? 'pkg/sw.js not built — run `solobase build` (REQUIRE_BUILT_PKG=1 makes this a failure)'
    : false;

test('sw.js bypasses /tools/ so tool pages stay static', { skip }, () => {
  assert.ok(existsSync(swPath), 'pkg/sw.js missing — run `solobase build` first');
  const src = readFileSync(swPath, 'utf8');
  assert.match(
    src,
    /startsWith\(['"]\/tools\/['"]\)/,
    'sw.js is missing the /tools/ bypass — check extra_bypass_prefix in solobase.toml',
  );
});

test('sw.js bypasses /ffmpeg-engine.js and /ffmpeg.js so the chat ffmpeg bridge loads statically', { skip }, () => {
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
