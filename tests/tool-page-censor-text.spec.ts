import { test, expect } from './fixtures';
test('censor-text page masks supplied words', async ({ page }) => {
  await page.goto('/tools/censor-text/');
  await page.fill('#in-text', 'foo and bar');
  await page.fill('#in-words', 'foo,bar');
  await expect(page.locator('#tool-output')).toHaveText('*** and ***', { timeout: 15000 });
});
test('censor-text query-param deep-link', async ({ page }) => {
  await page.goto('/tools/censor-text/?text=' + encodeURIComponent('oh damn') + '&words=damn');
  await expect(page.locator('#in-text')).toHaveValue('oh damn', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText('oh ****', { timeout: 15000 });
});
