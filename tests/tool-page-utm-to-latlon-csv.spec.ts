import { test, expect } from './fixtures';

const tool = '/tools/utm-to-latlon-csv/';
const sample = 'id,easting,northing,zone\np1,583960,4507523,18T';

async function runWasm(
  page: import('@playwright/test').Page,
  params: Partial<Record<string, string>> = {},
) {
  const p = {
    input: sample,
    direction: 'utm_to_latlon',
    easting_column: '',
    northing_column: '',
    zone_column: '',
    zone: '',
    hemisphere: 'auto',
    coord_format: 'decimal',
    decimals: '6',
    ellipsoid: 'wgs84',
    delimiter: 'auto',
    has_header: 'true',
    keep_columns: 'true',
    validate_ranges: 'true',
    output: 'csv',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/utm-to-latlon-csv/gizza_ai_utm_to_latlon_csv_web.js');
    await mod.default('/tools/utm-to-latlon-csv/gizza_ai_utm_to_latlon_csv_web_bg.wasm');
    return mod.run(
      args.input,
      args.direction,
      args.easting_column,
      args.northing_column,
      args.zone_column,
      args.zone,
      args.hemisphere,
      args.coord_format,
      args.decimals,
      args.ellipsoid,
      args.delimiter,
      args.has_header,
      args.keep_columns,
      args.validate_ranges,
      args.output,
    );
  }, p);
}

test('utm-to-latlon-csv page converts UTM rows to latitude and longitude', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-input', sample);

  const output = page.locator('#tool-output');
  await expect(output).toContainText('id,latitude,longitude', { timeout: 20_000 });
  await expect(output).toContainText('p1,40.714349,-74.005970');
});

test('utm-to-latlon-csv deep link can reverse lat/lon rows to UTM', async ({ page }) => {
  const qs = new URLSearchParams({
    input: 'id,latitude,longitude\nnyc,40.7128,-74.0060',
    direction: 'latlon_to_utm',
    decimals: '3',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-direction')).toHaveValue('latlon_to_utm', { timeout: 15_000 });
  await expect(page.locator('#in-decimals')).toHaveValue('3');
  await expect(page.locator('#in-input')).toHaveValue('id,latitude,longitude\nnyc,40.7128,-74.0060');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('id,easting,northing,zone,hemisphere', { timeout: 20_000 });
  await expect(output).toContainText('nyc,583959.372,4507350.998,18T,N');
});

test('utm-to-latlon-csv wasm covers advertised formats and validation', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-input');

  expect(await runWasm(page)).toBe('id,latitude,longitude\np1,40.714349,-74.005970\n');
  expect(await runWasm(page, { coord_format: 'dms' })).toBe(
    'id,latitude,longitude\np1,"40°42\'51.66""N","74°00\'21.49""W"\n',
  );
  expect(await runWasm(page, { coord_format: 'ddm' })).toBe(
    "id,latitude,longitude\np1,40°42.8610'N,74°00.3582'W\n",
  );
  expect(await runWasm(page, { output: 'tsv', decimals: '4' })).toBe(
    'id\tlatitude\tlongitude\np1\t40.7143\t-74.0060\n',
  );
  expect(await runWasm(page, { output: 'json', decimals: '4' })).toContain(
    '{"id": "p1", "latitude": 40.7143, "longitude": -74.0060}',
  );
  expect(await runWasm(page, { output: 'geojson' })).toContain(
    '"coordinates": [-74.005970, 40.714349]',
  );
  expect(await runWasm(page, { output: 'kml' })).toContain(
    '<coordinates>-74.005970,40.714349</coordinates>',
  );

  const south = await runWasm(page, {
    input: 'name,easting,northing,zone\nsydney,334519,6251430,56H',
  });
  expect(south).toBe('name,latitude,longitude\nsydney,-33.864482,151.211016\n');

  const semicolon = await runWasm(page, {
    input: 'easting;northing;zone\n583960;4507523;18T',
    decimals: '4',
  });
  expect(semicolon).toBe('latitude;longitude\n40.7143;-74.0060\n');

  const noHeader = await runWasm(page, {
    input: '583960,4507523',
    zone: '18N',
    has_header: 'false',
    keep_columns: 'false',
    decimals: '4',
  });
  expect(noHeader).toBe('40.7143,-74.0060\n');

  await expect(
    runWasm(page, { input: 'easting,northing,zone\n4507523,583960,18T' }),
  ).rejects.toThrow(/look swapped/);
});
