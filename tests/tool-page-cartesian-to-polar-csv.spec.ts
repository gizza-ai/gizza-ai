import { test, expect } from './fixtures';

const CSV = 'id,x,y\np1,3,4\np2,-3,-4\n';
const EXACT = 'id,r,theta\np1,5.00,53.13\np2,5.00,-126.87\n';

async function runWasm(
  page: any,
  csv = CSV,
  direction = 'cartesian_to_polar',
  x_column = '',
  y_column = '',
  angle_unit = 'degrees',
  angle_range = 'signed',
  decimals = '2',
  delimiter = 'auto',
  has_header = 'true',
  keep_columns = 'true',
  output = 'csv',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/cartesian-to-polar-csv/gizza_ai_cartesian_to_polar_csv_web.js');
    await mod.default('/tools/cartesian-to-polar-csv/gizza_ai_cartesian_to_polar_csv_web_bg.wasm');
    return mod.run(
      args.csv,
      args.direction,
      args.x_column,
      args.y_column,
      args.angle_unit,
      args.angle_range,
      args.decimals,
      args.delimiter,
      args.has_header,
      args.keep_columns,
      args.output,
    );
  }, { csv, direction, x_column, y_column, angle_unit, angle_range, decimals, delimiter, has_header, keep_columns, output });
}

test('cartesian-to-polar-csv wasm converts all quadrants with exact CSV output', async ({ page }) => {
  await page.goto('/tools/cartesian-to-polar-csv/');
  await page.waitForSelector('#in-csv');

  expect(await runWasm(page)).toBe(EXACT);
  expect(await runWasm(page, 'r,theta\n5,53.1301\n', 'polar_to_cartesian', '', '', 'degrees', 'signed', '3')).toBe('x,y\n3.000,4.000\n');
  expect(await runWasm(page, 'x,y\n-3,-4\n', 'cartesian_to_polar', '', '', 'degrees', 'positive', '2')).toBe('r,theta\n5.00,233.13\n');
});

test('cartesian-to-polar-csv page computes exact CSV from the form', async ({ page }) => {
  await page.goto('/tools/cartesian-to-polar-csv/');
  await page.fill('#in-csv', CSV);
  await page.selectOption('#in-direction', 'cartesian_to_polar');
  await page.selectOption('#in-angle_unit', 'degrees');
  await page.selectOption('#in-angle_range', 'signed');
  await page.fill('#in-decimals', '2');
  await page.selectOption('#in-delimiter', 'auto');
  await page.check('#in-has_header');
  await page.check('#in-keep_columns');
  await page.selectOption('#in-output', 'csv');

  await expect(page.locator('#tool-output')).toHaveText(EXACT, { timeout: 15_000 });
});

test('cartesian-to-polar-csv deep link covers radians, headerless rows, and unchecked keep_columns', async ({ page }) => {
  const params = new URLSearchParams({
    csv: '1,1\n0,2\n',
    direction: 'cartesian_to_polar',
    x_column: '',
    y_column: '',
    angle_unit: 'radians',
    angle_range: 'signed',
    decimals: '4',
    delimiter: 'auto',
    has_header: 'false',
    keep_columns: 'false',
    output: 'csv',
  });
  await page.goto(`/tools/cartesian-to-polar-csv/?${params.toString()}`);

  await expect(page.locator('#in-angle_unit')).toHaveValue('radians', { timeout: 15_000 });
  await expect(page.locator('#in-has_header')).not.toBeChecked();
  await expect(page.locator('#in-keep_columns')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('1.4142,0.7854\n2.0000,1.5708\n', { timeout: 15_000 });
});

test('cartesian-to-polar-csv covers delimiters, output enums, errors, and CLI example', async ({ page }) => {
  await page.goto('/tools/cartesian-to-polar-csv/');
  await page.waitForSelector('#in-csv');

  expect(await runWasm(page, 'x;y\n3;4\n', 'cartesian_to_polar', '', '', 'degrees', 'signed', '1', 'auto')).toBe('r;theta\n5.0;53.1\n');

  const json = await runWasm(page, 'id,x,y\np1,3,4\n', 'cartesian_to_polar', '', '', 'degrees', 'signed', '2', 'auto', 'true', 'true', 'json');
  expect(json).toBe('[\n  {"id": "p1", "r": 5.00, "theta": 53.13}\n]\n');

  const table = await runWasm(page, 'x,y\n3,4\n', 'cartesian_to_polar', '', '', 'degrees', 'signed', '2', 'auto', 'true', 'true', 'table');
  expect(table).toContain('theta');
  expect(table).toContain('53.13');

  await expect(runWasm(page, 'x,y\nnope,4\n')).rejects.toThrow(/is not a number/);
  await expect(runWasm(page, 'x,y\n1,2\n', 'cartesian_to_polar', '', '', 'degrees', 'signed', '16')).rejects.toThrow(/decimals/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool cartesian-to-polar-csv');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
