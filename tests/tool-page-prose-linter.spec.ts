import { test, expect } from './fixtures';

const SAMPLE = 'So the cat was stolen at the end of the day.';

async function runWasm(
  page: any,
  text = SAMPLE,
  checks = 'all',
  output = 'report',
  ignore = '',
  maxIssues = '200',
  longSentenceWords = '30',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/prose-linter/gizza_ai_prose_linter_web.js');
    await mod.default('/tools/prose-linter/gizza_ai_prose_linter_web_bg.wasm');
    return mod.run(
      args.text,
      args.checks,
      args.output,
      args.ignore,
      Number(args.maxIssues),
      Number(args.longSentenceWords),
    );
  }, { text, checks, output, ignore, maxIssues, longSentenceWords });
}

test('prose-linter page computes a real style report from the form', async ({ page }) => {
  await page.goto('/tools/prose-linter/');
  await page.fill('#in-text', SAMPLE);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('3 issues found in 11 words / 1 sentence.', { timeout: 15_000 });
  await expect(out).toContainText('so-start');
  await expect(out).toContainText('"was stolen" is passive voice');
  await expect(out).toContainText('"at the end of the day" is a cliché');
});

test('prose-linter deep link covers annotated output and ignored phrases', async ({ page }) => {
  const params = new URLSearchParams({
    text: 'The report was written. It is very good.',
    checks: 'passive,weasel',
    output: 'annotated',
    ignore: 'very',
    max_issues: '200',
    long_sentence_words: '30',
  });
  await page.goto(`/tools/prose-linter/?${params.toString()}`);

  await expect(page.locator('#in-output')).toHaveValue('annotated', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('1 issue found', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('passive: "was written" is passive voice');
  await expect(page.locator('#tool-output')).not.toContainText('"very" is a weasel word');
});

test('prose-linter wasm covers output enums, checks, issue cap, long-sentence boundary, and CLI example', async ({ page }) => {
  await page.goto('/tools/prose-linter/');
  await page.waitForSelector('#in-text');

  const report = await runWasm(page);
  expect(report).toContain('3 issues found in 11 words / 1 sentence.');
  expect(report).toContain('so-start');

  const annotated = await runWasm(page, 'So we shipped.', 'so-start', 'annotated');
  expect(annotated).toContain('^');
  expect(annotated).toContain('so-start: Sentence starts with "So"');

  const json = await runWasm(page, 'This is very good.', 'weasel', 'json');
  expect(JSON.parse(json).issues[0].rule).toBe('weasel');

  const eprime = await runWasm(page, 'There is a problem.', 'eprime', 'report');
  expect(eprime).toContain('eprime');

  const capped = await runWasm(page, 'So the cat was stolen at the end of the day.', 'all', 'report', '', '2', '30');
  expect(capped).toContain('Showing the first 2 of 3 issues');

  const longOff = await runWasm(page, 'One two three four five.', 'long-sentence', 'report', '', '200', '0');
  expect(longOff).toContain('No issues found');

  const longOn = await runWasm(page, 'One two three four five.', 'long-sentence', 'report', '', '200', '4');
  expect(longOn).toContain('long-sentence');

  await expect(runWasm(page, '', 'all')).rejects.toThrow(/text is empty/);
  await expect(runWasm(page, 'Hello.', 'spelling')).rejects.toThrow(/unknown check 'spelling'/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool prose-linter');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
