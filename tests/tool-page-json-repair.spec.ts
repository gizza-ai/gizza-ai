import { test, expect } from './fixtures';

// /tools/json-repair/ repairs malformed JSON in-browser (pure wasm).
// Output is multi-line for indented modes → compare textContent (toHaveText
// normalizes whitespace), via expect.poll so we wait for the run to finish.

const poll = (page: any) =>
  expect.poll(async () => await page.locator('#tool-output').textContent(), { timeout: 15000 });

test('json-repair fixes trailing commas, single quotes, unquoted keys (default 2-space indent)', async ({ page }) => {
  await page.goto('/tools/json-repair/');
  await page.fill('#in-json', "{'name': 'John', age: 30,}");
  await poll(page).toBe('{\n  "name": "John",\n  "age": 30\n}');
});

test('json-repair indent matrix: 4 / tab / minify all produce exact output', async ({ page }) => {
  await page.goto('/tools/json-repair/');
  await page.fill('#in-json', "{'name': 'John', age: 30,}");
  await page.selectOption('#in-indent', '4');
  await poll(page).toBe('{\n    "name": "John",\n    "age": 30\n}');
  await page.selectOption('#in-indent', 'tab');
  await poll(page).toBe('{\n\t"name": "John",\n\t"age": 30\n}');
  await page.selectOption('#in-indent', 'minify');
  await poll(page).toBe('{"name":"John","age":30}');
});

test('json-repair strips markdown fences and closes truncated LLM output', async ({ page }) => {
  await page.goto('/tools/json-repair/');
  await page.selectOption('#in-indent', 'minify');
  await page.fill('#in-json', '```json\n{"users": ["Alice", "Bob"\n```');
  await poll(page).toBe('{"users":["Alice","Bob"]}');
});

test('json-repair converts Python literals and NaN to JSON', async ({ page }) => {
  await page.goto('/tools/json-repair/');
  await page.selectOption('#in-indent', 'minify');
  await page.fill('#in-json', "{'active': True, 'deleted': None, 'count': NaN}");
  await poll(page).toBe('{"active":true,"deleted":null,"count":null}');
});

test('json-repair strips comments and inserts missing commas', async ({ page }) => {
  await page.goto('/tools/json-repair/');
  await page.selectOption('#in-indent', 'minify');
  await page.fill('#in-json', '{\n  // user record\n  "a": 1\n  "b": 2\n}');
  await poll(page).toBe('{"a":1,"b":2}');
});

test('json-repair deep-link pre-fills and runs (the generated URL example)', async ({ page }) => {
  await page.goto('/tools/json-repair/?json=%7B%27name%27%3A%20%27John%27%2C%20age%3A%2030%2C%7D&indent=2');
  await poll(page).toBe('{\n  "name": "John",\n  "age": 30\n}');
});

test('json-repair depth cap: exactly 200 levels repairs, 201 errors', async ({ page }) => {
  await page.goto('/tools/json-repair/');
  await page.selectOption('#in-indent', 'minify');
  await page.fill('#in-json', '['.repeat(200));
  await poll(page).toBe('['.repeat(200) + ']'.repeat(200));
  await page.fill('#in-json', '['.repeat(201));
  await expect(page.locator('#tool-output')).toContainText('200 levels', { timeout: 15000 });
});

test('json-repair reports comment-only input instead of emitting JSON', async ({ page }) => {
  await page.goto('/tools/json-repair/');
  await page.fill('#in-json', '// just a comment');
  await expect(page.locator('#tool-output')).toContainText('nothing to repair', { timeout: 15000 });
});

test('json-repair example chip prefills and runs (Python dict)', async ({ page }) => {
  await page.goto('/tools/json-repair/');
  await page.locator('.tool-example-chip', { hasText: 'Python dict' }).click();
  await poll(page).toBe(
    '{\n  "active": true,\n  "deleted": null,\n  "count": null\n}'
  );
});
