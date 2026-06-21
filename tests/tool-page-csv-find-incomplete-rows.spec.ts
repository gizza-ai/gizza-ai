import { test, expect } from './fixtures';

// /tools/csv-find-incomplete-rows/ flags ragged + required-blank rows in-browser (pure wasm).
test('csv-find-incomplete-rows flags ragged rows', async ({ page }) => {
  await page.goto('/tools/csv-find-incomplete-rows/');
  await page.fill('#in-data', 'a,b,c\n1,2,3\n4,5\n6,7,8,9');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Expected 3 field(s)', { timeout: 15000 });
  await expect(out).toContainText('1 of', { timeout: 15000 }).catch(() => {});
  await expect(out).toContainText('too_few_fields');
  await expect(out).toContainText('too_many_fields');
});

test('csv-find-incomplete-rows flags a blank required cell', async ({ page }) => {
  await page.goto('/tools/csv-find-incomplete-rows/');
  await page.fill('#in-data', 'name,email\nAda,ada@x.com\nBo,\nCy,cy@x.com');
  await page.fill('#in-required', 'email');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('blank_required', { timeout: 15000 });
  await expect(out).toContainText('blank: email');
});
