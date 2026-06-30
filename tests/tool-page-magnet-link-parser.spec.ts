import { test, expect } from './fixtures';

const MAGNET =
  'magnet:?xt=urn:btih:c12fe1c06bba254a9dc9f519b335aa7c1367a88a&dn=Some+File&tr=udp%3A%2F%2Ftracker.example.com%3A1337&xl=1048576';

test('magnet-link-parser parses a magnet link into its parts', async ({ page }) => {
  await page.goto('/tools/magnet-link-parser/');
  // Default mode is "parse" — paste a magnet link.
  await page.fill('#in-magnet', MAGNET);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('c12fe1c06bba254a9dc9f519b335aa7c1367a88a', {
    timeout: 15000,
  });
  await expect(out).toContainText('Some File');
  await expect(out).toContainText('udp://tracker.example.com:1337');
  await expect(out).toContainText('1.00 MB');
});

test('magnet-link-parser builds a magnet link from parts', async ({ page }) => {
  await page.goto('/tools/magnet-link-parser/');
  await page.selectOption('#in-mode', 'build');
  await page.fill('#in-info_hash', 'c12fe1c06bba254a9dc9f519b335aa7c1367a88a');
  await page.fill('#in-display_name', 'My File');
  await page.fill('#in-trackers', 'udp://a.example:1337');
  await expect(page.locator('#tool-output')).toHaveText(
    'magnet:?xt=urn:btih:c12fe1c06bba254a9dc9f519b335aa7c1367a88a&dn=My%20File&tr=udp%3A%2F%2Fa.example%3A1337',
    { timeout: 15000 },
  );
});

test('magnet-link-parser query-param deep-link prefills and parses', async ({ page }) => {
  await page.goto(
    '/tools/magnet-link-parser/?magnet=' + encodeURIComponent(MAGNET),
  );
  await expect(page.locator('#in-magnet')).toHaveValue(MAGNET, { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText(
    'c12fe1c06bba254a9dc9f519b335aa7c1367a88a',
    { timeout: 15000 },
  );
});
