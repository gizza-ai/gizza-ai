import { test, expect } from './fixtures';

async function setCss(page: import('@playwright/test').Page, value: string) {
  await page.locator('#in-css').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('css-validate reports real syntax and property diagnostics', async ({ page }) => {
  await page.goto('/tools/css-validate/');
  await setCss(page, '.hero, {\n  colr: #12zz width: 10px;\n}');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Invalid CSS', { timeout: 15_000 });
  const text = (await out.textContent()) ?? '';
  expect(text).toContain('selector list contains an empty selector');
  expect(text).toContain('unknown CSS property `colr`');
  expect(text).toContain('malformed hex color `#12zz`');
  expect(text).toContain('semicolon may be missing');
});

test('css-validate accepts valid snippets with stats enabled', async ({ page }) => {
  await page.goto('/tools/css-validate/');
  await setCss(page, '.card {\n  color: #333;\n  margin: 1rem;\n  --gap: 8px;\n  padding: var(--gap);\n}');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Valid CSS', { timeout: 15_000 });
  expect(await out.textContent()).toContain('Checked 1 rule(s), 4 declaration(s)');
});

test('css-validate covers enum choices and non-default checkbox state', async ({ page }) => {
  await page.goto('/tools/css-validate/');
  await setCss(page, '.box {\n  -webkit-transform: scale(1);\n  colr: red;\n  color: var(--missing);\n}');
  await page.selectOption('#in-format', 'json');
  await page.selectOption('#in-severity', 'warning');
  await page.selectOption('#in-unknown_properties', 'error');
  await page.selectOption('#in-vendor_prefixes', 'warn');
  await page.uncheck('#in-stats');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('vendor-prefixed property', { timeout: 15_000 });
  const json = JSON.parse((await out.textContent()) ?? '{}');
  expect(json.valid).toBe(false);
  expect(json.errors).toBe(1);
  expect(json.warnings).toBe(2);
  expect(json.stats).toBeUndefined();
  expect(json.issues.every((issue: { severity: string }) => issue.severity === 'warning')).toBe(true);
});

test('css-validate deep-links multiline CSS and strict unknowns', async ({ page }) => {
  const css = '.btn {\n  colr: red;\n  display: flex;\n}';
  const qs = new URLSearchParams({ css, unknown_properties: 'error', severity: 'error', stats: 'false' });
  await page.goto(`/tools/css-validate/?${qs.toString()}`);

  await expect(page.locator('#in-css')).toHaveValue(css);
  await expect(page.locator('#in-unknown_properties')).toHaveValue('error');
  await expect(page.locator('#in-severity')).toHaveValue('error');
  await expect(page.locator('#in-stats')).not.toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Invalid CSS', { timeout: 15_000 });
  expect(await out.textContent()).toContain('unknown CSS property `colr`');
});
