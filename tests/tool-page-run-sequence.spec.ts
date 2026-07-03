// Run-sequence guard (site/tool.js run()): overlapping ffmpeg runs must not
// let a stale slow run overwrite a newer result (or repaint after Reset).
// Driven on trim-audio, but the behavior under test is generic tool.js.
//
// ffmpeg.js is stubbed via route interception, so completion order is
// controlled causally (the stale run only resolves after the newer run's
// result has been consumed) — no real ffmpeg, no CDN, no timing guesses.
import { test, expect } from './fixtures';
import path from 'node:path';

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-3s.mp3');

// The fixture page is shared across every test in the worker (fixtures.ts
// reuses one persistent-context page), and page.route registrations OUTLIVE
// the test that added them. Without this cleanup, alphabetically-later specs
// (e.g. tool-page-trim-audio) load the gated stub instead of the real
// ffmpeg.js — whose release gate belongs to a dead document — and hang at
// "Processing…" until their 90 s expect timeout.
test.afterEach(async ({ page }) => {
  await page.unrouteAll({ behavior: 'ignoreErrors' });
});

// Base64 payloads distinguish which run's output landed in media.src.
const STALE_B64 = 'U1RBTEU='; // "STALE"
const FRESH_B64 = 'RlJFU0g='; // "FRESH"

// Stub for the stale-slow-run test. Runs with end=1.5 in their argv are the
// "stale" run and block until a "fresh" run (end=2) has completed AND the
// page has had a macrotask to consume its result; then the stale run
// resolves and, unguarded, would overwrite the fresh output.
const STALE_RUN_STUB = `
let release;
const released = new Promise((r) => { release = r; });
window.__staleResolved = false;
export async function ffmpegExec(argsJson, inputsJson, outputName) {
  if (argsJson.includes('1.5')) {
    await released;
    window.__staleResolved = true;
    return { exit_code: 0, output_b64: '${STALE_B64}', log: '' };
  }
  setTimeout(release, 0); // let the fresh result's microtask chain flush first
  return { exit_code: 0, output_b64: '${FRESH_B64}', log: '' };
}
`;

// Stub for the Reset test: every call blocks until the test releases it.
const GATED_STUB = `
window.__gateResolved = false;
const gate = new Promise((r) => { window.__releaseFfmpeg = () => { r(); }; });
export async function ffmpegExec(argsJson, inputsJson, outputName) {
  await gate;
  window.__gateResolved = true;
  return { exit_code: 0, output_b64: '${STALE_B64}', log: '' };
}
`;

test('a stale slow run must not overwrite a newer result', async ({ page }) => {
  await page.route('**/tools/trim-audio/ffmpeg.js', (route) =>
    route.fulfill({ contentType: 'text/javascript', body: STALE_RUN_STUB })
  );
  await page.goto('/tools/trim-audio/');
  await page.waitForSelector('#in-audio');
  await page.fill('#in-start', '0.5');
  await page.fill('#in-end', '1.5');
  await page.setInputFiles('#in-audio', FIXTURE); // run 1 (stale, gated)
  await page.fill('#in-end', '2'); // run 2 (fresh, completes first)

  const media = page.locator('#tool-output-media');
  await expect(media).toHaveAttribute('src', `data:audio/mpeg;base64,${FRESH_B64}`, {
    timeout: 15_000,
  });
  // Let the stale run resolve and (if unguarded) do its damage.
  await page.waitForFunction(() => (window as any).__staleResolved === true);
  await page.waitForTimeout(300);
  await expect(media).toHaveAttribute('src', `data:audio/mpeg;base64,${FRESH_B64}`);
  const dl = page.locator('#tool-output-download');
  await expect(dl).toHaveAttribute('href', `data:audio/mpeg;base64,${FRESH_B64}`);
});

test('a run resolving after Reset must not repaint the cleared output', async ({ page }) => {
  await page.route('**/tools/trim-audio/ffmpeg.js', (route) =>
    route.fulfill({ contentType: 'text/javascript', body: GATED_STUB })
  );
  await page.goto('/tools/trim-audio/');
  await page.waitForSelector('#in-audio');
  await page.fill('#in-start', '0.5');
  await page.fill('#in-end', '1.5');
  await page.setInputFiles('#in-audio', FIXTURE); // run in flight, gated
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('Processing…');

  await page.click('#tool-reset'); // clears file + output while run in flight
  await page.evaluate(() => (window as any).__releaseFfmpeg());
  await page.waitForFunction(() => (window as any).__gateResolved === true);
  await page.waitForTimeout(300);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeHidden();
  await expect(page.locator('#tool-output-download')).toBeHidden();
  await expect(out).toHaveText('');
});
