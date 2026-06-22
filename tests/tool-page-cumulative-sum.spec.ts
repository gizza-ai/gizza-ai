import { test, expect } from './fixtures';

test('cumulative-sum page shows a running total by default', async ({ page }) => {
  await page.goto('/tools/cumulative-sum/');

  await page.fill('#in-numbers', '1\n2\n3\n4');
  await expect(page.locator('#tool-output')).toHaveText(
    'value | cumulative_sum\n1 | 1\n2 | 3\n3 | 6\n4 | 10',
    { timeout: 15000 },
  );
});

test('cumulative-sum page adds running avg/min/max when checked', async ({ page }) => {
  await page.goto('/tools/cumulative-sum/');

  await page.fill('#in-numbers', '3, 1, 4');
  await page.check('#in-average');
  await page.check('#in-min');
  await page.check('#in-max');
  await expect(page.locator('#tool-output')).toHaveText(
    'value | cumulative_sum | running_avg | running_min | running_max\n' +
      '3 | 3 | 3 | 3 | 3\n' +
      '1 | 4 | 2 | 1 | 3\n' +
      '4 | 8 | 2.6666666667 | 1 | 4',
    { timeout: 15000 },
  );
});

test('cumulative-sum page reports a non-numeric value', async ({ page }) => {
  await page.goto('/tools/cumulative-sum/');
  await page.fill('#in-numbers', '1, two, 3');
  await expect(page.locator('#tool-output')).toContainText('not a valid number', {
    timeout: 15000,
  });
});

test('cumulative-sum query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto(
    '/tools/cumulative-sum/?numbers=' +
      encodeURIComponent('10,20,30') +
      '&average=true',
  );
  await expect(page.locator('#in-numbers')).toHaveValue('10,20,30', { timeout: 15000 });
  await expect(page.locator('#in-average')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText(
    'value | cumulative_sum | running_avg\n10 | 10 | 10\n20 | 30 | 15\n30 | 60 | 20',
    { timeout: 15000 },
  );
});
