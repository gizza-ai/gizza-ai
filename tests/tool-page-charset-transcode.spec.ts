import { test, expect } from './fixtures';

test('charset-transcode fixes windows-1252 mojibake', async ({ page }) => {
  await page.goto('/tools/charset-transcode/');
  await page.fill('#in-text', 'cafÃ©');
  await page.fill('#in-from', 'windows-1252');
  // Default errors = replace (select).
  await expect(page.locator('#tool-output')).toHaveText('café', {
    timeout: 15000,
  });
});

test('charset-transcode auto-detects the charset', async ({ page }) => {
  await page.goto('/tools/charset-transcode/');
  await page.fill('#in-text', 'cafÃ©');
  await page.fill('#in-from', 'auto');
  await expect(page.locator('#tool-output')).toHaveText('café', {
    timeout: 15000,
  });
});

test('charset-transcode passes un-nests double mojibake', async ({ page }) => {
  await page.goto('/tools/charset-transcode/');
  await page.fill('#in-text', 'cafÃƒÂ©'); // "café" mojibaked twice
  await page.fill('#in-from', 'windows-1252');
  await page.fill('#in-passes', '2');
  await expect(page.locator('#tool-output')).toHaveText('café', {
    timeout: 15000,
  });
});

test('charset-transcode latin1 alias re-decodes bytes', async ({ page }) => {
  await page.goto('/tools/charset-transcode/');
  await page.fill('#in-text', 'naÃ¯ve rÃ©sumÃ©');
  await page.fill('#in-from', 'latin1');
  await expect(page.locator('#tool-output')).toHaveText('naïve résumé', {
    timeout: 15000,
  });
});

test('charset-transcode strict mode surfaces a wrong-charset error', async ({ page }) => {
  await page.goto('/tools/charset-transcode/');
  // "é" re-encodes to byte 0xE9 (not valid UTF-8); in strict mode the repair
  // can't apply, so the wrong-charset guess is reported as an error.
  await page.fill('#in-text', 'é');
  await page.fill('#in-from', 'windows-1252');
  await page.selectOption('#in-errors', 'strict');
  await expect(page.locator('#tool-output')).toContainText("not the right 'from'", {
    timeout: 15000,
  });
});

test('charset-transcode query-param deep-link prefills + computes', async ({ page }) => {
  await page.goto(
    '/tools/charset-transcode/?text=' +
      encodeURIComponent('cafÃ©') +
      '&from=windows-1252',
  );
  await expect(page.locator('#in-text')).toHaveValue('cafÃ©', { timeout: 15000 });
  await expect(page.locator('#in-from')).toHaveValue('windows-1252');
  await expect(page.locator('#tool-output')).toHaveText('café', {
    timeout: 15000,
  });
});
