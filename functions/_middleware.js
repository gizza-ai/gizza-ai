// Pages Functions middleware: 301-redirect the Cloudflare project subdomain
// (gizza-ai.pages.dev) to the canonical apex domain. The mirror serves the
// exact same deployment as gizza.ai, so without this it can be crawled,
// indexed, or cited by AI agents in place of the real domain — the pages
// already carry <link rel="canonical" href="https://gizza.ai/..."> but that
// is only a hint, and this makes it a hard redirect.
//
// Only the production subdomain is matched. Preview deploys
// (<hash>.gizza-ai.pages.dev) and the real domain fall through untouched, so
// previews stay usable. Path and query string are preserved.
//
// Runs on every request; on the production domain it is a single hostname
// comparison then next(), so the cost is negligible.
export async function onRequest(context) {
  const url = new URL(context.request.url);
  if (url.hostname === "gizza-ai.pages.dev") {
    url.protocol = "https:";
    url.hostname = "gizza.ai";
    url.port = "";
    return Response.redirect(url.toString(), 301);
  }
  return context.next();
}
