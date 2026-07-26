import { test, expect } from './fixtures';

const SCHEMA = '{"type":"object","properties":{"id":{"type":"integer","minimum":1,"maximum":9},"email":{"type":"string","format":"email"},"role":{"enum":["admin","user"]}}}';

test('json-schema-faker generates deterministic compact JSON', async ({ page }) => {
  await page.goto('/tools/json-schema-faker/');
  await page.fill('#in-schema', SCHEMA);
  await page.fill('#in-count', '2');
  await page.fill('#in-seed', '42');
  await page.uncheck('#in-pretty');
  await page.selectOption('#in-output', 'json');

  await expect(page.locator('#tool-output')).toHaveText(
    '[{"email":"ethan.king@mock.dev","id":8,"role":"admin"},{"email":"oliver.roberts@example.com","id":3,"role":"user"}]',
    { timeout: 15000 },
  );
});

test('json-schema-faker supports JSON Lines and CSV output choices', async ({ page }) => {
  await page.goto('/tools/json-schema-faker/');
  await page.fill('#in-schema', SCHEMA);
  await page.fill('#in-count', '2');
  await page.fill('#in-seed', '42');
  await page.uncheck('#in-pretty');

  await page.selectOption('#in-output', 'jsonl');
  await expect(page.locator('#tool-output')).toHaveText(
    '{"email":"ethan.king@mock.dev","id":8,"role":"admin"}\n{"email":"oliver.roberts@example.com","id":3,"role":"user"}',
    { timeout: 15000 },
  );

  await page.selectOption('#in-output', 'csv');
  await expect(page.locator('#tool-output')).toHaveText(
    'email,id,role\nethan.king@mock.dev,8,admin\noliver.roberts@example.com,3,user',
    { timeout: 15000 },
  );
});

test('json-schema-faker deep-link pre-fills params and auto-runs CSV', async ({ page }) => {
  const params = new URLSearchParams({
    schema: SCHEMA,
    count: '2',
    seed: '42',
    pretty: 'false',
    output: 'csv',
  });

  await page.goto(`/tools/json-schema-faker/?${params.toString()}`);
  await expect(page.locator('#in-output')).toHaveValue('csv');
  await expect(page.locator('#in-pretty')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText(
    'email,id,role\nethan.king@mock.dev,8,admin\noliver.roberts@example.com,3,user',
    { timeout: 15000 },
  );
});
