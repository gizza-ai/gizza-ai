import { test, expect } from './fixtures';

async function decodeMidi(page: import('@playwright/test').Page) {
  return page.evaluate(async () => {
    const dl = document.getElementById('tool-output-download') as HTMLAnchorElement | null;
    const href = dl?.href || '';
    if (!href.startsWith('data:audio/midi;base64,')) return { error: 'no MIDI data URL: ' + href };
    const buf = new Uint8Array(await (await fetch(href)).arrayBuffer());
    const text = new TextDecoder('latin1').decode(buf);
    const dv = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
    if (text.slice(0, 4) !== 'MThd') return { error: 'missing MThd header' };
    if (!text.includes('MTrk')) return { error: 'missing MTrk chunk' };
    return {
      len: buf.length,
      download: dl?.getAttribute('download'),
      magic: text.slice(0, 4),
      tracks: dv.getUint16(10, false),
      ppq: dv.getUint16(12, false) & 0x7fff,
    };
  });
}

const RIFF =
  'e|-------------|\n' +
  'B|-------------|\n' +
  'G|-------------|\n' +
  'D|--2--2--5--5-|\n' +
  'A|--2--2--5--5-|\n' +
  'E|--0--0--3--3-|';

test('guitar-tab-to-midi page produces a real Standard MIDI download', async ({ page }) => {
  await page.goto('/tools/guitar-tab-to-midi/');
  await page.fill('#in-tab', RIFF);
  await page.selectOption('#in-instrument', 'distortion-guitar');
  await page.fill('#in-tempo', '132');

  await expect(page.locator('#tool-output')).toContainText('12 notes from 1 stave', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('Range: E2 to G3');
  await expect(page.locator('#tool-output-download')).toBeVisible();
  const midi = await decodeMidi(page);
  expect(midi.error).toBeUndefined();
  expect(midi.download).toBe('guitar-tab.mid');
  expect(midi.magic).toBe('MThd');
  expect(midi.tracks).toBe(1);
  expect(midi.ppq).toBe(480);
  expect(midi.len).toBeGreaterThan(100);
});

test('guitar-tab-to-midi deep link applies tuning, checkbox and timing controls', async ({ page }) => {
  const tab = encodeURIComponent('G|-----|\nD|-----|\nA|--5--|\nE|--x--|');
  await page.goto(
    `/tools/guitar-tab-to-midi/?tab=${tab}&tuning=auto&timing=events&note_duration=sixteenth&instrument=electric-bass-finger&muted_notes=true&velocity=64`,
  );

  await expect(page.locator('#in-timing')).toHaveValue('events');
  await expect(page.locator('#in-note_duration')).toHaveValue('sixteenth');
  await expect(page.locator('#in-muted_notes')).toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('2 notes from 1 stave', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('Tuning: bass-standard');
  const midi = await decodeMidi(page);
  expect(midi.error).toBeUndefined();
  expect(midi.magic).toBe('MThd');
  expect(midi.tracks).toBe(1);
  expect(midi.ppq).toBe(480);
  expect(midi.len).toBeGreaterThan(60);
});

test('guitar-tab-to-midi reports malformed tabs without exposing a download', async ({ page }) => {
  await page.goto('/tools/guitar-tab-to-midi/');
  await page.fill('#in-tab', 'not tablature at all');
  await expect(page.locator('#tool-output')).toContainText('no tablature found', { timeout: 15_000 });
  await expect(page.locator('#tool-output-download')).toBeHidden();
});
