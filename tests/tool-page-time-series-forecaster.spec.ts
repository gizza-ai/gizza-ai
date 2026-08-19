import { test, expect } from './fixtures';

const tool = '/tools/time-series-forecaster/';

async function setTextarea(locator, value: string) {
  await locator.evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v as string;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page,
  data: string,
  model = 'auto',
  horizon = '6',
  seasonLength = '0',
  alpha = '0',
  beta = '0',
  gamma = '0',
  phi = '0',
  confidence = '95',
  showFitted = 'false',
  header = 'auto',
  decimals = '3',
  format = 'text',
): Promise<string> {
  return await page.evaluate(
    async ({
      data,
      model,
      horizon,
      seasonLength,
      alpha,
      beta,
      gamma,
      phi,
      confidence,
      showFitted,
      header,
      decimals,
      format,
    }) => {
      const mod = await import('/tools/time-series-forecaster/gizza_ai_time_series_forecaster_web.js');
      await mod.default('/tools/time-series-forecaster/gizza_ai_time_series_forecaster_web_bg.wasm');
      return mod.run(
        data,
        model,
        horizon,
        seasonLength,
        alpha,
        beta,
        gamma,
        phi,
        confidence,
        showFitted,
        header,
        decimals,
        format,
      );
    },
    { data, model, horizon, seasonLength, alpha, beta, gamma, phi, confidence, showFitted, header, decimals, format },
  );
}

test('time-series-forecaster page renders a Holt trend forecast', async ({ page }) => {
  await page.goto(tool);
  await setTextarea(page.locator('#in-data'), 'month,sales\nJan,120\nFeb,132\nMar,141\nApr,158\nMay,166\nJun,181\nJul,190\nAug,203');
  await page.selectOption('#in-model', 'holt');
  await page.fill('#in-horizon', '3');
  await page.selectOption('#in-confidence', '95');
  await page.fill('#in-decimals', '2');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Holt linear trend', { timeout: 15_000 });
  await expect(out).toContainText('Forecast');
  await expect(out).toContainText('period');
  await expect(out).toContainText('lower');
  await expect(out).toContainText('upper');
});

test('time-series-forecaster deep link pre-fills and runs JSON output', async ({ page }) => {
  const qs = new URLSearchParams({
    data: '10\n12\n11\n14\n15\n17\n16\n19\n21\n22\n24\n26',
    model: 'damped',
    horizon: '5',
    season_length: '0',
    confidence: '90',
    show_fitted: 'true',
    header: 'auto',
    decimals: '3',
    format: 'json',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-data')).toHaveValue('10\n12\n11\n14\n15\n17\n16\n19\n21\n22\n24\n26', { timeout: 15_000 });
  await expect(page.locator('#in-model')).toHaveValue('damped');
  await expect(page.locator('#in-horizon')).toHaveValue('5');
  await expect(page.locator('#in-confidence')).toHaveValue('90');
  await expect(page.locator('#in-show_fitted')).toBeChecked();
  await expect(page.locator('#in-format')).toHaveValue('json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"model"');
  await expect(out).toContainText('"forecast"');
  await expect(out).toContainText('"fitted"');
});

test('time-series-forecaster wasm covers models, formats, boundaries, and validation', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-data');

  const trend = '10\n12\n14\n16\n18\n20\n22\n24';
  for (const model of ['auto', 'simple', 'holt', 'damped']) {
    const out = await runWasm(page, trend, model, '2', '0', '0', '0', '0', '0', '80', 'false', 'auto', '2', 'text');
    expect(out).toContain('Forecast');
    expect(out).toContain('lower');
  }

  const seasonal = '180\n240\n300\n210\n200\n260\n330\n230\n220\n290\n360\n250';
  expect(await runWasm(page, seasonal, 'holt-winters-additive', '4', '4', '0', '0', '0', '0', '99', 'false', 'auto', '1', 'csv')).toContain('forecast');
  const multiplicative = await runWasm(page, seasonal, 'holt-winters-multiplicative', '4', '4', '0', '0', '0', '0', '95', 'false', 'auto', '2', 'json');
  expect(JSON.parse(multiplicative).forecast).toHaveLength(4);

  const fitted = await runWasm(page, 'Jan,100\nFeb,110\nMar,120\nApr,130', 'holt', '1', '0', '0.5', '0.2', '0', '0', '90', 'true', 'no', '10', 'json');
  expect(JSON.parse(fitted).fitted.length).toBeGreaterThan(0);

  const maxHorizon = JSON.parse(await runWasm(page, trend, 'simple', '240', '0', '0', '0', '0', '0', '95', 'false', 'auto', '0', 'json'));
  expect(maxHorizon.forecast).toHaveLength(240);

  await expect(runWasm(page, '1\n2', 'auto')).rejects.toThrow(/need at least 3 observations/);
  await expect(runWasm(page, '1\n2\n3\n4', 'holt-winters-additive', '2', '4')).rejects.toThrow(/at least two full cycles/);
  await expect(runWasm(page, '1\n0\n2\n3\n1\n4\n2\n5', 'holt-winters-multiplicative', '2', '4')).rejects.toThrow(/strictly positive/);
});

test('time-series-forecaster ships example chips', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(4);
  await expect(page.locator('.tool-example-chip')).toContainText([
    'Monthly sales with a trend',
    'Quarterly demand with seasonality',
    'Flat series, damped trend',
    'Auto-select, JSON output',
  ]);
});
