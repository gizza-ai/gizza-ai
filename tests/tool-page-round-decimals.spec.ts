import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

const DEFAULT_INPUT = 'name,price,qty\nApple,1.005,3.14159\nPear,2.5,10';
const DEFAULT_EXPECTED = 'name,price,qty\nApple,1.01,3.14\nPear,2.5,10';

test('round-decimals page — rounds CSV decimals exactly', async ({ page }) => {
  await page.goto('/tools/round-decimals/');
  await page.fill('#in-data', DEFAULT_INPUT);
  await page.fill('#in-decimals', '2');
  await page.selectOption('#in-mode', 'half_up');
  await expect(page.locator('#tool-output')).toContainText('Apple,1.01,3.14', { timeout: 15000 });
  expect(await outputText(page)).toBe(DEFAULT_EXPECTED);
});

test("round-decimals page — banker's mode and zero-decimal boundary", async ({ page }) => {
  await page.goto('/tools/round-decimals/');
  await page.fill('#in-data', 'value\n0.5\n1.5\n2.5\n3.5');
  await page.fill('#in-decimals', '0');
  await page.selectOption('#in-mode', 'half_even');
  await expect(page.locator('#tool-output')).toContainText('value', { timeout: 15000 });
  expect(await outputText(page)).toBe('value\n0\n2\n2\n4');
});

test('round-decimals page — selected column, semicolon delimiter, and fixed zeros checkbox', async ({ page }) => {
  await page.goto('/tools/round-decimals/');
  await page.fill('#in-data', 'name;price;qty\nApple;1.005;3.14159\nPear;2.555;10');
  await page.fill('#in-decimals', '2');
  await page.fill('#in-columns', 'price');
  await page.fill('#in-delimiter', 'semicolon');
  await page.check('#in-trailing_zeros');
  await expect(page.locator('#tool-output')).toContainText('Apple;1.01;3.14159', { timeout: 15000 });
  expect(await outputText(page)).toBe('name;price;qty\nApple;1.01;3.14159\nPear;2.56;10');
});

test('round-decimals page — query-param deep-link prefills and auto-runs', async ({ page }) => {
  await page.goto(
    '/tools/round-decimals/?data=' +
      encodeURIComponent(DEFAULT_INPUT) +
      '&decimals=2&mode=half_up&columns=&header=true&delimiter=' +
      encodeURIComponent(',') +
      '&trailing_zeros=false',
  );
  await expect(page.locator('#in-data')).toHaveValue(DEFAULT_INPUT, { timeout: 15000 });
  await expect(page.locator('#in-mode')).toHaveValue('half_up');
  await expect(page.locator('#tool-output')).toContainText('Apple,1.01,3.14', { timeout: 15000 });
  expect(await outputText(page)).toBe(DEFAULT_EXPECTED);
});
