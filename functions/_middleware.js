// Pages Functions middleware, two jobs:
//
// 1. 301-redirect the Cloudflare project subdomain (gizza-ai.pages.dev) to
//    the canonical apex domain. The mirror serves the exact same deployment
//    as gizza.ai, so without this it can be crawled, indexed, or cited by AI
//    agents in place of the real domain — the pages already carry
//    <link rel="canonical" href="https://gizza.ai/..."> but that is only a
//    hint, and this makes it a hard redirect. Only the production subdomain
//    is matched; preview deploys (<hash>.gizza-ai.pages.dev) and the real
//    domain fall through untouched, so previews stay usable. Path and query
//    string are preserved.
//
// 2. 404 unknown /tools/ pages. The deployment ships no 404.html, so
//    Cloudflare Pages serves the SPA fallback (the homepage shell) with a
//    200 for any unmatched path — and /tools/ requests always take that
//    route because the service worker deliberately bypasses them (sw.js).
//    That soft-404 lets crawlers index arbitrary /tools/<garbage>/ URLs.
//    The guard checks the requested slug against the generated manifests
//    (`/tools/_index.json`, `_hubs.json`, `_pairs.json` — written by
//    tools/generator next to the pages themselves, so they cannot drift
//    from what is actually deployed) and answers a real 404 for unknown
//    slugs. Decision logic lives in js/tools-guard.js (covered by
//    `node --test`); manifests are fetched once per isolate and cached.

import { buildManifestSets, classifyToolsPath } from "../js/tools-guard.js";

let manifestSetsPromise = null;

async function loadManifestSets(context, origin) {
  const fetchJson = async (path) => {
    try {
      const res = await context.env.ASSETS.fetch(new URL(path, origin));
      if (!res.ok) return [];
      return await res.json();
    } catch {
      return []; // fail open — classifyToolsPath treats empty tools as "pass all"
    }
  };
  const [tools, hubs, pairs] = await Promise.all([
    fetchJson("/tools/_index.json"),
    fetchJson("/tools/_hubs.json"),
    fetchJson("/tools/_pairs.json"),
  ]);
  return buildManifestSets({ tools, hubs, pairs });
}

const NOT_FOUND_HTML = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="robots" content="noindex">
<title>Tool not found — gizza.ai</title>
<style>
  body{font-family:system-ui,sans-serif;display:grid;place-items:center;min-height:100vh;margin:0;background:#fafafa;color:#111}
  main{text-align:center;padding:2rem}
  h1{font-size:1.4rem;margin-bottom:.5rem}
  a{color:#2563eb}
  @media (prefers-color-scheme:dark){body{background:#111;color:#eee}a{color:#7ab7ff}}
</style>
</head>
<body>
<main>
  <h1>404 — no such tool</h1>
  <p>This tool doesn't exist (or isn't built yet).</p>
  <p><a href="/tools/">Browse all tools</a></p>
</main>
</body>
</html>
`;

export async function onRequest(context) {
  const url = new URL(context.request.url);
  if (url.hostname === "gizza-ai.pages.dev") {
    url.protocol = "https:";
    url.hostname = "gizza.ai";
    url.port = "";
    return Response.redirect(url.toString(), 301);
  }

  const method = context.request.method;
  if ((method === "GET" || method === "HEAD") && url.pathname.startsWith("/tools/")) {
    if (!manifestSetsPromise) {
      manifestSetsPromise = loadManifestSets(context, url.origin);
    }
    const manifests = await manifestSetsPromise;
    if (classifyToolsPath(url.pathname, manifests) === "not-found") {
      return new Response(method === "HEAD" ? null : NOT_FOUND_HTML, {
        status: 404,
        headers: { "content-type": "text/html; charset=utf-8" },
      });
    }
  }

  return context.next();
}
