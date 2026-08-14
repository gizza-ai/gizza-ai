import { test, expect } from './fixtures';

const EVENTS = `title,start,end,location
Team sync,2026-07-24 09:00,2026-07-24 09:30,Room 2
Conference,2026-07-27,2026-07-29,Berlin`;

async function runWasm(
  page: any,
  csv: string = EVENTS,
  timezone = 'floating',
  defaultDurationMinutes = '60',
  includeAlarm = 'false',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/csv-to-ics/gizza_ai_csv_to_ics_web.js');
    await mod.default('/tools/csv-to-ics/gizza_ai_csv_to_ics_web_bg.wasm');
    return mod.run(
      args.csv,
      args.timezone,
      args.defaultDurationMinutes,
      args.includeAlarm,
    );
  }, { csv, timezone, defaultDurationMinutes, includeAlarm });
}

test('csv-to-ics page converts event CSV to iCalendar text', async ({ page }) => {
  await page.goto('/tools/csv-to-ics/');
  await page.fill('#in-csv', EVENTS);

  const output = page.locator('#tool-output');
  await expect(output).toContainText('BEGIN:VCALENDAR', { timeout: 20_000 });
  await expect(output).toContainText('SUMMARY:Team sync');
  await expect(output).toContainText('DTSTART:20260724T090000');
  await expect(output).toContainText('DTSTART;VALUE=DATE:20260727');
});

test('csv-to-ics deep link enables UTC and alarm controls', async ({ page }) => {
  const params = new URLSearchParams({
    csv: 'summary,start,end,id\nRelease window,2026-07-24T22:00,2026-07-25T02:00,release-2026-07',
    timezone: 'utc',
    default_duration_minutes: '45',
    include_alarm: 'true',
  });
  await page.goto(`/tools/csv-to-ics/?${params.toString()}`);

  await expect(page.locator('#in-timezone')).toHaveValue('utc', { timeout: 15_000 });
  await expect(page.locator('#in-include_alarm')).toBeChecked();
  const output = page.locator('#tool-output');
  await expect(output).toContainText('DTSTART:20260724T220000Z', { timeout: 20_000 });
  await expect(output).toContainText('UID:release-2026-07');
  await expect(output).toContainText('BEGIN:VALARM');
});

test('csv-to-ics wasm covers defaults, aliases, boundary, errors and CLI example', async ({ page }) => {
  await page.goto('/tools/csv-to-ics/');

  const exact = (await runWasm(page)).trimEnd();
  expect(exact).toBe(`BEGIN:VCALENDAR\r
VERSION:2.0\r
PRODID:-//gizza-ai//csv-to-ics//EN\r
CALSCALE:GREGORIAN\r
BEGIN:VEVENT\r
UID:team-sync@1.local\r
DTSTAMP:19700101T000000Z\r
DTSTART:20260724T090000\r
DTEND:20260724T093000\r
SUMMARY:Team sync\r
LOCATION:Room 2\r
END:VEVENT\r
BEGIN:VEVENT\r
UID:conference@2.local\r
DTSTAMP:19700101T000000Z\r
DTSTART;VALUE=DATE:20260727\r
DTEND;VALUE=DATE:20260730\r
SUMMARY:Conference\r
LOCATION:Berlin\r
END:VEVENT\r
END:VCALENDAR`);

  const alias = await runWasm(page, 'event,date,duration_minutes,notes\nStandup,2026-07-24 09:00,15,Blockers only', 'floating', '60', 'true');
  expect(alias).toContain('DTEND:20260724T091500');
  expect(alias).toContain('DESCRIPTION:Blockers only');
  expect(alias).toContain('BEGIN:VALARM');

  const defaultDuration = await runWasm(page, 'title,start\nFocus,2026-07-24 10:00', 'floating', '30', 'false');
  expect(defaultDuration).toContain('DTEND:20260724T103000');

  const boundaryCsv = ['title,start', ...Array.from({ length: 5000 }, (_, i) => `Event ${i + 1},2026-07-24`)].join('\n');
  const boundaryOut = await runWasm(page, boundaryCsv);
  expect(boundaryOut).toContain('SUMMARY:Event 5000');
  await expect(runWasm(page, `${boundaryCsv}\nEvent 5001,2026-07-25`)).rejects.toThrow(/at most 5000/);

  await expect(runWasm(page, 'title,start\nBroken,not-a-date')).rejects.toThrow(/row 2/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool csv-to-ics');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
