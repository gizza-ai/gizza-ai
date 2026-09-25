import { test, expect } from './fixtures';

const spikeSeries = '10 11 9 10 12 10 11 40 10 11';

async function runWasm(
  page: any,
  series = spikeSeries,
  method = 'rolling_z',
  window = '5',
  min_periods = '3',
  threshold = '3',
  warn_threshold = '2',
  period = '7',
  tolerance = '0',
  direction = 'both',
  center = 'false',
  only_anomalies = 'false',
  decimals = '4',
  output = 'json',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/anomaly-timeseries/gizza_ai_anomaly_timeseries_web.js');
    await mod.default('/tools/anomaly-timeseries/gizza_ai_anomaly_timeseries_web_bg.wasm');
    return mod.run(
      args.series,
      args.method,
      args.window,
      args.min_periods,
      args.threshold,
      args.warn_threshold,
      args.period,
      args.tolerance,
      args.direction,
      args.center,
      args.only_anomalies,
      args.decimals,
      args.output,
    );
  }, { series, method, window, min_periods, threshold, warn_threshold, period, tolerance, direction, center, only_anomalies, decimals, output });
}

test('anomaly-timeseries page flags a rolling z-score spike', async ({ page }) => {
  await page.goto('/tools/anomaly-timeseries/');
  await page.fill('#in-series', spikeSeries);
  await page.selectOption('#in-method', 'rolling_z');
  await page.fill('#in-window', '5');
  await page.fill('#in-min_periods', '3');
  await page.fill('#in-threshold', '3');
  await page.fill('#in-warn_threshold', '2');
  await page.selectOption('#in-output', 'table');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('anomalies 1', { timeout: 15_000 });
  await expect(out).toContainText('8   40');
  await expect(out).toContainText('critical');
  await expect(out).toContainText('watch band');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool anomaly-timeseries');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('anomaly-timeseries deep-link prefills and returns CSV anomalies only', async ({ page }) => {
  const params = new URLSearchParams({
    series: spikeSeries,
    method: 'rolling_z',
    window: '5',
    min_periods: '3',
    threshold: '3',
    warn_threshold: '2',
    period: '7',
    tolerance: '0',
    direction: 'both',
    center: 'false',
    only_anomalies: 'true',
    decimals: '2',
    output: 'csv',
  });
  await page.goto(`/tools/anomaly-timeseries/?${params.toString()}`);
  await expect(page.locator('#in-series')).toHaveValue(spikeSeries, { timeout: 15_000 });
  await expect(page.locator('#in-output')).toHaveValue('csv');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('index,label,value,expected,deviation,score,lower,upper,severity,anomaly,rule', { timeout: 15_000 });
  await expect(out).toContainText('5,,12,10,2,2.45,7.55,12.45,warning,false,rolling');
  await expect(out).toContainText('8,,40,10.4,29.6,25.96,6.98,13.82,critical,true,rolling');
});

test('anomaly-timeseries wasm covers methods, direction and errors', async ({ page }) => {
  await page.goto('/tools/anomaly-timeseries/');
  await page.waitForSelector('#in-series');

  const robust = JSON.parse(await runWasm(page, '10 11 10 9 10 11 60 10 9 11 10 10 55', 'rolling_mad', '6', '3'));
  expect(robust.anomaly_indices).toContain(7);
  expect(robust.anomaly_indices).toContain(13);
  expect(robust.points[12].rule).toBe('rolling');

  const belowOnly = await runWasm(page, spikeSeries, 'rolling_z', '5', '3', '3', '2', '7', '0', 'below', 'false', 'false', '4', 'table');
  expect(belowOnly).toContain('anomalies 0');
  expect(belowOnly).not.toContain('critical,true');

  const seasonal = JSON.parse(await runWasm(page, '100 200 300 101 201 301 99 199 299 100 200 300 180 200 300', 'seasonal_z', '12', '0', '3', '2', '3', '0', 'both', 'false', 'false', '2', 'json'));
  expect(seasonal.anomaly_indices).toEqual([13]);
  expect(seasonal.points[12].rule).toBe('seasonal');

  await expect(runWasm(page, '1 2 3', 'prophet')).rejects.toThrow(/rolling_z, rolling_mad, seasonal_z, combined/);
});
