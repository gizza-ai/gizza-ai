import { test, expect } from './fixtures';

// /tools/descriptive-stats/ computes stats in-browser (pure wasm).
test('descriptive-stats reports mean/median/mode/std dev', async ({ page }) => {
  await page.goto('/tools/descriptive-stats/');
  await page.fill('#in-numbers', '2, 4, 4, 4, 5, 5, 7, 9');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('mean: 5', { timeout: 15000 });
  await expect(out).toContainText('median: 4.5');
  await expect(out).toContainText('mode: 4');
  await expect(out).toContainText('std dev (pop): 2');
});
