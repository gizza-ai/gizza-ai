import { test, expect } from './fixtures';

const DATA = `[
  {"name":"Ada","age":36,"team":{"name":"core"},"tags":["math","code"]},
  {"name":"Bo","age":24,"team":{"name":"infra"},"tags":["ops"]},
  {"name":"Cy","age":41,"team":{"name":"core"},"tags":["code","ops"]}
]`;

async function fillMongoQuery(page, opts: {
  data?: string;
  query?: string;
  projection?: string;
  sort?: string;
  skip?: string;
  limit?: string;
  format?: string;
  pretty?: boolean;
}) {
  if (opts.data !== undefined) await page.fill('#in-data', opts.data);
  if (opts.query !== undefined) await page.fill('#in-query', opts.query);
  if (opts.projection !== undefined) await page.fill('#in-projection', opts.projection);
  if (opts.sort !== undefined) await page.fill('#in-sort', opts.sort);
  if (opts.skip !== undefined) await page.fill('#in-skip', opts.skip);
  if (opts.limit !== undefined) await page.fill('#in-limit', opts.limit);
  if (opts.format !== undefined) await page.selectOption('#in-format', opts.format);
  if (opts.pretty !== undefined) opts.pretty ? await page.check('#in-pretty') : await page.uncheck('#in-pretty');
}

test('mongo-query filters, projects, sorts, and pretty-prints JSON', async ({ page }) => {
  await page.goto('/tools/mongo-query/');
  await fillMongoQuery(page, {
    data: DATA,
    query: '{"tags": {"$in": ["code"]}}',
    projection: 'name, age, -_id',
    sort: 'age:desc',
    format: 'json',
    pretty: true,
  });

  await expect(page.locator('#tool-output')).toHaveText('[\n  {\n    "name": "Cy",\n    "age": 41\n  },\n  {\n    "name": "Ada",\n    "age": 36\n  }\n]', { timeout: 20000 });
});

test('mongo-query supports all output formats and compact JSON', async ({ page }) => {
  await page.goto('/tools/mongo-query/');
  await fillMongoQuery(page, {
    data: DATA,
    query: '{"team.name":"core"}',
    projection: 'name,age,-_id',
    sort: 'name',
    pretty: false,
  });

  await page.selectOption('#in-format', 'json');
  await expect(page.locator('#tool-output')).toHaveText('[{"name":"Ada","age":36},{"name":"Cy","age":41}]', { timeout: 20000 });

  await page.selectOption('#in-format', 'ndjson');
  await expect(page.locator('#tool-output')).toHaveText('{"name":"Ada","age":36}\n{"name":"Cy","age":41}', { timeout: 20000 });

  await page.selectOption('#in-format', 'csv');
  await expect(page.locator('#tool-output')).toHaveText('name,age\nAda,36\nCy,41', { timeout: 20000 });

  await page.selectOption('#in-format', 'count');
  await expect(page.locator('#tool-output')).toHaveText('2', { timeout: 20000 });
});

test('mongo-query applies a deep-link query with limit', async ({ page }) => {
  const qs =
    '?data=' + encodeURIComponent(DATA) +
    '&query=' + encodeURIComponent('{name: /^A/}') +
    '&projection=' + encodeURIComponent('name,-_id') +
    '&sort=' + encodeURIComponent('age:desc') +
    '&skip=0' +
    '&limit=1' +
    '&format=ndjson' +
    '&pretty=false';

  await page.goto('/tools/mongo-query/' + qs);
  await expect(page.locator('#in-format')).toHaveValue('ndjson', { timeout: 15000 });
  await expect(page.locator('#in-pretty')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('{"name":"Ada"}', { timeout: 20000 });
});

test('mongo-query reports unsupported operators clearly', async ({ page }) => {
  await page.goto('/tools/mongo-query/');
  await fillMongoQuery(page, {
    data: DATA,
    query: '{"$where":"this.age > 30"}',
  });

  await expect(page.locator('#tool-output')).toHaveClass(/error/, { timeout: 20000 });
  await expect(page.locator('#tool-output')).toContainText('$where');
  await expect(page.locator('#tool-output')).toContainText('JavaScript');
});
