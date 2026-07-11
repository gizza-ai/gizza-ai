import { test, expect } from './fixtures';

const SAMPLE = [
  '2026-01-01, Netflix, 15.99',
  '2026-02-01, Netflix, 15.99',
  '2026-03-01, Netflix, 15.99',
  '2026-01-05, Spotify, 9.99',
  '2026-02-05, Spotify, 9.99',
  '2026-01-10, Corner Cafe, 4.50',
].join('\n');

const SAMPLE_OUTPUT =
  'Found 2 recurring charges · $25.98/mo · $311.76/yr projected\n\n' +
  '1. Netflix — $15.99 monthly ×3 · next ~2026-03-31 · $191.88/yr\n' +
  '2. Spotify — $9.99 monthly ×2 · next ~2026-03-08 · $119.88/yr\n\n' +
  'Total: $25.98/mo · $311.76/yr';

async function outputText(page): Promise<string> {
  const text = await page.locator('#tool-output').textContent();
  return (text ?? '').replace(/\s+$/, '');
}

test('subscription-finder page detects recurring charges with exact output', async ({ page }) => {
  await page.goto('/tools/subscription-finder/');
  await page.fill('#in-transactions', SAMPLE);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Found 2 recurring charges', { timeout: 15000 });
  expect(await outputText(page)).toBe(SAMPLE_OUTPUT);
});

test('subscription-finder page honours min occurrences and currency options', async ({ page }) => {
  await page.goto('/tools/subscription-finder/');
  await page.fill('#in-transactions', SAMPLE);
  await page.fill('#in-min_occurrences', '3');
  await page.fill('#in-currency', '£');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Found 1 recurring charge', { timeout: 15000 });
  const text = await outputText(page);
  expect(text).toContain('1. Netflix — £15.99 monthly ×3');
  expect(text).not.toContain('Spotify');
});

test('subscription-finder page parses EU slash dates when selected', async ({ page }) => {
  await page.goto('/tools/subscription-finder/');
  await page.fill(
    '#in-transactions',
    ['13/01/2026, Gym, 30.00', '13/02/2026, Gym, 30.00'].join('\n'),
  );
  await page.selectOption('#in-date_format', 'eu');
  await page.fill('#in-currency', '€');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Gym — €30.00 monthly ×2', { timeout: 15000 });
});

test('subscription-finder page deep-link prefills and computes', async ({ page }) => {
  await page.goto(
    '/tools/subscription-finder/?transactions=' +
      encodeURIComponent(SAMPLE) +
      '&min_occurrences=3&currency=%C2%A3&date_format=iso',
  );

  await expect(page.locator('#in-transactions')).toHaveValue(SAMPLE, { timeout: 15000 });
  await expect(page.locator('#in-min_occurrences')).toHaveValue('3');
  await expect(page.locator('#in-currency')).toHaveValue('£');
  await expect(page.locator('#in-date_format')).toHaveValue('iso');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Found 1 recurring charge', { timeout: 15000 });
  expect(await outputText(page)).toContain('1. Netflix — £15.99 monthly ×3');
});
