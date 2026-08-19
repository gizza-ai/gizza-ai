import { test, expect } from './fixtures';

const DOC = '{"id":"123","title":"My first post!","author":{"id":"1","name":"Paul"},"comments":[{"id":"324","commenter":{"id":"2","name":"Nicole"}},{"id":"325","commenter":{"id":"1","name":"Paul"}}]}';
const SCHEMA = 'articles: author -> users, comments -> [comments]\ncomments: commenter -> users\nusers:';
const EXACT = '{"entities":{"articles":{"123":{"id":"123","title":"My first post!","author":"1","comments":["324","325"]}},"comments":{"324":{"id":"324","commenter":"2"},"325":{"id":"325","commenter":"1"}},"users":{"1":{"id":"1","name":"Paul"},"2":{"id":"2","name":"Nicole"}}},"result":"123"}';

async function runWasm(
  page: any,
  json = DOC,
  schema = SCHEMA,
  root = 'articles',
  path = '',
  id_field = 'id',
  on_missing_id = 'error',
  on_conflict = 'merge',
  output = 'normalized',
  pretty = false,
  indent = 2,
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/json-normalize/gizza_ai_json_normalize_web.js');
    await mod.default('/tools/json-normalize/gizza_ai_json_normalize_web_bg.wasm');
    return mod.run(
      args.json,
      args.schema,
      args.root,
      args.path,
      args.id_field,
      args.on_missing_id,
      args.on_conflict,
      args.output,
      args.pretty,
      args.indent,
    );
  }, { json, schema, root, path, id_field, on_missing_id, on_conflict, output, pretty, indent });
}

test('json-normalize wasm normalizes nested objects to exact compact JSON', async ({ page }) => {
  await page.goto('/tools/json-normalize/');
  await page.waitForSelector('#in-json');

  const out = await runWasm(page);
  expect(out).toBe(EXACT);

  const report = await runWasm(page, DOC, SCHEMA, 'articles', '', 'id', 'error', 'merge', 'report', true, 2);
  expect(report).toContain('Root entity: articles');
  expect(report).toContain('users: 2 entities from 3 occurrences (1 merged)');
});

test('json-normalize page computes exact entities output from the form', async ({ page }) => {
  await page.goto('/tools/json-normalize/');
  await page.fill('#in-json', DOC);
  await page.fill('#in-schema', SCHEMA);
  await page.fill('#in-root', 'articles');
  await page.selectOption('#in-output', 'normalized');
  await page.uncheck('#in-pretty');

  await expect(page.locator('#tool-output')).toHaveText(EXACT, { timeout: 15_000 });
});

test('json-normalize deep link covers path, entities output, and pretty checkbox off', async ({ page }) => {
  const params = new URLSearchParams({
    json: '{"meta":{"page":1},"data":{"items":[{"id":"a"},{"id":"b"}]}}',
    schema: 'items:',
    root: 'items',
    path: 'data.items',
    id_field: 'id',
    on_missing_id: 'error',
    on_conflict: 'merge',
    output: 'entities',
    pretty: 'false',
    indent: '2',
  });
  await page.goto(`/tools/json-normalize/?${params.toString()}`);

  await expect(page.locator('#in-output')).toHaveValue('entities', { timeout: 15_000 });
  await expect(page.locator('#in-path')).toHaveValue('data.items');
  await expect(page.locator('#in-pretty')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('{"items":{"a":{"id":"a"},"b":{"id":"b"}}}', { timeout: 15_000 });
});

test('json-normalize covers custom id fields, missing-id hash, conflict enum, and CLI example', async ({ page }) => {
  await page.goto('/tools/json-normalize/');
  await page.waitForSelector('#in-json');

  const custom = await runWasm(
    page,
    '{"id_str":"123","user":{"id_str":"456","name":"Jimmy"}}',
    'tweets: user -> users\nusers:',
    'tweets',
    '',
    '{"*":"id_str"}',
    'error',
    'merge',
    'normalized',
    false,
    2,
  );
  expect(custom).toContain('"tweets":{"123"');
  expect(custom).toContain('"user":"456"');

  const report = await runWasm(
    page,
    '[{"id":1,"author":{"name":"Ada"}},{"id":2,"author":{"name":"Ada"}}]',
    'posts: author -> users\nusers:',
    'posts',
    '',
    'id',
    'hash',
    'merge',
    'report',
    true,
    2,
  );
  expect(report).toContain('Synthesized ids: 2');

  await expect(runWasm(page, '[{"id":1},{"id":1}]', 'rows:', 'rows', '', 'id', 'error', 'error', 'normalized', false, 2))
    .rejects.toThrow(/share the id/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool json-normalize');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
