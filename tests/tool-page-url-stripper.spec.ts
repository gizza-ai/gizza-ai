import { test, expect } from './fixtures';

test('url-stripper removes urls and tidies prose by default', async ({ page }) => {
  await page.goto('/tools/url-stripper/');
  await page.fill(
    '#in-input',
    'Visit https://example.com for docs, then see www.example.org.',
  );
  await expect(page.locator('#tool-output')).toHaveText(
    'Visit for docs, then see.',
    { timeout: 15000 },
  );
});

test('url-stripper deep link removes emails and uses a placeholder', async ({ page }) => {
  const qs =
    '?input=' + encodeURIComponent('Mail bob@example.com or visit https://example.com/page.') +
    '&remove_emails=true' +
    '&replacement=' + encodeURIComponent('[removed]');
  await page.goto('/tools/url-stripper/' + qs);
  await expect(page.locator('#in-input')).toHaveValue(
    'Mail bob@example.com or visit https://example.com/page.',
    { timeout: 15000 },
  );
  await expect(page.locator('#in-remove_emails')).toBeChecked();
  await expect(page.locator('#in-replacement')).toHaveValue('[removed]');
  await expect(page.locator('#tool-output')).toHaveText(
    'Mail [removed] or visit [removed].',
    { timeout: 15000 },
  );
});

test('url-stripper can leave www links when remove_www is unchecked', async ({ page }) => {
  await page.goto('/tools/url-stripper/');
  await page.fill(
    '#in-input',
    'Main site www.example.com but tracker https://track.example.com/a goes.',
  );
  await page.uncheck('#in-remove_www');
  await expect(page.locator('#tool-output')).toHaveText(
    'Main site www.example.com but tracker goes.',
    { timeout: 15000 },
  );
});
