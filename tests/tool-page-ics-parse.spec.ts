import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

const SAMPLE_ICS = [
  'BEGIN:VCALENDAR',
  'VERSION:2.0',
  'BEGIN:VEVENT',
  'UID:standup-1@example.com',
  'SUMMARY:Team Standup',
  'DTSTART:20240311T090000Z',
  'DTEND:20240311T091500Z',
  'LOCATION:Room 4',
  'DESCRIPTION:Daily sync',
  'ORGANIZER;CN=Jane Doe:mailto:jane@example.com',
  'ATTENDEE;CN=Bob:mailto:bob@example.com',
  'RRULE:FREQ=WEEKLY;BYDAY=MO,WE,FR;COUNT=12',
  'END:VEVENT',
  'END:VCALENDAR',
].join('\n');

const ALL_DAY_ICS = [
  'BEGIN:VCALENDAR',
  'BEGIN:VEVENT',
  'SUMMARY:Independence Day',
  'DTSTART;VALUE=DATE:20240704',
  'DTEND;VALUE=DATE:20240705',
  'CATEGORIES:Holiday,US',
  'STATUS:CONFIRMED',
  'END:VEVENT',
  'END:VCALENDAR',
].join('\n');

test('ics-parse page — parses VEVENT fields and recurrence', async ({ page }) => {
  await page.goto('/tools/ics-parse/');
  await page.fill('#in-ics', SAMPLE_ICS);
  await page.selectOption('#in-date_format', 'iso');
  await page.uncheck('#in-pretty');
  await page.check('#in-include_description');
  await expect(page.locator('#tool-output')).toContainText('Team Standup', { timeout: 15000 });
  expect(JSON.parse(await outputText(page))).toEqual([
    {
      uid: 'standup-1@example.com',
      summary: 'Team Standup',
      start: '2024-03-11T09:00:00Z',
      end: '2024-03-11T09:15:00Z',
      location: 'Room 4',
      description: 'Daily sync',
      organizer: { name: 'Jane Doe', email: 'jane@example.com' },
      attendees: [{ name: 'Bob', email: 'bob@example.com' }],
      recurrence: { freq: 'WEEKLY', byday: ['MO', 'WE', 'FR'], count: 12 },
    },
  ]);
});

test('ics-parse page — unix date format and description checkbox off', async ({ page }) => {
  await page.goto('/tools/ics-parse/');
  await page.fill('#in-ics', SAMPLE_ICS);
  await page.selectOption('#in-date_format', 'unix');
  await page.uncheck('#in-pretty');
  await page.uncheck('#in-include_description');
  await expect(page.locator('#tool-output')).toContainText('1710147600', { timeout: 15000 });
  const event = JSON.parse(await outputText(page))[0];
  expect(event.start).toBe(1710147600);
  expect(event.end).toBe(1710148500);
  expect(event.description).toBeUndefined();
});

test('ics-parse page — query-param deep-link prefills and auto-runs', async ({ page }) => {
  await page.goto(
    '/tools/ics-parse/?ics=' +
      encodeURIComponent(ALL_DAY_ICS) +
      '&date_format=iso&pretty=false&include_description=true',
  );
  await expect(page.locator('#in-ics')).toHaveValue(ALL_DAY_ICS, { timeout: 15000 });
  await expect(page.locator('#in-date_format')).toHaveValue('iso');
  await expect(page.locator('#tool-output')).toContainText('Independence Day', { timeout: 15000 });
  expect(JSON.parse(await outputText(page))).toEqual([
    {
      summary: 'Independence Day',
      start: '2024-07-04',
      end: '2024-07-05',
      all_day: true,
      status: 'CONFIRMED',
      categories: ['Holiday', 'US'],
    },
  ]);
});
