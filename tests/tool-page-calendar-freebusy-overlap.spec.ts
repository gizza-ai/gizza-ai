import { test, expect } from './fixtures';

// Small deterministic fixtures — Mon 2026-07-20 is a fixed, known day.
const CAL_A = [
  'BEGIN:VCALENDAR',
  'VERSION:2.0',
  'BEGIN:VEVENT',
  'UID:standup@a',
  'DTSTART:20260720T090000Z',
  'DTEND:20260720T100000Z',
  'SUMMARY:Standup',
  'END:VEVENT',
  'END:VCALENDAR',
].join('\n');

const CAL_B = [
  'BEGIN:VCALENDAR',
  'VERSION:2.0',
  'BEGIN:VEVENT',
  'UID:lunch@b',
  'DTSTART:20260720T120000Z',
  'DTEND:20260720T130000Z',
  'SUMMARY:Lunch sync',
  'END:VEVENT',
  'END:VCALENDAR',
].join('\n');

// Secondary input format: a VFREEBUSY availability-only export (no VEVENTs).
const CAL_FREEBUSY = [
  'BEGIN:VCALENDAR',
  'VERSION:2.0',
  'BEGIN:VFREEBUSY',
  'FREEBUSY;FBTYPE=BUSY:20260720T090000Z/20260720T120000Z',
  'END:VFREEBUSY',
  'END:VCALENDAR',
].join('\n');

// A TZID (non-UTC) event: 09:00–12:00 New York = 15:00–18:00 Berlin in July.
const CAL_NY = [
  'BEGIN:VCALENDAR',
  'VERSION:2.0',
  'BEGIN:VEVENT',
  'UID:ny@a',
  'DTSTART;TZID=America/New_York:20260720T090000',
  'DTEND;TZID=America/New_York:20260720T120000',
  'END:VEVENT',
  'END:VCALENDAR',
].join('\n');

async function fillCommon(page, calA: string, calB: string) {
  await page.fill('#in-calendar_a', calA);
  await page.fill('#in-calendar_b', calB);
  await page.fill('#in-start_date', '2026-07-20');
  await page.fill('#in-days', '1');
  await page.fill('#in-timezone', 'UTC');
}

test('calendar-freebusy-overlap page finds the exact common free slots', async ({ page }) => {
  await page.goto('/tools/calendar-freebusy-overlap/');
  await fillCommon(page, CAL_A, CAL_B);
  // day_start/day_end stay at their 09:00/17:00 defaults; output stays "text".
  const out = page.locator('#tool-output');
  await expect(out).toContainText('2 free slots', { timeout: 15000 });
  // Multi-line exact assertion (toHaveText normalizes whitespace — compare raw).
  const text = await out.textContent();
  expect(text).toBe(
    'Common free time — 2026-07-20 to 2026-07-20 · Mon–Fri 09:00–17:00 · UTC · ≥30m\n' +
      'Calendar A: 1 busy interval · Calendar B: 1 busy interval\n' +
      '\n' +
      'Mon 2026-07-20  10:00–12:00  (2h)\n' +
      'Mon 2026-07-20  13:00–17:00  (4h)\n' +
      '\n' +
      '2 free slots · 6h total\n',
  );
});

test('calendar-freebusy-overlap deep-link prefills and runs', async ({ page }) => {
  const qs = new URLSearchParams({
    calendar_a: CAL_A,
    calendar_b: CAL_B,
    start_date: '2026-07-20',
    days: '1',
    timezone: 'UTC',
    min_minutes: '30',
  });
  await page.goto('/tools/calendar-freebusy-overlap/?' + qs.toString());
  await expect(page.locator('#in-start_date')).toHaveValue('2026-07-20', { timeout: 15000 });
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Mon 2026-07-20  10:00–12:00  (2h)', { timeout: 15000 });
  await expect(out).toContainText('2 free slots · 6h total');
});

test('calendar-freebusy-overlap accepts VFREEBUSY input and emits JSON', async ({ page }) => {
  await page.goto('/tools/calendar-freebusy-overlap/');
  // A is an availability-only VFREEBUSY export busy 09:00–12:00; B busy 12–13.
  await fillCommon(page, CAL_FREEBUSY, CAL_B);
  await page.selectOption('#in-output', 'json');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"total_minutes": 240', { timeout: 15000 });
  await expect(out).toContainText('"start": "13:00"');
  await expect(out).toContainText('"end": "17:00"');
  await expect(out).toContainText('"start_iso": "2026-07-20T13:00:00+00:00"');
  await expect(out).toContainText('"calendar_a": 1');
});

test('calendar-freebusy-overlap emits an RFC 5545 VFREEBUSY (.ics) result', async ({ page }) => {
  await page.goto('/tools/calendar-freebusy-overlap/');
  await fillCommon(page, CAL_A, CAL_B);
  await page.selectOption('#in-output', 'ics');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('BEGIN:VCALENDAR', { timeout: 15000 });
  await expect(out).toContainText('FREEBUSY;FBTYPE=FREE:20260720T100000Z/20260720T120000Z');
  await expect(out).toContainText('FREEBUSY;FBTYPE=FREE:20260720T130000Z/20260720T170000Z');
  await expect(out).toContainText('END:VFREEBUSY');
});

test('calendar-freebusy-overlap weekends checkbox: off errors on a Saturday, on finds slots', async ({
  page,
}) => {
  await page.goto('/tools/calendar-freebusy-overlap/');
  await page.fill('#in-calendar_a', CAL_A);
  await page.fill('#in-calendar_b', CAL_B);
  await page.fill('#in-start_date', '2026-07-25'); // a Saturday
  await page.fill('#in-days', '1');
  await page.fill('#in-timezone', 'UTC');
  const out = page.locator('#tool-output');
  // Default (unchecked) → Saturday-only range has no working days.
  await expect(out).toContainText('no working days', { timeout: 15000 });
  // Non-default checkbox state → the Saturday is scanned (events are on Monday).
  await page.check('#in-weekends');
  await expect(out).toContainText('Sat 2026-07-25  09:00–17:00  (8h)', { timeout: 15000 });
  await expect(out).toContainText('Mon–Sun');
});

test('calendar-freebusy-overlap days cap: 60 works, 61 errors', async ({ page }) => {
  await page.goto('/tools/calendar-freebusy-overlap/');
  await fillCommon(page, CAL_A, CAL_B);
  await page.fill('#in-days', '60');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('2026-07-20 to 2026-09-17', { timeout: 15000 });
  await page.fill('#in-days', '61');
  await expect(out).toContainText('days must be between 1 and 60', { timeout: 15000 });
});

test('calendar-freebusy-overlap example chip prefills and runs to the worked-example output', async ({
  page,
}) => {
  await page.goto('/tools/calendar-freebusy-overlap/');
  await page.click('button.tool-example-chip[data-example="0"]');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('2 free slots · 6h total', { timeout: 15000 });
  await expect(out).toContainText('Mon 2026-07-20  10:00–12:00  (2h)');
  await expect(page.locator('#in-timezone')).toHaveValue('UTC');
});

test('calendar-freebusy-overlap converts TZID events into the selected timezone', async ({
  page,
}) => {
  await page.goto('/tools/calendar-freebusy-overlap/');
  // A: 09:00–12:00 America/New_York (= 15:00–18:00 Berlin). B: 12:00–13:00 UTC
  // (= 14:00–15:00 Berlin). Berlin working day 09:00–17:00 → free 09:00–14:00.
  await page.fill('#in-calendar_a', CAL_NY);
  await page.fill('#in-calendar_b', CAL_B);
  await page.fill('#in-start_date', '2026-07-20');
  await page.fill('#in-days', '1');
  await page.fill('#in-timezone', 'Europe/Berlin');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Mon 2026-07-20  09:00–14:00  (5h)', { timeout: 15000 });
  await expect(out).toContainText('1 free slot · 5h total');
  await expect(out).toContainText('Europe/Berlin');
});
