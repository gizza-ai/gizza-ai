import { test, expect } from './fixtures';

const THREE_MESSAGE_MBOX = [
  'From a@example.com Mon Sep 03 10:00:00 2018',
  'From: Alice <alice@example.com>',
  'Subject: Hello',
  'Message-ID: <same@example.com>',
  '',
  'first copy',
  '',
  'From b@example.com Mon Sep 03 11:00:00 2018',
  'From: Bob <bob@example.com>',
  'Subject: Other',
  'Message-ID: <other@example.com>',
  '',
  'unique message',
  '',
  'From c@example.com Mon Sep 03 12:00:00 2018',
  'From: Alice <alice@example.com>',
  'Subject: Hello again',
  'Message-ID: <same@example.com>',
  '',
  'duplicate copy',
].join('\n');

async function outputText(page) {
  return (await page.locator('#tool-output').textContent({ timeout: 15000 })) ?? '';
}

test('mbox-dedup keeps the first copy by default', async ({ page }) => {
  await page.goto('/tools/mbox-dedup/');
  await page.fill('#in-mbox', THREE_MESSAGE_MBOX);
  const out = await outputText(page);
  expect(out).toContain('first copy');
  expect(out).toContain('unique message');
  expect(out).not.toContain('duplicate copy');
  expect((out.match(/^From /gm) ?? []).length).toBe(2);
});

test('mbox-dedup can keep the last duplicate occurrence', async ({ page }) => {
  await page.goto('/tools/mbox-dedup/');
  await page.fill('#in-mbox', THREE_MESSAGE_MBOX);
  await page.selectOption('#in-keep', 'last');
  const out = await outputText(page);
  expect(out).not.toContain('first copy');
  expect(out).toContain('unique message');
  expect(out).toContain('duplicate copy');
  expect(out.indexOf('unique message')).toBeLessThan(out.indexOf('duplicate copy'));
});

test('mbox-dedup ignore_case collapses Message-ID case changes', async ({ page }) => {
  await page.goto('/tools/mbox-dedup/');
  await page.fill(
    '#in-mbox',
    'From a\nMessage-ID: <A@H>\n\none\n\nFrom b\nMessage-ID: <a@h>\n\ntwo',
  );
  let out = await outputText(page);
  expect(out).toContain('one');
  expect(out).toContain('two');
  await page.check('#in-ignore_case');
  out = await outputText(page);
  expect(out).toContain('one');
  expect(out).not.toContain('two');
});

test('mbox-dedup can drop messages without Message-ID', async ({ page }) => {
  await page.goto('/tools/mbox-dedup/');
  await page.fill(
    '#in-mbox',
    'From draft\nSubject: Draft\n\ndraft body\n\nFrom sent\nMessage-ID: <sent@example.com>\n\nsent body',
  );
  await page.selectOption('#in-no_message_id', 'drop');
  const out = await outputText(page);
  expect(out).not.toContain('draft body');
  expect(out).toContain('sent body');
});

test('mbox-dedup honours a query-param deep link', async ({ page }) => {
  const mbox = encodeURIComponent('From a\nMessage-ID: <same@example.com>\n\nold\n\nFrom b\nMessage-ID: <same@example.com>\n\nnew');
  await page.goto(`/tools/mbox-dedup/?mbox=${mbox}&keep=last&ignore_case=false&no_message_id=keep`);
  await expect(page.locator('#in-keep')).toHaveValue('last');
  const out = await outputText(page);
  expect(out).not.toContain('old');
  expect(out).toContain('new');
});
