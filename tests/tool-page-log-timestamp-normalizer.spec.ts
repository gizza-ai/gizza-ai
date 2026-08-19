import { test, expect } from './fixtures';

const MIXED = `2023-12-01T10:15:30Z INFO start
1701425735123 WARN cache miss
Dec  1 10:16:20 web01 nginx: GET /health 200`;

async function runWasm(
  page: any,
  log = MIXED,
  outputFormat = 'iso8601',
  outputTimezone = 'UTC',
  assumeTimezone = 'UTC',
  assumeYear = '0',
  sort = 'input',
  outputMode = 'replace',
  delta = 'true',
  deltaFormat = 'auto',
  gapThresholdSeconds = '0',
  unmatched = 'keep',
  summary = 'false',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/log-timestamp-normalizer/gizza_ai_log_timestamp_normalizer_web.js');
    await mod.default('/tools/log-timestamp-normalizer/gizza_ai_log_timestamp_normalizer_web_bg.wasm');
    return mod.run(
      args.log,
      args.outputFormat,
      args.outputTimezone,
      args.assumeTimezone,
      args.assumeYear,
      args.sort,
      args.outputMode,
      args.delta,
      args.deltaFormat,
      args.gapThresholdSeconds,
      args.unmatched,
      args.summary,
    );
  }, { log, outputFormat, outputTimezone, assumeTimezone, assumeYear, sort, outputMode, delta, deltaFormat, gapThresholdSeconds, unmatched, summary });
}

test('log-timestamp-normalizer page normalizes mixed formats and deltas', async ({ page }) => {
  await page.goto('/tools/log-timestamp-normalizer/');
  await page.fill('#in-log', MIXED);

  const output = page.locator('#tool-output');
  await expect(output).toContainText('2023-12-01T10:15:30+00:00 INFO start  (start)', { timeout: 20_000 });
  await expect(output).toContainText('2023-12-01T10:15:35+00:00 WARN cache miss  (+5.123s)');
  await expect(output).toContainText('2023-12-01T10:16:20+00:00 web01 nginx: GET /health 200  (+44.877s)');
});

test('log-timestamp-normalizer deep link covers non-default controls and gap marking', async ({ page }) => {
  const params = new URLSearchParams({
    log: `== deploy ==\n2023-12-01T10:15:30Z step one\n2023-12-01T10:20:31Z step two`,
    output_format: 'datetime',
    output_timezone: 'America/New_York',
    assume_timezone: 'UTC',
    assume_year: '0',
    sort: 'oldest',
    output_mode: 'prefix',
    delta: 'true',
    delta_format: 'milliseconds',
    gap_threshold_seconds: '60',
    unmatched: 'mark',
    summary: 'true',
  });
  await page.goto(`/tools/log-timestamp-normalizer/?${params.toString()}`);

  await expect(page.locator('#in-output_format')).toHaveValue('datetime', { timeout: 15_000 });
  await expect(page.locator('#in-output_mode')).toHaveValue('prefix');
  await expect(page.locator('#in-summary')).toBeChecked();
  const output = page.locator('#tool-output');
  await expect(output).toContainText('# 3 lines · 2 timestamps · 1 without one', { timeout: 20_000 });
  await expect(output).toContainText('== deploy ==  (no timestamp)');
  await expect(output).toContainText('2023-12-01 05:20:31  2023-12-01T10:20:31Z step two  (+301000ms GAP)');
});

test('log-timestamp-normalizer wasm covers formats, sorting, options and errors', async ({ page }) => {
  await page.goto('/tools/log-timestamp-normalizer/');

  expect((await runWasm(page)).trimEnd()).toBe(`2023-12-01T10:15:30+00:00 INFO start  (start)
2023-12-01T10:15:35+00:00 WARN cache miss  (+5.123s)
2023-12-01T10:16:20+00:00 web01 nginx: GET /health 200  (+44.877s)`);

  expect((await runWasm(page, '2023-12-01T10:15:30Z one', 'epoch_seconds', 'UTC', 'UTC', '0', 'input', 'timestamp', 'false')).trim()).toBe('1701425730');
  expect((await runWasm(page, '2023-12-01T10:15:30Z one', 'epoch_millis', 'UTC', 'UTC', '0', 'input', 'timestamp', 'false')).trim()).toBe('1701425730000');
  expect(await runWasm(page, '2023-12-01T10:15:30Z one', 'rfc2822', 'UTC', 'UTC', '0', 'input', 'timestamp', 'false')).toContain('Fri, 1 Dec 2023 10:15:30 +0000');

  const sorted = await runWasm(page, `2023-12-01T10:15:35Z second
2023-12-01T10:15:30Z first`, 'iso8601', 'UTC', 'UTC', '0', 'oldest', 'timestamp', 'false');
  expect(sorted.trimEnd()).toBe(`2023-12-01T10:15:30+00:00
2023-12-01T10:15:35+00:00`);

  await expect(runWasm(page, '2023-12-01T10:15:30Z x', 'iso8601', 'Mars/Olympus')).rejects.toThrow(/unknown output_timezone/);
  await expect(runWasm(page, '   ')).rejects.toThrow(/no log text/);
});

test('log-timestamp-normalizer ships a clean runnable CLI example', async ({ page }) => {
  await page.goto('/tools/log-timestamp-normalizer/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool log-timestamp-normalizer');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
