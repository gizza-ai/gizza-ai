import { test, expect } from './fixtures';

const DATA = `{"id":1,"status":"ok","latency_ms":12}
{"id":2,"status":"error","latency_ms":940,"err":{"code":"timeout"}}
{"id":3,"status":"ok","latency_ms":31}`;

async function runWasm(
  page: any,
  data = DATA,
  view = 'pretty',
  indent = '2',
  search = '',
  searchIn = 'any',
  caseSensitive = 'false',
  path = '',
  value = '',
  matchMode = 'contains',
  sortKeys = 'false',
  skip = '0',
  limit = '0',
  invalid = 'report',
  lineNumbers = 'false',
  stats = 'false',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/ndjson-viewer/gizza_ai_ndjson_viewer_web.js');
    await mod.default('/tools/ndjson-viewer/gizza_ai_ndjson_viewer_web_bg.wasm');
    return mod.run(
      args.data,
      args.view,
      args.indent,
      args.search,
      args.searchIn,
      args.caseSensitive,
      args.path,
      args.value,
      args.matchMode,
      args.sortKeys,
      args.skip,
      args.limit,
      args.invalid,
      args.lineNumbers,
      args.stats,
    );
  }, { data, view, indent, search, searchIn, caseSensitive, path, value, matchMode, sortKeys, skip, limit, invalid, lineNumbers, stats });
}

test('ndjson-viewer page pretty-prints and searches nested records', async ({ page }) => {
  await page.goto('/tools/ndjson-viewer/');
  await page.fill('#in-data', DATA);
  await page.fill('#in-search', 'timeout');
  await page.check('#in-line_numbers');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('# record 2 (line 2)', { timeout: 20_000 });
  await expect(output).toContainText('"status": "error"');
  await expect(output).toContainText('"code": "timeout"');
  await expect(output).not.toContainText('"id": 1');
});

test('ndjson-viewer deep link covers table view, exact path filtering and stats', async ({ page }) => {
  const params = new URLSearchParams({
    data: DATA,
    view: 'table',
    indent: '2',
    search: '',
    search_in: 'any',
    case_sensitive: 'false',
    path: 'status',
    value: 'ok',
    match_mode: 'exact',
    sort_keys: 'false',
    skip: '0',
    limit: '1',
    invalid: 'report',
    line_numbers: 'true',
    stats: 'true',
  });
  await page.goto(`/tools/ndjson-viewer/?${params.toString()}`);

  await expect(page.locator('#in-view')).toHaveValue('table', { timeout: 15_000 });
  await expect(page.locator('#in-line_numbers')).toBeChecked();
  await expect(page.locator('#in-stats')).toBeChecked();
  const output = page.locator('#tool-output');
  await expect(output).toContainText('# 3 non-blank lines read · 3 records · 0 invalid lines', { timeout: 20_000 });
  await expect(output).toContainText('line | id | status | latency_ms');
  await expect(output).toContainText('1    | 1  | ok     | 12');
  await expect(output).not.toContainText('940');
});

test('ndjson-viewer wasm covers output modes, invalid handling and errors', async ({ page }) => {
  await page.goto('/tools/ndjson-viewer/');

  expect(await runWasm(page, '{"b":1,"a":2}', 'pretty')).toBe('{\n  "b": 1,\n  "a": 2\n}');
  expect(await runWasm(page, '{ "id" : 1 }\n\n{ "id" : 2 }', 'compact')).toBe('{"id":1}\n{"id":2}');
  expect(await runWasm(page, '{"a":1}\n{"a":2}', 'array', '0')).toBe('[{"a":1},{"a":2}]');
  expect(await runWasm(page, '{"a":1,"b":2}\n{"a":3,"c":true}', 'table')).toBe('a | b | c\n--+---+-----\n1 | 2 |\n3 |   | true');

  const reported = await runWasm(page, '{"ok":true}\n{"broken":\n{"ok":false}', 'compact');
  expect(reported).toContain('# line 2: invalid JSON');
  expect(await runWasm(page, '{"ok":true}\n{"broken":', 'compact', '2', '', 'any', 'false', '', '', 'contains', 'false', '0', '0', 'skip')).toBe('{"ok":true}');
  await expect(runWasm(page, '{"ok":true}', 'compact', '9')).rejects.toThrow(/indent must be between/);
  await expect(runWasm(page, 'not json', 'compact', '2', '', 'any', 'false', '', '', 'contains', 'false', '0', '0', 'error')).rejects.toThrow(/line 1/);
});

test('ndjson-viewer ships a clean runnable CLI example', async ({ page }) => {
  await page.goto('/tools/ndjson-viewer/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool ndjson-viewer');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
