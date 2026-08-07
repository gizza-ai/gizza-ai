import { test, expect } from './fixtures';

const SAMPLE = '{"kind":"list","etag":"W/\\"9a1\\"","items":[{"title":"First","author":"Ada","stats":{"length":120,"views":3}},{"title":"Second","author":"Grace","stats":{"length":80,"views":9}}]}';
const EXPECTED = `{
  "kind": "list",
  "items": [
    {
      "title": "First",
      "stats": {
        "length": 120
      }
    },
    {
      "title": "Second",
      "stats": {
        "length": 80
      }
    }
  ]
}`;

async function runWasm(
  page,
  json = SAMPLE,
  mask = 'kind,items(title,stats/length)',
  mode = 'keep',
  format = 'pretty',
  onMissing = 'omit',
  empty = 'keep',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/json-mask/gizza_ai_json_mask_web.js');
    await mod.default('/tools/json-mask/gizza_ai_json_mask_web_bg.wasm');
    return mod.run(args.json, args.mask, args.mode, args.format, args.onMissing, args.empty);
  }, { json, mask, mode, format, onMissing, empty });
}

test('json-mask wasm preserves document shape exactly', async ({ page }) => {
  await page.goto('/tools/json-mask/');
  await expect(runWasm(page)).resolves.toBe(EXPECTED);
});

test('json-mask wasm covers advertised enum choices and errors', async ({ page }) => {
  await page.goto('/tools/json-mask/');

  await expect(runWasm(
    page,
    '{"items":[{"title":"First","author":"Ada","token":"x"},{"title":"Second","author":"Grace","token":"y"}],"next":"n"}',
    'items(token)',
    'remove',
    'compact',
  )).resolves.toBe('{"items":[{"title":"First","author":"Ada"},{"title":"Second","author":"Grace"}],"next":"n"}');

  await expect(runWasm(page, '[{"name":"Ada"},{"id":2}]', 'name,id', 'keep', 'compact', 'null', 'keep'))
    .resolves.toBe('[{"name":"Ada","id":null},{"id":2,"name":null}]');

  await expect(runWasm(page, '{"a":{"b":1}}', 'a/c', 'keep', 'pretty', 'error'))
    .rejects.toThrow("field 'c' not found at $.a");

  await expect(runWasm(page, '[{"a":1},{"b":2}]', 'a', 'keep', 'compact', 'omit', 'drop'))
    .resolves.toBe('[{"a":1}]');
});

test('json-mask page renders exact output and deep-link prefills params', async ({ page }) => {
  await page.goto('/tools/json-mask/');
  await page.fill('#in-json', SAMPLE);
  await page.fill('#in-mask', 'kind,items(title,stats/length)');
  await expect(page.locator('#tool-output')).toHaveText(EXPECTED, { timeout: 15_000 });

  const qs =
    '?json=' + encodeURIComponent(SAMPLE) +
    '&mask=' + encodeURIComponent('items(title)') +
    '&mode=keep' +
    '&format=compact' +
    '&on_missing=omit' +
    '&empty=keep';
  await page.goto('/tools/json-mask/' + qs);
  await expect(page.locator('#in-json')).toHaveValue(SAMPLE, { timeout: 15_000 });
  await expect(page.locator('#in-mask')).toHaveValue('items(title)');
  await expect(page.locator('#in-format')).toHaveValue('compact');
  await expect(page.locator('#tool-output')).toHaveText('{"items":[{"title":"First"},{"title":"Second"}]}', { timeout: 15_000 });
});
