import { test, expect } from './fixtures';

const output = (page) =>
  page.locator('#tool-output').evaluate((el) => el.textContent?.trim() ?? '');

test('correlated-feature-pruner drops a redundant numeric column', async ({ page }) => {
  await page.goto('/tools/correlated-feature-pruner/');
  await page.fill('#in-data', '1,2,5\n2,4,3\n3,6,9\n4,8,1');
  await page.fill('#in-labels', 'a,a2,b');
  await expect(page.locator('#tool-output')).toContainText('Dropped (1)', { timeout: 15000 });
  expect(await output(page)).toBe(
    'Kept 2 of 3 columns (threshold |r| > 0.90, pearson).\n\n' +
      'Kept (2): a, b\n' +
      'Dropped (1):\n' +
      '  a2 (|r|=1.00 with a)\n\n' +
      'Pruned data:\n' +
      'a,b\n' +
      '1,5\n' +
      '2,3\n' +
      '3,9\n' +
      '4,1'
  );
});

test('correlated-feature-pruner deep-link supports spearman and labels', async ({ page }) => {
  const params = new URLSearchParams({
    data: '1,1\n2,8\n3,27\n4,64',
    threshold: '0.95',
    method: 'spearman',
    labels: 'x,x3',
  });
  await page.goto(`/tools/correlated-feature-pruner/?${params.toString()}`);
  await expect(page.locator('#in-method')).toHaveValue('spearman');
  await expect(page.locator('#tool-output')).toContainText('x3 (|r|=1.00 with x)', { timeout: 15000 });
  expect(await output(page)).toContain('Kept 1 of 2 columns (threshold |r| > 0.95, spearman).');
  expect(await output(page)).toContain('Pruned data:\nx\n1\n2\n3\n4');
});
