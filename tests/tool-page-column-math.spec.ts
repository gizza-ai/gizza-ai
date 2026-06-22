import { test, expect } from './fixtures';

test('column-math page adds two columns by default', async ({ page }) => {
  await page.goto('/tools/column-math/');

  await page.fill('#in-a', '1\n2\n3');
  await page.fill('#in-b', '4\n5\n6');
  // Default operation is add (select=add).
  await expect(page.locator('#tool-output')).toHaveText('5\n7\n9', {
    timeout: 15000,
  });
});

test('column-math page divides element-wise', async ({ page }) => {
  await page.goto('/tools/column-math/');
  await page.fill('#in-a', '10, 9');
  await page.fill('#in-b', '2, 3');
  await page.selectOption('#in-operation', 'divide');
  await expect(page.locator('#tool-output')).toHaveText('5\n3', {
    timeout: 15000,
  });
});

test('column-math page reports a length mismatch', async ({ page }) => {
  await page.goto('/tools/column-math/');
  await page.fill('#in-a', '1,2,3');
  await page.fill('#in-b', '4,5');
  await expect(page.locator('#tool-output')).toContainText('same length', {
    timeout: 15000,
  });
});

test('column-math query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto(
    '/tools/column-math/?a=' +
      encodeURIComponent('2,3') +
      '&b=' +
      encodeURIComponent('4,5') +
      '&operation=multiply',
  );
  await expect(page.locator('#in-a')).toHaveValue('2,3', { timeout: 15000 });
  await expect(page.locator('#in-operation')).toHaveValue('multiply');
  await expect(page.locator('#tool-output')).toHaveText('8\n15', {
    timeout: 15000,
  });
});
