import { test, expect } from './fixtures';

async function setNotes(page: import('@playwright/test').Page, value: string) {
  await page.locator('#in-notes').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('notes-to-html-export builds a self-contained numbered HTML document', async ({ page }) => {
  await page.goto('/tools/notes-to-html-export/');
  await setNotes(page, '# Project handbook\n\n## Setup\n\nRun it.\n\n# Operations\n\n## Rollback\n\nRestore it.');
  await page.selectOption('#in-split', 'heading');
  await page.selectOption('#in-toc', 'sidebar');
  await page.locator('#in-toc_depth').fill('3');
  await page.locator('#in-number_sections').check();
  await page.locator('#in-title').fill('Project Handbook');
  await page.selectOption('#in-theme', 'dark');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('<!doctype html>', { timeout: 15_000 });
  const html = (await out.textContent()) ?? '';
  expect(html).toContain('<title>Project Handbook</title>');
  expect(html).toContain('class="layout with-sidebar"');
  expect(html).toContain('<span class="secno">1</span> Project handbook');
  expect(html).toContain('<a href="#rollback"><span class="secno">2.1</span> Rollback</a>');
  expect(html).toContain('--bg: #16181d');
  expect(html).not.toContain('<script');
  expect(html).not.toContain('<link');
});

test('notes-to-html-export deep link honors hr split top toc light theme', async ({ page }) => {
  const notes = 'Loose note\n\n---\n\n# Second\n\n## Details\n\nText';
  const qs = new URLSearchParams({ notes, split: 'hr', toc: 'top', toc_depth: '1', number_sections: 'false', title: 'Shared Notes', theme: 'light' });
  await page.goto(`/tools/notes-to-html-export/?${qs.toString()}`);

  await expect(page.locator('#in-notes')).toHaveValue(notes);
  await expect(page.locator('#in-split')).toHaveValue('hr');
  await expect(page.locator('#in-toc')).toHaveValue('top');
  await expect(page.locator('#in-toc_depth')).toHaveValue('1');
  await expect(page.locator('#in-number_sections')).not.toBeChecked();
  await expect(page.locator('#in-theme')).toHaveValue('light');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('<title>Shared Notes</title>', { timeout: 15_000 });
  const html = (await out.textContent()) ?? '';
  expect(html).toContain('toc toc-top');
  expect(html).toContain('<a href="#second">Second</a>');
  expect(html).not.toContain('<a href="#details">Details</a>');
  expect(html).toContain('<article class="note" id="note-2">');
});
