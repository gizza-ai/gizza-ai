import { test, expect } from './fixtures';

const PAYLOAD = '{"status":"ok","user":{"id":"u-42","roles":["admin","dev"]},"items":[{"sku":"A1","price":9.5},{"sku":"B2","price":250}]}';
const PASSING = '$.status equals ok\n$.user.id matches ^u-\\d+$\n$.user.roles contains admin\n$.items length 2\n$.items[*].price gt 0';

async function outputText(page) {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('json-assertion-runner reports passing DSL assertions', async ({ page }) => {
  await page.goto('/tools/json-assertion-runner/');
  await page.fill('#in-payload', PAYLOAD);
  await page.fill('#in-assertions', PASSING);
  await page.selectOption('#in-rules_format', 'dsl');

  await expect(page.locator('#tool-output')).toContainText('PASS — 5 of 5 assertions passed, 0 failed', { timeout: 15000 });
  const text = await outputText(page);
  expect(text).toContain('PASS  #1  $.status equals "ok"');
  expect(text).toContain('PASS  #5  $.items[*].price gt 0');
});

test('json-assertion-runner reports failing range location and actual value', async ({ page }) => {
  await page.goto('/tools/json-assertion-runner/');
  await page.fill('#in-payload', PAYLOAD);
  await page.fill('#in-assertions', '$.items[*].price lte 100');
  await page.selectOption('#in-rules_format', 'dsl');

  await expect(page.locator('#tool-output')).toContainText('FAIL — 0 of 1 assertions passed, 1 failed', { timeout: 15000 });
  const text = await outputText(page);
  expect(text).toContain("at $['items'][1]['price']: expected <= 100, got 250");
});

test('json-assertion-runner count asserts matched nodes and points at length', async ({ page }) => {
  await page.goto('/tools/json-assertion-runner/');
  await page.fill('#in-payload', PAYLOAD);
  await page.fill('#in-assertions', '$.items[*] count 2\n$.items count 2');

  await expect(page.locator('#tool-output')).toContainText('FAIL — 1 of 2 assertions passed, 1 failed', { timeout: 15000 });
  const text = await outputText(page);
  expect(text).toContain('PASS  #1  $.items[*] count 2');
  expect(text).toContain('expected count 2, got 1 matched node(s) — the path selected a single array; use `length` to assert its size');
});

test('json-assertion-runner deep-link supports JSON report and case-insensitive matching', async ({ page }) => {
  const params = new URLSearchParams({
    payload: '{"status":"OK","count":3}',
    assertions: '[{"path":"$.status","op":"equals","expected":"ok"},{"path":"$.count","op":"range","expected":"1..5"}]',
    rules_format: 'json',
    output: 'json',
    case_sensitive: 'false',
    stop_on_first_failure: 'false',
  });

  await page.goto(`/tools/json-assertion-runner/?${params.toString()}`);
  await expect(page.locator('#in-rules_format')).toHaveValue('json');
  await expect(page.locator('#in-output')).toHaveValue('json');
  await expect(page.locator('#in-case_sensitive')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('"passed": true', { timeout: 15000 });
  const parsed = JSON.parse(await outputText(page));
  expect(parsed.total).toBe(2);
  expect(parsed.failed_count).toBe(0);
  expect(parsed.assertions[0].status).toBe('pass');
});

test('json-assertion-runner stop-on-first-failure skips remaining checks', async ({ page }) => {
  await page.goto('/tools/json-assertion-runner/');
  await page.fill('#in-payload', PAYLOAD);
  await page.fill('#in-assertions', '$.status equals nope\n$.status exists\n$.items length 2');
  await page.check('#in-stop_on_first_failure');

  await expect(page.locator('#tool-output')).toContainText('FAIL — 0 of 3 assertions passed, 1 failed, 2 skipped', { timeout: 15000 });
  const text = await outputText(page);
  expect(text).toContain('SKIP  #3');
});
