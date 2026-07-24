import { test, expect } from './fixtures';

const ADA = JSON.stringify({
  name: 'Ada Lovelace',
  title: 'Software Engineer',
  email: 'ada@example.com',
  location: 'London',
  links: ['github.com/ada'],
  summary: 'Pioneering engineer who wrote the first published algorithm.',
  experience: [
    {
      role: 'Engineer',
      company: 'Analytical Co',
      dates: '1843–1852',
      location: 'London',
      bullets: ['Wrote the first algorithm', 'Designed loop constructs'],
    },
  ],
  education: [{ degree: 'Mathematics', school: 'Private tutoring', dates: '1830s' }],
  skills: ['Algorithms', 'Mathematics', 'Technical writing'],
  sections: [{ heading: 'Projects', items: ['Analytical Engine notes'] }],
});

async function fillData(page: import('@playwright/test').Page, data = ADA) {
  await page.locator('#in-data').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, data);
}

async function output(page: import('@playwright/test').Page) {
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<!DOCTYPE html>', { timeout: 15_000 });
  return out;
}

test('resume-scaffolder renders a full modern HTML resume and escapes input', async ({ page }) => {
  await page.goto('/tools/resume-scaffolder/');
  await fillData(
    page,
    JSON.stringify({
      name: 'Ada <script>alert(1)</script> Lovelace',
      title: 'Software Engineer',
      email: 'ada@example.com',
      summary: 'Builds safe HTML & print CSS.',
      experience: [{ role: 'Engineer', company: 'Analytical Co', bullets: ['Wrote <loops>'] }],
      skills: ['Algorithms', 'C++ & Rust'],
    }),
  );
  await page.selectOption('#in-theme', 'modern');
  await page.fill('#in-accent', '#22c55e'); // long hex color form
  await page.selectOption('#in-font', 'sans');
  await page.selectOption('#in-page_size', 'letter');

  const out = await output(page);
  await expect(out).toContainText('<main class="resume theme-modern">');
  await expect(out).toContainText('--accent: #22c55e;');
  await expect(out).toContainText('@page { size: letter;');
  await expect(out).toContainText('&lt;script&gt;alert(1)&lt;/script&gt;');
  await expect(out).not.toContainText('<script>alert(1)</script>');
  await expect(out).toContainText('C++ &amp; Rust');
  await expect(out).toContainText('<h2>Experience</h2>');
});

test('resume-scaffolder deep-links data and renders classic A4 serif with named accent', async ({ page }) => {
  const qs = new URLSearchParams({
    data: JSON.stringify({ name: 'Grace Hopper', skills: ['Compilers'] }),
    theme: 'classic',
    accent: 'navy',
    font: 'serif',
    page_size: 'a4',
  });
  await page.goto(`/tools/resume-scaffolder/?${qs.toString()}`);

  await expect(page.locator('#in-data')).toHaveValue(/Grace Hopper/);
  await expect(page.locator('#in-theme')).toHaveValue('classic');
  await expect(page.locator('#in-accent')).toHaveValue('navy');
  await expect(page.locator('#in-font')).toHaveValue('serif');
  await expect(page.locator('#in-page_size')).toHaveValue('a4');

  const out = await output(page);
  await expect(out).toContainText('<title>Grace Hopper — Résumé</title>');
  await expect(out).toContainText('theme-classic');
  await expect(out).toContainText('--accent: navy;');
  await expect(out).toContainText('@page { size: A4;');
  await expect(out).toContainText('Georgia, Cambria');
});

test('resume-scaffolder covers compact theme, short hex color, and serif/font enums', async ({ page }) => {
  await page.goto('/tools/resume-scaffolder/');
  await fillData(page, JSON.stringify({ name: 'Lin Chen', sections: [{ heading: 'Awards', items: ['Dean list'] }] }));
  await page.selectOption('#in-theme', 'compact');
  await page.fill('#in-accent', '#0af'); // short hex color form
  await page.selectOption('#in-font', 'serif');
  await page.selectOption('#in-page_size', 'letter');

  const out = await output(page);
  await expect(out).toContainText('theme-compact');
  await expect(out).toContainText('--accent: #0af;');
  await expect(out).toContainText('Georgia, Cambria');
  await expect(out).toContainText('<h2>Awards</h2>');
});

test('resume-scaffolder reports useful validation errors', async ({ page }) => {
  await page.goto('/tools/resume-scaffolder/');
  await fillData(page, JSON.stringify({ email: 'missing@example.com' }));
  await expect(page.locator('#tool-output')).toContainText("a 'name' field is required", { timeout: 15_000 });

  await fillData(page, '{not json');
  await expect(page.locator('#tool-output')).toContainText('invalid JSON', { timeout: 15_000 });

  await fillData(page, JSON.stringify({ name: 'Ada' }));
  await page.fill('#in-accent', 'red; } body { display:none');
  await expect(page.locator('#tool-output')).toContainText('invalid accent color', { timeout: 15_000 });
});
