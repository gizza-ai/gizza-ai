import { test, expect } from './fixtures';

const tool = '/tools/time-series-generator/';

async function outputText(page): Promise<string> {
  const text = await page.locator('#tool-output').textContent();
  return (text ?? '').trim();
}

test('time-series-generator page emits exact deterministic CSV', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-start', '2024-01-01');
  await page.fill('#in-interval', '1d');
  await page.fill('#in-count', '3');
  await page.fill('#in-base', '10');
  await page.selectOption('#in-trend', 'linear');
  await page.fill('#in-trend_strength', '2');
  await page.selectOption('#in-seasonality', 'none');
  await page.selectOption('#in-noise', 'none');
  await page.fill('#in-decimals', '1');
  await page.selectOption('#in-output', 'csv');
  await page.selectOption('#in-timestamp_format', 'date');
  await page.fill('#in-labels', 'value');

  await expect(page.locator('#tool-output')).toContainText('timestamp,value', { timeout: 15000 });
  expect(await outputText(page)).toBe('timestamp,value\n2024-01-01,10.0\n2024-01-02,12.0\n2024-01-03,14.0');
});

test('time-series-generator page supports JSON, multiple series and no header checkbox', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-count', '2');
  await page.fill('#in-base', '5');
  await page.selectOption('#in-trend', 'none');
  await page.selectOption('#in-seasonality', 'none');
  await page.selectOption('#in-noise', 'none');
  await page.fill('#in-series', '2');
  await page.fill('#in-labels', 'a,b');
  await page.selectOption('#in-output', 'json');
  await page.selectOption('#in-timestamp_format', 'index');
  await page.uncheck('#in-header');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"labels"', { timeout: 15000 });
  const parsed = JSON.parse(await outputText(page));
  expect(parsed.labels).toEqual(['a', 'b']);
  expect(parsed.rows[0]).toEqual({ index: 0, a: 5, b: 5 });
});

test('time-series-generator query-param deep-link can generate stats with non-default rates', async ({ page }) => {
  await page.goto(
    tool +
      '?count=12&output=stats&noise=uniform&noise_level=1&missing_rate=0.25&outlier_rate=0.2&outlier_direction=up&series=2&labels=' +
      encodeURIComponent('sensor_a,sensor_b') +
      '&seed=99',
  );

  await expect(page.locator('#in-output')).toHaveValue('stats', { timeout: 15000 });
  await expect(page.locator('#in-missing_rate')).toHaveValue('0.25');
  await expect(page.locator('#in-outlier_direction')).toHaveValue('up');
  const out = await outputText(page);
  expect(out).toContain('12 point(s) x 2 series');
  expect(out).toContain('sensor_a');
  expect(out).toContain('outliers');
});

test('time-series-generator page surfaces validation errors', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-count', '0');
  const out = page.locator('#tool-output');
  await expect(out).toHaveClass(/error/, { timeout: 15000 });
  await expect(out).toContainText('count must be between 1');
});
