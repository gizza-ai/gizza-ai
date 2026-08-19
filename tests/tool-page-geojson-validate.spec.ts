import { test, expect } from './fixtures';

const VALID_FC =
  '{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"name":"Block"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]]}}]}';
const UNCLOSED = '{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1]]]}';
const CLOCKWISE = '{"type":"Polygon","coordinates":[[[0,0],[0,4],[4,4],[4,0],[0,0]]]}';
const PROJECTED = '{"type":"Point","coordinates":[-13627361.03,4544761.85]}';
const WITH_BBOX = '{"type":"Point","bbox":[0,0,1,1],"coordinates":[0,0]}';

const VALID_FC_REPORT = [
  'VALID — 0 errors, 0 warnings',
  '',
  'Summary',
  '  document type: FeatureCollection',
  '  features: 1',
  '  geometries: 1 (Polygon 1)',
  '  positions: 5',
  '  rings: 1 (1 exterior, 0 interior)',
  '  bbox members: 0',
  '  bounds: longitude 0 .. 1, latitude 0 .. 1',
  '',
].join('\n');

const UNCLOSED_REPORT = [
  'INVALID — 1 error, 1 warning',
  '',
  'Errors',
  '  1. [ring-not-closed] (root).coordinates[0]: the ring is not closed — its last position must repeat its first ([0, 1] vs [0, 0])',
  '',
  'Warnings',
  '  1. [bare-geometry] (root): the document is a bare geometry — valid GeoJSON, but many tools expect a Feature or FeatureCollection wrapper',
  '',
  'Summary',
  '  document type: Polygon',
  '  features: 0',
  '  geometries: 1 (Polygon 1)',
  '  positions: 4',
  '  rings: 1 (1 exterior, 0 interior)',
  '  bbox members: 0',
  '  bounds: longitude 0 .. 1, latitude 0 .. 1',
  '',
].join('\n');

// strict_winding off + warn_problematic off: the winding violation survives as a
// warning (it is reported explicitly, not through the problematic family) while
// the bare-geometry note is suppressed — so the verdict flips to VALID.
const RELAXED_CLOCKWISE_REPORT = [
  'VALID — 0 errors, 1 warning',
  '',
  'Warnings',
  "  1. [winding-wrong] (root).coordinates[0]: the exterior ring winds clockwise but RFC 7946's right-hand rule wants it counterclockwise — reversed winding makes some renderers fill the rest of the world instead of this shape",
  '',
  'Summary',
  '  document type: Polygon',
  '  features: 0',
  '  geometries: 1 (Polygon 1)',
  '  positions: 5',
  '  rings: 1 (1 exterior, 0 interior)',
  '  bbox members: 0',
  '  bounds: longitude 0 .. 4, latitude 0 .. 4',
  '',
].join('\n');

async function runWasm(
  page: any,
  input: string,
  output = 'text',
  strictWinding = 'true',
  allowBbox = 'true',
  warnProblematic = 'true',
  maxPrecision = '6',
  maxIssues = '50',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/geojson-validate/gizza_ai_geojson_validate_web.js');
    await mod.default('/tools/geojson-validate/gizza_ai_geojson_validate_web_bg.wasm');
    return mod.run(
      args.input,
      args.output,
      args.strictWinding,
      args.allowBbox,
      args.warnProblematic,
      args.maxPrecision,
      args.maxIssues,
    );
  }, { input, output, strictWinding, allowBbox, warnProblematic, maxPrecision, maxIssues });
}

test('geojson-validate page reports a clean document exactly', async ({ page }) => {
  await page.goto('/tools/geojson-validate/');
  await page.fill('#in-input', VALID_FC);
  await expect(page.locator('#tool-output')).toContainText('VALID — 0 errors, 0 warnings', {
    timeout: 15_000,
  });
  expect(await page.locator('#tool-output').textContent()).toBe(VALID_FC_REPORT);
});

test('geojson-validate page reports every issue with rule id and JSON path', async ({ page }) => {
  await page.goto('/tools/geojson-validate/');
  await page.fill('#in-input', UNCLOSED);
  await expect(page.locator('#tool-output')).toContainText('ring-not-closed', { timeout: 15_000 });
  expect(await page.locator('#tool-output').textContent()).toBe(UNCLOSED_REPORT);
});

test('geojson-validate deep link pre-fills the form and emits the JSON report', async ({ page }) => {
  const params = new URLSearchParams({ input: PROJECTED, output: 'json' });
  await page.goto(`/tools/geojson-validate/?${params.toString()}`);

  await expect(page.locator('#in-input')).toHaveValue(PROJECTED, { timeout: 15_000 });
  await expect(page.locator('#in-output')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toContainText('"valid": false', { timeout: 15_000 });

  const report = JSON.parse((await page.locator('#tool-output').textContent())!);
  expect(report.valid).toBe(false);
  expect(report.error_count).toBe(2);
  expect(report.errors.map((e: any) => e.rule)).toEqual([
    'longitude-out-of-range',
    'latitude-out-of-range',
  ]);
  expect(report.errors[0].path).toBe('(root).coordinates[0]');
  expect(report.errors[0].message).toContain('is outside -180..180');
  expect(report.warnings.map((w: any) => w.rule)).toEqual(['bare-geometry']);
  expect(report.summary.document_type).toBe('Point');
  expect(report.summary.geometries).toEqual({ total: 1, by_type: { Point: 1 } });
  expect(report.summary.positions).toBe(1);
  expect(report.summary.bounds).toBeNull();
  expect(report.truncated).toEqual({ errors: false, warnings: false });
});

test('geojson-validate non-default checkbox states reach the wasm', async ({ page }) => {
  await page.goto('/tools/geojson-validate/');
  // Both boxes start CHECKED (descriptor default true) — uncheck the off-path.
  await expect(page.locator('#in-strict_winding')).toBeChecked();
  await expect(page.locator('#in-warn_problematic')).toBeChecked();
  await expect(page.locator('#in-allow_bbox')).toBeChecked();

  await page.fill('#in-input', CLOCKWISE);
  await expect(page.locator('#tool-output')).toContainText('INVALID — 1 error', { timeout: 15_000 });

  await page.uncheck('#in-strict_winding');
  await page.uncheck('#in-warn_problematic');
  await expect(page.locator('#tool-output')).toContainText('VALID — 0 errors, 1 warning', {
    timeout: 15_000,
  });
  expect(await page.locator('#tool-output').textContent()).toBe(RELAXED_CLOCKWISE_REPORT);
});

test('geojson-validate example chip prefills and runs', async ({ page }) => {
  await page.goto('/tools/geojson-validate/');
  const chips = page.locator('.tool-example-chip');
  await expect(chips).toHaveCount(4);
  await chips.nth(1).click(); // "Backwards polygon winding"
  await expect(page.locator('#in-input')).toHaveValue(CLOCKWISE);
  await expect(page.locator('#tool-output')).toContainText('[winding-wrong]', { timeout: 15_000 });
});

test('geojson-validate wasm covers every advertised option value', async ({ page }) => {
  await page.goto('/tools/geojson-validate/');
  await page.waitForSelector('#in-input');

  // Both enum values.
  expect(await runWasm(page, VALID_FC, 'text')).toBe(VALID_FC_REPORT);
  expect(JSON.parse(await runWasm(page, VALID_FC, 'json')).valid).toBe(true);
  await expect(runWasm(page, VALID_FC, 'yaml')).rejects.toThrow(/unknown output "yaml"/);

  // allow_bbox toggles a bbox member from accepted to flagged.
  expect(await runWasm(page, WITH_BBOX, 'text')).toContain('VALID — 0 errors');
  expect(await runWasm(page, WITH_BBOX, 'text', 'true', 'false')).toContain('[bbox-not-allowed]');
  expect(await runWasm(page, '{"type":"Point","bbox":[0,9,1,2],"coordinates":[0,0]}', 'text'))
    .toContain('[bbox-order]');

  // max_precision: threshold, boundaries, and the -1 off switch.
  const precise = '{"type":"Point","coordinates":[1.1234567,2]}';
  expect(await runWasm(page, precise, 'text')).toContain('[excess-precision]');
  expect(await runWasm(page, precise, 'text', 'true', 'true', 'true', '7')).not.toContain(
    '[excess-precision]',
  );
  expect(await runWasm(page, precise, 'text', 'true', 'true', 'true', '-1')).not.toContain(
    '[excess-precision]',
  );
  await expect(runWasm(page, precise, 'text', 'true', 'true', 'true', '15')).resolves.toContain(
    'VALID — 0 errors',
  );
  await expect(runWasm(page, precise, 'text', 'true', 'true', 'true', '16')).rejects.toThrow(
    /max_precision must be between -1 and 15/,
  );

  // max_issues: cap boundaries and truncation, with counts staying complete.
  const many =
    '{"type":"FeatureCollection","features":[' +
    Array.from(
      { length: 5 },
      () => '{"type":"Feature","properties":{},"geometry":{"type":"Point","coordinates":[999,0]}}',
    ).join(',') +
    ']}';
  const capped = await runWasm(page, many, 'text', 'true', 'true', 'true', '6', '2');
  expect(capped).toContain('INVALID — 5 errors, 0 warnings');
  expect(capped).toContain('… and 3 more');
  const cappedJson = JSON.parse(await runWasm(page, many, 'json', 'true', 'true', 'true', '6', '2'));
  expect(cappedJson.error_count).toBe(5);
  expect(cappedJson.errors).toHaveLength(2);
  expect(cappedJson.truncated.errors).toBe(true);
  await expect(runWasm(page, many, 'text', 'true', 'true', 'true', '6', '1000')).resolves.toContain(
    'INVALID — 5 errors',
  );
  await expect(runWasm(page, many, 'text', 'true', 'true', 'true', '6', '1001')).rejects.toThrow(
    /max_issues must be between 1 and 1000/,
  );
  await expect(runWasm(page, many, 'text', 'true', 'true', 'true', '6', '0')).rejects.toThrow(
    /max_issues must be between 1 and 1000/,
  );

  // Broken JSON is a finding, not a crash; empty input is a usage error.
  expect(await runWasm(page, '{"type":', 'text')).toContain('[invalid-json]');
  await expect(runWasm(page, '   ', 'text')).rejects.toThrow(/input is empty/);

  // Structural errors keep their paths inside a collection.
  const nested = await runWasm(
    page,
    '{"type":"FeatureCollection","features":[{"type":"Feature","geometry":{"type":"Point","coordinates":[0,0]}}]}',
    'text',
  );
  expect(nested).toContain('[missing-properties] features[0]');
});

test('geojson-validate generated CLI example is generic and runnable-looking', async ({ page }) => {
  await page.goto('/tools/geojson-validate/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool geojson-validate');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
