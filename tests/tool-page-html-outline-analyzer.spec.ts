import { test, expect } from './fixtures';

const htmlDoc = [
  '<!doctype html>',
  '<html><body>',
  '<h1>Getting started</h1>',
  '<h2 id="install">Install</h2>',
  '<p>Text</p>',
  '<h3>macOS</h3>',
  '<h3>Windows</h3>',
  '<h2>Configure</h2>',
  '<h4>Advanced flags</h4>',
  '</body></html>',
].join('\n');

async function setTextarea(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page: any,
  html: string,
  format = 'tree',
  minLevel = '1',
  maxLevel = '6',
  includeIssues = 'true',
  includeCounts = 'true',
  topTags = '20',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/html-outline-analyzer/gizza_ai_html_outline_analyzer_web.js');
    await mod.default('/tools/html-outline-analyzer/gizza_ai_html_outline_analyzer_web_bg.wasm');
    return mod.run(
      args.html,
      args.format,
      args.minLevel,
      args.maxLevel,
      args.includeIssues,
      args.includeCounts,
      args.topTags,
    );
  }, { html, format, minLevel, maxLevel, includeIssues, includeCounts, topTags });
}

test('html-outline-analyzer wasm renders exact tree outline and issue counts', async ({ page }) => {
  await page.goto('/tools/html-outline-analyzer/');
  await page.waitForSelector('#in-html');

  const out = await runWasm(page, htmlDoc);
  expect(out).toContain('HTML OUTLINE — 6 heading(s) · 9 element(s) · 7 distinct tag(s)');
  expect(out).toContain('Levels: h1:1  h2:2  h3:2  h4:1');
  expect(out).toContain('h2  Install  #install');
  expect(out).toContain('skipped-level: Heading level jumps from <h2>');
  expect(out).toContain('TAG COUNTS');
});

test('html-outline-analyzer wasm covers output enums, filters, and checkbox states', async ({ page }) => {
  await page.goto('/tools/html-outline-analyzer/');
  await page.waitForSelector('#in-html');

  await expect(runWasm(page, htmlDoc, 'markdown', '2', '3', 'true', 'false'))
    .resolves.toContain('## OUTLINE (h2–h3: 4 of 6 headings)');

  const json = await runWasm(page, '<h1>A</h1><h1>A</h1><div><span>x</span></div>', 'json', '1', '6', 'true', 'true', '2');
  expect(json).toContain('"headings": 2');
  expect(json).toContain('"multiple-h1"');
  expect(json).toContain('"tags_omitted": 1');

  const csv = await runWasm(page, '<h1>Title, with comma</h1>', 'csv', '1', '6', 'false', 'false', '20');
  expect(csv).toContain('index,level,tag,text,length,id,hidden,line,column');
  expect(csv).toContain('"Title, with comma"');
});

test('html-outline-analyzer page renders exact output and honors controls', async ({ page }) => {
  await page.goto('/tools/html-outline-analyzer/');
  await setTextarea(page, '#in-html', htmlDoc);
  await page.selectOption('#in-format', 'tree');
  await page.fill('#in-min_level', '2');
  await page.fill('#in-max_level', '3');
  await page.uncheck('#in-include_counts');

  await expect(page.locator('#tool-output')).toContainText('OUTLINE (h2–h3: 4 of 6 headings)', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('h2  Install  #install');
  await expect(page.locator('#tool-output')).not.toContainText('TAG COUNTS');
});

test('html-outline-analyzer deep-link prefills params and generated CLI example is generic', async ({ page }) => {
  const params = new URLSearchParams({
    html: '<main><h1>Guide</h1><h2>Install</h2></main>',
    format: 'json',
    min_level: '1',
    max_level: '6',
    include_issues: 'true',
    include_counts: 'false',
    top_tags: '20',
  });

  await page.goto(`/tools/html-outline-analyzer/?${params.toString()}`);
  await expect(page.locator('#in-html')).toHaveValue('<main><h1>Guide</h1><h2>Install</h2></main>', { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toContainText('"headings": 2', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).not.toContainText('"tags": [');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool html-outline-analyzer');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
