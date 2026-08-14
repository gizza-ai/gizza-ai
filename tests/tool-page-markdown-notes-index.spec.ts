import { test, expect } from './fixtures';

const NOTES = `# Getting started

Some intro text.

## Install

Run it.

# Weekly review

Notes about the week.`;

async function runWasm(
  page: any,
  notes: string = NOTES,
  split = 'heading',
  format = 'markdown',
  headingDepth = '2',
  groupBy = 'none',
  sort = 'input',
  linkStyle = 'anchor',
  includeToc = 'true',
  includeStats = 'true',
  inlineTags = 'true',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/markdown-notes-index/gizza_ai_markdown_notes_index_web.js');
    await mod.default('/tools/markdown-notes-index/gizza_ai_markdown_notes_index_web_bg.wasm');
    return mod.run(
      args.notes,
      args.split,
      args.format,
      args.headingDepth,
      args.groupBy,
      args.sort,
      args.linkStyle,
      args.includeToc,
      args.includeStats,
      args.inlineTags,
    );
  }, { notes, split, format, headingDepth, groupBy, sort, linkStyle, includeToc, includeStats, inlineTags });
}

test('markdown-notes-index page builds a linked index from pasted notes', async ({ page }) => {
  await page.goto('/tools/markdown-notes-index/');
  await page.fill('#in-notes', NOTES);

  const output = page.locator('#tool-output');
  await expect(output).toContainText('# Notes index', { timeout: 20_000 });
  await expect(output).toContainText('1. [Getting started](#getting-started)');
  await expect(output).toContainText('- Install');
  await expect(output).toContainText('## Weekly review');
});

test('markdown-notes-index deep link groups notes by tag and hides stats', async ({ page }) => {
  const params = new URLSearchParams({
    notes: '# One\n\n#alpha #beta\n\n# Two\n\n#beta\n\n# Three\n\nNo tags here.',
    split: 'heading',
    format: 'markdown',
    heading_depth: '2',
    group_by: 'tag',
    sort: 'input',
    link_style: 'anchor',
    include_toc: 'true',
    include_stats: 'false',
    inline_tags: 'true',
  });
  await page.goto(`/tools/markdown-notes-index/?${params.toString()}`);

  await expect(page.locator('#in-group_by')).toHaveValue('tag', { timeout: 15_000 });
  await expect(page.locator('#in-include_stats')).not.toBeChecked();
  const output = page.locator('#tool-output');
  await expect(output).toContainText('### #alpha', { timeout: 20_000 });
  await expect(output).toContainText('### #beta');
  await expect(output).toContainText('### Untagged');
});

test('markdown-notes-index wasm covers enum values, cap boundary, errors and CLI example', async ({ page }) => {
  await page.goto('/tools/markdown-notes-index/');

  const exactMarkdown = (await runWasm(page, NOTES, 'heading', 'markdown')).trimEnd();
  expect(exactMarkdown).toBe(`# Notes index

2 notes · 0 tags · 14 words

## Contents

1. [Getting started](#getting-started)
2. [Weekly review](#weekly-review)

## Getting started

8 words · 1 heading

- Install

## Weekly review

6 words · 0 headings`);

  const wiki = await runWasm(page, '# Meeting notes\n\n## Actions', 'heading', 'markdown', '2', 'none', 'input', 'wiki', 'true', 'false', 'true');
  expect(wiki).toContain('1. [[Meeting notes]]');
  expect(wiki).toContain('- [[Meeting notes#Actions]]');

  const csv = await runWasm(page, '=== notes/todo.md ===\n# Todo\n\n## Today\n\n=== notes/ideas.md ===\nIdeas without a heading.', 'file-marker', 'csv');
  expect(csv).toContain('title,file,tags,headings,words');
  expect(csv).toContain('Todo,notes/todo.md');
  expect(csv).toContain('ideas,notes/ideas.md');

  const json = await runWasm(page, '---\ntitle: Q3 plan\ntags: [planning, roadmap]\n---\n\nShip it.', 'heading', 'json', '2', 'tag', 'title');
  expect(json).toContain('"title": "Q3 plan"');
  expect(json).toContain('"groups"');

  const boundary = Array.from({ length: 500 }, (_, i) => `# Note ${i + 1}\n\nBody ${i + 1}.`).join('\n\n');
  const boundaryOut = await runWasm(page, boundary, 'heading', 'markdown', '0', 'none', 'input', 'none', 'false', 'false', 'false');
  expect(boundaryOut).toContain('500 notes');
  await expect(runWasm(page, `${boundary}\n\n# Note 501`, 'heading')).rejects.toThrow(/limit is 500/);

  await expect(runWasm(page, NOTES, 'file-marker')).rejects.toThrow(/no file markers found/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool markdown-notes-index');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
