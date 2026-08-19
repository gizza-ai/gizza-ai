import { test, expect } from './fixtures';

const BASE = `{
  "db": { "host": "localhost", "port": 5432 },
  "debug": false,
  "tags": ["base", "stable"]
}`;
const YAML_OVERRIDE = `db:
  host: staging.internal
tags:
  - canary`;
const ENV_LAYER = `DEBUG=true
DB__PORT=6543`;

async function runWasm(
  page: any,
  layer1: string,
  layer2 = '',
  layer3 = '',
  layer4 = '',
  layerNames = '',
  inputFormat = 'auto',
  output = 'json',
  objectMerge = 'deep',
  arrayMerge = 'replace',
  keyCase = 'match',
  nullDeletes = 'true',
  substitute = 'true',
  vars = '',
  sortKeys = 'false',
  indent = '2',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/config-merge/gizza_ai_config_merge_web.js');
    await mod.default('/tools/config-merge/gizza_ai_config_merge_web_bg.wasm');
    return mod.run(
      args.layer1,
      args.layer2,
      args.layer3,
      args.layer4,
      args.layerNames,
      args.inputFormat,
      args.output,
      args.objectMerge,
      args.arrayMerge,
      args.keyCase,
      args.nullDeletes,
      args.substitute,
      args.vars,
      args.sortKeys,
      args.indent,
    );
  }, { layer1, layer2, layer3, layer4, layerNames, inputFormat, output, objectMerge, arrayMerge, keyCase, nullDeletes, substitute, vars, sortKeys, indent });
}

test('config-merge page merges JSON, YAML and env layers', async ({ page }) => {
  await page.goto('/tools/config-merge/');
  await page.fill('#in-layer1', BASE);
  await page.fill('#in-layer2', YAML_OVERRIDE);
  await page.fill('#in-layer3', ENV_LAYER);

  const output = page.locator('#tool-output');
  await expect(output).toContainText('"host": "staging.internal"', { timeout: 20_000 });
  await expect(output).toContainText('"port": "6543"');
  await expect(output).toContainText('"debug": "true"');
});

test('config-merge deep link reports layer provenance', async ({ page }) => {
  const params = new URLSearchParams({
    layer1: BASE,
    layer2: YAML_OVERRIDE,
    layer_names: 'defaults.json,staging.yaml',
    output: 'report',
    array_merge: 'append',
  });
  await page.goto(`/tools/config-merge/?${params.toString()}`);

  await expect(page.locator('#in-output')).toHaveValue('report', { timeout: 15_000 });
  await expect(page.locator('#in-array_merge')).toHaveValue('append');
  const output = page.locator('#tool-output');
  await expect(output).toContainText('defaults.json (json) -> staging.yaml (yaml)', { timeout: 20_000 });
  await expect(output).toContainText('db.host = "staging.internal"  # staging.yaml');
  await expect(output).toContainText('db.host: defaults.json -> staging.yaml');
});

test('config-merge wasm covers outputs, merge enums, booleans, boundary and CLI example', async ({ page }) => {
  await page.goto('/tools/config-merge/');

  const json = await runWasm(page, BASE, YAML_OVERRIDE, ENV_LAYER);
  expect(json).toContain('"host": "staging.internal"');
  expect(json).toContain('"port": "6543"');

  await expect(runWasm(page, BASE, YAML_OVERRIDE, '', '', '', 'auto', 'yaml', 'deep', 'replace', 'match', 'true', 'true', '', 'false', '4')).resolves.toContain('    host: staging.internal');
  await expect(runWasm(page, '{"db":{"host":"h"},"port":80}', '', '', '', '', 'auto', 'toml')).resolves.toContain('[db]\nhost = "h"');
  await expect(runWasm(page, '{"db":{"host":"h"},"port":80}', '', '', '', '', 'auto', 'env')).resolves.toBe('DB__HOST=h\nPORT=80\n');

  const shallow = await runWasm(page, '{"db":{"host":"a","port":1}}', '{"db":{"host":"b"}}', '', '', '', 'auto', 'json', 'shallow');
  expect(shallow).not.toContain('"port"');

  const append = await runWasm(page, '{"tags":["api"]}', '{"tags":["api","canary"]}', '', '', '', 'auto', 'json', 'deep', 'append');
  expect(append.match(/"api"/g)?.length).toBe(2);
  const unique = await runWasm(page, '{"tags":["api"]}', '{"tags":["api","canary"]}', '', '', '', 'auto', 'json', 'deep', 'unique');
  expect(unique.match(/"api"/g)?.length).toBe(1);

  const preserveCase = await runWasm(page, '{"debug":false}', 'DEBUG=true', '', '', '', 'auto', 'json', 'deep', 'replace', 'preserve');
  expect(preserveCase).toContain('"debug": false');
  expect(preserveCase).toContain('"DEBUG": "true"');

  const keepNull = await runWasm(page, '{"cache":{"ttl":60}}', '{"cache":{"ttl":null}}', '', '', '', 'auto', 'json', 'deep', 'replace', 'match', 'false');
  expect(keepNull).toContain('"ttl": null');
  const noSub = await runWasm(page, '{"a":"${b}","b":"x"}', '', '', '', '', 'auto', 'json', 'deep', 'replace', 'match', 'true', 'false');
  expect(noSub).toContain('"a": "${b}"');
  const sorted = await runWasm(page, '{"z":1,"a":2}', '', '', '', '', 'auto', 'json', 'deep', 'replace', 'match', 'true', 'true', '', 'true');
  expect(sorted.indexOf('"a"')).toBeLessThan(sorted.indexOf('"z"'));

  const cap = `A=${'x'.repeat(256 * 1024 - 2)}`;
  await expect(runWasm(page, cap)).resolves.toContain('"A": "xxx');
  await expect(runWasm(page, `${cap}x`)).rejects.toThrow(/input too large/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool config-merge');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
