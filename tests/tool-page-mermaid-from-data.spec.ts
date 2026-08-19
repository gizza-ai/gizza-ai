import { test, expect } from './fixtures';

const tool = '/tools/mermaid-from-data/';

async function outputText(page): Promise<string> {
  const text = await page.locator('#tool-output').textContent();
  return (text ?? '').trim();
}

test('mermaid-from-data page generates exact flowchart source', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-edges', 'Start -> Build : implement\nBuild -> Ship : deploy');
  await page.selectOption('#in-direction', 'LR');
  await page.fill('#in-title', 'Release pipeline');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('title: Release pipeline', { timeout: 15000 });
  await expect(out).toContainText('flowchart LR');
  await expect(out).toContainText('Start -->|"implement"| Build');
  await expect(out).toContainText('Build -->|"deploy"| Ship');
});

test('mermaid-from-data page supports class diagrams and relationship styles', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-edges', 'Animal,Dog,,inheritance\nDog,Collar,wears,composition');
  await page.fill('#in-nodes', 'Animal | +String name; +eat()\nDog');
  await page.selectOption('#in-diagram', 'class');
  await page.selectOption('#in-direction', 'LR');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('classDiagram', { timeout: 15000 });
  await expect(out).toContainText('direction LR');
  await expect(out).toContainText('Animal <|-- Dog');
  await expect(out).toContainText('Dog *-- Collar : wears');
});

test('mermaid-from-data page supports ER diagrams', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-edges', 'CUSTOMER,ORDER,places,one-to-many');
  await page.fill('#in-nodes', 'CUSTOMER | string name PK; string email');
  await page.selectOption('#in-diagram', 'er');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('erDiagram', { timeout: 15000 });
  await expect(out).toContainText('CUSTOMER {');
  await expect(out).toContainText('CUSTOMER ||--o{ ORDER : "places"');
});

test('mermaid-from-data query-param deep-link prefills chain mode and fence output', async ({ page }) => {
  await page.goto(
    tool +
      '?edges=' +
      encodeURIComponent('Draft,Review,Publish') +
      '&row_mode=chain&delimiter=comma&node_shape=round&edge_style=thick&fence=true',
  );

  await expect(page.locator('#in-edges')).toHaveValue('Draft,Review,Publish', { timeout: 15000 });
  await expect(page.locator('#in-row_mode')).toHaveValue('chain');
  await expect(page.locator('#in-fence')).toBeChecked();
  const out = await outputText(page);
  expect(out).toContain('```mermaid');
  expect(out).toContain('Draft ==> Review');
  expect(out).toContain('Review ==> Publish');
});
