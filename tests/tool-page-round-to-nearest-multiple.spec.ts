import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

const PRICE_INPUT = 'name,price\nApple,1.23\nPear,2.42';
const PRICE_EXPECTED = 'name,price\nApple,1.25\nPear,2.4';

test('round-to-nearest-multiple page — rounds prices to nearest 0.05', async ({ page }) => {
  await page.goto('/tools/round-to-nearest-multiple/');
  await page.fill('#in-data', PRICE_INPUT);
  await page.fill('#in-step', '0.05');
  await page.selectOption('#in-mode', 'half_up');
  await expect(page.locator('#tool-output')).toContainText('Apple,1.25', { timeout: 15000 });
  expect(await outputText(page)).toBe(PRICE_EXPECTED);
});

test('round-to-nearest-multiple page — advertised rounding modes', async ({ page }) => {
  await page.goto('/tools/round-to-nearest-multiple/');
  const cases = [
    ['half_up', 'value\n0.125\n-1500', 'value\n0.15\n-1500'],
    ['half_down', 'value\n0.125\n-1500', 'value\n0.1\n-1500'],
    ['half_even', 'value\n2.5\n7.5', 'value\n0\n10'],
    ['ceil', 'value\n41\n-2.1', 'value\n45\n0'],
    ['floor', 'value\n49\n-2.1', 'value\n45\n-5'],
    ['truncate', 'value\n7.9\n-7.9', 'value\n5\n-5'],
  ];
  for (const [mode, input, expected] of cases) {
    await page.fill('#in-data', input);
    await page.fill('#in-step', mode === 'half_up' || mode === 'half_down' ? '0.05' : '5');
    await page.selectOption('#in-mode', mode);
    await expect(page.locator('#tool-output')).toContainText(expected.split('\n')[1], { timeout: 15000 });
    expect(await outputText(page)).toBe(expected);
  }
});

test('round-to-nearest-multiple page — selected column, semicolon delimiter, and fixed zeros checkbox', async ({ page }) => {
  await page.goto('/tools/round-to-nearest-multiple/');
  await page.fill('#in-data', 'name;price;qty\nApple;1.2;7\nPear;1.4;11');
  await page.fill('#in-step', '0.25');
  await page.fill('#in-columns', 'price');
  await page.fill('#in-delimiter', 'semicolon');
  await page.check('#in-trailing_zeros');
  await expect(page.locator('#tool-output')).toContainText('Apple;1.25;7', { timeout: 15000 });
  expect(await outputText(page)).toBe('name;price;qty\nApple;1.25;7\nPear;1.50;11');
});

test('round-to-nearest-multiple page — query-param deep-link prefills and auto-runs', async ({ page }) => {
  await page.goto(
    '/tools/round-to-nearest-multiple/?data=' +
      encodeURIComponent(PRICE_INPUT) +
      '&step=0.05&mode=half_up&columns=&header=true&delimiter=' +
      encodeURIComponent(',') +
      '&trailing_zeros=false',
  );
  await expect(page.locator('#in-data')).toHaveValue(PRICE_INPUT, { timeout: 15000 });
  await expect(page.locator('#in-mode')).toHaveValue('half_up');
  await expect(page.locator('#tool-output')).toContainText('Apple,1.25', { timeout: 15000 });
  expect(await outputText(page)).toBe(PRICE_EXPECTED);
});
