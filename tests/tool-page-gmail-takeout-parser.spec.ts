import { test, expect } from './fixtures';

const mbox =
  'From 1521178313854490905@xxx Mon Sep 03 10:00:00 +0000 2018\n' +
  'X-Gmail-Labels: Inbox,Important,Work\n' +
  'From: Alice Example <alice@example.com>\n' +
  'To: Bob <bob@example.org>\n' +
  'Subject: Project kickoff\n' +
  'Date: Mon, 3 Sep 2018 10:00:00 +0000\n' +
  'Message-ID: <k1@example.com>\n' +
  'Content-Type: text/plain; charset=utf-8\n' +
  '\n' +
  "Hi Bob, let's start the project on Monday.\n" +
  'Thanks, Alice\n' +
  'From 1611178313854490906@xxx Tue Sep 04 09:30:00 +0000 2018\n' +
  'X-Gmail-Labels: Sent\n' +
  'From: Bob <bob@example.org>\n' +
  'To: alice@example.com, carol@example.net\n' +
  'Cc: dave@example.com\n' +
  'Subject: Re: Project kickoff\n' +
  'Date: Tue, 4 Sep 2018 09:30:00 +0000\n' +
  'Message-ID: <k2@example.com>\n' +
  'Content-Type: text/plain\n' +
  '\n' +
  'Sounds good.\n';

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('converts a Gmail Takeout mbox to exact CSV with labels', async ({ page }) => {
  await page.goto('/tools/gmail-takeout-parser/');
  await page.fill('#in-input', mbox);
  await expect(page.locator('#tool-output')).toContainText('date,from,to,cc,subject,labels,message_id', { timeout: 15000 });
  expect(await output(page)).toBe(
    'date,from,to,cc,subject,labels,message_id\r\n' +
      '2018-09-03T10:00:00Z,Alice Example <alice@example.com>,Bob <bob@example.org>,,Project kickoff,"Inbox,Important,Work",k1@example.com\r\n' +
      '2018-09-04T09:30:00Z,Bob <bob@example.org>,"alice@example.com, carol@example.net",dave@example.com,Re: Project kickoff,Sent,k2@example.com',
  );
});

test('json format returns message objects without snippets by default', async ({ page }) => {
  await page.goto('/tools/gmail-takeout-parser/');
  await page.fill('#in-input', mbox);
  await page.selectOption('#in-format', 'json');
  await expect(page.locator('#tool-output')).toContainText('"labels": "Inbox,Important,Work"', { timeout: 15000 });
  const parsed = JSON.parse(await output(page));
  expect(parsed).toHaveLength(2);
  expect(parsed[0]).toMatchObject({
    from: 'Alice Example <alice@example.com>',
    to: 'Bob <bob@example.org>',
    subject: 'Project kickoff',
    labels: 'Inbox,Important,Work',
    message_id: 'k1@example.com',
  });
  expect(parsed[0].snippet).toBeUndefined();
});

test('include_body adds a capped snippet column', async ({ page }) => {
  await page.goto('/tools/gmail-takeout-parser/');
  await page.fill('#in-input', mbox);
  await page.check('#in-include_body');
  await page.fill('#in-snippet_chars', '10');
  await expect(page.locator('#tool-output')).toContainText('message_id,snippet', { timeout: 15000 });
  expect(await output(page)).toContain('k1@example.com,"Hi Bob, le"');
});

test('deep-link pre-fills JSON mode and include_body=false', async ({ page }) => {
  const input = encodeURIComponent(mbox);
  await page.goto(`/tools/gmail-takeout-parser/?input=${input}&format=json&include_body=false&snippet_chars=40`);
  await expect(page.locator('#tool-output')).toContainText('"subject": "Project kickoff"', { timeout: 15000 });
  await expect(page.locator('#in-format')).toHaveValue('json');
  await expect(page.locator('#in-include_body')).not.toBeChecked();
  await expect(page.locator('#in-snippet_chars')).toHaveValue('40');
  const parsed = JSON.parse(await output(page));
  expect(parsed[0].snippet).toBeUndefined();
}
);
