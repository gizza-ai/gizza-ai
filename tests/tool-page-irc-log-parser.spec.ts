import { test, expect } from './fixtures';

const IRSSI = `--- Log opened Fri Jan 05 20:00:00 2024
21:07 <alice> shipping the parser tonight
21:07 -!- bob [~bob@example.net] has joined #gizza
21:08  * alice waves
21:09 <bob> nice, I'll review it
21:10 -!- mode/#gizza [+o bob] by alice
21:11 -!- alice [~a@example.net] has quit [Ping timeout: 240 seconds]`;

const WEECHAT = [
  '2024-01-05 21:07:33\talice\tshipping the parser tonight',
  '2024-01-05 21:07:40\t-->\tbob (~bob@example.net) has joined #gizza',
  '2024-01-05 21:08:30\t *\talice waves',
  '2024-01-05 21:09:10\t<--\tbob (~bob@example.net) has quit (Client Quit)',
].join('\n');

const BRACKET = `[21:07:33] <alice> shipping the parser tonight
[21:07:40] *** Joins: bob (~bob@example.net)
[21:08:30] * carol was kicked by alice (spam)
[21:09:00] * alice sets mode: +m`;

const HEXCHAT = `**** BEGIN LOGGING AT Fri Jan  5 20:00:00 2024
Jan 05 21:07:33 <alice>\thello there
Jan 05 21:07:40 *\tbob has joined #gizza`;

async function outputText(page) {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

async function setLog(page, value: string) {
  await page.locator('#in-log').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v as string;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

/** Put every field into its documented default, with the irssi sample loaded. */
async function fillBase(page) {
  await setLog(page, IRSSI);
  await page.selectOption('#in-format', 'auto');
  await page.selectOption('#in-output', 'timeline');
  await page.fill('#in-date', '');
  await page.selectOption('#in-time_format', 'iso');
  await page.selectOption('#in-include', 'all');
  await page.fill('#in-nicks', '');
  await page.fill('#in-channel', '#gizza');
  await page.check('#in-strip_formatting');
  await page.uncheck('#in-include_raw');
  await page.fill('#in-limit', '0');
}

test('irc-log-parser renders the irssi sample as an exact timeline', async ({ page }) => {
  await page.goto('/tools/irc-log-parser/');
  await fillBase(page);

  await expect(page.locator('#tool-output')).toContainText('shipping the parser tonight', {
    timeout: 15000,
  });
  expect(await outputText(page)).toBe(
    [
      '--- Log opened Fri Jan 05 20:00:00 2024',
      '2024-01-05T21:07:00  <alice> shipping the parser tonight',
      '2024-01-05T21:07:00  --> bob (~bob@example.net) joined #gizza',
      '2024-01-05T21:08:00  * alice waves',
      "2024-01-05T21:09:00  <bob> nice, I'll review it",
      '2024-01-05T21:10:00  --  mode #gizza +o bob by alice',
      '2024-01-05T21:11:00  <-- alice quit (Ping timeout: 240 seconds)',
    ].join('\n'),
  );
});

test('irc-log-parser covers every output choice with real output', async ({ page }) => {
  await page.goto('/tools/irc-log-parser/');
  await fillBase(page);

  for (const [output, expected] of [
    ['timeline', '2024-01-05T21:07:00  <alice> shipping the parser tonight'],
    ['json', '"type": "message"'],
    ['ndjson', '{"line":2,"time":"2024-01-05T21:07:00","type":"message"'],
    ['csv', '3,2024-01-05T21:07:00,join,bob,~bob@example.net,#gizza,,'],
    ['markdown', '| 2024-01-05T21:10:00 | mode | alice | #gizza | mode +o bob |'],
  ]) {
    await page.selectOption('#in-output', output);
    await expect(page.locator('#tool-output')).toContainText(expected, { timeout: 15000 });
  }

  // The JSON record keeps all eight fields in a stable order.
  await page.selectOption('#in-output', 'json');
  await expect(page.locator('#tool-output')).toContainText('"nick": "bob"', { timeout: 15000 });
  const parsed = JSON.parse(await outputText(page));
  expect(parsed[6]).toEqual({
    line: 7,
    time: '2024-01-05T21:11:00',
    type: 'quit',
    nick: 'alice',
    host: '~a@example.net',
    channel: '#gizza',
    arg: '',
    text: 'Ping timeout: 240 seconds',
  });
});

test('irc-log-parser covers every log-format choice with a matching sample', async ({ page }) => {
  await page.goto('/tools/irc-log-parser/');
  await fillBase(page);
  await page.selectOption('#in-time_format', '24h');

  // weechat: tab-separated columns
  await setLog(page, WEECHAT);
  await page.selectOption('#in-format', 'weechat');
  await expect(page.locator('#tool-output')).toContainText(
    '21:09:10  <-- bob quit (Client Quit)',
    { timeout: 15000 },
  );
  expect(await outputText(page)).toContain('21:08:30  * alice waves');

  // auto must reach the same answer without being told
  await page.selectOption('#in-format', 'auto');
  await expect(page.locator('#tool-output')).toContainText('21:09:10  <-- bob quit (Client Quit)', {
    timeout: 15000,
  });

  // bracket: mIRC / ZNC wording
  await setLog(page, BRACKET);
  await page.selectOption('#in-format', 'bracket');
  await expect(page.locator('#tool-output')).toContainText(
    '21:07:40  --> bob (~bob@example.net) joined #gizza',
    { timeout: 15000 },
  );
  const bracketOut = await outputText(page);
  expect(bracketOut).toContain('21:08:30  <-- carol was kicked from #gizza by alice (spam)');
  expect(bracketOut).toContain('21:09:00  --  mode #gizza +m by alice');

  // hexchat: month/day stamp, year borrowed from the banner
  await setLog(page, HEXCHAT);
  await page.selectOption('#in-format', 'hexchat');
  await page.selectOption('#in-time_format', 'iso');
  await expect(page.locator('#tool-output')).toContainText(
    '2024-01-05T21:07:33  <alice> hello there',
    { timeout: 15000 },
  );

  // iso: plain ISO date + time
  await setLog(page, '2024-01-05 21:07:33 <alice> hello there');
  await page.selectOption('#in-format', 'iso');
  await expect(page.locator('#tool-output')).toContainText(
    '2024-01-05T21:07:33  <alice> hello there',
    { timeout: 15000 },
  );

  // irssi: bare HH:MM
  await setLog(page, '21:07 <alice> hello there');
  await page.selectOption('#in-format', 'irssi');
  await page.selectOption('#in-time_format', '12h');
  await expect(page.locator('#tool-output')).toContainText('9:07:00 PM  <alice> hello there', {
    timeout: 15000,
  });

  // plain: no timestamps at all
  await setLog(page, '<alice> hello there');
  await page.selectOption('#in-format', 'plain');
  await page.selectOption('#in-time_format', 'none');
  await expect(page.locator('#tool-output')).toContainText('<alice> hello there', {
    timeout: 15000,
  });
  expect(await outputText(page)).toBe('<alice> hello there');
});

test('irc-log-parser covers timestamp and include choices, nick filter and base date', async ({
  page,
}) => {
  await page.goto('/tools/irc-log-parser/');
  await fillBase(page);

  for (const [tf, expected] of [
    ['iso', '2024-01-05T21:07:00  <alice> shipping the parser tonight'],
    ['24h', '21:07:00  <alice> shipping the parser tonight'],
    ['12h', '9:07:00 PM  <alice> shipping the parser tonight'],
    ['original', '21:07  <alice> shipping the parser tonight'],
    ['none', '<alice> shipping the parser tonight'],
  ]) {
    await page.selectOption('#in-time_format', tf);
    await expect(page.locator('#tool-output')).toContainText(expected, { timeout: 15000 });
  }

  await page.selectOption('#in-time_format', 'none');
  await page.selectOption('#in-include', 'messages');
  await expect(page.locator('#tool-output')).toContainText('<alice> shipping', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    ['<alice> shipping the parser tonight', '* alice waves', "<bob> nice, I'll review it"].join(
      '\n',
    ),
  );

  await page.selectOption('#in-include', 'events');
  await expect(page.locator('#tool-output')).toContainText('--> bob', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    [
      '--> bob (~bob@example.net) joined #gizza',
      '--  mode #gizza +o bob by alice',
      '<-- alice quit (Ping timeout: 240 seconds)',
    ].join('\n'),
  );

  // Prefix glob on the nick filter.
  await page.selectOption('#in-include', 'all');
  await page.fill('#in-nicks', 'bo*');
  await expect(page.locator('#tool-output')).toContainText('--> bob', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    ['--> bob (~bob@example.net) joined #gizza', "<bob> nice, I'll review it"].join('\n'),
  );

  // A base date dates a log that has no day markers at all.
  await page.fill('#in-nicks', '');
  await setLog(page, '21:07 <alice> hello there');
  await page.fill('#in-date', '2024-03-09');
  await page.selectOption('#in-time_format', 'iso');
  await expect(page.locator('#tool-output')).toContainText('2024-03-09T21:07:00  <alice> hello there', {
    timeout: 15000,
  });
});

test('irc-log-parser honours both non-default checkbox states', async ({ page }) => {
  await page.goto('/tools/irc-log-parser/');
  await fillBase(page);
  // \u0003NN is a mIRC colour code, \u0002 is bold.
  await setLog(page, '21:07 <alice> \u000304red\u0003 and \u0002bold\u0002');
  await page.selectOption('#in-time_format', 'none');

  // Default: codes are stripped, so the "04" colour index disappears too.
  await expect(page.locator('#tool-output')).toContainText('<alice> red and bold', {
    timeout: 15000,
  });
  expect(await outputText(page)).not.toContain('04red');

  // NON-default: unchecked, so the control characters survive verbatim.
  await page.uncheck('#in-strip_formatting');
  await expect(page.locator('#tool-output')).toContainText('04red', { timeout: 15000 });
  expect(await outputText(page)).toContain('bold');

  // NON-default: include_raw adds the untouched source line.
  await page.check('#in-strip_formatting');
  await setLog(page, IRSSI);
  await page.selectOption('#in-output', 'csv');
  await page.check('#in-include_raw');
  await expect(page.locator('#tool-output')).toContainText(
    'line,time,type,nick,host,channel,arg,text,raw',
    { timeout: 15000 },
  );
  expect(await outputText(page)).toContain(
    '3,,join,bob,~bob@example.net,#gizza,,,21:07 -!- bob [~bob@example.net] has joined #gizza',
  );
});

test('irc-log-parser deep-links every parameter through the query string', async ({ page }) => {
  const params = new URLSearchParams({
    log: BRACKET,
    format: 'bracket',
    output: 'csv',
    date: '2024-01-05',
    time_format: 'iso',
    include: 'events',
    nicks: '',
    channel: '#gizza',
    strip_formatting: 'true',
    include_raw: 'false',
    limit: '0',
  });

  await page.goto(`/tools/irc-log-parser/?${params.toString()}`);
  await expect(page.locator('#in-format')).toHaveValue('bracket');
  await expect(page.locator('#in-output')).toHaveValue('csv');
  await expect(page.locator('#in-include')).toHaveValue('events');
  await expect(page.locator('#in-date')).toHaveValue('2024-01-05');

  await expect(page.locator('#tool-output')).toContainText('line,time,type,nick', {
    timeout: 15000,
  });
  const text = await outputText(page);
  expect(text).toContain('2,2024-01-05T21:07:40,join,bob,~bob@example.net,#gizza,,');
  expect(text).toContain('3,2024-01-05T21:08:30,kick,carol,,#gizza,alice,spam');
  expect(text).toContain('4,2024-01-05T21:09:00,mode,alice,,#gizza,+m,');
  expect(text).not.toContain('shipping the parser tonight');
});

test('irc-log-parser applies the record limit at its exact cap boundary', async ({ page }) => {
  await page.goto('/tools/irc-log-parser/');
  await fillBase(page);
  await page.selectOption('#in-time_format', 'none');

  await page.fill('#in-limit', '2');
  await expect(page.locator('#tool-output')).toContainText('Log opened', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    ['--- Log opened Fri Jan 05 20:00:00 2024', '<alice> shipping the parser tonight'].join('\n'),
  );

  // At the cap: accepted, every record returned.
  await page.fill('#in-limit', '200000');
  await expect(page.locator('#tool-output')).toContainText('<-- alice quit', { timeout: 15000 });
  expect((await outputText(page)).split('\n')).toHaveLength(7);

  // One over the cap: rejected with the documented message.
  await page.fill('#in-limit', '200001');
  await expect(page.locator('#tool-output')).toContainText(
    'limit must be between 0 (no limit) and 200000, got 200001',
    { timeout: 15000 },
  );
});

test('irc-log-parser reports an unrecognised log instead of returning nothing', async ({ page }) => {
  await page.goto('/tools/irc-log-parser/');
  await fillBase(page);
  await setLog(page, 'the quick brown fox\njumped over the lazy dog');

  await expect(page.locator('#tool-output')).toContainText('no IRC log lines were recognised', {
    timeout: 15000,
  });
});
