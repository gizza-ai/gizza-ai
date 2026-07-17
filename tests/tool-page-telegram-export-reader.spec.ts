import { test, expect } from './fixtures';

const SAMPLE = JSON.stringify({
  name: 'Weekend Trip',
  type: 'private_group',
  messages: [
    { id: 1, type: 'service', date: '2021-03-27T14:44:24', actor: 'Alice', action: 'create_group', title: 'Weekend Trip' },
    { id: 2, type: 'message', date: '2021-03-27T14:45:00', from: 'Alice', text: 'Hey everyone ready for the trip' },
    { id: 3, type: 'message', date: '2021-03-28T09:46:10', from: 'Bob', text: ['Yes ', { type: 'bold', text: 'so' }, ' excited 🎉'] },
    { id: 4, type: 'message', date: '2021-03-28T09:47:00', from: 'Bob', text: '', photo: 'photos/photo_1.jpg' },
  ],
});

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('telegram-export-reader renders exact stats and transcript from result.json', async ({ page }) => {
  await page.goto('/tools/telegram-export-reader/');
  await page.fill('#in-export', SAMPLE);
  await page.selectOption('#in-output', 'both');
  await expect(page.locator('#tool-output')).toContainText('Messages per sender', { timeout: 15_000 });
  const out = await output(page);
  expect(out).toContain('Chat: Weekend Trip (private group)');
  expect(out).toContain('Messages: 3');
  expect(out).toContain('Participants: 2');
  expect(out).toContain('Words: 9');
  expect(out).toContain('Media messages: 1');
  expect(out).toContain('      2   66.67%  Bob    (3 words)');
  expect(out).toContain('      1   33.33%  Alice  (6 words)');
  expect(out).toContain('      1  🎉');
  expect(out).toContain('[2021-03-27 14:45:00] Alice: Hey everyone ready for the trip');
  expect(out).toContain('[2021-03-28 09:46:10] Bob: Yes so excited 🎉');
  expect(out).toContain('[2021-03-28 09:47:00] Bob: [photo]');
  expect(out).not.toContain('created the group');
});

test('telegram-export-reader query params filter sender and include service messages', async ({ page }) => {
  await page.goto(
    '/tools/telegram-export-reader/?export=' +
      encodeURIComponent(SAMPLE) +
      '&output=transcript&sender_filter=Alice&include_service_messages=true&max_messages=2'
  );
  await expect(page.locator('#in-export')).toHaveValue(SAMPLE, { timeout: 15_000 });
  await expect(page.locator('#in-output')).toHaveValue('transcript');
  await expect(page.locator('#in-sender_filter')).toHaveValue('Alice');
  await expect(page.locator('#in-include_service_messages')).toBeChecked();
  await expect(page.locator('#in-max_messages')).toHaveValue('2');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('* Alice created the group "Weekend Trip"', { timeout: 15_000 });
  await expect(out).toContainText('[2021-03-27 14:45:00] Alice: Hey everyone ready for the trip');
  await expect(out).not.toContainText('Bob:');
});
