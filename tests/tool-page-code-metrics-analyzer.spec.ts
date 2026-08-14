import { test, expect } from './fixtures';

const JS_SAMPLE = `function grade(score) {
  if (score >= 90) return 'A';
  if (score >= 80) return 'B';
  return 'C';
}`;

const PY_SAMPLE = `def classify(name):
    if not name:
        return 'missing'
    elif name.startswith('A'):
        return 'a'
    return 'other'`;

async function outputJson(page) {
  const text = await page.locator('#tool-output').textContent({ timeout: 20000 });
  return JSON.parse(text ?? '');
}

test('code-metrics-analyzer page reports JavaScript summary metrics', async ({ page }) => {
  await page.goto('/tools/code-metrics-analyzer/');
  await page.fill('#in-source', JS_SAMPLE);
  await page.selectOption('#in-language', 'javascript');
  await page.fill('#in-complexity_threshold', '2');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('Language: javascript', { timeout: 20000 });
  await expect(output).toContainText('Lines: total 5, code 5, comment 0, blank 0');
  await expect(output).toContainText('Functions: 1');
  await expect(output).toContainText('Cyclomatic complexity: total 3, average 3.0, max 3');
  await expect(output).toContainText('- grade (line 1, 5 LOC): CCN 3');
});

test('code-metrics-analyzer page renders JSON and sort controls', async ({ page }) => {
  await page.goto('/tools/code-metrics-analyzer/');
  await page.fill('#in-source', PY_SAMPLE);
  await page.selectOption('#in-language', 'python');
  await page.selectOption('#in-output', 'json');
  await page.selectOption('#in-sort', 'complexity');
  await page.fill('#in-complexity_threshold', '2');
  await page.fill('#in-max_functions', '0');

  const report = await outputJson(page);
  expect(report.language).toBe('python');
  expect(report.functions_total).toBe(1);
  expect(report.functions[0].name).toBe('classify');
  expect(report.functions[0].cyclomatic).toBe(3);
  expect(report.complexity.over_threshold_count).toBe(1);
});

test('code-metrics-analyzer query-param deep-link prefills controls', async ({ page }) => {
  await page.goto(
    '/tools/code-metrics-analyzer/?source=' +
      encodeURIComponent(JS_SAMPLE) +
      '&language=javascript&output=functions&complexity_threshold=2&max_functions=1&sort=complexity',
  );

  await expect(page.locator('#in-source')).toHaveValue(JS_SAMPLE, { timeout: 15000 });
  await expect(page.locator('#in-language')).toHaveValue('javascript');
  await expect(page.locator('#in-output')).toHaveValue('functions');
  await expect(page.locator('#in-sort')).toHaveValue('complexity');
  await expect(page.locator('#tool-output')).toContainText('Name | Line | LOC | CCN', { timeout: 20000 });
  await expect(page.locator('#tool-output')).toContainText('grade ⚠ | 1 | 5 | 3');
});
