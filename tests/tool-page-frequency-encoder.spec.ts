import { test, expect } from './fixtures';

const CSV = 'product_id,price\nX,10\nY,20\nX,30\nZ,40\nX,50\nY,60';

const COUNT_REPLACE = `product_id,price
3,10
2,20
3,30
1,40
3,50
2,60`;

const FREQ_APPEND = `product_id,price,product_id_freq
X,10,0.5000
Y,20,0.3333
X,30,0.5000
Z,40,0.1667
X,50,0.5000
Y,60,0.3333`;

const POOLED_DEEP_LINK = `product_id,price,product_id_count
X,10,3
X,20,3
X,30,3
Y,40,4
Y,50,4
Z,60,4
W,70,4`;

test('frequency-encoder replaces a categorical column with counts', async ({ page }) => {
  await page.goto('/tools/frequency-encoder/');
  await page.fill('#in-data', CSV);
  await page.fill('#in-column', 'product_id');
  await page.selectOption('#in-mode', 'count');
  await page.selectOption('#in-output', 'replace');

  await expect(page.locator('#tool-output')).toHaveText(COUNT_REPLACE, { timeout: 15_000 });
});

test('frequency-encoder appends frequency shares', async ({ page }) => {
  await page.goto('/tools/frequency-encoder/');
  await page.fill('#in-data', CSV);
  await page.fill('#in-column', 'product_id');
  await page.selectOption('#in-mode', 'frequency');
  await page.selectOption('#in-output', 'append');

  await expect(page.locator('#tool-output')).toHaveText(FREQ_APPEND, { timeout: 15_000 });
});

test('frequency-encoder deep link pools rare values and toggles non-default controls', async ({ page }) => {
  const params = new URLSearchParams({
    data: 'product_id,price\nX,10\nX,20\nX,30\nY,40\nY,50\nZ,60\nW,70',
    column: 'product_id',
    mode: 'count',
    output: 'append',
    blank: 'count',
    min_count: '3',
    case_sensitive: 'false',
    decimals: '4',
    has_header: 'true',
    delimiter: 'comma',
  });

  await page.goto(`/tools/frequency-encoder/?${params.toString()}`);
  await expect(page.locator('#in-output')).toHaveValue('append');
  await expect(page.locator('#in-min_count')).toHaveValue('3');
  await expect(page.locator('#in-case_sensitive')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText(POOLED_DEEP_LINK, { timeout: 15_000 });
});
