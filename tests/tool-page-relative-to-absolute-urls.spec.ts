import { test, expect } from './fixtures';

const tool = '/tools/relative-to-absolute-urls/';
const sample = '<a href="../about.html">About</a>\n<img src="images/logo.png" alt="Logo">\n<a href="#top">Back to top</a>';
const expected = '<a href="https://example.com/about.html">About</a>\n<img src="https://example.com/blog/images/logo.png" alt="Logo">\n<a href="#top">Back to top</a>';

async function outputText(page): Promise<string> {
  const text = await page.locator('#tool-output').textContent();
  return text ?? '';
}

async function runWasm(
  page,
  html: string,
  base = 'https://example.com/blog/post.html',
  attributes = 'common',
  useBaseTag = 'true',
  protocolRelative = 'resolve',
  resolveFragments = 'false',
  styleUrls = 'false',
  output = 'html',
) {
  return await page.evaluate(
    async ({ html, base, attributes, useBaseTag, protocolRelative, resolveFragments, styleUrls, output }) => {
      const mod = await import('/tools/relative-to-absolute-urls/gizza_ai_relative_to_absolute_urls_web.js');
      await mod.default('/tools/relative-to-absolute-urls/gizza_ai_relative_to_absolute_urls_web_bg.wasm');
      return mod.run(html, base, attributes, useBaseTag, protocolRelative, resolveFragments, styleUrls, output);
    },
    { html, base, attributes, useBaseTag, protocolRelative, resolveFragments, styleUrls, output },
  );
}

test('relative-to-absolute-urls page rewrites href and src values without touching anchors', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-html', sample);
  await page.fill('#in-base', 'https://example.com/blog/post.html');
  await page.selectOption('#in-attributes', 'common');
  await page.check('#in-use_base_tag');
  await page.selectOption('#in-protocol_relative', 'resolve');
  await page.uncheck('#in-resolve_fragments');
  await page.uncheck('#in-style_urls');
  await page.selectOption('#in-output', 'html');

  await expect(page.locator('#tool-output')).toContainText('https://example.com/blog/images/logo.png', { timeout: 15000 });
  expect(await outputText(page)).toBe(expected);
});

test('relative-to-absolute-urls deep link prefills non-default switches and report output', async ({ page }) => {
  const html = '<base href="/assets/">\n<div style="background:url(img/hero.jpg)"><a href="#sale">Sale</a></div>';
  await page.goto(
    tool +
      '?html=' +
      encodeURIComponent(html) +
      '&base=' +
      encodeURIComponent('https://shop.example.com/email/august.html') +
      '&attributes=all&use_base_tag=false&protocol_relative=keep&resolve_fragments=true&style_urls=true&output=report',
  );

  await expect(page.locator('#in-html')).toHaveValue(html, { timeout: 15000 });
  await expect(page.locator('#in-base')).toHaveValue('https://shop.example.com/email/august.html');
  await expect(page.locator('#in-attributes')).toHaveValue('all');
  await expect(page.locator('#in-use_base_tag')).not.toBeChecked();
  await expect(page.locator('#in-protocol_relative')).toHaveValue('keep');
  await expect(page.locator('#in-resolve_fragments')).toBeChecked();
  await expect(page.locator('#in-style_urls')).toBeChecked();
  await expect(page.locator('#in-output')).toHaveValue('report');

  await expect(page.locator('#tool-output')).toContainText('base_tag_used,no', { timeout: 15000 });
  const report = await outputText(page);
  expect(report).toContain('effective_base,https://shop.example.com/email/august.html');
  expect(report).toContain('rewritten,3');
});

test('relative-to-absolute-urls wasm covers attributes, protocol-relative, CSS, outputs and cap', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-html');

  const rich = '<link href="site.css"><img srcset="photo.jpg 1x, retina/photo@2x.jpg 2x"><a ping="track/a track/b" href="docs/">Docs</a><meta http-equiv="refresh" content="5; url=next.html"><q cite="quote.html">Q</q>';
  expect(await runWasm(page, rich, 'https://example.com/blog/post.html', 'href-src')).toBe(
    '<link href="https://example.com/blog/site.css"><img srcset="photo.jpg 1x, retina/photo@2x.jpg 2x"><a ping="track/a track/b" href="https://example.com/blog/docs/">Docs</a><meta http-equiv="refresh" content="5; url=next.html"><q cite="quote.html">Q</q>',
  );
  expect(await runWasm(page, rich, 'https://example.com/blog/post.html', 'common')).toBe(
    '<link href="https://example.com/blog/site.css"><img srcset="https://example.com/blog/photo.jpg 1x, https://example.com/blog/retina/photo@2x.jpg 2x"><a ping="https://example.com/blog/track/a https://example.com/blog/track/b" href="https://example.com/blog/docs/">Docs</a><meta http-equiv="refresh" content="5; url=https://example.com/blog/next.html"><q cite="quote.html">Q</q>',
  );
  expect(await runWasm(page, rich, 'https://example.com/blog/post.html', 'all')).toContain(
    '<q cite="https://example.com/blog/quote.html">Q</q>',
  );

  const css = '<style>@import "css/site.css"; .hero{background:url(img/hero.jpg)}</style><div style="background:url(icons/x.svg)"></div>';
  expect(await runWasm(page, css)).toBe(css);
  expect(await runWasm(page, css, 'https://example.com/blog/post.html', 'common', 'true', 'resolve', 'false', 'true')).toBe(
    '<style>@import "https://example.com/blog/css/site.css"; .hero{background:url(https://example.com/blog/img/hero.jpg)}</style><div style="background:url(https://example.com/blog/icons/x.svg)"></div>',
  );

  expect(await runWasm(page, '<img src="//cdn.example.com/a.png"><a href="#top">Top</a>')).toBe(
    '<img src="https://cdn.example.com/a.png"><a href="#top">Top</a>',
  );
  expect(await runWasm(page, '<img src="//cdn.example.com/a.png"><a href="#top">Top</a>', 'https://example.com/blog/post.html', 'common', 'true', 'keep', 'true')).toBe(
    '<img src="//cdn.example.com/a.png"><a href="https://example.com/blog/post.html#top">Top</a>',
  );

  expect(await runWasm(page, sample, 'https://example.com/blog/post.html', 'common', 'true', 'resolve', 'false', 'false', 'urls')).toContain(
    '1,a,href,../about.html,https://example.com/about.html,rewritten',
  );
  expect(await runWasm(page, sample, 'https://example.com/blog/post.html', 'common', 'true', 'resolve', 'false', 'false', 'report')).toContain('rewritten,2');

  const result = await page.evaluate(async () => {
    const mod = await import('/tools/relative-to-absolute-urls/gizza_ai_relative_to_absolute_urls_web.js');
    await mod.default('/tools/relative-to-absolute-urls/gizza_ai_relative_to_absolute_urls_web_bg.wasm');
    const atCap = 'x'.repeat(5_000_000);
    const overCap = atCap + 'x';
    const call = (html: string) => {
      try {
        return { ok: true, value: mod.run(html, 'https://example.com/', 'common', 'true', 'resolve', 'false', 'false', 'html').length };
      } catch (e) {
        return { ok: false, value: String(e) };
      }
    };
    return { atCapBytes: atCap.length, overCapBytes: overCap.length, atCap: call(atCap), overCap: call(overCap) };
  });
  expect(result.atCapBytes).toBe(5_000_000);
  expect(result.overCapBytes).toBe(5_000_001);
  expect(result.atCap.ok).toBe(true);
  expect(result.overCap.ok).toBe(false);
  expect(result.overCap.value).toContain('over the 5000000-byte limit');
});
