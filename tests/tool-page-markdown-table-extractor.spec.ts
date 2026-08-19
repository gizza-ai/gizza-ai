import { test, expect } from './fixtures';

const tool = '/tools/markdown-table-extractor/';
const doc = '# Release\n\n## Downloads\n\n| file | size |\n| --- | ---: |\n| app-linux.tar.gz | 12 MB |\n| app-macos.zip | 14 MB |\n\n```md\n| not | table |\n| --- | --- |\n```\n\n## Plans\n\n| plan | seats |\n|:---|---:|\n| Free | 3 |\n| Team | 25 |';

async function outputText(page): Promise<string> {
  const text = await page.locator('#tool-output').textContent();
  return text ?? '';
}

async function runWasm(
  page,
  markdown: string,
  format = 'csv',
  table = 'all',
  header = 'true',
  delimiter = ',',
  quote = 'minimal',
  newline = 'lf',
  trim = 'true',
  stripFormatting = 'false',
  jsonIndent = '2',
  labels = 'true',
) {
  return await page.evaluate(
    async ({ markdown, format, table, header, delimiter, quote, newline, trim, stripFormatting, jsonIndent, labels }) => {
      const mod = await import('/tools/markdown-table-extractor/gizza_ai_markdown_table_extractor_web.js');
      await mod.default('/tools/markdown-table-extractor/gizza_ai_markdown_table_extractor_web_bg.wasm');
      return mod.run(markdown, format, table, header, delimiter, quote, newline, trim, stripFormatting, jsonIndent, labels);
    },
    { markdown, format, table, header, delimiter, quote, newline, trim, stripFormatting, jsonIndent, labels },
  );
}

test('markdown-table-extractor page exports every table as labelled CSV and ignores fenced code', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-markdown', doc);
  await page.selectOption('#in-format', 'csv');
  await page.fill('#in-table', 'all');
  await page.check('#in-header');
  await page.fill('#in-delimiter', ',');
  await page.selectOption('#in-quote', 'minimal');
  await page.selectOption('#in-newline', 'lf');
  await page.check('#in-trim');
  await page.uncheck('#in-strip_formatting');
  await page.fill('#in-json_indent', '2');
  await page.check('#in-labels');

  await expect(page.locator('#tool-output')).toContainText('# Table 0: Downloads', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    '# Table 0: Downloads\nfile,size\napp-linux.tar.gz,12 MB\napp-macos.zip,14 MB\n\n# Table 1: Plans\nplan,seats\nFree,3\nTeam,25',
  );
  expect(await outputText(page)).not.toContain('not,table');
});

test('markdown-table-extractor deep link selects a table, non-default checkboxes and JSON output', async ({ page }) => {
  await page.goto(
    tool +
      '?markdown=' +
      encodeURIComponent(doc) +
      '&format=json&table=1&header=false&delimiter=tab&quote=all&newline=crlf&trim=false&strip_formatting=true&json_indent=0&labels=false',
  );

  await expect(page.locator('#in-markdown')).toHaveValue(doc, { timeout: 15000 });
  await expect(page.locator('#in-format')).toHaveValue('json');
  await expect(page.locator('#in-table')).toHaveValue('1');
  await expect(page.locator('#in-header')).not.toBeChecked();
  await expect(page.locator('#in-delimiter')).toHaveValue('tab');
  await expect(page.locator('#in-quote')).toHaveValue('all');
  await expect(page.locator('#in-newline')).toHaveValue('crlf');
  await expect(page.locator('#in-trim')).not.toBeChecked();
  await expect(page.locator('#in-strip_formatting')).toBeChecked();
  await expect(page.locator('#in-json_indent')).toHaveValue('0');
  await expect(page.locator('#in-labels')).not.toBeChecked();

  await expect(page.locator('#tool-output')).toContainText('[["Free","3"]', { timeout: 15000 });
  expect(await outputText(page)).toBe('[["Free","3"],["Team","25"]]');
});

test('markdown-table-extractor wasm covers advertised formats, separators, formatting and cap', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-markdown');

  const json = await runWasm(page, doc, 'json', '1', 'true', ',', 'minimal', 'lf', 'true', 'false', '2', 'true');
  expect(json).toBe('[\n  {\n    "plan": "Free",\n    "seats": "3"\n  },\n  {\n    "plan": "Team",\n    "seats": "25"\n  }\n]');

  const list = await runWasm(page, doc, 'list');
  expect(list).toContain('"heading": "Downloads"');
  expect(list).toContain('"align": [\n      "left",\n      "right"');

  const jsonl = await runWasm(page, doc, 'jsonl', '0,1', 'true');
  expect(jsonl).toContain('{"table":0,"row":{"file":"app-linux.tar.gz","size":"12 MB"}}');
  expect(jsonl).toContain('{"table":1,"row":{"plan":"Team","seats":"25"}}');

  const formatted = '| package | docs |\n| --- | --- |\n| **core** | [Guide](https://example.com/guide) |\n| `cli` | line<br>break |';
  expect(await runWasm(page, formatted, 'csv', 'all', 'true', 'tab', 'all', 'crlf', 'true', 'true')).toBe(
    '"package"\t"docs"\r\n"core"\t"Guide"\r\n"cli"\t"line break"',
  );

  const result = await page.evaluate(async () => {
    const mod = await import('/tools/markdown-table-extractor/gizza_ai_markdown_table_extractor_web.js');
    await mod.default('/tools/markdown-table-extractor/gizza_ai_markdown_table_extractor_web_bg.wasm');
    const base = '| a |\n|---|\n| x |\n';
    const atCap = base + 'x'.repeat(1_000_000 - base.length);
    const overCap = atCap + 'x';
    const call = (markdown: string) => {
      try {
        return { ok: true, value: mod.run(markdown, 'csv', 'all', 'true', ',', 'minimal', 'lf', 'true', 'false', '2', 'true').slice(0, 3) };
      } catch (e) {
        return { ok: false, value: String(e) };
      }
    };
    return { atCapBytes: atCap.length, overCapBytes: overCap.length, atCap: call(atCap), overCap: call(overCap) };
  });
  expect(result.atCapBytes).toBe(1_000_000);
  expect(result.overCapBytes).toBe(1_000_001);
  expect(result.atCap.ok).toBe(true);
  expect(result.overCap.ok).toBe(false);
  expect(result.overCap.value).toContain('input is too large: 1000001 bytes (max 1000000)');
});
