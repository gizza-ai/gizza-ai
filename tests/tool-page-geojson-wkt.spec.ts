import { test, expect } from './fixtures';

const geojsonPoint = '{"type":"Point","coordinates":[30,10]}';
const ewktPolygon = 'SRID=4326;POLYGON((30 10,40 40,20 40,10 20,30 10))';

async function setTextarea(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page: any,
  input: string,
  from = 'auto',
  to = 'wkt',
  multi = 'collection',
  srid = '0',
  precision = '-1',
  wkbEncoding = 'hex',
  wkbEndian = 'little',
  pretty = 'true',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/geojson-wkt/gizza_ai_geojson_wkt_web.js');
    await mod.default('/tools/geojson-wkt/gizza_ai_geojson_wkt_web_bg.wasm');
    return mod.run(
      args.input,
      args.from,
      args.to,
      args.multi,
      args.srid,
      args.precision,
      args.wkbEncoding,
      args.wkbEndian,
      args.pretty,
    );
  }, { input, from, to, multi, srid, precision, wkbEncoding, wkbEndian, pretty });
}

test('geojson-wkt wasm converts GeoJSON, WKT, and WKB with exact outputs', async ({ page }) => {
  await page.goto('/tools/geojson-wkt/');
  await page.waitForSelector('#in-input');

  await expect(runWasm(page, geojsonPoint)).resolves.toBe('POINT(30 10)');

  const json = await runWasm(page, 'POINT Z (1 2 3)', 'wkt', 'geojson', 'collection', '0', '-1', 'hex', 'little', 'false');
  expect(json).toBe('{"type":"Point","coordinates":[1,2,3]}');

  const hex = await runWasm(page, 'POINT(1 2)', 'wkt', 'wkb', 'collection', '0', '-1', 'hex', 'little');
  expect(hex).toBe('0101000000000000000000F03F0000000000000040');

  const back = await runWasm(page, hex, 'wkb', 'wkt');
  expect(back).toBe('POINT(1 2)');
});

test('geojson-wkt wasm covers enum choices, boundaries, and error output', async ({ page }) => {
  await page.goto('/tools/geojson-wkt/');
  await page.waitForSelector('#in-input');

  const lines = await runWasm(
    page,
    '{"type":"FeatureCollection","features":[{"type":"Feature","properties":{},"geometry":{"type":"Point","coordinates":[1,2]}},{"type":"Feature","properties":{},"geometry":{"type":"Point","coordinates":[3,4]}}]}',
    'geojson',
    'wkt',
    'lines',
  );
  expect(lines).toBe('POINT(1 2)\nPOINT(3 4)');

  const ewkb = await runWasm(page, 'POINT(1 2)', 'wkt', 'wkb', 'collection', '999999', '0', 'base64', 'big');
  expect(ewkb).toMatch(/^[A-Za-z0-9+/=]+$/);
  const ewkt = await runWasm(page, ewkb, 'wkb', 'wkt');
  expect(ewkt).toContain('SRID=999999;POINT(1 2)');

  await expect(runWasm(page, 'CIRCULARSTRING(1 1,2 2,3 1)', 'wkt')).rejects.toThrow(/CIRCULARSTRING is not supported/);
});

test('geojson-wkt page renders exact output and honors controls', async ({ page }) => {
  await page.goto('/tools/geojson-wkt/');
  await setTextarea(page, '#in-input', ewktPolygon);
  await page.selectOption('#in-from', 'wkt');
  await page.selectOption('#in-to', 'geojson');
  await page.fill('#in-precision', '0');
  await page.uncheck('#in-pretty');

  await expect(page.locator('#tool-output')).toContainText('{"type":"Polygon","coordinates":[[[30,10],[40,40],[20,40],[10,20],[30,10]]]}', { timeout: 15_000 });
});

test('geojson-wkt deep-link prefills params and generated CLI example is generic', async ({ page }) => {
  const params = new URLSearchParams({
    input: geojsonPoint,
    from: 'geojson',
    to: 'wkt',
    multi: 'collection',
    srid: '4326',
    precision: '0',
    wkb_encoding: 'hex',
    wkb_endian: 'little',
    pretty: 'true',
  });

  await page.goto(`/tools/geojson-wkt/?${params.toString()}`);
  await expect(page.locator('#in-input')).toHaveValue(geojsonPoint, { timeout: 15_000 });
  await expect(page.locator('#in-to')).toHaveValue('wkt');
  await expect(page.locator('#tool-output')).toContainText('SRID=4326;POINT(30 10)', { timeout: 15_000 });

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool geojson-wkt');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
