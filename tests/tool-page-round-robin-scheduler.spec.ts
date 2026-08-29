import { test, expect } from './fixtures';

async function outText(page): Promise<string> {
  return (await page.locator('#tool-output').textContent()) ?? '';
}

async function setTextarea(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('renders a deterministic four-team round robin', async ({ page }) => {
  await page.goto('/tools/round-robin-scheduler/');
  await setTextarea(page, '#in-participants', 'Alice\nBob\nCarol\nDave');
  await page.uncheck('#in-include_summary');

  await expect(page.locator('#tool-output')).toContainText('Round 3', { timeout: 15000 });
  expect((await outText(page)).trimEnd()).toBe(
    'Round 1\n' +
      '  1. Alice vs Dave\n' +
      '  2. Bob vs Carol\n\n' +
      'Round 2\n' +
      '  1. Carol vs Alice\n' +
      '  2. Dave vs Bob\n\n' +
      'Round 3\n' +
      '  1. Alice vs Bob\n' +
      '  2. Carol vs Dave',
  );
});

test('deep-link pre-fills odd roster and rotates byes', async ({ page }) => {
  const participants = 'Alice\nBob\nCarol';
  await page.goto(
    '/tools/round-robin-scheduler/?participants=' +
      encodeURIComponent(participants) +
      '&schedule_type=single&output_format=text&include_byes=true&include_summary=false',
  );

  await expect(page.locator('#in-participants')).toHaveValue(participants, { timeout: 15000 });
  await expect(page.locator('#in-include_byes')).toBeChecked();
  const text = await outText(page);
  expect(text).toContain('Bye: Alice');
  expect(text).toContain('Bye: Bob');
  expect(text).toContain('Bye: Carol');
});

test('CSV output exercises court count and bare participant count', async ({ page }) => {
  await page.goto('/tools/round-robin-scheduler/');
  await setTextarea(page, '#in-participants', '4');
  await page.fill('#in-courts', '2');
  await page.selectOption('#in-output_format', 'csv');
  await page.uncheck('#in-include_summary');

  await expect(page.locator('#tool-output')).toContainText('round,match,home,away,court', { timeout: 15000 });
  expect((await outText(page)).trimEnd()).toBe(
    'round,match,home,away,court\n' +
      '1,1,Team 1,Team 4,Court 1\n' +
      '1,2,Team 2,Team 3,Court 2\n' +
      '2,1,Team 3,Team 1,Court 1\n' +
      '2,2,Team 4,Team 2,Court 2\n' +
      '3,1,Team 1,Team 2,Court 1\n' +
      '3,2,Team 3,Team 4,Court 2',
  );
});

test('double round robin JSON, start round and checkbox off states', async ({ page }) => {
  await page.goto('/tools/round-robin-scheduler/');
  await setTextarea(page, '#in-participants', 'Alice\nBob\nCarol\nDave');
  await page.selectOption('#in-schedule_type', 'double');
  await page.selectOption('#in-output_format', 'json');
  await page.fill('#in-start_round', '3');
  await page.uncheck('#in-include_byes');
  await page.uncheck('#in-include_summary');

  await expect(page.locator('#tool-output')).toContainText('"round": 8', { timeout: 15000 });
  const parsed = JSON.parse(await outText(page));
  expect(parsed).toHaveLength(12);
  expect(parsed[0]).toMatchObject({ round: 3, match: 1, home: 'Alice', away: 'Dave', bye: false });
  expect(parsed[11]).toMatchObject({ round: 8, match: 2, home: 'Dave', away: 'Carol', bye: false });
});

test('advertised cap boundary and duplicate-name error render on the page', async ({ page }) => {
  await page.goto('/tools/round-robin-scheduler/');
  await setTextarea(page, '#in-participants', '65');
  await expect(page.locator('#tool-output')).toContainText('at most 64 participants', { timeout: 15000 });

  await setTextarea(page, '#in-participants', 'Lions\nlions');
  await expect(page.locator('#tool-output')).toContainText("duplicate participant 'lions'", { timeout: 15000 });
});
