import { test, expect, type Page } from './fixtures';

const tool = '/tools/midi-note-extract/';

// The tiny one-note SMF from blocks/midi-note-extract/page/meta.toml: format 0,
// 96 ticks/quarter, 120 BPM, 4/4, track "Piano", a single C4 (pitch 60,
// velocity 64) running 96 ticks == 1 beat == 0.5 s.
const MIDI_HEX =
  '4d546864000000060000000100604d54726b0000002400ff03055069616e6f00ff510307a12000ff58040402180800903c4060803c0000ff2f00';

async function outputText(page: Page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('midi-note-extract page renders the standard seconds CSV exactly', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-input', MIDI_HEX);
  await page.selectOption('#in-encoding', 'hex');
  await page.selectOption('#in-columns', 'standard');
  await page.selectOption('#in-time_unit', 'seconds');

  await expect(page.locator('#tool-output')).toContainText('note_name', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    'track,channel,start,duration,pitch,note_name,velocity\n0,0,0.000,0.500,60,C4,64',
  );
});

test('midi-note-extract page supports minimal columns in ticks', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-input', MIDI_HEX);
  await page.selectOption('#in-encoding', 'hex');
  await page.selectOption('#in-columns', 'minimal');
  await page.selectOption('#in-time_unit', 'ticks');

  await expect(page.locator('#tool-output')).toContainText('start,duration,pitch,velocity', {
    timeout: 15000,
  });
  // Ticks are integers, so the 3-decimal setting must not touch them.
  expect(await outputText(page)).toBe('start,duration,pitch,velocity\n0,96,60,64');
});

test('midi-note-extract page drops the header row when the checkbox is cleared', async ({
  page,
}) => {
  await page.goto(tool);
  await page.fill('#in-input', MIDI_HEX);
  await page.selectOption('#in-encoding', 'hex');
  await page.selectOption('#in-columns', 'standard');
  await page.selectOption('#in-delimiter', 'tab');
  await expect(page.locator('#in-header')).toBeChecked();
  await page.uncheck('#in-header');

  await expect(page.locator('#tool-output')).toContainText('C4', { timeout: 15000 });
  expect(await outputText(page)).toBe('0\t0\t0.000\t0.500\t60\tC4\t64');
});

test('midi-note-extract query-param deep-link prefills and computes normalized beats', async ({
  page,
}) => {
  await page.goto(
    tool +
      '?input=' +
      MIDI_HEX +
      '&encoding=hex&columns=minimal&time_unit=beats&velocity_scale=normalized&delimiter=semicolon&header=false&decimals=2',
  );

  await expect(page.locator('#in-input')).toHaveValue(MIDI_HEX, { timeout: 15000 });
  await expect(page.locator('#in-encoding')).toHaveValue('hex');
  await expect(page.locator('#in-columns')).toHaveValue('minimal');
  await expect(page.locator('#in-time_unit')).toHaveValue('beats');
  await expect(page.locator('#in-velocity_scale')).toHaveValue('normalized');
  await expect(page.locator('#in-delimiter')).toHaveValue('semicolon');
  await expect(page.locator('#in-header')).not.toBeChecked();
  await expect(page.locator('#in-decimals')).toHaveValue('2');

  // 96 ticks == 1.00 beat; velocity 64/127 == 0.50 at two decimals.
  await expect(page.locator('#tool-output')).toContainText(';', { timeout: 15000 });
  expect(await outputText(page)).toBe('0.00;1.00;60;0.50');
});
