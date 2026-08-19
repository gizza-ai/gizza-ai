import { test, expect } from './fixtures';

const SAMPLE = `{"type":"version","version":1}
{"type":"team","team":{"name":"core","display_name":"Core Team","type":"O"}}
{"type":"channel","channel":{"team":"core","name":"town-square","display_name":"Town Square","type":"O"}}
{"type":"channel","channel":{"team":"core","name":"release","display_name":"Release Room","type":"P"}}
{"type":"user","user":{"username":"alice","first_name":"Alice","last_name":"Anderson"}}
{"type":"user","user":{"username":"bob","nickname":"Bobby"}}
{"type":"post","post":{"team":"core","channel":"town-square","user":"alice","message":"Standup in five","create_at":1705311000000,"reactions":[{"user":"bob","emoji_name":"thumbsup"}],"replies":[{"user":"bob","message":"On my way","create_at":1705311120000}]}}
{"type":"post","post":{"team":"core","channel":"release","user":"bob","message":"Cutting 9.4 today","create_at":1705397400000,"attachments":[{"path":"files/checklist.pdf"}]}}
{"type":"direct_channel","direct_channel":{"members":["alice","bob"]}}
{"type":"direct_post","direct_post":{"channel_members":["alice","bob"],"user":"bob","message":"thanks for the review","create_at":1705400000000}}`;

async function runWasm(
  page: any,
  exportText: string = SAMPLE,
  output = 'both',
  format = 'text',
  channel = '',
  userFilter = '',
  since = '',
  until = '',
  includeDirectMessages = 'true',
  includeReplies = 'true',
  maxMessages = '0',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/mattermost-export-reader/gizza_ai_mattermost_export_reader_web.js');
    await mod.default('/tools/mattermost-export-reader/gizza_ai_mattermost_export_reader_web_bg.wasm');
    return mod.run(
      args.exportText,
      args.output,
      args.format,
      args.channel,
      args.userFilter,
      args.since,
      args.until,
      args.includeDirectMessages,
      args.includeReplies,
      args.maxMessages,
    );
  }, { exportText, output, format, channel, userFilter, since, until, includeDirectMessages, includeReplies, maxMessages });
}

test('mattermost-export-reader page renders a real transcript and summary', async ({ page }) => {
  await page.goto('/tools/mattermost-export-reader/');
  await page.fill('#in-export', SAMPLE);

  const output = page.locator('#tool-output');
  await expect(output).toContainText('Bulk export format version: 1', { timeout: 20_000 });
  await expect(output).toContainText('Messages in export: 4 (2 channel posts, 1 direct messages, 1 thread replies)');
  await expect(output).toContainText('--- #town-square (Town Square) ---');
  await expect(output).toContainText('[2024-01-15 09:30:00 UTC] Alice Anderson: Standup in five [reactions: :thumbsup:]');
  await expect(output).toContainText('↳ [2024-01-15 09:32:00 UTC] Bobby: On my way');
  await expect(output).toContainText('--- Direct message: Alice Anderson, Bobby ---');
});

test('mattermost-export-reader deep link covers CSV, date filter, channel filter and checkbox state', async ({ page }) => {
  const params = new URLSearchParams({
    export: SAMPLE,
    output: 'transcript',
    format: 'csv',
    channel: 'release',
    since: '2024-01-16',
    until: '2024-01-16',
    include_direct_messages: 'false',
    include_replies: 'false',
    max_messages: '5',
  });
  await page.goto(`/tools/mattermost-export-reader/?${params.toString()}`);

  await expect(page.locator('#in-format')).toHaveValue('csv', { timeout: 15_000 });
  await expect(page.locator('#in-channel')).toHaveValue('release');
  await expect(page.locator('#in-include_direct_messages')).not.toBeChecked();
  const output = page.locator('#tool-output');
  await expect(output).toContainText('timestamp,channel,author,username,kind,message', { timeout: 20_000 });
  await expect(output).toContainText('2024-01-16T09:30:00Z,#release (Release Room) [private],Bobby,bob,post,Cutting 9.4 today [attachment: files/checklist.pdf]');
  await expect(output).not.toContainText('Standup in five');
});

test('mattermost-export-reader wasm covers enum values, cap boundary and CLI example', async ({ page }) => {
  await page.goto('/tools/mattermost-export-reader/');

  const markdown = await runWasm(page, SAMPLE, 'transcript', 'markdown', 'town-square');
  expect(markdown).toContain('### #town-square (Town Square)');
  expect(markdown).toContain('- **Alice Anderson** (2024-01-15 09:30:00 UTC): Standup in five [reactions: :thumbsup:]');
  expect(markdown).not.toContain('thanks for the review');

  const html = await runWasm(page, SAMPLE, 'transcript', 'html', '', 'bob', '', '', 'true', 'true', '0');
  expect(html).toContain('<h3>#town-square (Town Square)</h3>');
  expect(html).toContain('<strong>Bobby</strong>: On my way');
  expect(html).not.toContain('Alice Anderson</strong>: Standup');

  const statsCsv = await runWasm(page, SAMPLE, 'stats', 'csv', '', '', '', '', 'true', 'true', '2');
  expect(statsCsv).toContain('summary,Truncated to,2 messages');
  expect(statsCsv).toContain('channel,#town-square (Town Square),2 (50.00%)');

  const textNoReplies = await runWasm(page, SAMPLE, 'transcript', 'text', '', '', '', '', 'false', 'false', '0');
  expect(textNoReplies).toContain('Standup in five');
  expect(textNoReplies).not.toContain('On my way');
  expect(textNoReplies).not.toContain('thanks for the review');

  await expect(runWasm(page, 'not json', 'both')).rejects.toThrow(/line 1 is not valid JSON/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool mattermost-export-reader');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
