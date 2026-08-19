import { test, expect } from './fixtures';

const tool = '/tools/csv-coalesce-columns/';
const phones = 'name,mobile,office,home\nAnn,555-1,555-2,555-3\nBob,,555-4,555-5\nCleo,,,555-6';

async function outputText(page): Promise<string> {
  const text = await page.locator('#tool-output').textContent();
  return text ?? '';
}

async function runWasm(
  page,
  data: string,
  columns: string,
  output = '',
  position = 'end',
  fallback = '',
  dropSources = 'false',
  blankIsEmpty = 'true',
  nullTokens = '',
  header = 'true',
  delimiter = ',',
) {
  return await page.evaluate(
    async ({ data, columns, output, position, fallback, dropSources, blankIsEmpty, nullTokens, header, delimiter }) => {
      const mod = await import('/tools/csv-coalesce-columns/gizza_ai_csv_coalesce_columns_web.js');
      await mod.default('/tools/csv-coalesce-columns/gizza_ai_csv_coalesce_columns_web_bg.wasm');
      return mod.run(data, columns, output, position, fallback, dropSources, blankIsEmpty, nullTokens, header, delimiter);
    },
    { data, columns, output, position, fallback, dropSources, blankIsEmpty, nullTokens, header, delimiter },
  );
}

test('csv-coalesce-columns page merges columns in priority order with exact output', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-data', phones);
  await page.fill('#in-columns', 'mobile,office,home');
  await page.fill('#in-output', 'phone');
  await page.selectOption('#in-position', 'end');
  await page.fill('#in-fallback', '');
  await page.check('#in-blank_is_empty');
  await page.check('#in-header');
  await page.fill('#in-delimiter', ',');

  await expect(page.locator('#tool-output')).toContainText('Cleo,,,555-6,555-6', { timeout: 15000 });
  expect((await outputText(page)).trim()).toBe(
    'name,mobile,office,home,phone\nAnn,555-1,555-2,555-3,555-1\nBob,,555-4,555-5,555-4\nCleo,,,555-6,555-6',
  );
});

test('csv-coalesce-columns deep link prefills drop_sources and lands the column at the first source slot', async ({ page }) => {
  await page.goto(
    tool +
      '?data=' +
      encodeURIComponent('name,mobile,office,city\nAnn,555-1,555-2,Rome\nBob,,555-4,Oslo') +
      '&columns=mobile%2Coffice&output=phone&position=first-source&fallback=&drop_sources=true' +
      '&blank_is_empty=true&null_tokens=&header=true&delimiter=%2C',
  );

  await expect(page.locator('#in-data')).toHaveValue('name,mobile,office,city\nAnn,555-1,555-2,Rome\nBob,,555-4,Oslo', {
    timeout: 15000,
  });
  await expect(page.locator('#in-columns')).toHaveValue('mobile,office');
  await expect(page.locator('#in-drop_sources')).toBeChecked();
  await expect(page.locator('#in-position')).toHaveValue('first-source');

  await expect(page.locator('#tool-output')).toContainText('Ann,555-1,Rome');
  expect((await outputText(page)).trim()).toBe('name,phone,city\nAnn,555-1,Rome\nBob,555-4,Oslo');
});

test('csv-coalesce-columns page honours null_tokens and the all-empty fallback', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-data', 'id,primary_email,backup_email\n1,NULL,ann@example.com\n2,bob@example.com,NULL\n3,N/A,-');
  await page.fill('#in-columns', 'primary_email,backup_email');
  await page.fill('#in-output', 'email');
  await page.fill('#in-fallback', 'unknown');
  await page.fill('#in-null_tokens', 'NULL,N/A,-');

  await expect(page.locator('#tool-output')).toContainText('unknown', { timeout: 15000 });
  expect((await outputText(page)).trim()).toBe(
    'id,primary_email,backup_email,email\n' +
      '1,NULL,ann@example.com,ann@example.com\n' +
      '2,bob@example.com,NULL,bob@example.com\n' +
      '3,N/A,-,unknown',
  );
});

test('csv-coalesce-columns wasm covers headerless indices, delimiters, blank handling and errors', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-data');

  // header=false → 1-based indices, and position=start puts the new column first.
  expect(await runWasm(page, ',b1\na2,\n', '1,2', '', 'start', '-', 'false', 'true', '', 'false')).toBe('b1,,b1\na2,a2,\n');
  // Named and single-character delimiters.
  expect(await runWasm(page, 'a\tb\n\tz\n', 'a,b', 'v', 'end', '', 'false', 'true', '', 'true', 'tab')).toBe('a\tb\tv\n\tz\tz\n');
  expect(await runWasm(page, 'a;b\n;z\n', 'a,b', 'v', 'end', '', 'false', 'true', '', 'true', ';')).toBe('a;b;v\n;z;z\n');
  // blank_is_empty on (default) skips a whitespace-only cell; off makes it a real value.
  expect(await runWasm(page, 'a,b\n ,second\n', 'a,b', 'v')).toBe('a,b,v\n ,second,second\n');
  expect(await runWasm(page, 'a,b\n ,second\n', 'a,b', 'v', 'end', '', 'false', 'false')).toBe('a,b,v\n ,second, \n');
  // drop_sources replaces the sources in place.
  expect(await runWasm(page, 'name,mobile,office\nAnn,,555-2\n', 'mobile,office', 'phone', 'first-source', '', 'true')).toBe(
    'name,phone\nAnn,555-2\n',
  );
  // Short rows are padded, so a missing trailing cell falls through to the fallback.
  expect(await runWasm(page, 'a,b,c\nx\n,,z\n', 'b,c', 'v', 'end', '-')).toBe('a,b,c,v\nx,,,-\n,,z,z\n');

  await expect(runWasm(page, 'a,b\n1,2\n', 'a,nope')).rejects.toThrow(/column 'nope' not found in the header/);
  await expect(runWasm(page, 'a,b\n1,2\n', 'a,a')).rejects.toThrow(/column 'a' is listed twice/);
  await expect(runWasm(page, 'a,b\n1,2\n', '1,9')).rejects.toThrow(/out of range/);
  await expect(runWasm(page, 'a,b\n1,2\n', 'a,b', 'a')).rejects.toThrow(/a column named 'a' already exists/);
  await expect(runWasm(page, 'a,b\n1,2\n', 'a', 'v', 'middle')).rejects.toThrow(/position must be/);
  await expect(runWasm(page, '   ', 'a')).rejects.toThrow(/input is empty/);
});

test('csv-coalesce-columns page ships workflow example presets', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(4);
  await page.click('.tool-example-chip:has-text("Replace the sources")');
  await expect(page.locator('#in-columns')).toHaveValue('mobile,office,home');
  await expect(page.locator('#in-position')).toHaveValue('first-source');
  await expect(page.locator('#in-drop_sources')).toBeChecked();
});
