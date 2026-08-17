import { test, expect } from './fixtures';

const sample = 'id,name\n42,ada\n7,linus\n12345,grace';

test('zero-pad-ids page pads a named CSV column exactly', async ({ page }) => {
  await page.goto('/tools/zero-pad-ids/');
  await page.fill('#in-input', sample);
  await page.fill('#in-delimiter', 'comma');
  await page.fill('#in-columns', 'id');
  await page.fill('#in-width', '5');
  await page.selectOption('#in-mode', 'pad');
  await page.selectOption('#in-overflow', 'keep');
  await page.selectOption('#in-non_numeric', 'keep');
  await page.check('#in-header');
  await page.selectOption('#in-quote_style', 'minimal');
  await expect(page.locator('#tool-output')).toHaveText('id,name\n00042,ada\n00007,linus\n12345,grace', { timeout: 15_000 });
});

test('zero-pad-ids deep link strips zeros and reflects checkbox state', async ({ page }) => {
  const params = new URLSearchParams({
    input: '00042\n00007',
    delimiter: 'auto',
    columns: '',
    width: '0',
    mode: 'strip',
    overflow: 'keep',
    non_numeric: 'keep',
    header: 'false',
    quote_style: 'minimal',
  });
  await page.goto(`/tools/zero-pad-ids/?${params.toString()}`);
  await expect(page.locator('#in-input')).toHaveValue('00042\n00007', { timeout: 15_000 });
  await expect(page.locator('#in-mode')).toHaveValue('strip');
  await expect(page.locator('#in-header')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('42\n7', { timeout: 15_000 });
});

test('zero-pad-ids quote-style always preserves padded IDs as text fields', async ({ page }) => {
  await page.goto('/tools/zero-pad-ids/');
  await page.fill('#in-input', 'sku,price\n1234,9.99\n77,14.50');
  await page.fill('#in-delimiter', 'comma');
  await page.fill('#in-columns', 'sku');
  await page.fill('#in-width', '8');
  await page.selectOption('#in-mode', 'pad');
  await page.selectOption('#in-overflow', 'keep');
  await page.selectOption('#in-non_numeric', 'keep');
  await page.check('#in-header');
  await page.selectOption('#in-quote_style', 'always');
  await expect(page.locator('#tool-output')).toHaveText('"sku","price"\n"00001234","9.99"\n"00000077","14.50"', { timeout: 15_000 });
});
