import { test, expect } from './fixtures';

// /tools/levels-adjust/ applies a Levels transfer curve to numeric samples in-browser.
test('levels-adjust stretches input black/white points to output range', async ({ page }) => {
  await page.goto('/tools/levels-adjust/');
  await page.fill('#in-values', '64, 128, 192');
  await page.fill('#in-input_black', '64');
  await page.fill('#in-input_white', '192');
  await page.fill('#in-gamma', '1');
  await page.fill('#in-output_black', '0');
  await page.fill('#in-output_white', '255');

  await expect(page.locator('#tool-output')).toHaveText(
    'levels: input 64–192, gamma 1, output 0–255\n\n0\n127.5\n255',
    { timeout: 15000 },
  );
});

test('levels-adjust deep-link inverts output levels', async ({ page }) => {
  await page.goto(
    '/tools/levels-adjust/?values=' +
      encodeURIComponent('0, 128, 255') +
      '&input_black=0&input_white=255&gamma=1&output_black=255&output_white=0&clamp=true',
  );

  await expect(page.locator('#in-values')).toHaveValue('0, 128, 255', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText(
    'levels: input 0–255, gamma 1, output 255–0\n\n255\n127\n0',
    { timeout: 15000 },
  );
});

test('levels-adjust gamma brightens midtones and can lower output white', async ({ page }) => {
  await page.goto('/tools/levels-adjust/');
  await page.fill('#in-values', '0, 64, 128, 192, 255');
  await page.fill('#in-gamma', '2');
  await page.fill('#in-output_white', '200');

  await expect(page.locator('#tool-output')).toHaveText(
    'levels: input 0–255, gamma 2, output 0–200\n\n0\n100.195887\n141.698382\n173.544366\n200',
    { timeout: 15000 },
  );
});

test('levels-adjust supports unclamped extrapolation', async ({ page }) => {
  await page.goto('/tools/levels-adjust/');
  await page.fill('#in-values', '32, 224');
  await page.fill('#in-input_black', '64');
  await page.fill('#in-input_white', '192');
  await page.uncheck('#in-clamp');

  await expect(page.locator('#tool-output')).toHaveText(
    'levels: input 64–192, gamma 1, output 0–255 (unclamped)\n\n-63.75\n318.75',
    { timeout: 15000 },
  );
});
