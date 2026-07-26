import { test, expect } from './fixtures';

const CSV = 'text,label\nbuy now,spam\nhello,ham\nhi there,ham\nsee you,ham';

// Oversample, ratio 1.0: the single spam row is duplicated up to the ham count
// (3). Only one minority row exists, so the result is seed-independent: the
// originals stay in file order and the two duplicates are appended.
const OVERSAMPLED_CSV = `text,label
buy now,spam
hello,ham
hi there,ham
see you,ham
buy now,spam
buy now,spam`;

test('class-rebalancer oversamples the minority class to balance', async ({ page }) => {
  await page.goto('/tools/class-rebalancer/');
  await page.fill('#in-data', CSV);
  await page.fill('#in-label_column', 'label');
  await page.selectOption('#in-strategy', 'oversample');

  await expect(page.locator('#tool-output')).toHaveText(OVERSAMPLED_CSV, { timeout: 15000 });
});

test('class-rebalancer deep-link pre-fills params and returns a before/after summary', async ({ page }) => {
  const params = new URLSearchParams({
    data: CSV,
    label_column: 'label',
    strategy: 'oversample',
    target_ratio: '1',
    header: 'true',
    shuffle: 'false',
    seed: '42',
    output: 'summary',
  });

  await page.goto(`/tools/class-rebalancer/?${params.toString()}`);
  await expect(page.locator('#in-strategy')).toHaveValue('oversample');
  await expect(page.locator('#in-output')).toHaveValue('summary');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('"label": "spam", "before": 1, "after": 3', { timeout: 15000 });
  await expect(output).toContainText('"total_before": 4');
  await expect(output).toContainText('"total_after": 6');
});
