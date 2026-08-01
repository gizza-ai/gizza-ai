import { test, expect } from './fixtures';

const sql = 'CREATE TABLE users (\n  id INT PRIMARY KEY AUTO_INCREMENT,\n  email VARCHAR(255) NOT NULL UNIQUE\n);';

test('sql-schema-extractor page emits exact JSON model', async ({ page }) => {
  await page.goto('/tools/sql-schema-extractor/');
  await page.fill('#in-sql', sql);
  await page.selectOption('#in-output', 'json');
  await page.selectOption('#in-dialect', 'mysql');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"table_count": 1', { timeout: 15_000 });
  expect(await out.textContent()).toBe(`{
  "dialect": "mysql",
  "table_count": 1,
  "column_count": 2,
  "tables": [
    {
      "name": "users",
      "columns": [
        {
          "name": "id",
          "type": "INT",
          "nullable": false,
          "primary_key": true,
          "auto_increment": true
        },
        {
          "name": "email",
          "type": "VARCHAR(255)",
          "nullable": false,
          "unique": true
        }
      ],
      "primary_key": [
        "id"
      ],
      "foreign_keys": [],
      "unique_constraints": [],
      "checks": [],
      "indexes": []
    }
  ]
}`);
});

test('sql-schema-extractor deep link renders markdown and applies alter/index toggles', async ({ page }) => {
  const deepSql = 'CREATE TABLE orders (id INT PRIMARY KEY, user_id INT);\nALTER TABLE orders ADD CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES users(id);\nCREATE INDEX idx_orders_user ON orders(user_id);';
  const qs =
    '?sql=' + encodeURIComponent(deepSql) +
    '&output=markdown' +
    '&dialect=postgres' +
    '&apply_alter=true' +
    '&include_indexes=false';
  await page.goto('/tools/sql-schema-extractor/' + qs);

  await expect(page.locator('#in-output')).toHaveValue('markdown', { timeout: 15_000 });
  await expect(page.locator('#in-dialect')).toHaveValue('postgres');
  await expect(page.locator('#in-apply_alter')).toBeChecked();
  await expect(page.locator('#in-include_indexes')).not.toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('## orders', { timeout: 15_000 });
  const text = await out.textContent();
  expect(text).toContain('**Foreign key:** `fk_user` (user_id) → users (id)');
  expect(text).not.toContain('idx_orders_user');
});
