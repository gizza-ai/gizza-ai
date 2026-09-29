import { test, expect } from './fixtures';

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLInputElement | HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function fillBase(page: import('@playwright/test').Page, primary = '#6366f1') {
  await setField(page, '#in-primary', primary);
  await setField(page, '#in-accent', '');
  await page.selectOption('#in-neutral', 'zinc');
  await page.selectOption('#in-format', 'oklch');
  await page.selectOption('#in-tailwind', 'v4');
  await setField(page, '#in-radius', '0.625');
  await page.selectOption('#in-mode', 'both');
  await page.check('#in-charts');
  await page.check('#in-sidebar');
}

test('shadcn-theme-generator renders paste-ready light and dark CSS', async ({ page }) => {
  await page.goto('/tools/shadcn-theme-generator/');
  await fillBase(page);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('shadcn/ui theme — primary #6366f1', { timeout: 15_000 });
  await expect(out).toContainText(':root {');
  await expect(out).toContainText('.dark {');
  await expect(out).toContainText('--primary: oklch(0.585 0.204 277.12);');
  await expect(out).toContainText('--radius: 0.625rem;');
  await expect(out).toContainText('@theme inline {');
  await expect(out).toContainText('--color-primary: var(--primary);');
  await expect(out).toContainText('Contrast check (WCAG 2.x');
  await expect(out).toContainText('primary-foreground on primary');
});

test('shadcn-theme-generator honors deep-linked parameters', async ({ page }) => {
  const params = new URLSearchParams({
    primary: '#6366f1',
    accent: '#10b981',
    neutral: 'stone',
    format: 'hex',
    tailwind: 'v3',
    radius: '0.5',
    mode: 'light',
    charts: 'false',
    sidebar: 'false',
  });
  await page.goto(`/tools/shadcn-theme-generator/?${params.toString()}`);

  await expect(page.locator('#in-charts')).not.toBeChecked();
  await expect(page.locator('#in-sidebar')).not.toBeChecked();
  const out = page.locator('#tool-output');
  await expect(out).toContainText('primary #6366f1, accent #10b981, stone greys', {
    timeout: 15_000,
  });
  await expect(out).toContainText('@layer base {');
  await expect(out).toContainText(':root {');
  await expect(out).not.toContainText('.dark {');
  await expect(out).not.toContainText('--chart-1');
  await expect(out).not.toContainText('--sidebar');
});

test('shadcn-theme-generator covers advertised enum choices and color forms', async ({ page }) => {
  await page.goto('/tools/shadcn-theme-generator/');

  await fillBase(page, '#f00');
  await page.selectOption('#in-format', 'hex');
  await expect(page.locator('#tool-output')).toContainText('--primary: #ff0000;', {
    timeout: 15_000,
  });

  await fillBase(page, '6366F1');
  await page.selectOption('#in-format', 'hsl');
  await expect(page.locator('#tool-output')).toContainText('--primary: hsl(239 84% 67%);', {
    timeout: 15_000,
  });

  await fillBase(page, 'rgb(99, 102, 241)');
  await page.selectOption('#in-format', 'hsl');
  await page.selectOption('#in-format', 'oklch');
  await page.selectOption('#in-tailwind', 'v3');
  await expect(page.locator('#tool-output')).toContainText('@layer base {', { timeout: 15_000 });
  await page.selectOption('#in-format', 'hsl');
  await expect(page.locator('#tool-output')).toContainText('--primary: 239 84% 67%;');

  await fillBase(page, 'hsl(239, 84%, 67%)');
  await page.selectOption('#in-neutral', 'slate');
  await page.selectOption('#in-mode', 'dark');
  await expect(page.locator('#tool-output')).toContainText('slate greys', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).not.toContainText(':root {');
  await expect(page.locator('#tool-output')).toContainText('.dark {');
});

test('shadcn-theme-generator covers toggles and radius cap boundary', async ({ page }) => {
  await page.goto('/tools/shadcn-theme-generator/');
  await fillBase(page, '#dc2626');
  await setField(page, '#in-radius', '2');
  await page.selectOption('#in-neutral', 'neutral');
  await page.selectOption('#in-format', 'hex');
  await page.uncheck('#in-charts');
  await page.uncheck('#in-sidebar');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('--radius: 2rem;', { timeout: 15_000 });
  await expect(out).toContainText('neutral greys');
  await expect(out).not.toContainText('--chart-1');
  await expect(out).not.toContainText('--sidebar');

  await setField(page, '#in-radius', '2.125');
  await expect(out).toContainText('above the 2rem cap', { timeout: 15_000 });
});
