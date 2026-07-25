import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

const SAMPLE_TEXT = 'Alpha beta gamma delta. Epsilon zeta eta theta.';

test('chunk-text page — splits characters with exact JSON output', async ({ page }) => {
  await page.goto('/tools/chunk-text/');
  await page.fill('#in-text', SAMPLE_TEXT);
  await page.fill('#in-chunk_size', '20');
  await page.fill('#in-overlap', '0');
  await page.selectOption('#in-unit', 'characters');
  await page.selectOption('#in-boundary', 'word');
  await page.selectOption('#in-format', 'json');
  await expect(page.locator('#tool-output')).toContainText('Alpha beta gamma', { timeout: 15000 });
  expect(JSON.parse(await outputText(page))).toEqual([
    { id: 0, text: 'Alpha beta gamma ', chars: 17, tokens: 4, start: 0, end: 17 },
    { id: 1, text: 'delta. Epsilon zeta ', chars: 20, tokens: 5, start: 17, end: 37 },
    { id: 2, text: 'eta theta.', chars: 10, tokens: 3, start: 37, end: 47 },
  ]);
});

test('chunk-text page — JSONL format and trim checkbox', async ({ page }) => {
  await page.goto('/tools/chunk-text/');
  await page.fill('#in-text', '  one two three four five six  ');
  await page.fill('#in-chunk_size', '3');
  await page.fill('#in-overlap', '1');
  await page.selectOption('#in-unit', 'words');
  await page.selectOption('#in-boundary', 'word');
  await page.selectOption('#in-format', 'jsonl');
  await page.check('#in-trim');
  await expect(page.locator('#tool-output')).toContainText('"text":"one two three"', { timeout: 15000 });
  const lines = (await outputText(page)).split('\n');
  expect(lines.length).toBe(3);
  expect(JSON.parse(lines[0])).toMatchObject({ id: 0, text: 'one two three', start: 2, end: 15 });
  expect(JSON.parse(lines[1])).toMatchObject({ id: 1, text: 'three four five', start: 10, end: 25 });
  expect(JSON.parse(lines[2])).toMatchObject({ id: 2, text: 'five six', start: 21, end: 29 });
});

test('chunk-text page — query-param deep-link prefills and auto-runs', async ({ page }) => {
  await page.goto(
    '/tools/chunk-text/?text=' +
      encodeURIComponent(SAMPLE_TEXT) +
      '&chunk_size=20&overlap=0&unit=characters&boundary=word&chars_per_token=4&format=plain&trim=true',
  );
  await expect(page.locator('#in-text')).toHaveValue(SAMPLE_TEXT, { timeout: 15000 });
  await expect(page.locator('#in-unit')).toHaveValue('characters');
  await expect(page.locator('#in-trim')).toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('---', { timeout: 15000 });
  expect(await outputText(page)).toBe(['Alpha beta gamma', 'delta. Epsilon zeta', 'eta theta.'].join('\n\n---\n\n'));
});
