import { test, expect } from './fixtures';

const tool = '/tools/html-comment-stripper/';
const sample =
  '<!--! (c) 2026 Example Ltd -->\n<h1>Hello</h1>\n<!-- internal note: swap the hero copy -->\n<p>Ship it.</p>';
const kinds = '<p>a</p>\n<!-- note -->\n<!--! keep -->\n<!--#include x -->\n<!--[if IE]>z<![endif]-->\n';

async function outputText(page): Promise<string> {
  const text = await page.locator('#tool-output').textContent();
  return text ?? '';
}

async function runWasm(
  page,
  html: string,
  keepConditional = 'true',
  keepSsi = 'true',
  keepBang = 'true',
  pattern = '',
  patternMode = 'keep',
  removeCssComments = 'false',
  blankLines = 'keep',
  output = 'html',
) {
  return await page.evaluate(
    async ({
      html,
      keepConditional,
      keepSsi,
      keepBang,
      pattern,
      patternMode,
      removeCssComments,
      blankLines,
      output,
    }) => {
      const mod = await import('/tools/html-comment-stripper/gizza_ai_html_comment_stripper_web.js');
      await mod.default('/tools/html-comment-stripper/gizza_ai_html_comment_stripper_web_bg.wasm');
      return mod.run(
        html,
        keepConditional,
        keepSsi,
        keepBang,
        pattern,
        patternMode,
        removeCssComments,
        blankLines,
        output,
      );
    },
    {
      html,
      keepConditional,
      keepSsi,
      keepBang,
      pattern,
      patternMode,
      removeCssComments,
      blankLines,
      output,
    },
  );
}

test('html-comment-stripper page strips notes and keeps the banner, byte-for-byte otherwise', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-html', sample);
  await page.check('#in-keep_conditional');
  await page.check('#in-keep_ssi');
  await page.check('#in-keep_bang');
  await page.fill('#in-pattern', '');
  await page.selectOption('#in-pattern_mode', 'keep');
  await page.uncheck('#in-remove_css_comments');
  await page.selectOption('#in-blank_lines', 'keep');
  await page.selectOption('#in-output', 'html');

  await expect(page.locator('#tool-output')).toContainText('<p>Ship it.</p>', { timeout: 15000 });
  // The banner survives, the note is gone, and the line it sat on is left blank
  // because the default changes no whitespace at all.
  expect(await outputText(page)).toBe('<!--! (c) 2026 Example Ltd -->\n<h1>Hello</h1>\n\n<p>Ship it.</p>');

  // Same input, blank lines trimmed — the emptied line goes too.
  await page.selectOption('#in-blank_lines', 'trim');
  await expect
    .poll(async () => await outputText(page))
    .toBe('<!--! (c) 2026 Example Ltd -->\n<h1>Hello</h1>\n<p>Ship it.</p>');
});

test('html-comment-stripper deep link prefills non-default checkbox states and the report output', async ({ page }) => {
  const html = '<style>/* x */a{b:c}</style>\n<!--[if IE]>y<![endif]-->\n<!-- note -->\n<p>ok</p>';
  await page.goto(
    tool +
      '?html=' +
      encodeURIComponent(html) +
      '&keep_conditional=false&keep_ssi=true&keep_bang=true&pattern=&pattern_mode=keep' +
      '&remove_css_comments=true&blank_lines=trim&output=report',
  );

  await expect(page.locator('#in-html')).toHaveValue(html, { timeout: 15000 });
  // Both boxes carry a NON-default state: one default-on box off, one
  // default-off box on.
  await expect(page.locator('#in-keep_conditional')).not.toBeChecked();
  await expect(page.locator('#in-remove_css_comments')).toBeChecked();
  await expect(page.locator('#in-keep_ssi')).toBeChecked();
  await expect(page.locator('#in-keep_bang')).toBeChecked();
  await expect(page.locator('#in-blank_lines')).toHaveValue('trim');
  await expect(page.locator('#in-output')).toHaveValue('report');

  await expect(page.locator('#tool-output')).toContainText('percent_smaller');
  expect((await outputText(page)).trim()).toBe(
    'metric,value\ncomments_found,2\ncomments_removed,2\ncomments_kept,0\nremoved_plain,1\n' +
      'removed_conditional,1\nremoved_ssi,0\nremoved_bang,0\ncss_comments_removed,1\n' +
      'bytes_before,78\nbytes_after,31\nbytes_saved,47\npercent_smaller,60.26',
  );
});

test('html-comment-stripper wasm covers every advertised kind switch, pattern mode, blank-line rule and output', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-html');

  // Kinds: conditional / ssi / bang are kept by default, and each has its own switch.
  expect(await runWasm(page, kinds)).toBe(
    '<p>a</p>\n\n<!--! keep -->\n<!--#include x -->\n<!--[if IE]>z<![endif]-->\n',
  );
  expect(await runWasm(page, kinds, 'false', 'false', 'false', '', 'keep', 'false', 'trim')).toBe('<p>a</p>\n');
  expect(await runWasm(page, kinds, 'false', 'true', 'true', '', 'keep', 'false', 'trim')).toBe(
    '<p>a</p>\n<!--! keep -->\n<!--#include x -->\n',
  );
  expect(await runWasm(page, kinds, 'true', 'false', 'true', '', 'keep', 'false', 'trim')).toBe(
    '<p>a</p>\n<!--! keep -->\n<!--[if IE]>z<![endif]-->\n',
  );
  expect(await runWasm(page, kinds, 'true', 'true', 'false', '', 'keep', 'false', 'trim')).toBe(
    '<p>a</p>\n<!--#include x -->\n<!--[if IE]>z<![endif]-->\n',
  );

  // Checkbox marshaling: the page sends "true"/"false", but every truthy form parses.
  expect(await runWasm(page, kinds, 'on', 'yes', '1', '', 'keep', '0', 'trim')).toBe(
    '<p>a</p>\n<!--! keep -->\n<!--#include x -->\n<!--[if IE]>z<![endif]-->\n',
  );

  // pattern_mode: keep = protect-list, only = inverted removal.
  expect(await runWasm(page, '<!-- RELEASE 7 --><!-- note --><p>x</p>', 'true', 'true', 'true', 'RELEASE', 'keep')).toBe(
    '<!-- RELEASE 7 --><p>x</p>',
  );
  expect(
    await runWasm(
      page,
      '<!-- wp:paragraph --><p>x</p><!-- /wp:paragraph --><!-- keep this -->',
      'true',
      'true',
      'true',
      '^\\s*/?wp:',
      'only',
    ),
  ).toBe('<p>x</p><!-- keep this -->');

  // blank_lines: keep / trim / collapse.
  expect(await runWasm(page, '<p>a</p>\n<!-- x -->\n\n\n<p>b</p>\n')).toBe('<p>a</p>\n\n\n\n<p>b</p>\n');
  expect(await runWasm(page, '<p>a</p>\n<!-- x -->\n\n\n<p>b</p>\n', 'true', 'true', 'true', '', 'keep', 'false', 'trim')).toBe(
    '<p>a</p>\n\n\n<p>b</p>\n',
  );
  expect(
    await runWasm(page, '<p>a</p>\n<!-- x -->\n\n\n<p>b</p>\n', 'true', 'true', 'true', '', 'keep', 'false', 'collapse'),
  ).toBe('<p>a</p>\n\n<p>b</p>\n');

  // remove_css_comments: opt-in and string-aware; raw text and attributes are never touched.
  const css = '<style>/* hide */a{color:red}b{content:"/* keep */"}</style>';
  expect(await runWasm(page, css)).toBe(css);
  expect(await runWasm(page, css, 'true', 'true', 'true', '', 'keep', 'true')).toBe(
    '<style>a{color:red}b{content:"/* keep */"}</style>',
  );
  expect(
    await runWasm(page, '<script>var s = "<!-- x -->";</script><a title="<!-- y -->">z</a><!-- gone -->'),
  ).toBe('<script>var s = "<!-- x -->";</script><a title="<!-- y -->">z</a>');

  // output: html / report / comments.
  expect(await runWasm(page, kinds, 'true', 'true', 'true', '', 'keep', 'false', 'keep', 'comments')).toBe(
    'line,kind,action,comment\n2,plain,removed,<!-- note -->\n3,bang,kept,<!--! keep -->\n' +
      '4,ssi,kept,<!--#include x -->\n5,conditional,kept,<!--[if IE]>z<![endif]-->\n',
  );
  expect(await runWasm(page, kinds, 'true', 'true', 'true', '', 'keep', 'false', 'keep', 'report')).toBe(
    'metric,value\ncomments_found,4\ncomments_removed,1\ncomments_kept,3\nremoved_plain,1\n' +
      'removed_conditional,0\nremoved_ssi,0\nremoved_bang,0\ncss_comments_removed,0\n' +
      'bytes_before,83\nbytes_after,70\nbytes_saved,13\npercent_smaller,15.66\n',
  );

  // Errors are reported, never silently swallowed.
  await expect(runWasm(page, '<p>a</p>\n<!-- oops\n')).rejects.toThrow(/unterminated comment.*line 2/);
  await expect(runWasm(page, '<p><!--x--></p>', 'true', 'true', 'true', '(')).rejects.toThrow(/invalid pattern/);
  await expect(runWasm(page, '<p><!--x--></p>', 'true', 'true', 'true', '', 'only')).rejects.toThrow(/needs a pattern/);
  await expect(runWasm(page, '   ')).rejects.toThrow(/no HTML input/);
  await expect(
    runWasm(page, '<p><!--x--></p>', 'true', 'true', 'true', '', 'keep', 'false', 'nope'),
  ).rejects.toThrow(/unknown blank_lines/);
});

test('html-comment-stripper enforces the advertised 5,000,000-byte cap at the boundary', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-html');

  const result = await page.evaluate(async () => {
    const mod = await import('/tools/html-comment-stripper/gizza_ai_html_comment_stripper_web.js');
    await mod.default('/tools/html-comment-stripper/gizza_ai_html_comment_stripper_web_bg.wasm');
    const atCap = '<p>' + 'x'.repeat(5_000_000 - 3);
    const overCap = atCap + 'x';
    const call = (html: string) => {
      try {
        return { ok: true, value: mod.run(html, 'true', 'true', 'true', '', 'keep', 'false', 'keep', 'html').slice(0, 5) };
      } catch (e) {
        return { ok: false, value: String(e) };
      }
    };
    return { atCapBytes: atCap.length, overCapBytes: overCap.length, atCap: call(atCap), overCap: call(overCap) };
  });

  expect(result.atCapBytes).toBe(5_000_000);
  expect(result.overCapBytes).toBe(5_000_001);
  expect(result.atCap.ok).toBe(true);
  expect(result.atCap.value).toBe('<p>xx');
  expect(result.overCap.ok).toBe(false);
  expect(result.overCap.value).toMatch(/over the 5000000-byte limit/);
});

test('html-comment-stripper page ships workflow example presets', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(5);

  await page.click('.tool-example-chip:has-text("Remove WordPress block markers only")');
  await expect(page.locator('#in-pattern')).toHaveValue('^\\s*/?wp:');
  await expect(page.locator('#in-pattern_mode')).toHaveValue('only');
  await expect(page.locator('#tool-output')).toContainText('<!-- release notes: keep me -->', { timeout: 15000 });

  await page.click('.tool-example-chip:has-text("Drop legacy IE conditional comments")');
  await expect(page.locator('#in-keep_conditional')).not.toBeChecked();
  await expect(page.locator('#in-blank_lines')).toHaveValue('trim');
  await expect.poll(async () => await outputText(page)).toBe('<p>Modern browsers only.</p>');
});
