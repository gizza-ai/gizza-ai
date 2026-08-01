import { test, expect } from './fixtures';

const SAMPLE = JSON.stringify({
  activeNotes: [
    {
      id: 'abc-123',
      content: 'Grocery List\nMilk\nEggs',
      tags: ['home', 'shopping list'],
      creationDate: '2026-01-15T09:30:00.000Z',
      lastModified: '2026-01-16T10:00:00.000Z',
      pinned: true,
    },
  ],
  trashedNotes: [{ id: 'old-9', content: 'Deleted idea', tags: [], creationDate: '2025-12-01T00:00:00.000Z' }],
});

const FRONTMATTER_OUTPUT = '==== 2026-01-15-grocery-list.md ====\n---\ntitle: "Grocery List"\ncreated: 2026-01-15T09:30:00.000Z\nupdated: 2026-01-16T10:00:00.000Z\ntags: ["home", "shopping list"]\npinned: true\n---\n\n# Grocery List\n\nMilk\nEggs';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('simplenote-export-to-markdown page converts modern Simplenote JSON with frontmatter', async ({ page }) => {
  await page.goto('/tools/simplenote-export-to-markdown/');
  await page.fill('#in-input', SAMPLE);
  await expect(page.locator('#tool-output')).toContainText('==== 2026-01-15-grocery-list.md ====', { timeout: 15000 });
  expect(await outputText(page)).toBe(FRONTMATTER_OUTPUT);
});

test('simplenote-export-to-markdown supports inline hashtags and title filenames', async ({ page }) => {
  await page.goto('/tools/simplenote-export-to-markdown/');
  await page.fill('#in-input', SAMPLE);
  await page.selectOption('#in-filename_style', 'title');
  await page.selectOption('#in-metadata', 'inline');
  await expect(page.locator('#tool-output')).toContainText('==== grocery-list.md ====', { timeout: 15000 });
  const out = await outputText(page);
  expect(out).toContain('#home #shopping-list');
  expect(out).not.toContain('---');
});

test('simplenote-export-to-markdown includes trashed notes when checked', async ({ page }) => {
  await page.goto('/tools/simplenote-export-to-markdown/');
  await page.fill('#in-input', SAMPLE);
  await page.check('#in-include_trashed');
  await expect(page.locator('#tool-output')).toContainText('Deleted idea', { timeout: 15000 });
  const out = await outputText(page);
  expect(out).toContain('==== 2025-12-01-deleted-idea.md ====');
});

test('simplenote-export-to-markdown deep-link pre-fills and auto-runs', async ({ page }) => {
  const params = new URLSearchParams({
    input: SAMPLE,
    filename_style: 'id',
    metadata: 'frontmatter',
    include_trashed: 'true',
  });
  await page.goto(`/tools/simplenote-export-to-markdown/?${params.toString()}`);
  await expect(page.locator('#in-input')).toHaveValue(SAMPLE, { timeout: 15000 });
  await expect(page.locator('#in-filename_style')).toHaveValue('id');
  await expect(page.locator('#in-include_trashed')).toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('==== abc-123.md ====', { timeout: 15000 });
});
