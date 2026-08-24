import { test, expect } from './fixtures';

async function setBigTextarea(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page: any,
  input: string,
  mode = 'transliterate',
  unmapped = 'keep',
  replacement = '?',
  keep = '',
  lowercase = 'false',
  collapseWhitespace = 'false',
  includeReport = 'false',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/accent-stripper/gizza_ai_accent_stripper_web.js');
    await mod.default('/tools/accent-stripper/gizza_ai_accent_stripper_web_bg.wasm');
    return mod.run(
      args.input,
      args.mode,
      args.unmapped,
      args.replacement,
      args.keep,
      args.lowercase,
      args.collapseWhitespace,
      args.includeReport,
    );
  }, { input, mode, unmapped, replacement, keep, lowercase, collapseWhitespace, includeReport });
}

test('accent-stripper wasm transliterates accented and non-Latin text exactly', async ({ page }) => {
  await page.goto('/tools/accent-stripper/');
  await page.waitForSelector('#in-input');

  await expect(runWasm(page, 'Crème Brûlée à la Française — Straße, Ærøskøbing, Москва'))
    .resolves.toBe('Creme Brulee a la Francaise -- Strasse, AEroskobing, Moskva');
});

test('accent-stripper wasm covers modes, policies, keep list, and report', async ({ page }) => {
  await page.goto('/tools/accent-stripper/');
  await page.waitForSelector('#in-input');

  await expect(runWasm(page, 'café Straße Ærøskøbing Москва', 'marks-only'))
    .resolves.toBe('cafe Straße Ærøskøbing Москва');
  await expect(runWasm(page, 'a𝕏ø', 'marks-only', 'remove'))
    .resolves.toBe('a');
  await expect(runWasm(page, 'a𝕏ø', 'marks-only', 'replace', '_'))
    .resolves.toBe('a__');
  await expect(runWasm(page, 'mañana café', 'transliterate', 'keep', '?', 'ñ'))
    .resolves.toBe('mañana cafe');
  await expect(runWasm(page, '  ÉCOLE   Normale \n  Supérieure  ', 'transliterate', 'keep', '?', '', 'true', 'true'))
    .resolves.toBe('ecole normale\nsuperieure');

  const report = await runWasm(page, 'café ', 'transliterate', 'replace', '_', '', 'false', 'false', 'true');
  expect(report).toContain('"result": "cafe _"');
  expect(report).toContain('"converted": 1');
  expect(report).toContain('"unmapped": 1');
});

test('accent-stripper page renders exact output and honors controls', async ({ page }) => {
  await page.goto('/tools/accent-stripper/');
  await setBigTextarea(page, '#in-input', '  Déjà Vu: Señor Piñata!  ');
  await page.check('#in-lowercase');
  await page.check('#in-collapse_whitespace');

  await expect(page.locator('#tool-output')).toHaveText('deja vu: senor pinata!', { timeout: 15_000 });
});

test('accent-stripper deep-link prefills controls and runs exact output', async ({ page }) => {
  const params = new URLSearchParams({
    input: 'mañana café',
    mode: 'transliterate',
    unmapped: 'keep',
    replacement: '?',
    keep: 'ñ',
    lowercase: 'false',
    collapse_whitespace: 'false',
    include_report: 'false',
  });

  await page.goto(`/tools/accent-stripper/?${params.toString()}`);
  await expect(page.locator('#in-input')).toHaveValue('mañana café', { timeout: 15_000 });
  await expect(page.locator('#in-mode')).toHaveValue('transliterate');
  await expect(page.locator('#in-keep')).toHaveValue('ñ');
  await expect(page.locator('#tool-output')).toHaveText('mañana cafe', { timeout: 15_000 });

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool accent-stripper');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
