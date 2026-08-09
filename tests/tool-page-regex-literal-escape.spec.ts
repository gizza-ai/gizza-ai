import { test, expect } from './fixtures';

const PCRE_EXACT = 'a\\.b\\*c\\+\\(d\\)';

test('regex-literal-escape page emits exact PCRE escaping', async ({ page }) => {
  await page.goto('/tools/regex-literal-escape/');
  await page.fill('#in-text', 'a.b*c+(d)');
  await page.selectOption('#in-flavor', 'pcre');

  await expect(page.locator('#tool-output')).toHaveText(PCRE_EXACT, { timeout: 15_000 });
});

test('regex-literal-escape honours delimiter and source string literal output', async ({ page }) => {
  await page.goto('/tools/regex-literal-escape/');
  await page.fill('#in-text', '$40 for a g3/400');
  await page.selectOption('#in-flavor', 'pcre');
  await page.fill('#in-delimiter', '/');
  await page.check('#in-string_literal');

  await expect(page.locator('#tool-output')).toHaveText('\\\\$40 for a g3\\\\/400', { timeout: 15_000 });
});

test('regex-literal-escape deep-link covers strict JS and whitespace escaping', async ({ page }) => {
  const params = new URLSearchParams({
    text: 'foo-bar baz',
    flavor: 'javascript-strict',
    delimiter: '',
    escape_whitespace: 'true',
    string_literal: 'false',
  });
  await page.goto(`/tools/regex-literal-escape/?${params.toString()}`);

  await expect(page.locator('#in-text')).toHaveValue('foo-bar baz', { timeout: 15_000 });
  await expect(page.locator('#in-flavor')).toHaveValue('javascript-strict');
  await expect(page.locator('#in-escape_whitespace')).toBeChecked();
  await expect(page.locator('#in-string_literal')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('\\x66oo\\x2dbar\\x20baz', { timeout: 15_000 });
});

test('regex-literal-escape covers advertised flavors', async ({ page }) => {
  await page.goto('/tools/regex-literal-escape/');
  await page.fill('#in-text', 'a-b#c/d');

  await page.selectOption('#in-flavor', 're2');
  await expect(page.locator('#tool-output')).toHaveText('a-b#c/d', { timeout: 15_000 });

  await page.selectOption('#in-flavor', 'rust');
  await expect(page.locator('#tool-output')).toHaveText('a\\-b\\#c/d', { timeout: 15_000 });

  await page.selectOption('#in-flavor', 'java');
  await expect(page.locator('#tool-output')).toHaveText('\\Qa-b#c/d\\E', { timeout: 15_000 });

  await page.selectOption('#in-flavor', 'python');
  await expect(page.locator('#tool-output')).toHaveText('a\\-b\\#c/d', { timeout: 15_000 });
});

test('regex-literal-escape generated CLI example is exact and generic', async ({ page }) => {
  await page.goto('/tools/regex-literal-escape/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool regex-literal-escape');
  expect(cli).toContain('a.b*c+(d)');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
