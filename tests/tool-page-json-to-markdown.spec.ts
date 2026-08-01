import { test, expect } from './fixtures';

async function setJson(page: import('@playwright/test').Page, value: string) {
  await page.locator('#in-json').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('json-to-markdown array_mode auto renders a table, list forces bullets', async ({ page }) => {
  await page.goto('/tools/json-to-markdown/');
  await setJson(page, '[{"name":"Ada","role":"Editor"},{"name":"Bob","role":"Reviewer"}]');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('| name | role |', { timeout: 15_000 });
  expect(await out.textContent()).toBe(
    '| name | role |\n| --- | --- |\n| Ada | Editor |\n| Bob | Reviewer |',
  );

  await page.selectOption('#in-array_mode', 'list');
  await expect(out).toContainText('- **name**: Ada', { timeout: 15_000 });
  expect(await out.textContent()).toBe(
    '-\n  - **name**: Ada\n  - **role**: Editor\n-\n  - **name**: Bob\n  - **role**: Reviewer',
  );
});

test('json-to-markdown sort_keys checkbox reorders keys alphabetically', async ({ page }) => {
  await page.goto('/tools/json-to-markdown/');
  await setJson(page, '{"b":1,"a":2}');

  const out = page.locator('#tool-output');
  // Default: document order preserved.
  await expect(out).toContainText('- **b**: 1', { timeout: 15_000 });
  expect(await out.textContent()).toBe('- **b**: 1\n- **a**: 2');

  await page.locator('#in-sort_keys').check();
  await expect(out).toContainText('- **a**: 2', { timeout: 15_000 });
  expect(await out.textContent()).toBe('- **a**: 2\n- **b**: 1');
});

test('json-to-markdown heading level and max depth boundaries', async ({ page }) => {
  await page.goto('/tools/json-to-markdown/');
  await setJson(page, '{"a":{"b":1}}');

  const out = page.locator('#tool-output');

  // Lower boundaries: heading level 1, max depth 1 → the subtree collapses to a
  // fenced JSON block immediately.
  await page.locator('#in-heading_level').fill('1');
  await page.locator('#in-max_depth').fill('1');
  await expect(out).toContainText('# a', { timeout: 15_000 });
  expect(await out.textContent()).toBe('# a\n\n```json\n{\n  "b": 1\n}\n```');

  // Upper heading boundary: level 6 is the deepest heading; nesting stays capped.
  await page.locator('#in-max_depth').fill('6');
  await page.locator('#in-heading_level').fill('6');
  await expect(out).toContainText('###### a', { timeout: 15_000 });
  expect(await out.textContent()).toBe('###### a\n\n- **b**: 1');
});

test('json-to-markdown deep-link applies heading level and sort_keys', async ({ page }) => {
  const json = '{"b":"two","a":{"c":1}}';
  const qs = new URLSearchParams({
    json,
    heading_level: '3',
    array_mode: 'auto',
    sort_keys: 'true',
    max_depth: '6',
  });
  await page.goto(`/tools/json-to-markdown/?${qs.toString()}`);

  await expect(page.locator('#in-json')).toHaveValue(json);
  await expect(page.locator('#in-heading_level')).toHaveValue('3');
  await expect(page.locator('#in-array_mode')).toHaveValue('auto');
  await expect(page.locator('#in-sort_keys')).toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('### a', { timeout: 15_000 });
  expect(await out.textContent()).toBe('### a\n\n- **c**: 1\n\n- **b**: two');
});
