import { test, expect } from './fixtures';

async function outText(page): Promise<string> {
  return (await page.locator('#tool-output').textContent()) ?? '';
}

const BROKEN = '<html><body>\n<h2>Welcome</h2>\n<img src="hero.png">\n<form><input name="email"></form>\n</body></html>';

test('default text report flags real accessibility issues', async ({ page }) => {
  await page.goto('/tools/html-accessibility-checker/');
  await page.fill('#in-html', BROKEN);
  await expect(page.locator('#tool-output')).toContainText('HTML accessibility report', { timeout: 15000 });
  const out = await outText(page);
  expect(out).toContain('WCAG level AA, full document scanned');
  expect(out).toContain('img-missing-alt — WCAG 1.1.1 (A)');
  expect(out).toContain('input-missing-label — WCAG 3.3.2 (A)');
  expect(out).toContain('missing-lang — WCAG 3.1.1 (A)');
  expect(out).toContain('heading-skipped-level — Best practice');
  expect(out).toContain('<img src="hero.png">');
});

test('deep-link pre-fills and auto-runs errors-only mode', async ({ page }) => {
  await page.goto(
    `/tools/html-accessibility-checker/?html=${encodeURIComponent(BROKEN)}&min_severity=error&level=aa&max_issues=200`
  );
  await expect(page.locator('#tool-output')).toContainText('ERRORS', { timeout: 15000 });
  const out = await outText(page);
  expect(out).toContain('img-missing-alt');
  expect(out).toContain('input-missing-label');
  expect(out).not.toContain('heading-skipped-level');
  expect(out).not.toContain('SUGGESTIONS');
});

test('JSON output and show_passed checkbox expose structured audit data', async ({ page }) => {
  await page.goto('/tools/html-accessibility-checker/');
  await page.fill('#in-html', '<html lang="en"><head><title>T</title></head><body><main><h1>T</h1><button></button></main></body></html>');
  await page.selectOption('#in-format', 'json');
  await page.check('#in-show_passed');
  await expect(page.locator('#tool-output')).toContainText('"code": "button-empty"', { timeout: 15000 });
  const parsed = JSON.parse(await outText(page));
  expect(parsed.counts.error).toBe(1);
  expect(parsed.issues[0].code).toBe('button-empty');
  expect(parsed.issues[0].wcag).toBe('WCAG 4.1.2 (A)');
  expect(parsed.passed.length).toBeGreaterThan(0);
});

test('CSV format and max_issues cap limit advertised values', async ({ page }) => {
  await page.goto('/tools/html-accessibility-checker/');
  await page.fill('#in-html', BROKEN);
  await page.selectOption('#in-format', 'csv');
  await page.fill('#in-max_issues', '2');
  await expect(page.locator('#tool-output')).toContainText('severity,code,wcag,line,column,element,message', { timeout: 15000 });
  const rows = (await outText(page)).trim().split('\n');
  expect(rows[0]).toBe('severity,code,wcag,line,column,element,message');
  expect(rows).toHaveLength(3);
});

test('AAA level enables generic link text checks', async ({ page }) => {
  await page.goto('/tools/html-accessibility-checker/');
  const html = '<html lang="en"><head><title>T</title></head><body><main><h1>T</h1><a href="/more">click here</a></main></body></html>';
  await page.fill('#in-html', html);
  await expect(page.locator('#tool-output')).not.toContainText('link-generic-text', { timeout: 15000 });
  await page.selectOption('#in-level', 'aaa');
  await expect(page.locator('#tool-output')).toContainText('link-generic-text', { timeout: 15000 });
});
