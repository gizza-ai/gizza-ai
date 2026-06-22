import { test, expect } from './fixtures';

test('gzip-size-estimator page reports raw + gzipped size', async ({ page }) => {
  await page.goto('/tools/gzip-size-estimator/');

  // Highly repetitive input compresses dramatically.
  await page.fill('#in-input', 'abcabcabc'.repeat(100));
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Raw size:', { timeout: 15000 });
  await expect(out).toContainText('Gzipped size:');
  await expect(out).toContainText('Reduction:');
  await expect(out).toContainText('Compression ratio:');
  await expect(out).toContainText('Brotli size:');
  // 900 raw bytes of "abc" repeats; default level 6.
  await expect(out).toContainText('900 bytes');
  await expect(out).toContainText('[level 6]');
});

test('gzip-size-estimator page honours the level field', async ({ page }) => {
  await page.goto('/tools/gzip-size-estimator/');
  await page.fill('#in-input', 'hello world '.repeat(50));
  await page.fill('#in-level', '9');
  await expect(page.locator('#tool-output')).toContainText('[level 9]', {
    timeout: 15000,
  });
});

test('gzip-size-estimator page reports gzip overhead on tiny input', async ({ page }) => {
  await page.goto('/tools/gzip-size-estimator/');
  await page.fill('#in-input', 'hi');
  await expect(page.locator('#tool-output')).toContainText('gzip overhead', {
    timeout: 15000,
  });
});

test('gzip-size-estimator query-param deep-link prefills + computes', async ({ page }) => {
  await page.goto(
    '/tools/gzip-size-estimator/?input=' +
      encodeURIComponent('xyzxyzxyz'.repeat(20)),
  );
  await expect(page.locator('#in-input')).toHaveValue(
    'xyzxyzxyz'.repeat(20),
    { timeout: 15000 },
  );
  await expect(page.locator('#tool-output')).toContainText('Gzipped size:', {
    timeout: 15000,
  });
});
