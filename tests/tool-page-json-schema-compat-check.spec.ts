import { test, expect } from './fixtures';

const tool = '/tools/json-schema-compat-check/';

const oldObject = '{"type":"object","required":["id"],"properties":{"id":{"type":"string"},"email":{"type":"string"}}}';
const newRequired = '{"type":"object","required":["id","email"],"properties":{"id":{"type":"string"},"email":{"type":"string"}}}';

async function setTextarea(locator, value: string) {
  await locator.evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v as string;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page,
  oldSchema: string,
  newSchema: string,
  direction = 'both',
  strictRequired = 'false',
): Promise<string> {
  return await page.evaluate(
    async ({ oldSchema, newSchema, direction, strictRequired }) => {
      const mod = await import('/tools/json-schema-compat-check/gizza_ai_json_schema_compat_check_web.js');
      await mod.default('/tools/json-schema-compat-check/gizza_ai_json_schema_compat_check_web_bg.wasm');
      return mod.run(oldSchema, newSchema, direction, strictRequired);
    },
    { oldSchema, newSchema, direction, strictRequired },
  );
}

test('json-schema-compat-check page reports an added required field as breaking', async ({ page }) => {
  await page.goto(tool);
  await setTextarea(page.locator('#in-old_schema'), oldObject);
  await setTextarea(page.locator('#in-new_schema'), newRequired);
  await page.selectOption('#in-direction', 'both');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Verdict: breaking', { timeout: 15000 });
  await expect(out).toContainText('property "email" was added to "required"');
  await expect(out).toContainText('Consumer compatibility');
  await expect(out).toContainText('Producer compatibility');
});

test('json-schema-compat-check deep link pre-fills producer-only strict state', async ({ page }) => {
  const qs = new URLSearchParams({
    old_schema: '{"type":"string","enum":["draft","sent"]}',
    new_schema: '{"type":"string","enum":["draft","sent","archived"]}',
    direction: 'producer',
    strict_required: 'true',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-old_schema')).toHaveValue('{"type":"string","enum":["draft","sent"]}', { timeout: 15000 });
  await expect(page.locator('#in-new_schema')).toHaveValue('{"type":"string","enum":["draft","sent","archived"]}');
  await expect(page.locator('#in-direction')).toHaveValue('producer');
  await expect(page.locator('#in-strict_required')).toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('"enum" values were added: "archived"');
  await expect(page.locator('#tool-output')).not.toContainText('Consumer compatibility');
});

test('json-schema-compat-check wasm covers directions and validation errors', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-old_schema');

  expect(await runWasm(page, oldObject, newRequired, 'consumer')).toContain('Consumer compatibility');
  expect(await runWasm(page, oldObject, newRequired, 'producer')).toContain('Verdict: compatible');
  expect(await runWasm(page, '{"type":"integer","minimum":0}', '{"type":"integer","minimum":-5}', 'producer')).toContain(
    'the lower bound was lowered from >= 0 to >= -5',
  );
  await expect(runWasm(page, '{bad json', newRequired, 'both')).rejects.toThrow(/old_schema is not valid JSON/);
  await expect(runWasm(page, oldObject, newRequired, 'sideways')).rejects.toThrow(/direction must be one of/);
});

test('json-schema-compat-check page ships example chips', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(4);
  await expect(page.locator('.tool-example-chip')).toContainText([
    'Added required field',
    'Enum narrowed',
    'Producer widening',
    'Compatible annotation edit',
  ]);
});
