import { test, expect } from './fixtures';

async function setTextarea(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page: any,
  urls: string,
  base = '',
  scheme = 'preserve',
  www = 'preserve',
  stripDefaultPort = 'true',
  dotSegments = 'true',
  collapseSlashes = 'false',
  lowercasePath = 'false',
  encoding = 'normalize',
  dropIndex = 'false',
  trailingSlash = 'preserve',
  sortQuery = 'key',
  dedupeQuery = 'false',
  dropEmptyParams = 'false',
  dropTracking = 'false',
  dropFragment = 'false',
  dedupeUrls = 'false',
  onInvalid = 'keep',
  output = 'urls',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/url-normalizer/gizza_ai_url_normalizer_web.js');
    await mod.default('/tools/url-normalizer/gizza_ai_url_normalizer_web_bg.wasm');
    return mod.run(
      args.urls,
      args.base,
      args.scheme,
      args.www,
      args.stripDefaultPort,
      args.dotSegments,
      args.collapseSlashes,
      args.lowercasePath,
      args.encoding,
      args.dropIndex,
      args.trailingSlash,
      args.sortQuery,
      args.dedupeQuery,
      args.dropEmptyParams,
      args.dropTracking,
      args.dropFragment,
      args.dedupeUrls,
      args.onInvalid,
      args.output,
    );
  }, {
    urls,
    base,
    scheme,
    www,
    stripDefaultPort,
    dotSegments,
    collapseSlashes,
    lowercasePath,
    encoding,
    dropIndex,
    trailingSlash,
    sortQuery,
    dedupeQuery,
    dropEmptyParams,
    dropTracking,
    dropFragment,
    dedupeUrls,
    onInvalid,
    output,
  });
}

test('url-normalizer page renders exact canonical URL', async ({ page }) => {
  await page.goto('/tools/url-normalizer/');
  await setTextarea(page, '#in-urls', 'HTTP://Example.COM:80/a/b/../c?b=2&a=1');

  await expect(page.locator('#tool-output')).toHaveText('http://example.com/a/c?a=1&b=2', { timeout: 15_000 });
});

test('url-normalizer deep-link prefills controls and performs SEO cleanup', async ({ page }) => {
  const params = new URLSearchParams({
    urls: 'http://www.example.com:80/blog/index.html?utm_source=news&id=42#intro',
    base: '',
    scheme: 'https',
    www: 'strip',
    strip_default_port: 'true',
    dot_segments: 'true',
    collapse_slashes: 'false',
    lowercase_path: 'false',
    encoding: 'normalize',
    drop_index: 'true',
    trailing_slash: 'remove',
    sort_query: 'key',
    dedupe_query: 'false',
    drop_empty_params: 'false',
    drop_tracking: 'true',
    drop_fragment: 'true',
    dedupe_urls: 'false',
    on_invalid: 'keep',
    output: 'urls',
  });

  await page.goto(`/tools/url-normalizer/?${params.toString()}`);
  await expect(page.locator('#in-urls')).toHaveValue('http://www.example.com:80/blog/index.html?utm_source=news&id=42#intro', { timeout: 15_000 });
  await expect(page.locator('#in-scheme')).toHaveValue('https');
  await expect(page.locator('#in-www')).toHaveValue('strip');
  await expect(page.locator('#in-drop_index')).toBeChecked();
  await expect(page.locator('#in-drop_tracking')).toBeChecked();
  await expect(page.locator('#in-drop_fragment')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('https://example.com/blog?id=42', { timeout: 15_000 });

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool url-normalizer');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('url-normalizer wasm covers enum values and optional checkboxes', async ({ page }) => {
  await page.goto('/tools/url-normalizer/');
  await page.waitForSelector('#in-urls');

  await expect(runWasm(page, 'https://e.com?b=2&a=1', '', 'preserve', 'preserve', 'true', 'true', 'false', 'false', 'normalize', 'false', 'preserve', 'none'))
    .resolves.toBe('https://e.com/?b=2&a=1');
  await expect(runWasm(page, 'https://e.com?t=z&t=a&s=1', '', 'preserve', 'preserve', 'true', 'true', 'false', 'false', 'normalize', 'false', 'preserve', 'key-value'))
    .resolves.toBe('https://e.com/?s=1&t=a&t=z');
  await expect(runWasm(page, 'http://example.com/a', '', 'https'))
    .resolves.toBe('https://example.com/a');
  await expect(runWasm(page, 'https://example.com/a', '', 'http'))
    .resolves.toBe('http://example.com/a');
  await expect(runWasm(page, 'https://www.example.com/a', '', 'preserve', 'strip'))
    .resolves.toBe('https://example.com/a');
  await expect(runWasm(page, 'https://example.com/a', '', 'preserve', 'add'))
    .resolves.toBe('https://www.example.com/a');
  await expect(runWasm(page, 'https://e.com/a%2Fb/%2E%2E/c', '', 'preserve', 'preserve', 'true', 'true', 'false', 'false', 'decode'))
    .resolves.toBe('https://e.com/a/c');
  await expect(runWasm(page, 'https://e.com/a%2db', '', 'preserve', 'preserve', 'true', 'true', 'false', 'false', 'preserve'))
    .resolves.toBe('https://e.com/a%2db');
  await expect(runWasm(page, 'https://e.com/a/index.html', '', 'preserve', 'preserve', 'true', 'true', 'false', 'false', 'normalize', 'true', 'add'))
    .resolves.toBe('https://e.com/a/');
  await expect(runWasm(page, 'https://e.com/a/', '', 'preserve', 'preserve', 'true', 'true', 'false', 'false', 'normalize', 'false', 'remove'))
    .resolves.toBe('https://e.com/a');
  await expect(runWasm(page, 'https://e.com/a?a=1&a=1&b=&utm_source=x#frag', '', 'preserve', 'preserve', 'true', 'true', 'false', 'false', 'normalize', 'false', 'preserve', 'key', 'true', 'true', 'true', 'true'))
    .resolves.toBe('https://e.com/a?a=1');
  await expect(runWasm(page, 'https://e.com/A//B', '', 'preserve', 'preserve', 'true', 'true', 'true', 'true'))
    .resolves.toBe('https://e.com/a/b');
});

test('url-normalizer reports, relative URLs, invalid handling, and exact cap', async ({ page }) => {
  await page.goto('/tools/url-normalizer/');

  await expect(runWasm(page, '../images/logo.png\n?print=1&b=2&a=1', 'https://example.com/docs/guide/index.html'))
    .resolves.toBe('https://example.com/docs/images/logo.png\nhttps://example.com/docs/guide/index.html?a=1&b=2&print=1');
  await expect(runWasm(page, 'http://bad host/a', '', 'preserve', 'preserve', 'true', 'true', 'false', 'false', 'normalize', 'false', 'preserve', 'key', 'false', 'false', 'false', 'false', 'false', 'drop'))
    .resolves.toBe('');
  await expect(runWasm(page, 'https://e.com?b=2&a=1\nhttps://e.com?a=1&b=2', '', 'preserve', 'preserve', 'true', 'true', 'false', 'false', 'normalize', 'false', 'preserve', 'key', 'false', 'false', 'false', 'false', 'true', 'keep', 'report'))
    .resolves.toContain('2,https://e.com?a=1&b=2,https://e.com/?a=1&b=2,duplicate');
  await expect(runWasm(page, 'https://e.com?b=2&a=1', '', 'preserve', 'preserve', 'true', 'true', 'false', 'false', 'normalize', 'false', 'preserve', 'key', 'false', 'false', 'false', 'false', 'false', 'keep', 'changed'))
    .resolves.toBe('https://e.com/?a=1&b=2');
  await expect(runWasm(page, 'https://e.com/a', '', 'preserve', 'preserve', 'true', 'true', 'false', 'false', 'normalize', 'false', 'preserve', 'key', 'false', 'false', 'false', 'false', 'false', 'keep', 'summary'))
    .resolves.toContain('lines_in,1');

  const atCap = 'a'.repeat(1_000_000);
  await expect(runWasm(page, atCap)).resolves.toBe(atCap);
  await expect(runWasm(page, `${atCap}x`)).rejects.toThrow(/over the 1000000-byte limit/);
});
