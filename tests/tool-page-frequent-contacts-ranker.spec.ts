import { test, expect } from './fixtures';

const MBOX = `From 1@x Mon Sep 03 10:00:00 +0000 2018
From: Alice Example <alice@example.com>
To: Bob <bob@example.org>, Carol <carol@example.net>
Date: Mon, 3 Sep 2018 10:00:00 +0000

Hi both.

From 2@x Tue Sep 04 09:30:00 +0000 2018
From: Bob <bob@example.org>
To: alice@example.com
Cc: Dave <dave@example.com>
Date: Tue, 4 Sep 2018 09:30:00 +0000

Sounds good.

From 3@x Wed Sep 05 08:00:00 +0000 2018
From: Alice Example <alice@example.com>
To: Bob <bob@example.org>
Date: Wed, 5 Sep 2018 08:00:00 +0000

Slides attached.
`;

async function runWasm(
  page: any,
  mbox: string,
  count = 'both',
  includeCc = 'true',
  exclude = 'alice@example.com',
  skipAutomated = 'true',
  halfLifeDays = '180',
  minMessages = '1',
  limit = '25',
  sort = 'score',
  format = 'report',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/frequent-contacts-ranker/gizza_ai_frequent_contacts_ranker_web.js');
    await mod.default('/tools/frequent-contacts-ranker/gizza_ai_frequent_contacts_ranker_web_bg.wasm');
    return mod.run(
      args.mbox,
      args.count,
      args.includeCc,
      args.exclude,
      args.skipAutomated,
      args.halfLifeDays,
      args.minMessages,
      args.limit,
      args.sort,
      args.format,
    );
  }, { mbox, count, includeCc, exclude, skipAutomated, halfLifeDays, minMessages, limit, sort, format });
}

test('frequent-contacts-ranker page ranks contacts from an mbox', async ({ page }) => {
  await page.goto('/tools/frequent-contacts-ranker/');
  await page.fill('#in-mbox', MBOX);
  await page.fill('#in-exclude', 'alice@example.com');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('Top 3 of 3 contacts · 3 messages', { timeout: 20_000 });
  await expect(output).toContainText('Bob <bob@example.org>');
  await expect(output).toContainText('Dave <dave@example.com>');
  await expect(output).toContainText('Carol <carol@example.net>');
});

test('frequent-contacts-ranker deep link produces a paste-ready recipient list', async ({ page }) => {
  const params = new URLSearchParams({
    mbox: MBOX,
    count: 'recipients',
    exclude: 'alice@example.com',
    format: 'list',
    include_cc: 'true',
  });
  await page.goto(`/tools/frequent-contacts-ranker/?${params.toString()}`);

  await expect(page.locator('#in-count')).toHaveValue('recipients', { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('list');
  const output = page.locator('#tool-output');
  await expect(output).toContainText('Bob <bob@example.org>', { timeout: 20_000 });
  await expect(output).toContainText('Carol <carol@example.net>');
  await expect(output).toContainText('Dave <dave@example.com>');
  await expect(output).not.toContainText('alice@example.com');
});

test('frequent-contacts-ranker wasm covers enum choices, booleans, caps and CLI example', async ({ page }) => {
  await page.goto('/tools/frequent-contacts-ranker/');

  const report = await runWasm(page, MBOX);
  expect(report).toContain('Bob <bob@example.org>');
  expect(report).toContain('score');

  const senders = await runWasm(page, MBOX, 'senders', 'true', 'alice@example.com', 'true', '180', '1', '25', 'messages', 'csv');
  expect(senders).toContain('rank,name,email,messages,to,from,first_seen,last_seen,score');
  expect(senders).toContain('Bob,bob@example.org');
  expect(senders).not.toContain('Carol');

  const noCc = await runWasm(page, MBOX, 'recipients', 'false', 'alice@example.com', 'true', '0', '1', '0', 'name', 'list');
  expect(noCc).toContain('Bob <bob@example.org>');
  expect(noCc).toContain('Carol <carol@example.net>');
  expect(noCc).not.toContain('Dave');

  const newsletters = `From 1@x Mon Sep 03 10:00:00 +0000 2018
From: Deals <no-reply@shop.example.io>
To: alice@example.com
Date: Mon, 3 Sep 2018 10:00:00 +0000

Deals.
`;
  await expect(runWasm(page, newsletters, 'senders', 'false', 'alice@example.com', 'true')).rejects.toThrow(/no contacts left/);
  await expect(runWasm(page, newsletters, 'senders', 'false', 'alice@example.com', 'false')).resolves.toContain('no-reply@shop.example.io');

  await expect(runWasm(page, MBOX, 'both', 'true', 'alice@example.com', 'true', '180', '4')).rejects.toThrow(/no contacts left/);
  await expect(runWasm(page, '', 'both')).rejects.toThrow(/input is empty/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool frequent-contacts-ranker');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
