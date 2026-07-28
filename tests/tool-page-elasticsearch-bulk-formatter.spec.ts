import { test, expect } from './fixtures';

const DOCS = '[{"id":"1","title":"hello"},{"id":"2","title":"world"}]';
const INDEX_OUTPUT = '{"index":{"_index":"my-index","_id":"1"}}\n{"title":"hello"}\n{"index":{"_index":"my-index","_id":"2"}}\n{"title":"world"}\n';
const UPDATE_OUTPUT = '{"update":{"_index":"tasks","_id":"7"}}\n{"doc":{"status":"done"},"doc_as_upsert":true}\n';
const DELETE_OUTPUT = '{"delete":{"_index":"docs","_id":"3"}}\n{"delete":{"_index":"docs","_id":"4"}}\n';

test('elasticsearch-bulk-formatter page builds index NDJSON', async ({ page }) => {
  await page.goto('/tools/elasticsearch-bulk-formatter/');
  await page.fill('#in-documents', DOCS);
  await page.selectOption('#in-action', 'index');
  await page.fill('#in-index', 'my-index');
  await page.fill('#in-id_field', 'id');
  await expect(page.locator('#tool-output')).toHaveText(INDEX_OUTPUT, { timeout: 15_000 });
});

test('elasticsearch-bulk-formatter deep-link supports update upsert', async ({ page }) => {
  const params = new URLSearchParams({
    documents: '[{"id":"7","status":"done"}]',
    action: 'update',
    index: 'tasks',
    id_field: 'id',
    doc_as_upsert: 'true',
  });
  await page.goto(`/tools/elasticsearch-bulk-formatter/?${params.toString()}`);
  await expect(page.locator('#in-documents')).toHaveValue('[{"id":"7","status":"done"}]', { timeout: 15_000 });
  await expect(page.locator('#in-action')).toHaveValue('update');
  await expect(page.locator('#in-index')).toHaveValue('tasks');
  await expect(page.locator('#in-id_field')).toHaveValue('id');
  await expect(page.locator('#in-doc_as_upsert')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText(UPDATE_OUTPUT, { timeout: 15_000 });
});

test('elasticsearch-bulk-formatter page supports delete metadata-only output', async ({ page }) => {
  await page.goto('/tools/elasticsearch-bulk-formatter/');
  await page.fill('#in-documents', '[{"id":"3"},{"id":"4"}]');
  await page.selectOption('#in-action', 'delete');
  await page.fill('#in-index', 'docs');
  await page.fill('#in-id_field', 'id');
  await expect(page.locator('#tool-output')).toHaveText(DELETE_OUTPUT, { timeout: 15_000 });
});
