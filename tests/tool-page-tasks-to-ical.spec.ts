import { test, expect } from './fixtures';

const TASKS = `(A) Pay the hosting invoice +admin @office due:2026-08-25
Draft the quarterly report +work t:2026-08-20 due:2026-08-28
Buy milk @errands`;

async function runWasm(
  page: any,
  tasks: string = TASKS,
  component = 'vtodo',
  include = 'dated',
  skipCompleted = 'false',
  durationMinutes = '60',
  reminderMinutes = '0',
  timezone = 'floating',
  calendarName = '',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/tasks-to-ical/gizza_ai_tasks_to_ical_web.js');
    await mod.default('/tools/tasks-to-ical/gizza_ai_tasks_to_ical_web_bg.wasm');
    return mod.run(
      args.tasks,
      args.component,
      args.include,
      args.skipCompleted,
      args.durationMinutes,
      args.reminderMinutes,
      args.timezone,
      args.calendarName,
    );
  }, { tasks, component, include, skipCompleted, durationMinutes, reminderMinutes, timezone, calendarName });
}

test('tasks-to-ical page converts todo.txt deadlines to iCalendar to-dos', async ({ page }) => {
  await page.goto('/tools/tasks-to-ical/');
  await page.fill('#in-tasks', TASKS);

  const output = page.locator('#tool-output');
  await expect(output).toContainText('BEGIN:VCALENDAR', { timeout: 20_000 });
  await expect(output).toContainText('BEGIN:VTODO');
  await expect(output).toContainText('SUMMARY:Pay the hosting invoice');
  await expect(output).toContainText('DUE;VALUE=DATE:20260825');
  await expect(output).toContainText('CATEGORIES:admin,office');
  await expect(output).not.toContainText('SUMMARY:Buy milk');
});

test('tasks-to-ical deep link fills event/reminder controls and renders UTC output', async ({ page }) => {
  const params = new URLSearchParams({
    tasks: 'Submit the grant application +work due:2026-08-25T14:30',
    component: 'vevent',
    include: 'dated',
    skip_completed: 'false',
    duration_minutes: '45',
    reminder_minutes: '30',
    timezone: 'utc',
    calendar_name: 'Deadlines',
  });
  await page.goto(`/tools/tasks-to-ical/?${params.toString()}`);

  await expect(page.locator('#in-component')).toHaveValue('vevent', { timeout: 15_000 });
  await expect(page.locator('#in-timezone')).toHaveValue('utc');
  await expect(page.locator('#in-duration_minutes')).toHaveValue('45');
  await expect(page.locator('#in-reminder_minutes')).toHaveValue('30');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('X-WR-CALNAME:Deadlines', { timeout: 20_000 });
  await expect(output).toContainText('BEGIN:VEVENT');
  await expect(output).toContainText('DTSTART:20260825T143000Z');
  await expect(output).toContainText('DTEND:20260825T151500Z');
  await expect(output).toContainText('BEGIN:VALARM');
  await expect(output).toContainText('TRIGGER:-PT30M');
});

test('tasks-to-ical wasm covers defaults, enums, boundaries, checkbox and errors', async ({ page }) => {
  await page.goto('/tools/tasks-to-ical/');

  const exact = (await runWasm(page, '(A) Pay invoice +admin @office due:2026-08-25')).trimEnd();
  expect(exact).toBe(`BEGIN:VCALENDAR\r
VERSION:2.0\r
PRODID:-//gizza-ai//tasks-to-ical//EN\r
CALSCALE:GREGORIAN\r
BEGIN:VTODO\r
UID:pay-invoice@1.local\r
DTSTAMP:19700101T000000Z\r
DUE;VALUE=DATE:20260825\r
SUMMARY:Pay invoice\r
DESCRIPTION:(A) Pay invoice +admin @office due:2026-08-25\r
CATEGORIES:admin,office\r
PRIORITY:1\r
STATUS:NEEDS-ACTION\r
END:VTODO\r
END:VCALENDAR`);

  const event = await runWasm(
    page,
    'Submit filing due:2026-08-25T14:30',
    'vevent',
    'dated',
    'false',
    '1440',
    '10080',
    'utc',
    'Deadlines',
  );
  expect(event).toContain('BEGIN:VEVENT');
  expect(event).toContain('X-WR-CALNAME:Deadlines');
  expect(event).toContain('DTSTART:20260825T143000Z');
  expect(event).toContain('DTEND:20260826T143000Z');
  expect(event).toContain('TRIGGER:-PT10080M');

  const includeAll = await runWasm(page, 'Buy milk\nRenew passport due:2026-09-01', 'vtodo', 'all');
  expect(includeAll).toContain('SUMMARY:Buy milk');
  expect(includeAll.match(/BEGIN:VTODO/g)?.length).toBe(2);

  const datedOnly = await runWasm(page, 'Buy milk\nRenew passport due:2026-09-01', 'vtodo', 'dated');
  expect(datedOnly).not.toContain('SUMMARY:Buy milk');
  expect(datedOnly.match(/BEGIN:VTODO/g)?.length).toBe(1);

  const skipDone = await runWasm(
    page,
    'x 2026-08-20 Done thing due:2026-08-19\nOpen thing due:2026-08-25',
    'vtodo',
    'dated',
    'true',
  );
  expect(skipDone).not.toContain('Done thing');
  expect(skipDone).toContain('SUMMARY:Open thing');

  await expect(runWasm(page, 'Broken due:next-friday')).rejects.toThrow(/due:next-friday/);
  await expect(runWasm(page, 'Buy milk', 'vevent', 'all')).rejects.toThrow(/calendar event needs one/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool tasks-to-ical');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
