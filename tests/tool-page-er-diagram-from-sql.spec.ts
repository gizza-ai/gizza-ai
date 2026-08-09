import { test, expect } from './fixtures';

const relationSql = `CREATE TABLE users (
  id INT PRIMARY KEY,
  email VARCHAR(255) NOT NULL UNIQUE
);
CREATE TABLE orders (
  id INT PRIMARY KEY,
  user_id INT NOT NULL REFERENCES users(id),
  total DECIMAL(10,2)
);`;

const relationOutput = `erDiagram
    users {
        INT id PK
        VARCHAR(255) email UK
    }
    orders {
        INT id PK
        INT user_id FK
        DECIMAL(10_2) total
    }
    users ||--o{ orders : "user_id"`;

test('er-diagram-from-sql page emits exact Mermaid ER source', async ({ page }) => {
  await page.goto('/tools/er-diagram-from-sql/');
  await page.fill('#in-sql', relationSql);
  await page.selectOption('#in-dialect', 'mysql');
  await page.selectOption('#in-attributes', 'all');

  await expect(page.locator('#tool-output')).toHaveText(relationOutput, { timeout: 15_000 });
});

test('er-diagram-from-sql honours deep-link options for inference, direction and fences', async ({ page }) => {
  const sql = `CREATE TABLE companies (id INT PRIMARY KEY);
CREATE TABLE employees (id INT PRIMARY KEY, company_id INT NOT NULL);`;
  const qs =
    '?sql=' + encodeURIComponent(sql) +
    '&dialect=generic' +
    '&attributes=keys' +
    '&key_markers=true' +
    '&mark_nullable=true' +
    '&infer_relations=true' +
    '&relationship_label=none' +
    '&direction=LR' +
    '&fence=true';
  await page.goto('/tools/er-diagram-from-sql/' + qs);

  await expect(page.locator('#in-dialect')).toHaveValue('generic', { timeout: 15_000 });
  await expect(page.locator('#in-attributes')).toHaveValue('keys');
  await expect(page.locator('#in-infer_relations')).toBeChecked();
  await expect(page.locator('#in-mark_nullable')).toBeChecked();
  await expect(page.locator('#in-relationship_label')).toHaveValue('none');
  await expect(page.locator('#in-direction')).toHaveValue('LR');
  await expect(page.locator('#in-fence')).toBeChecked();

  await expect(page.locator('#tool-output')).toHaveText(
    `\`\`\`mermaid
erDiagram
    direction LR
    companies {
        INT id PK
    }
    employees {
        INT id PK
        INT company_id FK
    }
    companies ||--o{ employees : ""
\`\`\``,
    { timeout: 15_000 },
  );
});
