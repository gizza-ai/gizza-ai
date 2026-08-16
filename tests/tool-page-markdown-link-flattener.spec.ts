import { test, expect } from './fixtures';

const tool = '/tools/markdown-link-flattener/';
const sample =
  'Read [the docs](https://example.com/docs) and see ![diagram](diagram.png).\n\n' +
  '[old-ref]: https://example.com/old\n\n' +
  '`[example](kept)`';

async function outputText(page): Promise<string> {
  const text = await page.locator('#tool-output').textContent();
  return text ?? '';
}

async function runWasm(
  page,
  markdown: string,
  linkMode = 'text',
  imageMode = 'alt_text',
  referenceDefinitions = 'drop',
  preserveCode = 'true',
) {
  return await page.evaluate(
    async ({ markdown, linkMode, imageMode, referenceDefinitions, preserveCode }) => {
      const mod = await import('/tools/markdown-link-flattener/gizza_ai_markdown_link_flattener_web.js');
      await mod.default('/tools/markdown-link-flattener/gizza_ai_markdown_link_flattener_web_bg.wasm');
      return mod.run(markdown, linkMode, imageMode, referenceDefinitions, preserveCode);
    },
    { markdown, linkMode, imageMode, referenceDefinitions, preserveCode },
  );
}

test('markdown-link-flattener page flattens links with exact output', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-markdown', sample);
  await page.selectOption('#in-link_mode', 'text');
  await page.selectOption('#in-image_mode', 'alt_text');
  await page.selectOption('#in-reference_definitions', 'drop');
  await page.check('#in-preserve_code');

  await expect(page.locator('#tool-output')).toContainText('Read the docs', { timeout: 15000 });
  expect(await outputText(page)).toBe('Read the docs and see diagram.\n\n\n`[example](kept)`');
});

test('markdown-link-flattener deep link prefills text-url mode and non-default checkbox', async ({ page }) => {
  const markdown = 'Use [the API](https://example.com/api).\n\n```md\n[flatten](inside-code)\n```';
  await page.goto(
    tool +
      '?markdown=' +
      encodeURIComponent(markdown) +
      '&link_mode=text_url&image_mode=keep_markdown&reference_definitions=keep&preserve_code=false',
  );

  await expect(page.locator('#in-markdown')).toHaveValue(markdown, { timeout: 15000 });
  await expect(page.locator('#in-link_mode')).toHaveValue('text_url');
  await expect(page.locator('#in-image_mode')).toHaveValue('keep_markdown');
  await expect(page.locator('#in-reference_definitions')).toHaveValue('keep');
  await expect(page.locator('#in-preserve_code')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('the API (https://example.com/api)');
  await expect(page.locator('#tool-output')).toContainText('flatten');
});

test('markdown-link-flattener wasm covers advertised modes and errors', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-markdown');

  expect(await runWasm(page, 'Read [docs](https://example.com/docs).')).toBe('Read docs.');
  expect(await runWasm(page, 'Read [docs](https://example.com/docs "title").', 'text_url')).toBe(
    'Read docs (https://example.com/docs).',
  );
  expect(await runWasm(page, 'Read [docs](<https://example.com/docs>).', 'url')).toBe('Read https://example.com/docs.');

  expect(await runWasm(page, 'Logo ![Acme](logo.png)', 'text', 'alt_text')).toBe('Logo Acme');
  expect(await runWasm(page, 'Logo ![Acme](logo.png)', 'text', 'alt_url')).toBe('Logo Acme (logo.png)');
  expect(await runWasm(page, 'Logo ![Acme](logo.png)', 'text', 'drop')).toBe('Logo ');
  expect(await runWasm(page, 'Logo ![Acme](logo.png) and [site](https://example.com)', 'text', 'keep_markdown')).toBe(
    'Logo ![Acme](logo.png) and site',
  );

  const refs = 'See [docs][d].\n\n[d]: https://example.com\n';
  expect(await runWasm(page, refs, 'text', 'alt_text', 'drop')).toBe('See [docs][d].\n\n');
  expect(await runWasm(page, refs, 'text', 'alt_text', 'keep')).toBe(refs);

  const code = '`[x](y)` and [real](url)\n```\n[a](b)\n```';
  expect(await runWasm(page, code, 'text', 'alt_text', 'drop', 'true')).toBe('`[x](y)` and real\n```\n[a](b)\n```');
  expect(await runWasm(page, code, 'text', 'alt_text', 'drop', 'false')).toBe('`x` and real\n```\na\n```');

  await expect(runWasm(page, '', 'text')).rejects.toThrow(/markdown is empty/);
  await expect(runWasm(page, '[x](y)', 'bad')).rejects.toThrow(/link_mode must be/);
  await expect(runWasm(page, '[x](y)', 'text', 'bad')).rejects.toThrow(/image_mode must be/);
});

test('markdown-link-flattener enforces the advertised 1,000,000-byte cap at the boundary', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-markdown');

  const result = await page.evaluate(async () => {
    const mod = await import('/tools/markdown-link-flattener/gizza_ai_markdown_link_flattener_web.js');
    await mod.default('/tools/markdown-link-flattener/gizza_ai_markdown_link_flattener_web_bg.wasm');
    const atCap = '[x](y)' + 'a'.repeat(1_000_000 - 6);
    const overCap = atCap + 'a';
    const call = (markdown: string) => {
      try {
        return { ok: true, value: mod.run(markdown, 'text', 'alt_text', 'drop', 'true').slice(0, 3) };
      } catch (e) {
        return { ok: false, value: String(e) };
      }
    };
    return { atCapBytes: atCap.length, overCapBytes: overCap.length, atCap: call(atCap), overCap: call(overCap) };
  });

  expect(result.atCapBytes).toBe(1_000_000);
  expect(result.overCapBytes).toBe(1_000_001);
  expect(result.atCap.ok).toBe(true);
  expect(result.atCap.value).toBe('xaa');
  expect(result.overCap.ok).toBe(false);
  expect(result.overCap.value).toMatch(/over the 1000000 byte limit/);
});

test('markdown-link-flattener page ships workflow example presets', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(5);

  await page.click('.tool-example-chip:has-text("Keep URLs as citations")');
  await expect(page.locator('#in-link_mode')).toHaveValue('text_url');
  await expect(page.locator('#tool-output')).toContainText('the docs (https://example.com/docs)', { timeout: 15000 });

  await page.click('.tool-example-chip:has-text("Keep image Markdown")');
  await expect(page.locator('#in-image_mode')).toHaveValue('keep_markdown');
  await expect(page.locator('#tool-output')).toContainText('![launch diagram](assets/launch.png)');
});
