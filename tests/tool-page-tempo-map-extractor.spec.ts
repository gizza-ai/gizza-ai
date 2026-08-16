import { test, expect } from './fixtures';

const tool = '/tools/tempo-map-extractor/';

async function outputText(page): Promise<string> {
  const text = await page.locator('#tool-output').textContent();
  return (text ?? '').trim();
}

async function setTextarea(page, selector: string, value: string): Promise<void> {
  await page.locator(selector).evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v as string;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('tempo-map-extractor page emits exact default CSV', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-beats', '0.000\n0.500\n1.000\n1.500');

  await expect(page.locator('#tool-output')).toContainText('time_seconds,bpm,beat,interval_ms', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    'time_seconds,bpm,beat,interval_ms\n0.000,120.00,1,500.0\n0.500,120.00,2,500.0\n1.000,120.00,3,500.0',
  );
});

test('tempo-map-extractor deep-link supports summary output and half-note beat units', async ({ page }) => {
  await page.goto(
    tool +
      '?beats=' +
      encodeURIComponent('0\n1\n2\n3') +
      '&beat_unit=half&output=summary&decimals=1',
  );

  await expect(page.locator('#in-beat_unit')).toHaveValue('half', { timeout: 15000 });
  await expect(page.locator('#in-output')).toHaveValue('summary');
  const out = await outputText(page);
  expect(out).toContain('Mean BPM: 120.0');
  expect(out).toContain('Overall average across the take: 120.0 BPM');
  expect(out).toContain('Stability: rock steady');
});

test('tempo-map-extractor smooths jitter, filters double taps, and samples a grid', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-beats', '0\n0.02\n0.5\n1.5\n2.0');
  await page.fill('#in-min_interval_ms', '100');
  await page.fill('#in-smoothing', '3');
  await page.selectOption('#in-smooth_method', 'median');
  await page.fill('#in-grid_seconds', '1');
  await page.fill('#in-decimals', '1');

  await expect(page.locator('#tool-output')).toContainText('0.000,90.0,1,500.0', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    'time_seconds,bpm,beat,interval_ms\n0.000,90.0,1,500.0\n1.000,120.0,2,1000.0\n2.000,90.0,3,500.0',
  );
});

test('tempo-map-extractor page accepts the exact 20000-beat cap in summary mode', async ({ page }) => {
  await page.goto(tool);
  const beats = Array.from({ length: 20000 }, (_, i) => (i * 0.5).toFixed(1)).join('\n');
  await setTextarea(page, '#in-beats', beats);
  await page.selectOption('#in-output', 'summary');

  await expect(page.locator('#tool-output')).toContainText('Beats: 20000', { timeout: 30000 });
  const out = await outputText(page);
  expect(out).toContain('(19999 tempo readings over 9999.500 s)');
  expect(out).toContain('Mean BPM: 120.00');
});

test('tempo-map-extractor page surfaces validation errors', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-beats', '0\n1\n0.5');
  const out = page.locator('#tool-output');
  await expect(out).toHaveClass(/error/, { timeout: 15000 });
  await expect(out).toContainText('beat times must increase');
});
