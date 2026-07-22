import { test, expect } from './fixtures';

const before = 'data:image/png;base64,iVBORw0KGgo=';
const after = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAAB';

test('before-after-slider page emits a standalone horizontal slider document', async ({ page }) => {
  await page.goto('/tools/before-after-slider/');
  await page.fill('#in-before', before);
  await page.fill('#in-after', after);
  await page.fill('#in-before_label', 'Original');
  await page.fill('#in-after_label', 'Edited');
  await page.fill('#in-start_position', '40');
  await page.fill('#in-width', '720');

  await expect(async () => {
    const out = (await page.locator('#tool-output').textContent()) ?? '';
    expect(out).toContain('<!DOCTYPE html>');
    expect(out).toContain('class="bas-container"');
    expect(out).toContain('style="--pos:40%"');
    expect(out).toContain('max-width:720px;');
    expect(out).toContain('Original');
    expect(out).toContain('Edited');
    expect(out).toContain('data-axis="x"');
  }).toPass({ timeout: 15000 });
});

test('before-after-slider query-param deep-link can produce vertical embed with hover', async ({ page }) => {
  await page.goto(
    '/tools/before-after-slider/?before=' +
      encodeURIComponent(before) +
      '&after=' +
      encodeURIComponent(after) +
      '&orientation=vertical&start_position=25&move_on_hover=true&handle_color=%23ff3366&output=embed'
  );

  await expect(page.locator('#in-orientation')).toHaveValue('vertical', { timeout: 15000 });
  await expect(page.locator('#in-output')).toHaveValue('embed', { timeout: 15000 });
  await expect(page.locator('#in-move_on_hover')).toBeChecked({ timeout: 15000 });
  await expect(async () => {
    const out = (await page.locator('#tool-output').textContent()) ?? '';
    expect(out).toContain('data-axis="y"');
    expect(out).not.toContain('<!DOCTYPE html>');
    expect(out).toContain('data-hover="1"');
    expect(out).toContain('--pos:25%');
    expect(out).toContain('#ff3366');
    expect(out).toContain('inset(0 0 calc(100% - var(--pos)) 0)');
  }).toPass({ timeout: 15000 });
});

test('before-after-slider page escapes image sources in generated html', async ({ page }) => {
  await page.goto('/tools/before-after-slider/');
  await page.fill('#in-before', 'https://example.com/before.jpg"><script>alert(1)</script>');
  await page.fill('#in-after', after);
  await page.selectOption('#in-output', 'embed');

  await expect(async () => {
    const out = (await page.locator('#tool-output').textContent()) ?? '';
    expect(out).toContain('&quot;&gt;&lt;script&gt;alert(1)&lt;/script&gt;');
    expect(out).not.toContain('src="https://example.com/before.jpg"><script>alert(1)</script>"');
  }).toPass({ timeout: 15000 });
});
