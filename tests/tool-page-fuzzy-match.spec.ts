import { test, expect } from './fixtures';

const TEXT_TOP3 = `rank  score   candidate  reason
   1   96.7  applet  — subsequence span 5, best contiguous run 5
   2   92.0  apple pie  — contains the query
   3   92.0  pineapple  — contains the query`;

async function runWasm(
  page: any,
  query = 'apple',
  candidates = 'apple pie\napplet\nbanana\napply\npineapple',
  algorithm = 'hybrid',
  limit = '3',
  threshold = '0',
  caseSensitive = 'false',
  includeReasons = 'true',
  outputFormat = 'text',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/fuzzy-match/gizza_ai_fuzzy_match_web.js');
    await mod.default('/tools/fuzzy-match/gizza_ai_fuzzy_match_web_bg.wasm');
    return mod.run(
      args.query,
      args.candidates,
      args.algorithm,
      args.limit,
      args.threshold,
      args.caseSensitive,
      args.includeReasons,
      args.outputFormat,
    );
  }, { query, candidates, algorithm, limit, threshold, caseSensitive, includeReasons, outputFormat });
}

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('fuzzy-match wasm returns exact ranked text', async ({ page }) => {
  await page.goto('/tools/fuzzy-match/');
  await page.waitForSelector('#in-query');

  await expect(runWasm(page)).resolves.toBe(TEXT_TOP3);
});

test('fuzzy-match wasm covers CSV, threshold and comma-separated input', async ({ page }) => {
  await page.goto('/tools/fuzzy-match/');
  await page.waitForSelector('#in-query');

  const out = await runWasm(page, 'acme ltd', 'ACME Limited,Acme Incorporated,Ace Metal,Globex LLC', 'hybrid', '10', '40', 'false', 'true', 'csv');
  expect(out).toContain('rank,score,candidate,edit_distance,reason');
  expect(out).toContain('ACME Limited');
  expect(out).not.toContain('Globex LLC');
});

test('fuzzy-match page renders exact output and honors non-default checkbox', async ({ page }) => {
  await page.goto('/tools/fuzzy-match/');
  await page.fill('#in-query', 'apple');
  await page.fill('#in-candidates', 'apple pie\napplet\nbanana\napply\npineapple');
  await page.selectOption('#in-algorithm', 'hybrid');
  await page.fill('#in-limit', '3');
  await page.fill('#in-threshold', '0');
  await page.uncheck('#in-case_sensitive');
  await page.check('#in-include_reasons');
  await page.selectOption('#in-output_format', 'text');

  await expect(page.locator('#tool-output')).toHaveText(TEXT_TOP3, { timeout: 15_000 });
});

test('fuzzy-match deep-link prefills and runs JSON without reasons', async ({ page }) => {
  const params = new URLSearchParams({
    query: 'fb',
    candidates: 'foo_bar.rs\nfast_build.sh\nformat-bytes.ts\nREADME.md',
    algorithm: 'subsequence',
    limit: '2',
    threshold: '0',
    case_sensitive: 'false',
    include_reasons: 'false',
    output_format: 'json',
  });

  await page.goto(`/tools/fuzzy-match/?${params.toString()}`);
  await expect(page.locator('#in-query')).toHaveValue('fb', { timeout: 15_000 });
  await expect(page.locator('#in-output_format')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toContainText('"candidate":"foo_bar.rs"', { timeout: 15_000 });
  expect(await outputText(page)).not.toContain('reason');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool fuzzy-match');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
