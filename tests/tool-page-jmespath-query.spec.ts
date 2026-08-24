import { test, expect } from './fixtures';

const SAMPLE = '{"people":[{"name":"Alice","age":34,"state":"WA","skills":["rust","go"]},{"name":"Bob","age":25,"state":"OR","skills":["python"]},{"name":"Carol","age":41,"state":"WA","skills":["rust"]}],"company":{"name":"Initech","locations":["Seattle","Portland"]}}';

async function outputText(page) {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('jmespath-query page filters and reshapes JSON', async ({ page }) => {
  await page.goto('/tools/jmespath-query/');
  await page.fill('#in-expression', 'people[?age > `30`].{name: name, state: state}');
  await page.fill('#in-json', SAMPLE);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"name": "Alice"', { timeout: 15_000 });
  await expect(out).toContainText('"state": "WA"');
  const text = await outputText(page);
  expect(text).toContain('Carol');
  expect(text).not.toContain('Bob');
});

test('jmespath-query page honors deep-linked compact output', async ({ page }) => {
  const params = new URLSearchParams({
    expression: 'company.locations[:1]',
    json: SAMPLE,
    pretty: 'false',
    raw: 'false',
  });

  await page.goto(`/tools/jmespath-query/?${params.toString()}`);
  await expect(page.locator('#in-pretty')).not.toBeChecked();
  await expect(page.locator('#in-raw')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('["Seattle"]', { timeout: 15_000 });
  expect(await outputText(page)).toBe('["Seattle"]');
});

test('jmespath-query page emits raw string arrays one line per item', async ({ page }) => {
  await page.goto('/tools/jmespath-query/');
  await page.fill('#in-expression', 'people[*].name');
  await page.fill('#in-json', SAMPLE);
  await page.check('#in-raw');

  await expect(page.locator('#tool-output')).toContainText('Alice\nBob\nCarol', { timeout: 15_000 });
  expect(await outputText(page)).toBe('Alice\nBob\nCarol');
});

test('jmespath-query page reports invalid expressions clearly', async ({ page }) => {
  await page.goto('/tools/jmespath-query/');
  await page.fill('#in-expression', 'people[?');
  await page.fill('#in-json', SAMPLE);

  await expect(page.locator('#tool-output')).toContainText('invalid JMESPath expression:', { timeout: 15_000 });
});
