import { test, expect } from './fixtures';

async function setTextarea(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page: any,
  markdown: string,
  marker = 'dash',
  indent = '2',
  normalizeIndent = 'true',
  ordered = 'keep',
  markerSpace = '1',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/markdown-list-marker-normalizer/gizza_ai_markdown_list_marker_normalizer_web.js');
    await mod.default('/tools/markdown-list-marker-normalizer/gizza_ai_markdown_list_marker_normalizer_web_bg.wasm');
    return mod.run(
      args.markdown,
      args.marker,
      args.indent,
      args.normalizeIndent,
      args.ordered,
      args.markerSpace,
    );
  }, { markdown, marker, indent, normalizeIndent, ordered, markerSpace });
}

test('markdown-list-marker-normalizer page renders exact normalized Markdown', async ({ page }) => {
  await page.goto('/tools/markdown-list-marker-normalizer/');
  await setTextarea(page, '#in-markdown', '* Setup\n   + Install\n- Usage\n\t* Run');

  await expect(page.locator('#tool-output')).toHaveText([
    '- Setup',
    '  - Install',
    '- Usage',
    '  - Run',
  ].join('\n'), { timeout: 15_000 });
});

test('markdown-list-marker-normalizer deep-link prefills controls and runs', async ({ page }) => {
  const params = new URLSearchParams({
    markdown: '- Fruit\n  - Citrus\n    - Lemon',
    marker: 'sublist',
    indent: '2',
    normalize_indent: 'true',
    ordered: 'keep',
    marker_space: '1',
  });

  await page.goto(`/tools/markdown-list-marker-normalizer/?${params.toString()}`);
  await expect(page.locator('#in-markdown')).toHaveValue('- Fruit\n  - Citrus\n    - Lemon', { timeout: 15_000 });
  await expect(page.locator('#in-marker')).toHaveValue('sublist');
  await expect(page.locator('#in-indent')).toHaveValue('2');
  await expect(page.locator('#in-normalize_indent')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('- Fruit\n  * Citrus\n    + Lemon', { timeout: 15_000 });

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool markdown-list-marker-normalizer');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('markdown-list-marker-normalizer wasm covers every marker and ordered value', async ({ page }) => {
  await page.goto('/tools/markdown-list-marker-normalizer/');
  await page.waitForSelector('#in-markdown');

  await expect(runWasm(page, '* a\n+ b\n- c\n', 'dash')).resolves.toBe('- a\n- b\n- c\n');
  await expect(runWasm(page, '- a\n+ b\n', 'asterisk')).resolves.toBe('* a\n* b\n');
  await expect(runWasm(page, '- a\n* b\n', 'plus')).resolves.toBe('+ a\n+ b\n');
  await expect(runWasm(page, '+ a\n- b\n* c\n', 'consistent')).resolves.toBe('+ a\n+ b\n+ c\n');
  await expect(runWasm(page, '- a\n  - b\n    - c\n', 'sublist')).resolves.toBe('- a\n  * b\n    + c\n');

  const ordered = '3. alpha\n7. beta\n9. gamma\n';
  await expect(runWasm(page, ordered, 'dash', '2', 'true', 'keep')).resolves.toBe(ordered);
  await expect(runWasm(page, ordered, 'dash', '2', 'true', 'ordered')).resolves.toBe('3. alpha\n4. beta\n5. gamma\n');
  await expect(runWasm(page, ordered, 'dash', '2', 'true', 'one')).resolves.toBe('1. alpha\n1. beta\n1. gamma\n');
  await expect(runWasm(page, ordered, 'dash', '2', 'true', 'zero')).resolves.toBe('0. alpha\n0. beta\n0. gamma\n');
});

test('markdown-list-marker-normalizer controls cover non-default checkbox, sliders, and cap boundary', async ({ page }) => {
  await page.goto('/tools/markdown-list-marker-normalizer/');
  await setTextarea(page, '#in-markdown', '* alpha\n     + beta');
  await page.selectOption('#in-marker', 'asterisk');
  await page.fill('#in-indent', '4');
  await page.uncheck('#in-normalize_indent');
  await page.fill('#in-marker_space', '2');

  await expect(page.locator('#tool-output')).toHaveText('*  alpha\n     *  beta', { timeout: 15_000 });

  const atCap = '- a\n'.repeat(125_000);
  expect(atCap.length).toBe(500_000);
  await expect(runWasm(page, atCap)).resolves.toBe(atCap);
  await expect(runWasm(page, `${atCap}x`)).rejects.toThrow(/character limit/);
});
