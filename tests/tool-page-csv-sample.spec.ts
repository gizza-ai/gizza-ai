import { test, expect } from './fixtures';

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

const DATA = 'name,group\nAlice,east\nBob,west\nCarol,east\nDan,west';

test('csv-sample top method returns header plus first N rows', async ({ page }) => {
  await page.goto('/tools/csv-sample/');
  await page.fill('#in-data', DATA);
  await page.selectOption('#in-method', 'top');
  await page.fill('#in-n', '2');
  await expect(page.locator('#tool-output')).toHaveText('name,group\nAlice,east\nBob,west', { timeout: 15000 });
});

test('csv-sample bottom method returns last N rows', async ({ page }) => {
  await page.goto('/tools/csv-sample/');
  await page.fill('#in-data', DATA);
  await page.selectOption('#in-method', 'bottom');
  await page.fill('#in-n', '2');
  await expect(page.locator('#tool-output')).toHaveText('name,group\nCarol,east\nDan,west', { timeout: 15000 });
});

test('csv-sample stratified sampling keeps groups represented', async ({ page }) => {
  await page.goto('/tools/csv-sample/');
  await page.fill('#in-data', DATA);
  await page.selectOption('#in-method', 'stratified');
  await page.fill('#in-n', '2');
  await page.fill('#in-stratify_column', 'group');
  await page.fill('#in-seed', '1');
  await expect(page.locator('#tool-output')).toHaveText('name,group\nAlice,east\nBob,west', { timeout: 15000 });
});

test('csv-sample percent overrides n at boundary 100', async ({ page }) => {
  await page.goto('/tools/csv-sample/');
  await page.fill('#in-data', DATA);
  await page.selectOption('#in-method', 'top');
  await page.fill('#in-n', '1');
  await page.fill('#in-percent', '100');
  await expect(page.locator('#tool-output')).toHaveText(DATA, { timeout: 15000 });
});

test('csv-sample supports non-default checkbox and tab delimiter', async ({ page }) => {
  await page.goto('/tools/csv-sample/');
  await page.fill('#in-data', 'A\tx\nB\ty\nC\tz');
  await page.selectOption('#in-method', 'top');
  await page.fill('#in-n', '2');
  await page.uncheck('#in-header');
  await page.selectOption('#in-delimiter', 'tab');
  await expect(page.locator('#in-header')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('A\tx\nB\ty', { timeout: 15000 });
});

test('csv-sample deep-links pre-fill params and auto-run', async ({ page }) => {
  const params = new URLSearchParams({
    data: DATA,
    method: 'bottom',
    n: '1',
    percent: '0',
    seed: '42',
    header: 'true',
    delimiter: 'comma',
  });
  await page.goto(`/tools/csv-sample/?${params.toString()}`);
  await expect(page.locator('#in-data')).toHaveValue(DATA, { timeout: 15000 });
  await expect(page.locator('#in-method')).toHaveValue('bottom');
  await expect(page.locator('#tool-output')).toHaveText('name,group\nDan,west', { timeout: 15000 });
});
