import { test, expect } from './fixtures';

const HEADING_NOTES = `## What is mitosis?
Cell division that makes two identical cells.

## What is meiosis?
Cell division that makes gametes.`;

async function runWasm(
  page: any,
  markdown: string = HEADING_NOTES,
  mode = 'auto',
  separator = 'auto',
  headingLevel = '0',
  fieldSeparator = 'tab',
  fieldFormat = 'html',
  notetype = 'Basic',
  deck = '',
  tags = '',
  tagsFromHeadings = 'false',
  includeHeaders = 'true',
  dedupe = 'true',
  output = 'anki',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/markdown-flashcards/gizza_ai_markdown_flashcards_web.js');
    await mod.default('/tools/markdown-flashcards/gizza_ai_markdown_flashcards_web_bg.wasm');
    return mod.run(
      args.markdown,
      args.mode,
      args.separator,
      args.headingLevel,
      args.fieldSeparator,
      args.fieldFormat,
      args.notetype,
      args.deck,
      args.tags,
      args.tagsFromHeadings,
      args.includeHeaders,
      args.dedupe,
      args.output,
    );
  }, { markdown, mode, separator, headingLevel, fieldSeparator, fieldFormat, notetype, deck, tags, tagsFromHeadings, includeHeaders, dedupe, output });
}

test('markdown-flashcards page converts heading notes to Anki TSV', async ({ page }) => {
  await page.goto('/tools/markdown-flashcards/');
  await page.fill('#in-markdown', HEADING_NOTES);

  const output = page.locator('#tool-output');
  await expect(output).toContainText('#separator:Tab', { timeout: 20_000 });
  await expect(output).toContainText('#notetype:Basic');
  await expect(output).toContainText('What is mitosis?\tCell division that makes two identical cells.');
  await expect(output).toContainText('What is meiosis?\tCell division that makes gametes.');
});

test('markdown-flashcards deep link covers separator CSV, deck, tags and checkbox state', async ({ page }) => {
  const params = new URLSearchParams({
    markdown: 'gato :: cat\nperro :: dog',
    mode: 'separator',
    separator: 'auto',
    field_separator: 'comma',
    field_format: 'plain',
    deck: 'Spanish::Week 1',
    tags: 'exam week1',
    include_headers: 'false',
    dedupe: 'false',
    output: 'anki',
  });
  await page.goto(`/tools/markdown-flashcards/?${params.toString()}`);

  await expect(page.locator('#in-mode')).toHaveValue('separator', { timeout: 15_000 });
  await expect(page.locator('#in-field_separator')).toHaveValue('comma');
  await expect(page.locator('#in-include_headers')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('gato,cat,exam week1\nperro,dog,exam week1', { timeout: 20_000 });
});

test('markdown-flashcards wasm covers enum values, cap boundary and CLI example', async ({ page }) => {
  await page.goto('/tools/markdown-flashcards/');

  const qa = await runWasm(page, 'Q: Capital of France?\nA: Paris', 'qa', 'auto', '0', 'tab', 'html', 'Basic', '', '', 'false', 'false', 'true', 'preview');
  expect(qa).toBe('1 card · mode: qa · note type: Basic\n\n1. Q: Capital of France?\n   A: Paris');

  const table = await runWasm(page, '| Front | Back | Tags |\n| --- | --- | --- |\n| ser | to be | spanish verbs |', 'table', 'auto', '0', 'semicolon', 'plain', 'Basic', '', '', 'false', 'true', 'true', 'anki');
  expect(table).toContain('#separator:Semicolon');
  expect(table).toContain('ser;to be;spanish verbs');

  const cloze = await runWasm(page, 'The **mitochondrion** is the **powerhouse** of the cell.', 'cloze', 'auto', '0', 'pipe', 'html', 'Basic', '', '', 'false', 'true', 'true', 'anki');
  expect(cloze).toContain('#notetype:Cloze');
  expect(cloze).toContain('The {{c1::mitochondrion}} is the {{c2::powerhouse}} of the cell.|');

  const json = await runWasm(page, '## Biology\n\n### Nucleus\nHolds DNA.', 'heading', 'auto', '3', 'tab', 'markdown', 'Basic (type in the answer)', 'Science', 'exam', 'true', 'true', 'true', 'json');
  expect(json).toContain('"notetype": "Basic (type in the answer)"');
  expect(json).toContain('"front": "Nucleus"');
  expect(json).toContain('"tags": ["Biology"]');

  const boundary = Array.from({ length: 5000 }, (_, i) => `term${i} :: def${i}`).join('\n');
  const boundaryOut = await runWasm(page, boundary, 'separator', 'auto', '0', 'tab', 'plain', 'Basic', '', '', 'false', 'false', 'true', 'preview');
  expect(boundaryOut).toContain('5000 cards · mode: separator · note type: Basic');

  await expect(runWasm(page, 'plain unstructured notes', 'auto')).rejects.toThrow(/could not detect/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool markdown-flashcards');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
