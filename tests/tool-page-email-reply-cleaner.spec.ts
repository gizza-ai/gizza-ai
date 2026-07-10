import { test, expect } from './fixtures';

// Output is multi-line, so assert exact textContent (toHaveText collapses
// whitespace and can't verify the newline structure).
async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

const REPLIED =
  'Hi Bob,\n\nFriday works — see you at 10.\n\nBest,\nAlice\n\n-- \nAlice Smith\nAcme Corp\n\nOn Mon, Jan 1, 2024 at 10:00 AM Bob <bob@example.com> wrote:\n> Are you free on Friday?\n> Let me know.';

const REPLIED_CLEANED = [
  'Hi Bob,',
  '',
  'Friday works — see you at 10.',
  '',
  'Best,',
  'Alice',
].join('\n');

test('email-reply-cleaner page — default cleans replied email', async ({ page }) => {
  await page.goto('/tools/email-reply-cleaner/');
  await page.fill('#in-text', REPLIED);
  await expect(page.locator('#tool-output')).toContainText('Friday works', { timeout: 15000 });
  expect(await outputText(page)).toBe(REPLIED_CLEANED);
});

test('email-reply-cleaner page — unchecking Remove quoted lines keeps > lines', async ({
  page,
}) => {
  await page.goto('/tools/email-reply-cleaner/');
  await page.fill('#in-text', 'Keep this.\n> quoted line\nMore.');
  await expect(page.locator('#tool-output')).toContainText('Keep this.', { timeout: 15000 });
  // Default: quote line stripped.
  expect(await outputText(page)).toBe('Keep this.\nMore.');
  // Toggle the pass off: the quoted line survives.
  await page.uncheck('#in-remove_quotes');
  await expect(page.locator('#tool-output')).toContainText('> quoted line', { timeout: 15000 });
  expect(await outputText(page)).toBe('Keep this.\n> quoted line\nMore.');
});

test('email-reply-cleaner page — query-param deep-link prefills and auto-runs', async ({
  page,
}) => {
  await page.goto('/tools/email-reply-cleaner/?text=' + encodeURIComponent(REPLIED));
  await expect(page.locator('#in-text')).toHaveValue(REPLIED, { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('Friday works', { timeout: 15000 });
  expect(await outputText(page)).toBe(REPLIED_CLEANED);
});
