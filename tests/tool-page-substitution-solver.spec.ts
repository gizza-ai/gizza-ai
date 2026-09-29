import { test, expect } from './fixtures';

const atbashKey = 'zyxwvutsrqponmlkjihgfedcba';

test('substitution-solver decodes a supplied Atbash key', async ({ page }) => {
  await page.goto('/tools/substitution-solver/');
  await page.fill('#in-text', 'Gsv jfrxp yildm ulc.');
  await page.selectOption('#in-mode', 'decode');
  await page.fill('#in-key', atbashKey);
  await expect(page.locator('#tool-output')).toContainText('Decoded with the key you supplied', {
    timeout: 15000,
  });
  await expect(page.locator('#tool-output')).toContainText('The quick brown fox.');
});

test('substitution-solver analyze mode reports frequency evidence', async ({ page }) => {
  await page.goto('/tools/substitution-solver/');
  await page.fill('#in-text', 'Qda coazqapq cilox fk ifufkc ifap klq fk kauao sziifkc.');
  await page.selectOption('#in-mode', 'analyze');
  await expect(page.locator('#tool-output')).toContainText('Frequency analysis', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('Index of coincidence');
  await expect(page.locator('#tool-output')).toContainText('Frequency-matched starting key');
});

test('substitution-solver query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto(
    '/tools/substitution-solver/?text=' +
      encodeURIComponent('Gsv jfrxp yildm ulc.') +
      '&mode=decode&key=' +
      atbashKey +
      '&effort=standard&keep_layout=true',
  );
  await expect(page.locator('#in-mode')).toHaveValue('decode', { timeout: 15000 });
  await expect(page.locator('#in-key')).toHaveValue(atbashKey);
  await expect(page.locator('#tool-output')).toContainText('The quick brown fox.', { timeout: 15000 });
});

test('substitution-solver grouped output checkbox path', async ({ page }) => {
  await page.goto('/tools/substitution-solver/');
  await page.fill('#in-text', 'Gsv jfrxp!');
  await page.selectOption('#in-mode', 'decode');
  await page.fill('#in-key', atbashKey);
  await page.uncheck('#in-keep_layout');
  await expect(page.locator('#tool-output')).toContainText('THEQU ICK', { timeout: 15000 });
});
