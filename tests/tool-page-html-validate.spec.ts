import { test, expect } from './fixtures';

async function setHtml(page: import('@playwright/test').Page, value: string) {
  await page.locator('#in-html').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('html-validate reports misnested tags with exact line and column', async ({ page }) => {
  await page.goto('/tools/html-validate/');
  await setHtml(page, '<div><p>Hello</div>');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Invalid HTML', { timeout: 15_000 });
  expect(await out.textContent()).toBe(
    'Invalid HTML: 1 error(s), 0 warning(s) in 1 element(s).\n\n  error   line 1:14  `<p>` (opened at line 1:6) is not closed before `</div>` — overlapping/misnested tags',
  );
});

test('html-validate accepts valid snippets and void elements', async ({ page }) => {
  await page.goto('/tools/html-validate/');
  await setHtml(page, '<section><h2>Title</h2><p>Hello<br><img src="x.png"></p></section>');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Valid HTML', { timeout: 15_000 });
  expect(await out.textContent()).toContain('no syntax errors, unclosed tags, or nesting issues');
});

test('html-validate covers JSON output enum choice', async ({ page }) => {
  await page.goto('/tools/html-validate/');
  await setHtml(page, '<p>ok</p></span>');
  await page.selectOption('#in-format', 'json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('unexpected closing tag', { timeout: 15_000 });
  const json = JSON.parse((await out.textContent()) ?? '{}');
  expect(json).toMatchObject({ valid: false, errors: 1, warnings: 0, elements: 1 });
  expect(json.issues[0]).toMatchObject({ severity: 'error', line: 1, column: 10 });
});

test('html-validate deep-links report format and multiline input', async ({ page }) => {
  const html = '<section>\n  <div>text';
  const qs = new URLSearchParams({ html, format: 'report' });
  await page.goto(`/tools/html-validate/?${qs.toString()}`);

  await expect(page.locator('#in-html')).toHaveValue(html);
  await expect(page.locator('#in-format')).toHaveValue('report');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('line 2:3', { timeout: 15_000 });
  expect(await out.textContent()).toContain('`<div>` is never closed with `</div>`');
});
