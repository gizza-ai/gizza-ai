import { test, expect } from './fixtures';

// Exact merged-timeline output (multi-line → assert textContent, not toHaveText,
// which collapses whitespace). Two header-delimited sources, default asc order,
// aligned [source] tags on by default.
test('log-merger interleaves two sources into one aligned timeline', async ({ page }) => {
  await page.goto('/tools/log-merger/');
  await page.fill(
    '#in-logs',
    '--- api.log ---\n' +
      '2024-06-01T10:00:02Z GET /users 200\n' +
      '2024-06-01T10:00:05Z GET /orders 200\n' +
      '--- worker.log ---\n' +
      '2024-06-01T10:00:01Z job started\n' +
      '2024-06-01T10:00:04Z job done',
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('worker.log', { timeout: 15000 });
  const expected =
    '[worker.log] 2024-06-01T10:00:01Z job started\n' +
    '[api.log]    2024-06-01T10:00:02Z GET /users 200\n' +
    '[worker.log] 2024-06-01T10:00:04Z job done\n' +
    '[api.log]    2024-06-01T10:00:05Z GET /orders 200';
  expect(await out.textContent()).toBe(expected);
});

// Deep-link: the page pre-fills #in-logs from ?logs= and auto-runs.
test('log-merger deep-link pre-fills and merges', async ({ page }) => {
  const logs =
    '--- a ---\n2024-01-01T00:00:02Z started\n--- b ---\n2024-01-01T00:00:01Z connect';
  await page.goto('/tools/log-merger/?logs=' + encodeURIComponent(logs));
  const out = page.locator('#tool-output');
  await expect(out).toContainText('connect', { timeout: 15000 });
  const expected =
    '[b] 2024-01-01T00:00:01Z connect\n[a] 2024-01-01T00:00:02Z started';
  expect(await out.textContent()).toBe(expected);
});
