import { test, expect } from './fixtures';

// /tools/json-structure-analyzer/ reports a JSON document's structure (depth, key
// frequency, per-path types, array stats) in-browser via WebAssembly. Pure compute.
const SAMPLE =
  '{"users":[{"id":1,"name":"Ada","roles":["admin"]},{"id":2,"name":"Bo","roles":[]}],"page":1,"total":2}';

test('json-structure-analyzer text mode reports depth, key frequency, and array stats', async ({
  page,
}) => {
  await page.goto('/tools/json-structure-analyzer/');
  await page.fill('#in-json', SAMPLE);
  await page.selectOption('#in-format', 'text');
  const out = page.locator('#tool-output');
  // Human-readable report sections.
  await expect(out).toContainText('Max depth:', { timeout: 15000 });
  await expect(out).toContainText('Key frequency');
  await expect(out).toContainText('Arrays');
  // Recurring keys are counted across the whole document.
  await expect(out).toContainText('id');
  await expect(out).toContainText('name');
});

test('json-structure-analyzer JSON mode (enum default) emits a structured report', async ({
  page,
}) => {
  await page.goto('/tools/json-structure-analyzer/');
  await page.fill('#in-json', SAMPLE);
  await page.selectOption('#in-format', 'json');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"root_type": "object"', { timeout: 15000 });
  await expect(out).toContainText('"max_depth"');
  // array indices collapse to [] so every element shares one path.
  await expect(out).toContainText('$.users[].name');
  await expect(out).toContainText('"key_frequency"');
});

test('json-structure-analyzer deep-link prefills and renders text report', async ({ page }) => {
  const qs = '?json=' + encodeURIComponent(SAMPLE) + '&format=text';
  await page.goto('/tools/json-structure-analyzer/' + qs);
  await expect(page.locator('#in-json')).toHaveValue(SAMPLE, { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('Max depth:', { timeout: 15000 });
});

test('json-structure-analyzer top_keys=0/top_paths=0 lists everything (no truncation)', async ({
  page,
}) => {
  const qs =
    '?json=' + encodeURIComponent(SAMPLE) + '&format=json&top_keys=0&top_paths=0';
  await page.goto('/tools/json-structure-analyzer/' + qs);
  const out = page.locator('#tool-output');
  // With caps disabled, both lists render fully and no truncation flag is raised.
  await expect(out).toContainText('"key_frequency_truncated": false', { timeout: 15000 });
  await expect(out).toContainText('"paths_truncated": false');
  // every distinct key is present in the frequency ranking.
  await expect(out).toContainText('"users"');
  await expect(out).toContainText('"total"');
});
