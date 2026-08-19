import { test, expect } from './fixtures';

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

const NOTE_JSON = JSON.stringify({
  color: 'BLUE',
  isPinned: true,
  labels: [{ name: 'Shopping' }, { name: 'Home' }],
  createdTimestampUsec: 1768469400000000,
  userEditedTimestampUsec: 1768557600000000,
  title: 'Grocery List',
  listContent: [
    { text: 'Milk', isChecked: false },
    { text: 'Eggs', isChecked: true },
  ],
});

test('converts a Keep JSON checklist note to Markdown with exact output', async ({ page }) => {
  await page.goto('/tools/keep-to-markdown/');
  await page.fill('#in-input', NOTE_JSON);
  await page.selectOption('#in-metadata', 'frontmatter');
  await page.selectOption('#in-filename_style', 'date-title');
  await page.selectOption('#in-checkbox_style', 'task-list');
  await expect(page.locator('#tool-output')).toContainText('==== 2026-01-15-grocery-list.md ====', {
    timeout: 15000,
  });
  expect(await output(page)).toBe(
    '==== 2026-01-15-grocery-list.md ====\n' +
      '---\n' +
      'title: "Grocery List"\n' +
      'created: 2026-01-15T09:30:00Z\n' +
      'updated: 2026-01-16T10:00:00Z\n' +
      'labels: ["Shopping", "Home"]\n' +
      'pinned: true\n' +
      'color: BLUE\n' +
      '---\n\n' +
      '# Grocery List\n\n' +
      '- [ ] Milk\n' +
      '- [x] Eggs',
  );
});

test('supports enum choices, non-default checkbox states, and deep links', async ({ page }) => {
  const notes = JSON.stringify([
    { title: 'Grocery List', labels: [{ name: 'Shopping' }], listContent: [{ text: 'Milk', isChecked: false }] },
    { title: 'Archived', textContent: 'hidden by default', isArchived: true },
  ]);
  await page.goto(
    '/tools/keep-to-markdown/?input=' +
      encodeURIComponent(notes) +
      '&metadata=inline&filename_style=label-title&checkbox_style=bullet&include_archived=false&include_trashed=false&link_attachments=false',
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('==== shopping/grocery-list.md ====', { timeout: 15000 });
  const text = await output(page);
  expect(text).toContain('- Milk');
  expect(text).toContain('#shopping');
  expect(text).not.toContain('Archived');
  expect(text).not.toContain('[ ]');
});
