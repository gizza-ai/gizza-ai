import { test, expect } from './fixtures';
import * as fs from 'fs';
import * as path from 'path';

// Proves real ffmpeg runs in a PAGE context (where import()/Worker work, unlike
// the Service Worker) via ffmpeg-engine.js — the mechanism the chat ffmpeg bridge
// relies on. Hosted on a static, SW-bypassed tool page so the test does NOT depend
// on the chat runtime booting: the chat's <h1> comes from an external CDN web
// component (sa-header), which is flaky in headless/sandboxed runs. The
// ffmpeg-engine.js code path is identical in any page context (and tool pages
// already run ffmpeg). The SW<->page postMessage plumbing is covered by the node
// round-trip unit test; the full LLM-driven chat path is a manual smoke (Task 8).
test('ffmpeg runs page-side via ffmpeg-engine.js', async ({ page }) => {
  test.setTimeout(120_000); // first run downloads the @ffmpeg core from the CDN

  await page.goto('/tools/calculator/', { waitUntil: 'domcontentloaded' });

  const pngB64 = fs
    .readFileSync(path.resolve(__dirname, 'fixtures/red-2x2.png'))
    .toString('base64');

  const resp = await page.evaluate(async (b64) => {
    const m = await import('/ffmpeg-engine.js');
    const inputs_json = JSON.stringify([{ name: 'in.png', bytes_b64: b64 }]);
    const args_json = JSON.stringify(['-i', 'in.png', '-vf', 'format=gray', 'out.png']);
    return await m.runFfmpegRequest({ args_json, inputs_json, output_name: 'out.png' });
  }, pngB64);

  expect(resp.exit_code).toBe(0);
  expect(resp.output_b64.length).toBeGreaterThan(0);
});
