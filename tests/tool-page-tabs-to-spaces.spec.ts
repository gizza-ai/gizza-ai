import { test, expect } from './fixtures';

test('tabs-to-spaces expands tabs with real tab stops', async ({ page }) => {
  await page.goto('/tools/tabs-to-spaces/');
  await page.fill('#in-text', 'ab\tcd\n\tindent');
  await expect(page.locator('#tool-output')).toHaveText('ab  cd\n    indent', { timeout: 15000 });
});

test('tabs-to-spaces unexpands spaces back to tabs', async ({ page }) => {
  await page.goto('/tools/tabs-to-spaces/');
  await page.fill('#in-text', '    code\n      more');
  await page.selectOption('#in-direction', 'unexpand');
  await expect(page.locator('#tool-output')).toHaveText('\tcode\n\t  more', { timeout: 15000 });
});

test('tabs-to-spaces leading scope leaves inline tabs untouched', async ({ page }) => {
  await page.goto('/tools/tabs-to-spaces/');
  await page.fill('#in-text', '\tname\t= value');
  await page.selectOption('#in-scope', 'leading');
  await expect(page.locator('#tool-output')).toHaveText('    name\t= value', { timeout: 15000 });
});

test('tabs-to-spaces deep-links pre-fill and auto-run', async ({ page }) => {
  await page.goto('/tools/tabs-to-spaces/?text=ab%09cd&tab_width=4&direction=expand&scope=all');
  await expect(page.locator('#tool-output')).toHaveText('ab  cd', { timeout: 15000 });
});
