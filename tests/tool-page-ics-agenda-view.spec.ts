import { test, expect } from './fixtures';

const ICS = `BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:a@example
DTSTART:20260309T090000Z
DTEND:20260309T093000Z
SUMMARY:Standup
LOCATION:Room 2
END:VEVENT
BEGIN:VEVENT
UID:b@example
DTSTART:20260309T110000Z
DTEND:20260309T120000Z
SUMMARY:Design review
END:VEVENT
END:VCALENDAR
`;

const RECURRING = `BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:r@example
DTSTART:20260309T090000Z
DTEND:20260309T093000Z
SUMMARY:Standup
RRULE:FREQ=WEEKLY;BYDAY=MO,WE
END:VEVENT
END:VCALENDAR
`;

async function outputText(page) {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('ics-agenda-view page renders exact agenda with free gaps', async ({ page }) => {
  await page.goto('/tools/ics-agenda-view/');
  await page.fill('#in-ics', ICS);
  await page.fill('#in-start_date', '2026-03-09');
  await page.fill('#in-days', '1');
  await page.fill('#in-timezone', 'UTC');
  await page.fill('#in-day_start', '09:00');
  await page.fill('#in-day_end', '18:00');
  await page.fill('#in-min_gap_minutes', '30');
  await page.selectOption('#in-details', 'normal');
  await page.selectOption('#in-output', 'text');

  await expect(page.locator('#tool-output')).toContainText('Agenda 2026-03-09', {
    timeout: 20_000,
  });
  expect(await outputText(page)).toBe(
    [
      'Agenda 2026-03-09 to 2026-03-09 · UTC',
      'Free gaps 09:00-18:00, at least 30m',
      '',
      'Mon 2026-03-09',
      '  09:00-09:30   Standup · Room 2',
      '    free 1h 30m (09:30-11:00)',
      '  11:00-12:00   Design review',
      '    free 6h (12:00-18:00)',
      '',
      'Totals: 2 events · 1h 30m booked · 2 free gaps · 7h 30m free',
    ].join('\n'),
  );
});

test('ics-agenda-view page honors deep-linked params and JSON output', async ({ page }) => {
  const ics = encodeURIComponent(RECURRING);
  await page.goto(
    `/tools/ics-agenda-view/?ics=${ics}&start_date=2026-03-09&days=7&timezone=Europe%2FBerlin&day_start=09%3A00&day_end=17%3A00&min_gap_minutes=120&show_gaps=false&filter=standup&expand_recurring=true&include_cancelled=false&details=compact&output=json`,
  );

  await expect(page.locator('#in-start_date')).toHaveValue('2026-03-09');
  await expect(page.locator('#in-days')).toHaveValue('7');
  await expect(page.locator('#in-timezone')).toHaveValue('Europe/Berlin');
  await expect(page.locator('#in-show_gaps')).not.toBeChecked();
  await expect(page.locator('#in-filter')).toHaveValue('standup');
  await expect(page.locator('#in-expand_recurring')).toBeChecked();
  await expect(page.locator('#in-include_cancelled')).not.toBeChecked();
  await expect(page.locator('#in-details')).toHaveValue('compact');
  await expect(page.locator('#in-output')).toHaveValue('json');

  await expect(page.locator('#tool-output')).toContainText('"timezone": "Europe/Berlin"', {
    timeout: 20_000,
  });
  const linked = JSON.parse(await outputText(page));
  expect(linked.timezone).toBe('Europe/Berlin');
  expect(linked.range.days).toBe(7);
  expect(linked.totals.events).toBe(2);
  expect(linked.days.some((day: { date: string }) => day.date === '2026-03-11')).toBe(true);
});

test('ics-agenda-view page covers markdown, cancelled events and non-default checkbox states', async ({
  page,
}) => {
  const cancelled = `BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:c@example
DTSTART:20260309T140000Z
DTEND:20260309T150000Z
SUMMARY:Cancelled sync
STATUS:CANCELLED
END:VEVENT
END:VCALENDAR
`;

  await page.goto('/tools/ics-agenda-view/');
  await page.fill('#in-ics', cancelled);
  await page.fill('#in-start_date', '2026-03-09');
  await page.fill('#in-days', '1');
  await page.uncheck('#in-show_gaps');
  await page.uncheck('#in-expand_recurring');
  await page.check('#in-include_cancelled');
  await page.selectOption('#in-details', 'full');
  await page.selectOption('#in-output', 'markdown');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('## Monday, 2026-03-09', { timeout: 20_000 });
  await expect(out).toContainText('Cancelled sync');
  await expect(out).toContainText('cancelled');
});
