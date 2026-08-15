import { test, expect } from './fixtures';

// Format 0, 96 PPQ, one track: track name "Piano", tempo 500000 µs (120 BPM),
// a 4/4 time signature, then one middle-C quarter note.
const FIXTURE_HEX =
  '4d546864000000060000000100604d54726b0000002400ff03055069616e6f00ff510307a12000ff58040402180800903c4060803c0000ff2f00';
const FIXTURE_B64 =
  'TVRoZAAAAAYAAAABAGBNVHJrAAAAJAD/AwVQaWFubwD/UQMHoSAA/1gEBAIYCACQPEBggDwAAP8vAA==';

// Pull the real bytes back out of the download anchor and decode the parts that
// prove the retiming worked: the header, every FF 51 03 tempo meta event, and
// the note-off delta (which only moves when keep_duration re-notates).
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

    // Every tempo meta event, in microseconds per quarter note.
    const tempos: number[] = [];
    for (let i = 0; i + 5 < buf.length; i++) {
      if (buf[i] === 0xff && buf[i + 1] === 0x51 && buf[i + 2] === 0x03) {
        tempos.push((buf[i + 3] << 16) | (buf[i + 4] << 8) | buf[i + 5]);
      }
    }
    // The single note-off (0x80, channel 0) and its variable-length delta.
    let noteOffDelta: number | null = null;
    for (let i = 1; i < buf.length; i++) {
      if (buf[i] === 0x80 && buf[i + 1] === 0x3c) {
        let start = i - 1;
        while (start > 0 && (buf[start - 1] & 0x80) !== 0) start--;
        let value = 0;
        for (let j = start; j < i; j++) value = (value << 7) | (buf[j] & 0x7f);
        noteOffDelta = value;
        break;
      }
    }
    return {
      len: buf.length,
      download: dl?.getAttribute('download'),
      magic: text.slice(0, 4),
      tracks: dv.getUint16(10, false),
      ppq: dv.getUint16(12, false) & 0x7fff,
      tempos,
      noteOffDelta,
      hasTrackName: text.includes('Piano'),
    };
  });
}

test('midi-tempo-change sets an exact BPM and leaves the notes alone', async ({ page }) => {
  await page.goto('/tools/midi-tempo-change/');
  await page.fill('#in-input', FIXTURE_HEX);
  await page.selectOption('#in-encoding', 'hex');
  await page.fill('#in-bpm', '140');

  await expect(page.locator('#tool-output')).toContainText('Tempo: 120.00 → 140.00 BPM (1.167×).', {
    timeout: 15_000,
  });
  await expect(page.locator('#tool-output')).toContainText('1 track, 96 PPQ, 1 note unchanged.');
  await expect(page.locator('#tool-output')).toContainText('Playing time: 0.500 s → 0.429 s.');
  await expect(page.locator('#tool-output-download')).toBeVisible();

  const midi = await decodeMidi(page);
  expect(midi.error).toBeUndefined();
  expect(midi.download).toBe('tempo-changed.mid');
  expect(midi.magic).toBe('MThd');
  expect(midi.tracks).toBe(1);
  // The tick grid is never re-gridded, and the note keeps its 96-tick length.
  expect(midi.ppq).toBe(96);
  expect(midi.noteOffDelta).toBe(96);
  expect(midi.hasTrackName).toBe(true);
  // 500000 µs / (140/120) = 428571 µs per quarter note.
  expect(midi.tempos).toEqual([428571]);
});

test('midi-tempo-change scale mode doubles the speed', async ({ page }) => {
  await page.goto('/tools/midi-tempo-change/');
  await page.fill('#in-input', FIXTURE_HEX);
  await page.selectOption('#in-encoding', 'hex');
  await page.selectOption('#in-mode', 'scale');
  await page.fill('#in-factor', '2');

  await expect(page.locator('#tool-output')).toContainText('120.00 → 240.00 BPM', {
    timeout: 15_000,
  });
  const midi = await decodeMidi(page);
  expect(midi.error).toBeUndefined();
  expect(midi.tempos).toEqual([250000]);
  expect(midi.noteOffDelta).toBe(96);
});

test('midi-tempo-change deep link drives base64 auto-detect and the flatten select', async ({
  page,
}) => {
  await page.goto(
    `/tools/midi-tempo-change/?input=${encodeURIComponent(
      FIXTURE_B64,
    )}&encoding=auto&mode=set-bpm&bpm=100&tempo_map=flatten`,
  );

  await expect(page.locator('#in-encoding')).toHaveValue('auto');
  await expect(page.locator('#in-tempo_map')).toHaveValue('flatten');
  await expect(page.locator('#tool-output')).toContainText('120.00 → 100.00 BPM', {
    timeout: 15_000,
  });
  await expect(page.locator('#tool-output')).toContainText(
    'Tempo map flattened to one constant tempo.',
  );
  const midi = await decodeMidi(page);
  expect(midi.error).toBeUndefined();
  expect(midi.tempos).toEqual([600000]);
});

test('midi-tempo-change keep_duration re-notates so the playing time is unchanged', async ({
  page,
}) => {
  // Non-default checkbox state, driven from the deep link.
  await page.goto(
    `/tools/midi-tempo-change/?input=${FIXTURE_HEX}&encoding=hex&mode=set-bpm&bpm=240&keep_duration=true`,
  );

  await expect(page.locator('#in-keep_duration')).toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('Playing time: 0.500 s → 0.500 s.', {
    timeout: 15_000,
  });
  await expect(page.locator('#tool-output')).toContainText(
    'Note positions rescaled, so the playing time is unchanged.',
  );
  const midi = await decodeMidi(page);
  expect(midi.error).toBeUndefined();
  expect(midi.tempos).toEqual([250000]);
  // The quarter note became a half note: 96 ticks → 192 ticks.
  expect(midi.noteOffDelta).toBe(192);
});

test('midi-tempo-change reports a non-MIDI paste without exposing a download', async ({ page }) => {
  await page.goto('/tools/midi-tempo-change/');
  await page.fill('#in-input', '68656c6c6f20776f726c64');
  await page.selectOption('#in-encoding', 'hex');

  await expect(page.locator('#tool-output')).toContainText('MThd', { timeout: 15_000 });
  await expect(page.locator('#tool-output-download')).toBeHidden();
});

test('midi-tempo-change shows a neutral idle state before anything is pasted', async ({ page }) => {
  await page.goto('/tools/midi-tempo-change/');
  await expect(page.locator('#tool-output')).toContainText('your retimed .mid download', {
    timeout: 15_000,
  });
  await expect(page.locator('#tool-output-download')).toBeHidden();
});
