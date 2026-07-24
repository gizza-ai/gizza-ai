import { test, expect } from './fixtures';

const SAMPLE_ENEX = `<en-export><note><title>Migration note</title><content><![CDATA[<en-note><h1>Hello</h1><p>Some <b>bold</b> text and a <a href="https://example.com">link</a>.</p></en-note>]]></content><created>20230101T090000Z</created><updated>20230102T101500Z</updated><note-attributes><source-url>https://example.com/article</source-url></note-attributes><tag>import</tag><resource><data encoding="base64">aGVsbG8gd29ybGQ=</data><mime>image/png</mime><resource-attributes><file-name>diagram.png</file-name></resource-attributes></resource></note></en-export>`;

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('enex-to-markdown converts ENEX body, frontmatter, tags, and attachments', async ({ page }) => {
  await page.goto('/tools/enex-to-markdown/');
  await page.fill('#in-enex', SAMPLE_ENEX);
  await expect(page.locator('#tool-output')).toContainText('title: Migration note', { timeout: 15000 });
  const out = await outputText(page);
  expect(out).toContain('created: 2023-01-01T09:00:00Z');
  expect(out).toContain('tags: [import]');
  expect(out).toContain('# Hello');
  expect(out).toContain('**bold**');
  expect(out).toContain('[link](https://example.com)');
  expect(out).toContain('`diagram.png` (image/png, 11 B)');
});

test('enex-to-markdown supports inline metadata and disabled attachments', async ({ page }) => {
  await page.goto('/tools/enex-to-markdown/');
  await page.fill('#in-enex', SAMPLE_ENEX);
  await page.selectOption('#in-metadata', 'inline');
  await page.uncheck('#in-attachments');
  await expect(page.locator('#tool-output')).toContainText('# Migration note', { timeout: 15000 });
  const out = await outputText(page);
  expect(out).toContain('#import');
  expect(out).not.toContain('diagram.png');
});

test('enex-to-markdown plain text mode strips markdown syntax', async ({ page }) => {
  await page.goto('/tools/enex-to-markdown/');
  await page.fill('#in-enex', SAMPLE_ENEX);
  await page.selectOption('#in-format', 'text');
  await page.selectOption('#in-metadata', 'none');
  await expect(page.locator('#tool-output')).toContainText('Migration note', { timeout: 15000 });
  const out = await outputText(page);
  expect(out).toContain('Some bold text and a link');
  expect(out).not.toContain('**bold**');
});

test('enex-to-markdown deep-link pre-fills and auto-runs', async ({ page }) => {
  const enex = '<en-export><note><title>N</title><content><![CDATA[<en-note><p>Hi</p></en-note>]]></content></note></en-export>';
  await page.goto(`/tools/enex-to-markdown/?enex=${encodeURIComponent(enex)}&format=markdown&metadata=none&attachments=false`);
  await expect(page.locator('#in-enex')).toHaveValue(enex, { timeout: 15000 });
  await expect(page.locator('#in-metadata')).toHaveValue('none');
  await expect(page.locator('#in-attachments')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('# N', { timeout: 15000 });
});

test('enex-to-markdown reports malformed input errors', async ({ page }) => {
  await page.goto('/tools/enex-to-markdown/');
  await page.fill('#in-enex', '<en-export><note><title>Oops</content></note>');
  await expect(page.locator('#tool-output')).toContainText('malformed ENEX XML', { timeout: 15000 });
});
