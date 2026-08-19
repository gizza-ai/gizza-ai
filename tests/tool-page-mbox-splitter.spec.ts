import { test, expect } from './fixtures';

const TWO = `From alice@example.com Mon Sep 03 10:00:00 2018
From: Alice <alice@example.com>
Subject: Quarterly report
Message-ID: <a1@example.com>
Date: Mon, 3 Sep 2018 10:00:00 +0000

first body

From bob@example.com Mon Sep 03 11:30:00 2018
From: Bob <bob@example.com>
Subject: Lunch?
Message-ID: <b2@example.com>
Date: Mon, 3 Sep 2018 11:30:00 +0000

second body
`;

async function runWasm(
  page: any,
  mbox: string,
  output = 'files',
  naming = 'index',
  message = '0',
  unescapeFrom = 'true',
  keepPostmark = 'false',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/mbox-splitter/gizza_ai_mbox_splitter_web.js');
    await mod.default('/tools/mbox-splitter/gizza_ai_mbox_splitter_web_bg.wasm');
    return mod.run(
      args.mbox,
      args.output,
      args.naming,
      args.message,
      args.unescapeFrom,
      args.keepPostmark,
    );
  }, { mbox, output, naming, message, unescapeFrom, keepPostmark });
}

test('mbox-splitter page lists messages from a pasted archive', async ({ page }) => {
  await page.goto('/tools/mbox-splitter/');
  await page.fill('#in-mbox', TWO);
  await page.selectOption('#in-output', 'list');
  await page.selectOption('#in-naming', 'subject');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('2 message(s)', { timeout: 20_000 });
  await expect(output).toContainText('001-quarterly-report.eml');
  await expect(output).toContainText('002-lunch.eml');
  await expect(output).toContainText('alice@example.com');
});

test('mbox-splitter deep link pulls out one raw eml', async ({ page }) => {
  const params = new URLSearchParams({
    mbox: TWO,
    output: 'eml',
    naming: 'index',
    message: '2',
  });
  await page.goto(`/tools/mbox-splitter/?${params.toString()}`);

  await expect(page.locator('#in-output')).toHaveValue('eml', { timeout: 15_000 });
  await expect(page.locator('#in-message')).toHaveValue('2');
  const output = page.locator('#tool-output');
  await expect(output).toContainText('From: Bob <bob@example.com>', { timeout: 20_000 });
  await expect(output).toContainText('second body');
  await expect(output).not.toContainText('first body');
});

test('mbox-splitter wasm covers enums, booleans, boundary and CLI example', async ({ page }) => {
  await page.goto('/tools/mbox-splitter/');

  const files = await runWasm(page, TWO, 'files', 'index');
  expect(files).toContain('===== 001.eml (');
  expect(files).toContain('first body');
  expect(files).toContain('===== 002.eml (');

  const json = await runWasm(page, TWO, 'json', 'message-id');
  expect(json).toContain('001-a1-example-com.eml');
  expect(json).toContain('002-b2-example-com.eml');

  const byDate = await runWasm(page, TWO, 'list', 'date');
  expect(byDate).toContain('001-2018-09-03-1000.eml');
  expect(byDate).toContain('002-2018-09-03-1130.eml');

  const one = await runWasm(page, TWO, 'eml', 'index', '1');
  expect(one).toContain('From: Alice <alice@example.com>');
  expect(one).not.toContain('From alice@example.com Mon Sep 03 10:00:00 2018');

  const kept = await runWasm(page, TWO, 'eml', 'index', '1', 'true', 'true');
  expect(kept).toContain('From alice@example.com Mon Sep 03 10:00:00 2018');

  const quoted = `From x@example.com Mon Sep 03 10:00:00 2018
Subject: q

>From the desk
`;
  await expect(runWasm(page, quoted, 'eml', 'index', '1', 'false')).resolves.toContain('>From the desk');
  await expect(runWasm(page, TWO, 'eml', 'index', '3')).rejects.toThrow(/does not exist/);
  await expect(runWasm(page, '', 'files')).rejects.toThrow(/input is empty/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool mbox-splitter');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
