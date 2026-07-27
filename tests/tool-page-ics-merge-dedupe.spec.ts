import { test, expect } from './fixtures';

const DUPLICATE_UID = `BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:evt-1@example.com
DTSTART:20240309T081530Z
SUMMARY:Team Standup
END:VEVENT
END:VCALENDAR
BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:evt-1@example.com
DTSTART:20240309T081530Z
SUMMARY:Team Standup
END:VEVENT
END:VCALENDAR`;

const EXPECTED_UID_OUTPUT = [
  'BEGIN:VCALENDAR',
  'VERSION:2.0',
  'PRODID:-//gizza-ai//ics-merge-dedupe//EN',
  'CALSCALE:GREGORIAN',
  'BEGIN:VEVENT',
  'UID:evt-1@example.com',
  'DTSTART:20240309T081530Z',
  'SUMMARY:Team Standup',
  'END:VEVENT',
  'END:VCALENDAR',
].join('\r\n');

test('ics-merge-dedupe page merges two calendars and drops a duplicate UID', async ({ page }) => {
  await page.goto('/tools/ics-merge-dedupe/');
  await page.fill('#in-ics', DUPLICATE_UID);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('UID:evt-1@example.com', { timeout: 15_000 });
  expect(await out.textContent()).toBe(EXPECTED_UID_OUTPUT);
});

test('ics-merge-dedupe deep link supports start-title matching and unsorted output', async ({ page }) => {
  const sample = `BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:google-abc
DTSTART:20240704
SUMMARY:Independence Day
END:VEVENT
BEGIN:VEVENT
UID:apple-xyz
DTSTART:20240704
SUMMARY:Independence  Day
END:VEVENT
END:VCALENDAR`;
  const params = new URLSearchParams({
    ics: sample,
    dedupe_by: 'start_title',
    keep: 'first',
    sort: 'false',
    calendar_name: 'Merged Holidays',
  });

  await page.goto(`/tools/ics-merge-dedupe/?${params.toString()}`);
  await expect(page.locator('#in-dedupe_by')).toHaveValue('start_title', { timeout: 15_000 });
  await expect(page.locator('#in-sort')).not.toBeChecked();
  await expect(page.locator('#in-calendar_name')).toHaveValue('Merged Holidays');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('X-WR-CALNAME:Merged Holidays', { timeout: 15_000 });
  expect((await out.textContent())?.match(/BEGIN:VEVENT/g)?.length).toBe(1);
  await expect(out).toContainText('UID:google-abc');
  await expect(out).not.toContainText('UID:apple-xyz');
});
