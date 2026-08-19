import { test, expect } from './fixtures';

const FEATURE = '{"type":"Feature","properties":{"name":"Park","note":"","rank":2},"geometry":{"type":"Point","coordinates":[12.3456789,-9.8765432]}}';
const COLLECTION = '{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"name":"A","note":"","rank":2},"geometry":{"type":"Point","coordinates":[12.3456789,-9.8765432]}}]}';
const LINE_COLLECTION = '{"type":"FeatureCollection","features":[{"type":"Feature","properties":{},"geometry":{"type":"LineString","coordinates":[[0,1],[4,-2]]}}]}';
const BAD_BUT_FORMATTABLE = '{"type":"Point","coordinates":[200,10]}';
const BAD_POLYGON = '{"type":"Polygon","coordinates":[[[0,0],[0,1],[1,1],[1,0]]]}';
const WRONG_WINDING = '{"type":"Polygon","coordinates":[[[0,0],[0,10],[10,10],[10,0],[0,0]],[[2,2],[4,2],[4,4],[2,4],[2,2]]]}';

async function runWasm(
  page: any,
  input = FEATURE,
  indent = '2',
  indentChar = 'space',
  precision = '-1',
  keyOrder = 'keep',
  bbox = 'keep',
  winding = 'keep',
  keepProperties = '',
  dropProperties = '',
  dropEmptyProperties = 'false',
  validate = 'true',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/geojson-format/gizza_ai_geojson_format_web.js');
    await mod.default('/tools/geojson-format/gizza_ai_geojson_format_web_bg.wasm');
    return mod.run(
      args.input,
      args.indent,
      args.indentChar,
      args.precision,
      args.keyOrder,
      args.bbox,
      args.winding,
      args.keepProperties,
      args.dropProperties,
      args.dropEmptyProperties,
      args.validate,
    );
  }, { input, indent, indentChar, precision, keyOrder, bbox, winding, keepProperties, dropProperties, dropEmptyProperties, validate });
}

test('geojson-format page pretty-prints real GeoJSON from the form', async ({ page }) => {
  await page.goto('/tools/geojson-format/');
  await page.fill('#in-input', FEATURE);
  await expect(page.locator('#tool-output')).toContainText('"type": "Feature"', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('"coordinates"');
  await expect(page.locator('#tool-output')).toContainText('12.3456789');
});

test('geojson-format deep link covers minify, rounding, bbox and validate=false checkbox', async ({ page }) => {
  const params = new URLSearchParams({
    input: BAD_BUT_FORMATTABLE,
    indent: '0',
    precision: '1',
    key_order: 'canonical',
    bbox: 'add',
    validate: 'false',
  });
  await page.goto(`/tools/geojson-format/?${params.toString()}`);

  await expect(page.locator('#in-indent')).toHaveValue('0', { timeout: 15_000 });
  await expect(page.locator('#in-precision')).toHaveValue('1');
  await expect(page.locator('#in-key_order')).toHaveValue('canonical');
  await expect(page.locator('#in-bbox')).toHaveValue('add');
  await expect(page.locator('#in-validate')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('{"type":"Point","bbox":[200.0,10.0,200.0,10.0],"coordinates":[200.0,10.0]}', { timeout: 15_000 });
});

test('geojson-format wasm covers formatting options and enum values', async ({ page }) => {
  await page.goto('/tools/geojson-format/');
  await page.waitForSelector('#in-input');

  const pretty = await runWasm(page, FEATURE);
  expect(pretty).toContain('\n  "type": "Feature"');
  expect(pretty).toContain('"note": ""');

  const minified = await runWasm(page, FEATURE, '0');
  expect(minified).not.toContain('\n');
  expect(minified).toContain('{"type":"Feature"');

  const rounded = await runWasm(page, FEATURE, '0', 'space', '4');
  expect(rounded).toContain('[12.3457,-9.8765]');

  const tabbed = await runWasm(page, FEATURE, '1', 'tab');
  expect(tabbed).toContain('\n\t"type": "Feature"');

  const canonical = await runWasm(page, '{"geometry":{"coordinates":[1,2],"type":"Point"},"properties":{"z":1,"a":2},"type":"Feature","id":7}', '0', 'space', '-1', 'canonical');
  expect(canonical).toBe('{"type":"Feature","id":7,"geometry":{"type":"Point","coordinates":[1,2]},"properties":{"z":1,"a":2}}');

  const alpha = await runWasm(page, '{"type":"Feature","properties":{"z":1,"a":2},"geometry":{"type":"Point","coordinates":[1,2]}}', '0', 'space', '-1', 'alpha');
  expect(alpha).toBe('{"geometry":{"coordinates":[1,2],"type":"Point"},"properties":{"a":2,"z":1},"type":"Feature"}');

  const addBbox = await runWasm(page, LINE_COLLECTION, '0', 'space', '-1', 'keep', 'features');
  const addBboxJson = JSON.parse(addBbox);
  expect(addBboxJson.bbox).toEqual([0, -2, 4, 1]);
  expect(addBboxJson.features[0].bbox).toEqual([0, -2, 4, 1]);

  const stripped = await runWasm(page, '{"type":"FeatureCollection","bbox":[0,0,1,1],"features":[{"type":"Feature","bbox":[0,0,1,1],"properties":{},"geometry":{"type":"Point","coordinates":[0,0]}}]}', '0', 'space', '-1', 'keep', 'strip');
  expect(stripped).not.toContain('bbox');

  const rewound = JSON.parse(await runWasm(page, WRONG_WINDING, '0', 'space', '-1', 'keep', 'keep', 'rfc7946'));
  expect(rewound.coordinates[0][0]).toEqual([0, 0]);
  expect(rewound.coordinates[0][1]).toEqual([10, 0]);
  expect(rewound.coordinates[1][1]).toEqual([2, 4]);
});

test('geojson-format wasm covers property pruning, validation errors and boundaries', async ({ page }) => {
  await page.goto('/tools/geojson-format/');
  await page.waitForSelector('#in-input');

  const keepOnly = await runWasm(page, COLLECTION, '0', 'space', '-1', 'keep', 'keep', 'keep', 'name');
  expect(keepOnly).toContain('"properties":{"name":"A"}');

  const dropEmpty = await runWasm(page, COLLECTION, '0', 'space', '-1', 'keep', 'keep', 'keep', '', 'rank', 'true');
  expect(dropEmpty).toContain('"properties":{"name":"A"}');

  await expect(runWasm(page, BAD_BUT_FORMATTABLE)).rejects.toThrow(/longitude 200 is outside/);
  expect(await runWasm(page, BAD_BUT_FORMATTABLE, '0', 'space', '-1', 'keep', 'keep', 'keep', '', '', 'false', 'false'))
    .toBe('{"type":"Point","coordinates":[200,10]}');
  await expect(runWasm(page, BAD_POLYGON)).rejects.toThrow(/ring is not closed/);

  await expect(runWasm(page, FEATURE, '8')).resolves.toContain('\n        "type"');
  await expect(runWasm(page, FEATURE, '9')).rejects.toThrow(/indent must be between 0 and 8/);
  await expect(runWasm(page, FEATURE, '0', 'space', '15')).resolves.toContain('12.3456789');
  await expect(runWasm(page, FEATURE, '0', 'space', '16')).rejects.toThrow(/precision must be between -1 and 15/);
});

test('geojson-format generated CLI example is generic and runnable-looking', async ({ page }) => {
  await page.goto('/tools/geojson-format/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool geojson-format');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
