import { test, expect } from './fixtures';

const cosineData = '1,0,-1,0,1,0,-1,0';
const cosineOutput = `Spectrum
  samples           8
  transform length  8
  sample rate       8.0000 /s
  resolution        1.0000 per bin
  nyquist           4.0000
  window            rectangular
  scale             amplitude
  spectrum          one-sided

Peaks
  1. bin 2  freq 2.0000  amplitude 1.0000  phase(deg) 0.0000

Bins
  bin  frequency  amplitude  phase(deg)
  0  0.0000  0.0000  0.0000
  1  1.0000  0.0000  0.0000
  2  2.0000  1.0000  0.0000
  3  3.0000  0.0000  0.0000
  4  4.0000  0.0000  0.0000
`;

test('fft-analyzer page emits exact spectrum output for a 2 Hz cosine', async ({ page }) => {
  await page.goto('/tools/fft-analyzer/');
  await page.fill('#in-data', cosineData);
  await page.fill('#in-sample_rate', '8');
  await page.fill('#in-peaks', '1');
  await page.fill('#in-decimals', '4');

  await expect(page.locator('#tool-output')).toHaveText(cosineOutput, { timeout: 15_000 });
});

test('fft-analyzer honours deep-link chart and window params', async ({ page }) => {
  const params = new URLSearchParams({
    data: '3,1.6667,-1.1481,-2.9424,-2.1213,0.5853,2.7716,2.4944,0,-2.4944,-2.7716,-0.5853,2.1213,2.9424,1.1481,-1.6667,-3,-1.6667,1.1481,2.9424,2.1213,-0.5853,-2.7716,-2.4944,0,2.4944,2.7716,0.5853,-2.1213,-2.9424,-1.1481,1.6667',
    sample_rate: '32',
    window: 'hann',
    pad: 'pow2',
    spectrum: 'auto',
    scale: 'amplitude',
    phase_unit: 'degrees',
    peaks: '1',
    decimals: '3',
    format: 'chart',
  });
  await page.goto(`/tools/fft-analyzer/?${params.toString()}`);

  await expect(page.locator('#in-window')).toHaveValue('hann', { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('chart');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('coherent gain     0.500');
  await expect(out).toContainText('amplitude by frequency');
  await expect(out).toContainText('4.000');
  await expect(out).toContainText('█');
});

test('fft-analyzer covers complex JSON and non-default checkbox', async ({ page }) => {
  await page.goto('/tools/fft-analyzer/');
  await page.fill('#in-data', '1+i, 2+i, 3+i, 4+i');
  await page.fill('#in-sample_rate', '4');
  await page.check('#in-remove_dc');
  await page.selectOption('#in-spectrum', 'two-sided');
  await page.selectOption('#in-format', 'json');
  await page.fill('#in-peaks', '2');
  await page.fill('#in-decimals', '4');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"complex_input": true', { timeout: 15_000 });
  await expect(out).toContainText('"spectrum": "two-sided"');
  await expect(out).toContainText('"removed_dc": 2.5000');
  await expect(out).toContainText('"frequency": -1.0000');
});
