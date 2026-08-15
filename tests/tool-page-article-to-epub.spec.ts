import { test, expect, type Page } from './fixtures';

const tool = '/tools/article-to-epub/';
const TEXT_ARTICLE =
  '# The Tide Pools of Muir Beach\n\n' +
  'At low tide the rocks north of the parking lot turn into a museum.\n\n' +
  '## What to look for\n\n' +
  '- Ochre sea stars\n' +
  '- Green anemones\n\n' +
  '# Getting there\n\n' +
  'Park early; the lot fills by nine.';
const HTML_ARTICLE =
  '<h1>Release Notes</h1><p>The <em>autumn</em> build ships today.</p><h2>Fixes</h2><ul><li>Faster startup</li><li>Fewer crashes</li></ul>';

async function outputText(page: Page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

async function epubInfo(page: Page) {
  return page.evaluate(async () => {
    const a = document.getElementById('tool-output-download') as HTMLAnchorElement | null;
    const href = a?.href || '';
    if (!href.startsWith('data:application/epub+zip;base64,')) {
      return { error: 'no EPUB data URL: ' + href };
    }
    const buf = new Uint8Array(await (await fetch(href)).arrayBuffer());
    const text = new TextDecoder('latin1').decode(buf);
    return {
      download: a?.getAttribute('download'),
      bytes: buf.length,
      zipMagic: text.slice(0, 2),
      hasMimetype: text.includes('application/epub+zip'),
      hasContainer: text.includes('META-INF/container.xml'),
      hasNav: text.includes('nav.xhtml'),
    };
  });
}

test('article-to-epub converts Markdown-ish text into a real EPUB download', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-content', TEXT_ARTICLE);
  await page.selectOption('#in-input_format', 'text');
  await page.fill('#in-title', 'The Tide Pools of Muir Beach');
  await page.fill('#in-author', 'Jane Doe');
  await page.selectOption('#in-split_level', 'h1');

  await expect(page.locator('#tool-output')).toContainText('EPUB ready', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('2 chapters');
  await expect(page.locator('#tool-output-download')).toBeVisible();
  const epub = await epubInfo(page);
  expect(epub.error).toBeUndefined();
  expect(epub.download).toBe('the-tide-pools-of-muir-beach.epub');
  expect(epub.bytes).toBeGreaterThan(2500);
  expect(epub.zipMagic).toBe('PK');
  expect(epub.hasMimetype).toBe(true);
  expect(epub.hasContainer).toBe(true);
  expect(epub.hasNav).toBe(true);
});

test('article-to-epub accepts cleaned HTML and splits at h1/h2', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-content', HTML_ARTICLE);
  await page.selectOption('#in-input_format', 'html');
  await page.fill('#in-title', 'Release Notes');
  await page.selectOption('#in-split_level', 'h1-h2');

  await expect(page.locator('#tool-output')).toContainText('2 chapters', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('read as html');
  const epub = await epubInfo(page);
  expect(epub.error).toBeUndefined();
  expect(epub.download).toBe('release-notes.epub');
  expect(epub.hasNav).toBe(true);
});

test('article-to-epub deep link prefills and disables the title page checkbox', async ({ page }) => {
  await page.goto(
    tool +
      '?content=' +
      encodeURIComponent('A short essay with no headings at all. It should arrive as one readable chapter.') +
      '&input_format=text&title=A%20Short%20Essay&author=Jane%20Doe&language=en&publisher=Coastal%20Review&split_level=none&include_title_page=false&base_font_size=13',
  );

  await expect(page.locator('#in-title')).toHaveValue('A Short Essay', { timeout: 15_000 });
  await expect(page.locator('#in-input_format')).toHaveValue('text');
  await expect(page.locator('#in-split_level')).toHaveValue('none');
  await expect(page.locator('#in-include_title_page')).not.toBeChecked();
  await expect(page.locator('#in-base_font_size')).toHaveValue('13');
  await expect(page.locator('#tool-output')).toContainText('1 chapter', { timeout: 15_000 });
  const epub = await epubInfo(page);
  expect(epub.error).toBeUndefined();
  expect(epub.download).toBe('a-short-essay.epub');
});

test('article-to-epub reports invalid metadata without exposing a download', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-content', TEXT_ARTICLE);
  await page.fill('#in-language', 'not a language tag!');

  await expect(page.locator('#tool-output')).toContainText('is not a language tag', {
    timeout: 15_000,
  });
  await expect(page.locator('#tool-output')).toContainText("expected something like 'en'");
  await expect(page.locator('#tool-output-download')).toBeHidden();
});

test('article-to-epub shows a neutral idle state before content is pasted', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('#tool-output')).toContainText('your EPUB download will appear here', {
    timeout: 15_000,
  });
  await expect(page.locator('#tool-output-download')).toBeHidden();
});
