import { test, expect } from './fixtures';

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

const SERIES = '104 102 99.5 98.5 97 101 104.5 107 110.5 107.5 103 102.5 ' +
  '110 108 105.5 104.5 103 107 110.5 113 116.5 113.5 109 108.5';

test('renders an SVG decomposition with real panels and metadata', async ({ page }) => {
  await page.goto('/tools/ts-decompose/');
  await page.fill('#in-data', SERIES);
  await page.fill('#in-period', '12');
  await page.fill('#in-title', 'Monthly sales');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('<svg', { timeout: 15_000 });
  await expect(out).toContainText('Monthly sales');
  await expect(out).toContainText('Observed');
  await expect(out).toContainText('Trend');
  await expect(out).toContainText('Seasonal');
  await expect(out).toContainText('Residual');
});

test('CSV output is exact for a known monthly series and precision', async ({ page }) => {
  await page.goto('/tools/ts-decompose/');
  await page.fill('#in-data', SERIES);
  await page.fill('#in-period', '12');
  await page.fill('#in-precision', '2');
  await page.selectOption('#in-output', 'csv');

  await expect(page.locator('#tool-output')).toContainText('seasonally_adjusted', { timeout: 15_000 });
  expect((await output(page)).split('\n').slice(0, 4).join('\n')).toBe(
    'index,label,observed,trend,seasonal,residual,seasonally_adjusted\n' +
      '1,,104,101.07,2.69,0.24,101.31\n' +
      '2,,102,101.45,0.31,0.24,101.69\n' +
      '3,,99.5,101.84,-2.58,0.24,102.08',
  );
});

test('covers enum choices, color control, and non-default checkbox states', async ({ page }) => {
  await page.goto('/tools/ts-decompose/');
  await page.fill('#in-data', SERIES);
  await page.fill('#in-period', '12');
  await page.selectOption('#in-method', 'classical');
  await page.selectOption('#in-model', 'additive');
  await page.selectOption('#in-residual_style', 'line');
  await page.selectOption('#in-theme', 'dark');
  await page.fill('#in-color', '#f00');
  await page.uncheck('#in-two_sided');
  await page.uncheck('#in-extrapolate_trend');
  await page.uncheck('#in-grid');
  await page.check('#in-show_adjusted');
  await page.selectOption('#in-output', 'json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"method":"classical"', { timeout: 15_000 });
  await expect(out).toContainText('"period":12');
  await expect(out).toContainText('"points"');
});

test('deep link pre-fills and auto-runs table output', async ({ page }) => {
  const qs = new URLSearchParams({
    data: SERIES,
    period: '12',
    output: 'table',
    precision: '1',
    title: 'Deep link series',
  });
  await page.goto(`/tools/ts-decompose/?${qs.toString()}`);

  await expect(page.locator('#in-period')).toHaveValue('12', { timeout: 15_000 });
  await expect(page.locator('#in-output')).toHaveValue('table');
  await expect(page.locator('#tool-output')).toContainText('Seasonal indices');
  await expect(page.locator('#tool-output')).toContainText('strength of trend');
});
