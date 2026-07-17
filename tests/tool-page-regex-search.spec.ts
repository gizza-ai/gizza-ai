import { test, expect } from './fixtures';

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('returns matching lines grep-style with line numbers by default', async ({ page }) => {
  await page.goto('/tools/regex-search/');
  await page.fill('#in-text', 'starting up\nERROR disk full\nretrying\nERROR timeout\ndone');
  await page.fill('#in-pattern', 'ERROR');
  await expect(page.locator('#tool-output')).toContainText('matching lines', { timeout: 15000 });
  // line_numbers defaults on → each hit is prefixed `line-number:`.
  await expect(page.locator('#in-line_numbers')).toBeChecked();
  expect(await output(page)).toBe('2 matching lines:\n2:ERROR disk full\n4:ERROR timeout');
});

test('ignore_case matches every casing', async ({ page }) => {
  await page.goto('/tools/regex-search/');
  await page.fill('#in-text', 'todo: refactor\nTODO: write tests\nfixed the bug\nToDo: docs');
  await page.fill('#in-pattern', 'todo');
  await page.check('#in-ignore_case');
  await expect(page.locator('#tool-output')).toContainText('3 matching lines', { timeout: 15000 });
  expect(await output(page)).toBe('3 matching lines:\n1:todo: refactor\n2:TODO: write tests\n4:ToDo: docs');
});

test('whole_word excludes substrings', async ({ page }) => {
  await page.goto('/tools/regex-search/');
  await page.fill('#in-text', 'cat\ncategory\nthe cat sat');
  await page.fill('#in-pattern', 'cat');
  await page.check('#in-whole_word');
  await expect(page.locator('#tool-output')).toContainText('matching lines', { timeout: 15000 });
  // "category" is dropped; "cat" and "the cat sat" survive.
  expect(await output(page)).toBe('2 matching lines:\n1:cat\n3:the cat sat');
});

test('invert returns the non-matching lines', async ({ page }) => {
  await page.goto('/tools/regex-search/');
  await page.fill('#in-text', '# a comment\nkeep this\n# another comment\nkeep that');
  await page.fill('#in-pattern', '^#');
  await page.check('#in-invert');
  await expect(page.locator('#tool-output')).toContainText('matching lines', { timeout: 15000 });
  expect(await output(page)).toBe('2 matching lines:\n2:keep this\n4:keep that');
});

test('context includes surrounding lines with a - prefix', async ({ page }) => {
  await page.goto('/tools/regex-search/');
  await page.fill('#in-text', 'line 1\nline 2\ntarget\nline 4\nline 5');
  await page.fill('#in-pattern', 'target');
  await page.fill('#in-context', '1');
  await expect(page.locator('#tool-output')).toContainText('target', { timeout: 15000 });
  // hit uses `:`, context lines use `-`.
  expect(await output(page)).toBe('1 matching line:\n2-line 2\n3:target\n4-line 4');
});

test('line_numbers can be turned off (default-true checkbox)', async ({ page }) => {
  await page.goto('/tools/regex-search/');
  await page.fill('#in-text', 'apple\nbanana\navocado');
  await page.fill('#in-pattern', '^a');
  await page.uncheck('#in-line_numbers');
  await expect(page.locator('#tool-output')).toContainText('matching lines', { timeout: 15000 });
  expect(await output(page)).toBe('2 matching lines:\napple\navocado');
});

test('highlight wraps matched substrings in « »', async ({ page }) => {
  await page.goto('/tools/regex-search/');
  await page.fill('#in-text', 'the cat and the cat');
  await page.fill('#in-pattern', 'cat');
  await page.check('#in-highlight');
  await expect(page.locator('#tool-output')).toContainText('«cat»', { timeout: 15000 });
  expect(await output(page)).toBe('1 matching line:\n1:the «cat» and the «cat»');
});

test('deep-link pre-fills text + pattern and runs on load', async ({ page }) => {
  const text = encodeURIComponent('starting up\nERROR disk full\nretrying\nERROR timeout\ndone');
  await page.goto(`/tools/regex-search/?text=${text}&pattern=ERROR&line_numbers=true`);
  await expect(page.locator('#tool-output')).toContainText('matching lines', { timeout: 15000 });
  await expect(page.locator('#in-pattern')).toHaveValue('ERROR');
  await expect(page.locator('#in-line_numbers')).toBeChecked();
  expect(await output(page)).toBe('2 matching lines:\n2:ERROR disk full\n4:ERROR timeout');
});
