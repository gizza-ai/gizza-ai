import { test, expect } from './fixtures';

const SAMPLE = '[[users]]\nname = "Ada"\nrole = "admin"\ntags = ["founder", "ops"]\n\n[[users]]\nname = "Linus"\nrole = "dev"';

async function runWasm(
  page: any,
  params: Partial<{
    input: string;
    table: string;
    nested: string;
    array_format: string;
    columns: string;
    delimiter: string;
    include_header: string;
  }> = {},
): Promise<string> {
  const p = {
    input: SAMPLE,
    table: '',
    nested: 'flatten',
    array_format: 'json',
    columns: 'union',
    delimiter: 'comma',
    include_header: 'true',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/toml-to-csv/gizza_ai_toml_to_csv_web.js');
    await mod.default('/tools/toml-to-csv/gizza_ai_toml_to_csv_web_bg.wasm');
    return mod.run(
      args.input,
      args.table,
      args.nested,
      args.array_format,
      args.columns,
      args.delimiter,
      args.include_header,
    );
  }, p);
}

test('toml-to-csv page converts an array of tables to CSV', async ({ page }) => {
  await page.goto('/tools/toml-to-csv/');
  await page.fill('#in-input', SAMPLE);

  await expect
    .poll(async () => await page.locator('#tool-output').textContent(), { timeout: 15_000 })
    .toBe('name,role,tags\nAda,admin,"[""founder"",""ops""]"\nLinus,dev,\n');
});

test('toml-to-csv deep link prefills nested table, TSV and no header', async ({ page }) => {
  const input = '[[users]]\nname = "Ada"\n\n[servers]\n[[servers.pool]]\nhost = "a.example.com"\nport = 443\n\n[[servers.pool]]\nhost = "b.example.com"\nport = 8443';
  const qs = new URLSearchParams({
    input,
    table: 'servers.pool',
    nested: 'flatten',
    array_format: 'json',
    columns: 'union',
    delimiter: 'tab',
    include_header: 'false',
  });

  await page.goto(`/tools/toml-to-csv/?${qs.toString()}`);
  await expect(page.locator('#in-input')).toHaveValue(input, { timeout: 15_000 });
  await expect(page.locator('#in-table')).toHaveValue('servers.pool');
  await expect(page.locator('#in-delimiter')).toHaveValue('tab');
  await expect(page.locator('#in-include_header')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('a.example.com\t443\nb.example.com\t8443\n', { timeout: 15_000 });
});

test('toml-to-csv wasm covers modes, delimiters, header toggle, errors and CLI example', async ({ page }) => {
  await page.goto('/tools/toml-to-csv/');

  expect(await runWasm(page, { array_format: 'columns' })).toBe('name,role,tags.1,tags.2\nAda,admin,founder,ops\nLinus,dev,,\n');
  expect(await runWasm(page, { array_format: 'join' })).toBe('name,role,tags\nAda,admin,founder; ops\nLinus,dev,\n');
  expect(await runWasm(page, { columns: 'sorted' })).toContain('name,role,tags');
  expect(await runWasm(page, { columns: 'first', input: '[[x]]\nb = 2\n\n[[x]]\na = 1\nb = 3' })).toBe('b\n2\n3\n');
  expect(await runWasm(page, { delimiter: 'semicolon' })).toContain('name;role;tags');
  expect(await runWasm(page, { delimiter: 'pipe' })).toContain('name|role|tags');
  expect(await runWasm(page, { include_header: 'false' })).toBe('Ada,admin,"[""founder"",""ops""]"\nLinus,dev,\n');
  expect(await runWasm(page, { nested: 'json', input: '[[servers]]\nname = "web"\naddr = { city = "Berlin", zip = "10115" }' })).toContain('web,"{""city"":""Berlin"",""zip"":""10115""}"');
  expect(await runWasm(page, { nested: 'skip', input: '[[servers]]\nname = "web"\naddr = { city = "Berlin" }' })).toBe('name\nweb\n');
  await expect(runWasm(page, { input: '[[users]]\nname = ' })).rejects.toThrow(/invalid TOML/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool toml-to-csv');
  expect(cli).toContain('[[users]]');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
