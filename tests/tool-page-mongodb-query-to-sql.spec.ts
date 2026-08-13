import { test, expect } from './fixtures';

const BASIC = 'db.users.find({ age: { $gte: 21 }, status: "active" })';
const SELECT_QUERY = 'db.orders.find({ status: { $in: ["paid", "shipped"] }, total: { $gt: 100 } }, { _id: 0, orderId: 1, total: 1 }).sort({ total: -1 }).limit(10).skip(20)';
const SELECT_SQL = 'SELECT "orderId", "total"\nFROM "orders"\nWHERE "status" IN (\'paid\', \'shipped\') AND "total" > 100\nORDER BY "total" DESC\nLIMIT 10 OFFSET 20;';

async function runWasm(
  page: any,
  query = BASIC,
  output = 'where',
  dialect = 'ansi',
  table = '',
  nested = 'column',
  quote_identifiers = true,
  rename_id = false,
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/mongodb-query-to-sql/gizza_ai_mongodb_query_to_sql_web.js');
    await mod.default('/tools/mongodb-query-to-sql/gizza_ai_mongodb_query_to_sql_web_bg.wasm');
    return mod.run(
      args.query,
      args.output,
      args.dialect,
      args.table,
      args.nested,
      args.quote_identifiers ? 'true' : 'false',
      args.rename_id ? 'true' : 'false',
    );
  }, { query, output, dialect, table, nested, quote_identifiers, rename_id });
}

test('mongodb-query-to-sql wasm emits exact SQL', async ({ page }) => {
  await page.goto('/tools/mongodb-query-to-sql/');
  await page.waitForSelector('#in-query');

  expect(await runWasm(page)).toBe('WHERE "age" >= 21 AND "status" = \'active\'');
  expect(await runWasm(page, '{ name: "Ada" }', 'condition')).toBe('"name" = \'Ada\'');
  expect(await runWasm(page, SELECT_QUERY, 'select')).toBe(SELECT_SQL);
});

test('mongodb-query-to-sql page computes output from form controls', async ({ page }) => {
  await page.goto('/tools/mongodb-query-to-sql/');
  await page.fill('#in-query', BASIC);
  await page.selectOption('#in-output', 'where');
  await page.selectOption('#in-dialect', 'ansi');
  await page.fill('#in-table', '');
  await page.selectOption('#in-nested', 'column');
  await page.check('#in-quote_identifiers');
  await expect(page.locator('#in-rename_id')).not.toBeChecked();

  await expect(page.locator('#tool-output')).toHaveText('WHERE "age" >= 21 AND "status" = \'active\'', { timeout: 15_000 });
});

test('mongodb-query-to-sql deep link wires json paths and checkbox states', async ({ page }) => {
  const params = new URLSearchParams({
    query: '{ "address.city": "Berlin", "meta.score": { $gte: 10 } }',
    output: 'condition',
    dialect: 'postgres',
    table: '',
    nested: 'json',
    quote_identifiers: 'false',
    rename_id: 'true',
  });
  await page.goto(`/tools/mongodb-query-to-sql/?${params.toString()}`);

  await expect(page.locator('#in-dialect')).toHaveValue('postgres', { timeout: 15_000 });
  await expect(page.locator('#in-nested')).toHaveValue('json');
  await expect(page.locator('#in-quote_identifiers')).not.toBeChecked();
  await expect(page.locator('#in-rename_id')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText("address->>'city' = 'Berlin' AND (meta->>'score')::numeric >= 10", { timeout: 15_000 });
});

test('mongodb-query-to-sql advertised dialects and switches stay wired', async ({ page }) => {
  await page.goto('/tools/mongodb-query-to-sql/');
  await page.waitForSelector('#in-query');

  expect(await runWasm(page, '{ name: /^ada/i }', 'condition', 'postgres')).toBe('"name" ~* \'^ada\'');
  expect(await runWasm(page, '{ name: /^ada/ }', 'condition', 'mysql')).toBe('REGEXP_LIKE(`name`, \'^ada\', \'c\')');
  expect(await runWasm(page, 'db.users.find({ active: true }).sort({ createdAt: -1 }).skip(10).limit(5)', 'select', 'sqlserver'))
    .toBe('SELECT *\nFROM [users]\nWHERE [active] = 1\nORDER BY [createdAt] DESC\nOFFSET 10 ROWS FETCH NEXT 5 ROWS ONLY;');
  expect(await runWasm(page, '{ _id: ObjectId("64b1") }', 'where', 'ansi', '', 'column', false, true)).toBe("WHERE id = '64b1'");
  await expect(runWasm(page, '{ a: { $wat: 1 } }')).rejects.toThrow(/unsupported operator \$wat/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool mongodb-query-to-sql');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
