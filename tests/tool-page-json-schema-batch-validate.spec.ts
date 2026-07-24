import { test, expect } from './fixtures';

const draft7Schema =
  '{"$schema":"http://json-schema.org/draft-07/schema#","type":"object","required":["id","name"],"properties":{"id":{"type":"integer"},"name":{"type":"string"}}}';
const arrayRecords = '[{"id":1,"name":"Ada"},{"id":"2","name":"Grace"},{"name":"Kay"}]';

test('json-schema-batch-validate page validates a JSON array batch and lists failing records', async ({ page }) => {
  await page.goto('/tools/json-schema-batch-validate/');
  await page.fill('#in-schema', draft7Schema);
  await page.fill('#in-records', arrayRecords);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Batch validation: FAIL', { timeout: 15_000 });
  await expect(out).toContainText('Draft: draft7 (from $schema)');
  await expect(out).toContainText('Records: 3 (1 passed, 2 failed)');
  await expect(out).toContainText('Total errors: 2');
  // Record 1 (id is a string, not integer) and record 2 (missing id) both fail.
  await expect(out).toContainText('Record #1');
  await expect(out).toContainText('/id: "2" is not of type "integer"');
  await expect(out).toContainText('Record #2');
  await expect(out).toContainText('(root): "id" is a required property');
});

test('json-schema-batch-validate page honors a deep-linked NDJSON batch', async ({ page }) => {
  const ndjsonSchema =
    '{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","required":["email"],"properties":{"email":{"type":"string","format":"email"},"age":{"type":"integer","minimum":0,"maximum":120}}}';
  const ndjsonRecords = '{"email":"a@b.com","age":30}\n{"email":"nope","age":200}\n{"age":45}';
  const qs =
    '?schema=' + encodeURIComponent(ndjsonSchema) +
    '&records=' + encodeURIComponent(ndjsonRecords) +
    '&input_format=ndjson' +
    '&draft=auto' +
    '&max_errors=50' +
    '&output=text';
  await page.goto('/tools/json-schema-batch-validate/' + qs);

  // Deep-linked params populate the controls.
  await expect(page.locator('#in-input_format')).toHaveValue('ndjson', { timeout: 15_000 });
  await expect(page.locator('#in-schema')).toHaveValue(ndjsonSchema);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Batch validation: FAIL', { timeout: 15_000 });
  await expect(out).toContainText('Input: ndjson');
  await expect(out).toContainText('Records: 3 (1 passed, 2 failed)');
  await expect(out).toContainText('/age: 200 is greater than the maximum of 120');
  await expect(out).toContainText('(root): "email" is a required property');
});

test('json-schema-batch-validate page emits an all-pass JSON report', async ({ page }) => {
  await page.goto('/tools/json-schema-batch-validate/');
  await page.fill('#in-schema', '{"type":"integer","minimum":0}');
  await page.fill('#in-records', '[1,2,3]');
  await page.selectOption('#in-output', 'json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"valid": true', { timeout: 15_000 });
  const report = JSON.parse((await out.textContent()) ?? '{}');
  expect(report.valid).toBe(true);
  expect(report.total).toBe(3);
  expect(report.passed).toBe(3);
  expect(report.failed).toBe(0);
});

test('json-schema-batch-validate page reports invalid schema JSON clearly', async ({ page }) => {
  await page.goto('/tools/json-schema-batch-validate/');
  await page.fill('#in-schema', '{not json');
  await page.fill('#in-records', '[{"a":1}]');
  await expect(page.locator('#tool-output')).toContainText('schema is not valid JSON', { timeout: 15_000 });
});
