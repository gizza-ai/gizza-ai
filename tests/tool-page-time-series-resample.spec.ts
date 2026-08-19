import { test, expect } from './fixtures';

async function runWasm(
  page,
  data: string,
  interval = '1h',
  aggregate = 'mean',
  timeColumn = '',
  valueColumns = '',
  label = 'start',
  closed = 'left',
  fill = 'skip',
  origin = 'epoch',
  offset = '',
  timeFormat = 'iso',
  output = 'csv',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/time-series-resample/gizza_ai_time_series_resample_web.js');
    await mod.default('/tools/time-series-resample/gizza_ai_time_series_resample_web_bg.wasm');
    return mod.run(
      args.data,
      args.interval,
      args.aggregate,
      args.timeColumn,
      args.valueColumns,
      args.label,
      args.closed,
      args.fill,
      args.origin,
      args.offset,
      args.timeFormat,
      args.output,
    );
  }, { data, interval, aggregate, timeColumn, valueColumns, label, closed, fill, origin, offset, timeFormat, output });
}

test('time-series-resample wasm computes hourly mean exactly', async ({ page }) => {
  await page.goto('/tools/time-series-resample/');
  const input = 'time,temp\n2024-05-01T10:00:00Z,10\n2024-05-01T10:30:00Z,20\n2024-05-01T11:15:00Z,30';
  const out = await runWasm(page, input);
  expect(out).toBe('time,temp\n2024-05-01T10:00:00Z,15\n2024-05-01T11:00:00Z,30');
});

test('time-series-resample wasm covers advertised enum choices and value forms', async ({ page }) => {
  await page.goto('/tools/time-series-resample/');

  const daily = 'day,sales\n2024-01-01,5\n2024-01-02,7\n2024-01-05,3\n2024-01-08,10\n2024-01-11,4';
  await expect(runWasm(page, daily, '1w', 'sum', '', '', 'start', 'left', 'skip', 'epoch', '', 'date'))
    .resolves.toBe('day,sales\n2024-01-01,15\n2024-01-08,14');

  const ticks = 't,px\n2024-03-04T09:00:00Z,10\n2024-03-04T09:20:00Z,14\n2024-03-04T09:40:00Z,9\n2024-03-04T09:50:00Z,12\n2024-03-04T10:05:00Z,13';
  await expect(runWasm(page, ticks, '1h', 'ohlc'))
    .resolves.toBe('t,px_open,px_high,px_low,px_close\n2024-03-04T09:00:00Z,10,14,9,12\n2024-03-04T10:00:00Z,13,13,13,13');

  const sparse = 't,v\n2024-01-01T00:00:00Z,4\n2024-01-01T03:00:00Z,8';
  await expect(runWasm(page, sparse, '1h', 'mean', '', '', 'end', 'left', 'linear'))
    .resolves.toBe('t,v\n2024-01-01T01:00:00Z,4\n2024-01-01T02:00:00Z,5.3333333333\n2024-01-01T03:00:00Z,6.6666666667\n2024-01-01T04:00:00Z,8');

  await expect(runWasm(page, 'ts,v\n2024-01-04,3\n2024-02-08,7', '1mo', 'sum', '', '', 'start', 'left', 'skip', 'epoch', '', 'date', 'json'))
    .resolves.toContain('"ts": "2024-01-01"');
});

test('time-series-resample page renders exact output and deep-link prefills params', async ({ page }) => {
  const input = 'time,temp\n2024-05-01T10:00:00Z,10\n2024-05-01T10:30:00Z,20\n2024-05-01T11:15:00Z,30';
  await page.goto('/tools/time-series-resample/');
  await page.fill('#in-data', input);
  await page.fill('#in-interval', '1h');
  await page.selectOption('#in-aggregate', 'mean');
  await expect(page.locator('#tool-output')).toHaveText('time,temp\n2024-05-01T10:00:00Z,15\n2024-05-01T11:00:00Z,30', { timeout: 15_000 });

  const qs =
    '?data=' + encodeURIComponent(input) +
    '&interval=1h' +
    '&aggregate=mean' +
    '&label=start' +
    '&closed=left' +
    '&fill=skip' +
    '&origin=epoch' +
    '&time_format=iso' +
    '&output=csv';
  await page.goto('/tools/time-series-resample/' + qs);
  await expect(page.locator('#in-data')).toHaveValue(input, { timeout: 15_000 });
  await expect(page.locator('#in-aggregate')).toHaveValue('mean');
  await expect(page.locator('#tool-output')).toHaveText('time,temp\n2024-05-01T10:00:00Z,15\n2024-05-01T11:00:00Z,30', { timeout: 15_000 });
});
