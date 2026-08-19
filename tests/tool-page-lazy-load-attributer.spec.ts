import { test, expect } from './fixtures';

const INPUT = '<img src="photo.jpg"><iframe src="https://example.com/embed"></iframe>';
const EXACT = '<img src="photo.jpg" loading="lazy" decoding="async"><iframe src="https://example.com/embed" loading="lazy"></iframe>';

async function runWasm(
  page: any,
  html = INPUT,
  targets = 'both',
  decoding = 'async',
  skip_first = '0',
  eager_first = 'false',
  fetchpriority_first = 'false',
  respect_skip_markers = 'true',
  output = 'html',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/lazy-load-attributer/gizza_ai_lazy_load_attributer_web.js');
    await mod.default('/tools/lazy-load-attributer/gizza_ai_lazy_load_attributer_web_bg.wasm');
    return mod.run(
      args.html,
      args.targets,
      args.decoding,
      args.skip_first,
      args.eager_first,
      args.fetchpriority_first,
      args.respect_skip_markers,
      args.output,
    );
  }, { html, targets, decoding, skip_first, eager_first, fetchpriority_first, respect_skip_markers, output });
}

test('lazy-load-attributer wasm adds exact attributes to image and iframe', async ({ page }) => {
  await page.goto('/tools/lazy-load-attributer/');
  await page.waitForSelector('#in-html');

  expect(await runWasm(page)).toBe(EXACT);
  expect(await runWasm(page, '<img src="hero.jpg"><img src="b.jpg">', 'images', 'async', '1', 'true', 'true'))
    .toBe('<img src="hero.jpg" loading="eager" decoding="async" fetchpriority="high"><img src="b.jpg" loading="lazy" decoding="async">');
  expect(await runWasm(page, '<img src="a.jpg" class="skip-lazy"><iframe src="v"></iframe>', 'iframes', 'none'))
    .toBe('<img src="a.jpg" class="skip-lazy"><iframe src="v" loading="lazy"></iframe>');
});

test('lazy-load-attributer page computes exact HTML output from the form', async ({ page }) => {
  await page.goto('/tools/lazy-load-attributer/');
  await page.fill('#in-html', INPUT);
  await page.selectOption('#in-targets', 'both');
  await page.selectOption('#in-decoding', 'async');
  await page.fill('#in-skip_first', '0');
  await page.uncheck('#in-eager_first');
  await page.uncheck('#in-fetchpriority_first');
  await page.check('#in-respect_skip_markers');
  await page.selectOption('#in-output', 'html');

  await expect(page.locator('#tool-output')).toHaveText(EXACT, { timeout: 15_000 });
});

test('lazy-load-attributer deep link covers LCP guard and non-default checkboxes', async ({ page }) => {
  const params = new URLSearchParams({
    html: '<img src="hero.jpg"><img src="gallery.jpg">',
    targets: 'images',
    decoding: 'async',
    skip_first: '1',
    eager_first: 'true',
    fetchpriority_first: 'true',
    respect_skip_markers: 'false',
    output: 'html',
  });
  await page.goto(`/tools/lazy-load-attributer/?${params.toString()}`);

  await expect(page.locator('#in-targets')).toHaveValue('images', { timeout: 15_000 });
  await expect(page.locator('#in-skip_first')).toHaveValue('1');
  await expect(page.locator('#in-eager_first')).toBeChecked();
  await expect(page.locator('#in-fetchpriority_first')).toBeChecked();
  await expect(page.locator('#in-respect_skip_markers')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('<img src="hero.jpg" loading="eager" decoding="async" fetchpriority="high"><img src="gallery.jpg" loading="lazy" decoding="async">', { timeout: 15_000 });
});

test('lazy-load-attributer report, error boundary, and CLI example are valid', async ({ page }) => {
  await page.goto('/tools/lazy-load-attributer/');
  await page.waitForSelector('#in-html');

  const report = await runWasm(page, '<img src="hero.jpg" loading="eager"><img alt="x"><iframe src="v"></iframe>', 'both', 'none', '0', 'false', 'false', 'true', 'report');
  expect(report).toContain('<img> tags found:       2');
  expect(report).toContain('<iframe> tags found:    1');
  expect(report).toContain('loading="lazy" added:    1');

  await expect(runWasm(page, '', 'both', 'async', '0')).rejects.toThrow(/no HTML input/);
  await expect(runWasm(page, '<img src="a.jpg">', 'both', 'async', '51')).rejects.toThrow(/skip_first/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool lazy-load-attributer');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
