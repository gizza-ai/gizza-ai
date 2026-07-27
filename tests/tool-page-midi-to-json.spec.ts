import { test, expect } from './fixtures';

const MIDI_HEX =
  '4d546864000000060000000100604d54726b0000002400ff03055069616e6f00ff510307a12000ff58040402180800903c4060803c0000ff2f00';

async function outputJson(page): Promise<any> {
  const text = ((await page.locator('#tool-output').textContent()) ?? '').trim();
  return JSON.parse(text);
}

test('midi-to-json page — summary output parses real MIDI bytes', async ({ page }) => {
  await page.goto('/tools/midi-to-json/');
  await page.fill('#in-input', MIDI_HEX);
  await page.selectOption('#in-encoding', 'hex');
  await page.selectOption('#in-format', 'summary');
  await expect(page.locator('#tool-output')).toContainText('"noteCount": 1', { timeout: 15000 });
  const json = await outputJson(page);
  expect(json.format).toBe(0);
  expect(json.trackCount).toBe(1);
  expect(json.noteCount).toBe(1);
  expect(json.tempoBpm).toBe(120);
  expect(json.timeSignature).toBe('4/4');
  expect(json.trackNames).toEqual(['Piano']);
});

test('midi-to-json page — notes output includes paired note timing', async ({ page }) => {
  await page.goto('/tools/midi-to-json/');
  await page.fill('#in-input', MIDI_HEX);
  await page.selectOption('#in-encoding', 'hex');
  await page.selectOption('#in-format', 'notes');
  await expect(page.locator('#tool-output')).toContainText('"name": "C4"', { timeout: 15000 });
  const json = await outputJson(page);
  const note = json.tracks[0].notes[0];
  expect(note.midi).toBe(60);
  expect(note.durationTicks).toBe(96);
  expect(note.duration).toBe(0.5);
  expect(note.velocity).toBe(64);
});

test('midi-to-json page — query-param deep-link prefills and auto-runs', async ({ page }) => {
  await page.goto(
    '/tools/midi-to-json/?input=' +
      encodeURIComponent(MIDI_HEX) +
      '&encoding=hex&format=events',
  );
  await expect(page.locator('#in-input')).toHaveValue(MIDI_HEX, { timeout: 15000 });
  await expect(page.locator('#in-encoding')).toHaveValue('hex');
  await expect(page.locator('#in-format')).toHaveValue('events');
  await expect(page.locator('#tool-output')).toContainText('"type": "noteOn"', { timeout: 15000 });
  const json = await outputJson(page);
  expect(json.tracks[0].some((event) => event.type === 'noteOn' && event.note === 60)).toBe(true);
});
