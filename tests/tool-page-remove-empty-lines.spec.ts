import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('remove-empty-lines page — removes blank and whitespace-only lines', async ({ page }) => {
  await page.goto('/tools/remove-empty-lines/');
  await page.fill('#in-text', 'First line\n\nSecond line\n   \nThird line\n');
  await expect(page.locator('#tool-output')).toContainText('First line', { timeout: 15000 });
  expect(await outputText(page)).toBe('First line\nSecond line\nThird line');
});

test('remove-empty-lines page — collapse mode keeps one paragraph gap', async ({ page }) => {
  await page.goto('/tools/remove-empty-lines/');
  await page.fill('#in-text', 'Paragraph one\n\n\n\nParagraph two\n\n\nParagraph three');
  await page.selectOption('#in-mode', 'collapse');
  await expect(page.locator('#tool-output')).toContainText('Paragraph two', { timeout: 15000 });
  expect(await outputText(page)).toBe('Paragraph one\n\nParagraph two\n\nParagraph three');
});

test('remove-empty-lines page — whitespace-only checkbox off keeps space lines', async ({ page }) => {
  await page.goto('/tools/remove-empty-lines/');
  await page.fill('#in-text', 'a\n\nb\n   \nc');
  await page.uncheck('#in-whitespace_only');
  await expect(page.locator('#tool-output')).toContainText('   ', { timeout: 15000 });
  expect(await outputText(page)).toBe('a\nb\n   \nc');
});

test('remove-empty-lines page — trim lines checkbox tidies kept lines', async ({ page }) => {
  await page.goto('/tools/remove-empty-lines/');
  await page.fill('#in-text', '  alpha  \n\n\tbeta\t\n \ngamma');
  await page.check('#in-trim_lines');
  await expect(page.locator('#tool-output')).toContainText('alpha', { timeout: 15000 });
  expect(await outputText(page)).toBe('alpha\nbeta\ngamma');
});

test('remove-empty-lines page — query-param deep-link prefills and auto-runs', async ({ page }) => {
  const input = 'one\n\n\n\ntwo\n\n\nthree';
  await page.goto(
    '/tools/remove-empty-lines/?text=' +
      encodeURIComponent(input) +
      '&mode=collapse&whitespace_only=true&trim_lines=false',
  );
  await expect(page.locator('#in-text')).toHaveValue(input, { timeout: 15000 });
  await expect(page.locator('#in-mode')).toHaveValue('collapse');
  await expect(page.locator('#tool-output')).toContainText('two', { timeout: 15000 });
  expect(await outputText(page)).toBe('one\n\ntwo\n\nthree');
});
