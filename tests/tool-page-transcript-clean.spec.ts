import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

const RAW = '[00:01:23] Alice: um i think we should ship this\n[00:01:26] Alice: uh it looks good [laughter]\nBob: like, agreed';
const CLEANED = 'Alice: I think we should ship this. It looks good.\nBob: Like, agreed.';

test('transcript-clean page — default removes timestamps, fillers, cues, and merges speakers', async ({ page }) => {
  await page.goto('/tools/transcript-clean/');
  await page.fill('#in-input', RAW);
  await expect(page.locator('#tool-output')).toContainText('Alice: I think we should ship this.', { timeout: 15000 });
  expect(await outputText(page)).toBe(CLEANED);
});

test('transcript-clean page — aggressive filler removal and checkbox toggle', async ({ page }) => {
  await page.goto('/tools/transcript-clean/');
  await page.fill('#in-input', 'Alice: like, basically we you know need to actually decide\nAlice: uh today');
  await page.selectOption('#in-filler_level', 'aggressive');
  await expect(page.locator('#tool-output')).toContainText('Alice: We need to decide. Today.', { timeout: 15000 });
  expect(await outputText(page)).toBe('Alice: We need to decide. Today.');

  await page.uncheck('#in-merge_speakers');
  await expect(page.locator('#tool-output')).toContainText('Alice: Today.', { timeout: 15000 });
  expect(await outputText(page)).toBe('Alice: We need to decide.\nAlice: Today.');
});

test('transcript-clean page — query-param deep-link prefills and auto-runs', async ({ page }) => {
  await page.goto('/tools/transcript-clean/?input=' + encodeURIComponent(RAW));
  await expect(page.locator('#in-input')).toHaveValue(RAW, { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('Alice: I think we should ship this.', { timeout: 15000 });
  expect(await outputText(page)).toBe(CLEANED);
});
