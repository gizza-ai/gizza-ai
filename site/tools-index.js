// tools-index.js — pure search core shared by tools-modal.js and header.js.
// Importable under node:test (no DOM access).

/** Pure: substring match (case-insensitive) over title + description + slug + tags.
 * Slug is included so a tool's common name (e.g. "clock") finds it even when the
 * title doesn't contain it ("Current UTC Time"); tags are extra search keywords
 * (not displayed) authored in each tool's meta.toml. */
export function filterTools(list, query) {
  const q = query.trim().toLowerCase();
  if (!q) return list;
  return list.filter((t) => {
    const hay = [t.title, t.description, t.slug, ...(t.tags || [])].join(' ').toLowerCase();
    return hay.includes(q);
  });
}
