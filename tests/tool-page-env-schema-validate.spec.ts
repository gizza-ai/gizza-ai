import { test, expect } from './fixtures';

async function runWasm(
  page: import('@playwright/test').Page,
  env: string,
  schema: string,
  schemaFormat = 'auto',
  unknownKeys = 'warn',
  emptyIsMissing = 'true',
  output = 'report',
) {
  return await page.evaluate(async ({ env, schema, schemaFormat, unknownKeys, emptyIsMissing, output }) => {
    const mod = await import('/tools/env-schema-validate/gizza_ai_env_schema_validate_web.js');
    await mod.default('/tools/env-schema-validate/gizza_ai_env_schema_validate_web_bg.wasm');
    return mod.run(env, schema, schemaFormat, unknownKeys, emptyIsMissing, output);
  }, { env, schema, schemaFormat, unknownKeys, emptyIsMissing, output });
}

const BAD_ENV = [
  '# staging',
  'export NODE_ENV=staging',
  'PORT=99999',
  'DATABASE_URL=localhost',
  'API_KEY=short',
  'LEGACY_FLAG=1',
].join('\n');

const RULE_SCHEMA = [
  'NODE_ENV=required|enum:development,staging,production',
  'PORT=required|port',
  'DATABASE_URL=required|url',
  'API_KEY=required|min:32',
  'TIMEOUT=number|min:1|max:60|default:30',
].join('\n');

test('env-schema-validate wasm returns the worked-example report exactly', async ({ page }) => {
  await page.goto('/tools/env-schema-validate/');
  await page.waitForSelector('#in-env');

  const out = await runWasm(page, BAD_ENV, RULE_SCHEMA);
  expect(out).toBe([
    '.env schema check: FAILED — 3 errors, 1 warning (5 declared keys, 5 keys in the file)',
    "  line 3    error    type                    'PORT' must be a TCP port between 1 and 65535, got '99999'",
    "  line 4    error    type                    'DATABASE_URL' must be a URL with a scheme and host (e.g. https://api.example.com), got 'localhost'",
    "  line 5    error    min                     'API_KEY' must be at least 32 characters long, got 5",
    "  line 6    warning  unknown-key             'LEGACY_FLAG' is set in the .env file but is not declared in the schema",
  ].join('\n'));
});

test('env-schema-validate wasm covers advertised schema formats, enums and option values', async ({ page }) => {
  await page.goto('/tools/env-schema-validate/');
  await page.waitForSelector('#in-env');

  const strict = await runWasm(page, 'PORT=8080\nEXTRA=1', 'PORT=required|port', 'rules', 'error');
  expect(strict).toContain('.env schema check: FAILED');
  expect(strict).toContain('unknown-key');

  const ignored = await runWasm(page, 'PORT=8080\nEXTRA=1', 'PORT=required|port', 'rules', 'ignore');
  expect(ignored).toBe('.env schema check: PASSED — 0 errors, 0 warnings (1 declared key, 2 keys in the file)\n  every declared key is present and valid.');

  const emptyString = await runWasm(page, 'TOKEN=', 'TOKEN=required|min:3', 'rules', 'warn', 'false');
  expect(emptyString).toContain("'TOKEN' must be at least 3 characters long");

  const jsonSchema = '{"type":"object","required":["PORT","NODE_ENV"],"properties":{"PORT":{"type":"integer","minimum":1,"maximum":65535},"NODE_ENV":{"type":"string","enum":["dev","prod"]}}}';
  const jsonOut = await runWasm(page, 'PORT=8080\nNODE_ENV=staging', jsonSchema, 'json-schema', 'warn', 'true', 'json');
  expect(jsonOut).toContain('"ok": false');
  expect(jsonOut).toContain('"rule": "enum"');

  const example = await runWasm(page, 'PORT=8080', 'PORT=3000\nDATABASE_URL=postgres://localhost/app', 'example');
  expect(example).toContain("'DATABASE_URL' is required");
});

test('env-schema-validate page renders exact values and honors the non-default checkbox', async ({ page }) => {
  await page.goto('/tools/env-schema-validate/');
  await page.fill('#in-env', 'TOKEN=');
  await page.fill('#in-schema', 'TOKEN=required|min:3');
  await page.selectOption('#in-schema_format', 'rules');
  await page.uncheck('#in-empty_is_missing');

  await expect(page.locator('#tool-output')).toContainText("'TOKEN' must be at least 3 characters long", { timeout: 15_000 });
});

test('env-schema-validate deep-link prefills fields and outputs strict JSON', async ({ page }) => {
  const qs = new URLSearchParams({
    env: 'PORT=abc\nEXTRA=1',
    schema: 'PORT=required|port',
    schema_format: 'rules',
    unknown_keys: 'error',
    empty_is_missing: 'true',
    output: 'json',
  });
  await page.goto(`/tools/env-schema-validate/?${qs.toString()}`);

  await expect(page.locator('#in-env')).toHaveValue('PORT=abc\nEXTRA=1', { timeout: 15_000 });
  await expect(page.locator('#in-schema')).toHaveValue('PORT=required|port');
  await expect(page.locator('#in-schema_format')).toHaveValue('rules');
  await expect(page.locator('#in-unknown_keys')).toHaveValue('error');
  await expect(page.locator('#in-empty_is_missing')).toBeChecked();
  await expect(page.locator('#in-output')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toContainText('"error_count": 2', { timeout: 15_000 });
});
