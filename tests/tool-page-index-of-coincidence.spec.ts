import { test, expect } from './fixtures';

test('index-of-coincidence page computes a normalized IC', async ({ page }) => {
  await page.goto('/tools/index-of-coincidence/');

  await page.fill(
    '#in-text',
    'The Index of Coincidence is a statistic used in cryptanalysis to measure the unevenness of a letter distribution.',
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Index of Coincidence (normalized):', {
    timeout: 15000,
  });
  // English prose lands well above the random value of ~1.0.
  await expect(out).toContainText('monoalphabetic');
});

test('index-of-coincidence period analysis + counts', async ({ page }) => {
  await page.goto('/tools/index-of-coincidence/');

  await page.fill('#in-text', 'LXFOPVEFRNHRLXFOPVEFRNHRLXFOPVEFRNHR');
  await page.fill('#in-max_period', '4');
  await page.check('#in-show_counts');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Key-length (period) estimation', {
    timeout: 15000,
  });
  await expect(out).toContainText('Letter frequencies:');
  await expect(out).toContainText('Likely key length:');
});

test('index-of-coincidence query-param deep-link prefills + computes', async ({
  page,
}) => {
  await page.goto(
    '/tools/index-of-coincidence/?text=' + encodeURIComponent('HELLO WORLD'),
  );
  await expect(page.locator('#in-text')).toHaveValue('HELLO WORLD', {
    timeout: 15000,
  });
  await expect(page.locator('#tool-output')).toContainText(
    'Index of Coincidence (normalized):',
    { timeout: 15000 },
  );
});
