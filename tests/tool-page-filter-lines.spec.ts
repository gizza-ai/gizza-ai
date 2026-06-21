import { test, expect } from './fixtures';

const SAMPLE = 'INFO ok\nERROR boom\nwarn low\nERROR again\ndebug x';

// /tools/filter-lines/ filters lines in-browser (pure wasm).
test('filter-lines keeps lines matching a substring', async ({ page }) => {
  await page.goto('/tools/filter-lines/');
  await page.fill('#in-text', SAMPLE);
  await page.fill('#in-pattern', 'ERROR');
  await page.selectOption('#in-mode', 'keep');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('ERROR boom\nERROR again', { timeout: 15000 });
});

test('filter-lines drops lines matching a regex', async ({ page }) => {
  await page.goto('/tools/filter-lines/');
  await page.fill('#in-text', SAMPLE);
  await page.fill('#in-pattern', '^(INFO|debug)');
  await page.selectOption('#in-mode', 'drop');
  await page.check('#in-regex');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('ERROR boom\nwarn low\nERROR again', { timeout: 15000 });
});
