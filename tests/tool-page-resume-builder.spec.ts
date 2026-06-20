import { test, expect } from './fixtures';
test('resume-builder page renders Markdown', async ({ page }) => {
  await page.goto('/tools/resume-builder/');
  await page.fill('#in-data', '{"name":"Ada","title":"Engineer","skills":["Algorithms"]}');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('# Ada', { timeout: 15000 });
  await expect(out).toContainText('## Skills');
});
test('resume-builder query-param deep-link', async ({ page }) => {
  await page.goto('/tools/resume-builder/?data=' + encodeURIComponent('{"name":"Bob"}'));
  await expect(page.locator('#in-data')).toHaveValue('{"name":"Bob"}', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('# Bob', { timeout: 15000 });
});
