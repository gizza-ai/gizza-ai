import { test, expect } from './fixtures';

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

const posting = [
  'Senior Backend Engineer',
  'Company: Acme Analytics',
  'Location: Remote - US / Toronto',
  'Compensation: $150,000 - $185,000 USD',
  'Full-time',
  'We need Rust, Python, PostgreSQL, Docker, Kubernetes, AWS, GraphQL and CI/CD experience.',
].join('\n');

test('job-posting-parser page extracts JSON fields from a pasted posting', async ({ page }) => {
  await page.goto('/tools/job-posting-parser/');
  await page.fill('#in-posting', posting);

  await expect(page.locator('#tool-output')).toContainText('Senior Backend Engineer', { timeout: 15_000 });
  const parsed = JSON.parse(await output(page));
  expect(parsed.title).toBe('Senior Backend Engineer');
  expect(parsed.company).toBe('Acme Analytics');
  expect(parsed.salary).toBe('$150,000 - $185,000 USD');
  expect(parsed.work_mode).toBe('remote');
  expect(parsed.skills).toEqual(expect.arrayContaining(['Rust', 'Python', 'PostgreSQL', 'Kubernetes']));
});

test('job-posting-parser deep link supports markdown and evidence off', async ({ page }) => {
  const qs = new URLSearchParams({
    posting,
    output: 'markdown',
    include_evidence: 'false',
  });
  await page.goto(`/tools/job-posting-parser/?${qs.toString()}`);

  await expect(page.locator('#in-output')).toHaveValue('markdown', { timeout: 15_000 });
  await expect(page.locator('#in-include_evidence')).not.toBeChecked();
  const text = await output(page);
  expect(text).toContain('## Parsed job posting');
  expect(text).toContain('**Company:** Acme Analytics');
  expect(text).not.toContain('### Evidence');
});

test('job-posting-parser reports low-signal text', async ({ page }) => {
  await page.goto('/tools/job-posting-parser/');
  await page.fill('#in-posting', 'hello world');

  await expect(page.locator('#tool-output')).toContainText('does not look like a job ad', { timeout: 15_000 });
});
