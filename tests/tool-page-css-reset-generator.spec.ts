import { test, expect } from './fixtures';

async function runWasm(
  page: any,
  preset = 'modern',
  include = '',
  exclude = '',
  selectorStyle = 'standard',
  layer = '',
  lineHeight = '1.5',
  bodyMinHeight = '100svh',
  comments = 'true',
  minify = 'false',
  indent = '2',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/css-reset-generator/gizza_ai_css_reset_generator_web.js');
    await mod.default('/tools/css-reset-generator/gizza_ai_css_reset_generator_web_bg.wasm');
    return mod.run(
      args.preset,
      args.include,
      args.exclude,
      args.selectorStyle,
      args.layer,
      args.lineHeight,
      args.bodyMinHeight,
      args.comments,
      args.minify,
      args.indent,
    );
  }, { preset, include, exclude, selectorStyle, layer, lineHeight, bodyMinHeight, comments, minify, indent });
}

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('css-reset-generator wasm covers presets, enums, minify and boundaries', async ({ page }) => {
  await page.goto('/tools/css-reset-generator/');
  await page.waitForSelector('#in-preset');

  const modern = await runWasm(page);
  expect(modern).toContain('/* CSS reset — preset: modern');
  expect(modern).toContain('box-sizing: border-box;');
  expect(modern).toContain('min-height: 100svh;');
  expect(modern).toContain('@media (prefers-reduced-motion: reduce)');

  for (const preset of ['minimal', 'normalize', 'classic', 'preflight']) {
    await expect(runWasm(page, preset, '', '', 'standard', '', '1.5', 'none')).resolves.toContain('{');
  }
  for (const height of ['100dvh', '100vh', 'none']) {
    await expect(runWasm(page, 'minimal', '', '', 'standard', '', '1', height, 'false', 'false', '0')).resolves.toContain('box-sizing: border-box');
  }

  const layered = await runWasm(page, 'modern', 'focus-visible smooth-scroll', 'font-smoothing', 'where', 'base.reset', '3', '100dvh', 'true', 'false', '8');
  expect(layered).toContain('@layer base.reset {');
  expect(layered).toContain(':where(:focus-visible)');
  expect(layered).toContain('min-height: 100dvh;');
  expect(layered).not.toContain('font-smoothing');

  const minified = await runWasm(page, 'minimal', '', '', 'standard', '', '1.5', '100svh', 'true', 'true', '2');
  expect(minified).not.toContain('\n');
  expect(minified).toContain('*,*::before,*::after{box-sizing:border-box}');
});

test('css-reset-generator page renders exact minified output and non-default checkbox', async ({ page }) => {
  const params = new URLSearchParams({
    preset: 'minimal',
    include: 'focus-visible',
    exclude: 'media',
    selector_style: 'where',
    layer: 'base.reset',
    line_height: '1',
    body_min_height: 'none',
    comments: 'false',
    minify: 'true',
    indent: '0',
  });
  await page.goto(`/tools/css-reset-generator/?${params.toString()}`);
  await expect(page.locator('#in-preset')).toHaveValue('minimal', { timeout: 15_000 });
  await expect(page.locator('#in-include')).toHaveValue('focus-visible');
  await expect(page.locator('#in-exclude')).toHaveValue('media');
  await expect(page.locator('#in-minify')).toBeChecked();

  const expected = '@layer base.reset{:where(*),*::before,*::after{box-sizing:border-box}:where(body,h1,h2,h3,h4,h5,h6,p,figure,figcaption,blockquote,dl,dd,pre){margin:0}:where(input,button,textarea,select){font:inherit;color:inherit;letter-spacing:inherit}:where(:focus-visible){outline:2px solid currentColor;outline-offset:2px}}';
  await expect(page.locator('#tool-output')).toHaveText(expected, { timeout: 15_000 });

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool css-reset-generator');
  expect(cli).toContain("'preset=modern'");
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('css-reset-generator deep-link pre-fills and outputs layered css', async ({ page }) => {
  const params = new URLSearchParams({
    preset: 'modern',
    include: 'focus-visible smooth-scroll',
    exclude: 'font-smoothing',
    selector_style: 'where',
    layer: 'base.reset',
    line_height: '1.75',
    body_min_height: '100dvh',
    comments: 'true',
    minify: 'false',
    indent: '4',
  });

  await page.goto(`/tools/css-reset-generator/?${params.toString()}`);
  await expect(page.locator('#in-preset')).toHaveValue('modern', { timeout: 15_000 });
  await expect(page.locator('#in-include')).toHaveValue('focus-visible smooth-scroll');
  await expect(page.locator('#in-selector_style')).toHaveValue('where');
  await expect(page.locator('#in-body_min_height')).toHaveValue('100dvh');
  await expect(page.locator('#tool-output')).toContainText('@layer base.reset {', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('line-height: 1.75;');
  await expect(page.locator('#tool-output')).toContainText(':where(:focus-visible)');
  expect(await outputText(page)).not.toContain('font-smoothing');
});
