import { test, expect } from './fixtures';

const fc = '{"type":"FeatureCollection","features":[{"type":"Feature","id":"a","properties":{"name":"Springfield","country":"US","population":120000},"geometry":{"type":"Point","coordinates":[-89.65,39.78]}},{"type":"Feature","id":"b","properties":{"name":"Shelbyville","country":"US","population":30000},"geometry":{"type":"Point","coordinates":[-88.79,39.40]}},{"type":"Feature","id":"c","properties":{"name":"Ogdenville","country":"CA","population":250000},"geometry":{"type":"Point","coordinates":[-114.07,51.05]}},{"type":"Feature","id":"d","properties":{"name":"Zone","country":"US"},"geometry":{"type":"Polygon","coordinates":[[[-90,39],[-89,39],[-89,40],[-90,40],[-90,39]]]}}]}';

async function setTextarea(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page: any,
  input: string,
  query = 'SELECT *',
  bbox = '',
  format = 'geojson',
  indent = '0',
  includeBBox = 'false',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/geojson-query/gizza_ai_geojson_query_web.js');
    await mod.default('/tools/geojson-query/gizza_ai_geojson_query_web_bg.wasm');
    return mod.run(args.input, args.query, args.bbox, args.format, args.indent, args.includeBBox);
  }, { input, query, bbox, format, indent, includeBBox });
}

test('geojson-query wasm filters, projects, sorts, and emits JSON rows', async ({ page }) => {
  await page.goto('/tools/geojson-query/');
  await page.waitForSelector('#in-input');

  const out = await runWasm(
    page,
    fc,
    "SELECT name, population WHERE country = 'US' ORDER BY population DESC",
    '',
    'json',
  );
  expect(out).toBe('[{"name":"Springfield","population":120000},{"name":"Shelbyville","population":30000},{"name":"Zone","population":null}]');
});

test('geojson-query wasm covers spatial predicates, bbox parameter, and CSV aggregate', async ({ page }) => {
  await page.goto('/tools/geojson-query/');
  await page.waitForSelector('#in-input');

  const bboxOut = await runWasm(page, fc, 'SELECT name ORDER BY name', '-90,39,-88,40', 'json');
  expect(bboxOut).toBe('[{"name":"Shelbyville"},{"name":"Springfield"},{"name":"Zone"}]');

  const containsOut = await runWasm(page, fc, 'SELECT name WHERE contains(-89.5, 39.5)', '', 'json');
  expect(containsOut).toBe('[{"name":"Zone"}]');

  const csv = await runWasm(
    page,
    fc,
    'SELECT country, count(*) AS features, sum(population) AS people GROUP BY country ORDER BY people DESC',
    '',
    'csv',
  );
  expect(csv).toBe('country,features,people\nCA,1,250000\nUS,3,150000\n');
});

test('geojson-query page renders result and honors controls', async ({ page }) => {
  await page.goto('/tools/geojson-query/');
  await setTextarea(page, '#in-input', fc);
  await setTextarea(page, '#in-query', "SELECT name WHERE country = 'CA'");
  await page.selectOption('#in-format', 'json');
  await page.fill('#in-indent', '0');

  await expect(page.locator('#tool-output')).toContainText('[{"name":"Ogdenville"}]', { timeout: 15_000 });
});

test('geojson-query deep-link prefills params and generated CLI example is generic', async ({ page }) => {
  const params = new URLSearchParams({
    input: fc,
    query: "SELECT name WHERE $type = 'Polygon'",
    bbox: '',
    format: 'json',
    indent: '0',
    include_bbox: 'false',
  });

  await page.goto(`/tools/geojson-query/?${params.toString()}`);
  await expect(page.locator('#in-input')).toHaveValue(fc, { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toContainText('[{"name":"Zone"}]', { timeout: 15_000 });

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool geojson-query');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('geojson-query wasm returns actionable parse errors', async ({ page }) => {
  await page.goto('/tools/geojson-query/');
  await page.waitForSelector('#in-input');

  await expect(runWasm(page, fc, 'SELECT * WHERE bbox(1, 2)')).rejects.toThrow(/4 numbers/);
});
