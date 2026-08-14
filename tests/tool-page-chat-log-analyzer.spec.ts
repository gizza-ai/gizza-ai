import { test, expect } from './fixtures';

const IRC_LOG = '2024-01-05 21:07:33 <alice> pizza tonight? https://example.com/menu\n2024-01-05 21:08:01 <@bob> pizza sounds great\n2024-01-05 21:09:00 -!- carol [~c@host] has joined #food\n2024-01-06 09:15:00 <bob> morning\n2024-01-06 09:16:10 * alice waves';

async function runWasm(
  page: any,
  log: string = IRC_LOG,
  output = 'summary',
  top = '10',
  minWordLength = '3',
  ignoreStopwords = 'true',
  excludeNicks = '',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/chat-log-analyzer/gizza_ai_chat_log_analyzer_web.js');
    await mod.default('/tools/chat-log-analyzer/gizza_ai_chat_log_analyzer_web_bg.wasm');
    return mod.run(
      args.log,
      args.output,
      args.top,
      args.minWordLength,
      args.ignoreStopwords,
      args.excludeNicks,
    );
  }, { log, output, top, minWordLength, ignoreStopwords, excludeNicks });
}

test('chat-log-analyzer page computes a real IRC report from the form', async ({ page }) => {
  await page.goto('/tools/chat-log-analyzer/');
  await page.fill('#in-log', IRC_LOG);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Messages: 4', { timeout: 15_000 });
  await expect(out).toContainText('Participants: 2');
  await expect(out).toContainText('Busiest hour: 09:00');
  await expect(out).toContainText('Joins: 1');
  await expect(out).toContainText('Actions (/me): 1');
  await expect(out).toContainText('example.com');
});

test('chat-log-analyzer deep link covers JSON output, bot exclusion, and unchecked stopwords', async ({ page }) => {
  const params = new URLSearchParams({
    log: 'alice: the cat and the hat\nbob: the cat\ngizzabot: the automated bot notice',
    output: 'json',
    top: '2',
    min_word_length: '1',
    ignore_stopwords: 'false',
    exclude_nicks: 'gizzabot',
  });
  await page.goto(`/tools/chat-log-analyzer/?${params.toString()}`);

  await expect(page.locator('#in-output')).toHaveValue('json', { timeout: 15_000 });
  await expect(page.locator('#in-ignore_stopwords')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('"messages": 2', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('"excluded_lines": 1');
  await expect(page.locator('#tool-output')).toContainText('"word": "the"');
});

test('chat-log-analyzer wasm covers enum values, ranking caps, boundaries, and CLI example', async ({ page }) => {
  await page.goto('/tools/chat-log-analyzer/');

  const summary = await runWasm(page);
  expect(summary).toContain('Messages: 4');
  expect(summary).toContain('Busiest weekday: Friday');

  const json = await runWasm(page, IRC_LOG, 'json');
  expect(json).toContain('"messages": 4');
  expect(json).toContain('"busiest_hour": 9');

  const capped = await runWasm(page, '<alice> alpha bravo charlie\n<bob> alpha bravo\n<carol> alpha', 'summary', '2', '1', 'false');
  expect(capped).toContain('(top 2 of 3 participants)');
  expect(capped).toContain('     3  alpha');
  expect(capped).not.toContain('carol');

  const all = await runWasm(page, '<alice> alpha bravo charlie\n<bob> alpha bravo\n<carol> alpha', 'summary', '0', '1', 'false');
  expect(all).toContain('Participants: 3');
  expect(all).toContain('carol');

  await expect(runWasm(page, '', 'summary')).rejects.toThrow(/chat log is empty/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool chat-log-analyzer');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
