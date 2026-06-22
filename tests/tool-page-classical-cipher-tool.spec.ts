import { test, expect } from './fixtures';

test('classical-cipher-tool caesar encrypt (default select=caesar/encrypt)', async ({ page }) => {
  await page.goto('/tools/classical-cipher-tool/');
  await page.fill('#in-key', '3');
  await page.fill('#in-text', 'Hello, World!');
  await expect(page.locator('#tool-output')).toHaveText('Khoor, Zruog!', {
    timeout: 15000,
  });
});

test('classical-cipher-tool vigenere round-trips', async ({ page }) => {
  await page.goto('/tools/classical-cipher-tool/');
  await page.selectOption('#in-cipher', 'vigenere');
  await page.fill('#in-key', 'LEMON');
  await page.fill('#in-text', 'ATTACKATDAWN');
  await expect(page.locator('#tool-output')).toHaveText('LXFOPVEFRNHR', {
    timeout: 15000,
  });
  // Decrypt recovers the plaintext.
  await page.selectOption('#in-operation', 'decrypt');
  await page.fill('#in-text', 'LXFOPVEFRNHR');
  await expect(page.locator('#tool-output')).toHaveText('ATTACKATDAWN', {
    timeout: 15000,
  });
});

test('classical-cipher-tool atbash is keyless', async ({ page }) => {
  await page.goto('/tools/classical-cipher-tool/');
  await page.selectOption('#in-cipher', 'atbash');
  await page.fill('#in-text', 'Hello');
  await expect(page.locator('#tool-output')).toHaveText('Svool', {
    timeout: 15000,
  });
});

test('classical-cipher-tool caesar brute-force lists shift 3', async ({ page }) => {
  await page.goto('/tools/classical-cipher-tool/');
  await page.selectOption('#in-operation', 'brute-force');
  await page.fill('#in-text', 'Khoor');
  await expect(page.locator('#tool-output')).toContainText('shift  3: Hello', {
    timeout: 15000,
  });
});

test('classical-cipher-tool query-param deep-link prefills + computes', async ({ page }) => {
  await page.goto(
    '/tools/classical-cipher-tool/?cipher=rail-fence&operation=encrypt&key=3&text=' +
      encodeURIComponent('WEAREDISCOVEREDFLEEATONCE'),
  );
  await expect(page.locator('#in-cipher')).toHaveValue('rail-fence', { timeout: 15000 });
  await expect(page.locator('#in-text')).toHaveValue('WEAREDISCOVEREDFLEEATONCE');
  await expect(page.locator('#tool-output')).toHaveText('WECRLTEERDSOEEFEAOCAIVDEN', {
    timeout: 15000,
  });
});
