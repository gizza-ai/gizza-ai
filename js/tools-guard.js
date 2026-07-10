// Decide whether a /tools/ request maps to a page this deployment actually
// ships, so the Pages middleware can 404 unknown slugs instead of letting
// Cloudflare's SPA fallback serve the homepage shell with a 200 (soft-404 —
// crawlers index garbage tool URLs).
//
// Pure module: the middleware feeds it the generated manifests
// (`pkg/tools/_index.json`, `_hubs.json`, `_pairs.json` — written by
// tools/generator alongside the pages themselves, so they cannot drift from
// what was deployed) and a pathname; it answers 'pass' or 'not-found'.
// Kept free of Workers APIs so `node --test` covers it directly.

/**
 * Build fast lookup sets from the three parsed manifest arrays.
 * Missing/failed manifests should be passed as empty arrays.
 */
export function buildManifestSets({ tools = [], hubs = [], pairs = [] }) {
  return {
    tools: new Set(tools.map((t) => t.slug)),
    hubs: new Set(hubs.map((h) => h.slug)),
    pairs: new Set(pairs.map((p) => p.path)),
  };
}

/**
 * Classify a request pathname against the deployed tool/hub/pair manifests.
 *
 * Returns 'pass' (let the asset server handle it) or 'not-found' (the
 * middleware should answer 404).
 *
 * Rules:
 * - Only `/tools/…` paths are ever guarded; everything else passes.
 * - `/tools/` and `/tools/index.html` (the hub) pass.
 * - A first segment containing a dot is a file directly under /tools/
 *   (the `_*.json` manifests, feed fragments) — pass.
 * - A first segment found in the tool or hub manifests passes at any depth.
 * - Under a known hub, a second *page* segment must be a known pair path
 *   (`hub/pair`); files (dotted) under a known hub pass.
 * - Anything else is 'not-found'.
 * - Fail open when the tool manifest is empty: a deployment with zero tools
 *   is not a real state, so an empty set means the manifests could not be
 *   loaded — availability wins over strictness.
 */
export function classifyToolsPath(pathname, manifests) {
  if (!pathname.startsWith('/tools/')) return 'pass';
  if (manifests.tools.size === 0) return 'pass';

  let decoded;
  try {
    decoded = decodeURIComponent(pathname);
  } catch {
    return 'not-found';
  }

  const segments = decoded.slice('/tools/'.length).split('/').filter(Boolean);
  if (segments.length === 0) return 'pass'; // /tools/ itself

  const [first, second] = segments;
  if (first === 'index.html') return 'pass';
  if (first.includes('.')) return 'pass'; // _index.json etc.

  if (manifests.tools.has(first)) return 'pass';
  if (manifests.hubs.has(first)) {
    if (segments.length === 1) return 'pass';
    if (second.includes('.')) return 'pass'; // file under a hub page
    return manifests.pairs.has(`${first}/${second}`) ? 'pass' : 'not-found';
  }
  return 'not-found';
}
