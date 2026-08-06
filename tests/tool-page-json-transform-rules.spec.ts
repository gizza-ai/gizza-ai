import { test, expect } from './fixtures';

const API_JSON = `{
  "user": {"id": 7, "name": "Ada Lovelace", "email": "ada@example.com"},
  "orders": [{"total": 19.5}, {"total": 30.5}]
}`;

const MAP_RULES = `id = $.user.id
name = $.user.name
email = $.user.email
total = $..total
source = "import"`;

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('json-transform-rules page reshapes JSON with shorthand mapping rules', async ({ page }) => {
  await page.goto('/tools/json-transform-rules/');
  await page.fill('#in-json', API_JSON);
  await page.fill('#in-rules', MAP_RULES);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Ada Lovelace', { timeout: 15_000 });
  expect(JSON.parse(await output(page))).toEqual({
    id: 7,
    name: 'Ada Lovelace',
    email: 'ada@example.com',
    total: [19.5, 30.5],
    source: 'import',
  });
});

test('json-transform-rules deep link fans out records with ?param= values', async ({ page }) => {
  const json = JSON.stringify({
    automobiles: [
      { maker: 'Honda', model: 'Jazz', year: 2010 },
      { maker: 'Ford', model: 'Ka', year: 2015 },
    ],
  });
  const rules = 'title = $.model\nyear = $.year';
  const qs =
    '?json=' + encodeURIComponent(json) +
    '&rules=' + encodeURIComponent(rules) +
    '&each=' + encodeURIComponent('$.automobiles[*]') +
    '&array_mode=first';

  await page.goto('/tools/json-transform-rules/' + qs);

  await expect(page.locator('#in-each')).toHaveValue('$.automobiles[*]', { timeout: 15_000 });
  await expect(page.locator('#in-array_mode')).toHaveValue('first');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Jazz', { timeout: 15_000 });
  expect(JSON.parse(await output(page))).toEqual([
    { title: 'Jazz', year: 2010 },
    { title: 'Ka', year: 2015 },
  ]);
});

test('json-transform-rules page supports report output and non-default pretty checkbox', async ({ page }) => {
  await page.goto('/tools/json-transform-rules/');
  await page.fill('#in-json', '{"user":{"id":7,"name":"Ada"}}');
  await page.fill('#in-rules', 'id = $.user.id\nemail = $.user.email');
  await page.selectOption('#in-output', 'report');
  await page.uncheck('#in-pretty');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('rules that never wrote a value: email', { timeout: 15_000 });
  expect(await output(page)).toContain('1. id <- $.user.id');
});
