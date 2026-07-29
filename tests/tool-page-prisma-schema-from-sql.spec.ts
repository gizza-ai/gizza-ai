import { test, expect } from './fixtures';

const relationSql = `CREATE TABLE users (id SERIAL PRIMARY KEY);
CREATE TABLE orders (
  id SERIAL PRIMARY KEY,
  user_id INT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  total DECIMAL(10,2) NOT NULL
);`;

test('prisma-schema-from-sql page emits exact Prisma schema with relation', async ({ page }) => {
  await page.goto('/tools/prisma-schema-from-sql/');
  await page.fill('#in-input', relationSql);
  await page.selectOption('#in-provider', 'postgresql');

  await expect(page.locator('#tool-output')).toHaveText(
    `generator client {
  provider = "prisma-client-js"
}

datasource db {
  provider = "postgresql"
  url      = env("DATABASE_URL")
}

model users {
  id Int @id @default(autoincrement())
}

model orders {
  id Int @id @default(autoincrement())
  user_id Int
  total Decimal @db.Decimal(10, 2)
  user users @relation(fields: [user_id], references: [id], onDelete: Cascade)
}`,
    { timeout: 15000 },
  );
});

test('prisma-schema-from-sql honours deep link provider and naming toggles', async ({ page }) => {
  const sql = 'CREATE TABLE blog_posts (post_id INT PRIMARY KEY AUTO_INCREMENT, full_title VARCHAR(200) NOT NULL, is_active TINYINT(1) DEFAULT 1);';
  const qs =
    '?input=' + encodeURIComponent(sql) +
    '&provider=mysql' +
    '&header=false' +
    '&relations=false' +
    '&native_types=false' +
    '&map_names=true';
  await page.goto('/tools/prisma-schema-from-sql/' + qs);

  await expect(page.locator('#in-provider')).toHaveValue('mysql', { timeout: 15000 });
  await expect(page.locator('#in-header')).not.toBeChecked();
  await expect(page.locator('#in-relations')).not.toBeChecked();
  await expect(page.locator('#in-native_types')).not.toBeChecked();
  await expect(page.locator('#in-map_names')).toBeChecked();

  await expect(page.locator('#tool-output')).toHaveText(
    `model BlogPost {
  postId Int @id @default(autoincrement()) @map("post_id")
  fullTitle String @map("full_title")
  isActive Boolean? @default(true) @map("is_active")

  @@map("blog_posts")
}`,
    { timeout: 15000 },
  );
});
