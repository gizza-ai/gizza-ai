// Cloudflare Pages Function: rewrite tool subdomains to /tools/<sub>/ and serve
// the corresponding static asset. apex/www serve the app unchanged.
import { resolve } from "./routing.mjs";

export async function onRequest(context) {
  const { request, next } = context;
  const url = new URL(request.url);
  const decision = resolve(url.hostname, url.pathname);

  if (decision.type === "redirect") {
    return Response.redirect(decision.location, 302);
  }

  if (decision.type === "tool") {
    const rewritten = new URL(request.url);
    rewritten.pathname = decision.path;
    // Serve the static asset at the rewritten path. Force GET (and drop any
    // body): the ASSETS binding only serves GET/HEAD, so forwarding the original
    // method/body for a stray POST would 405. Preserve headers for caching.
    return context.env.ASSETS.fetch(
      new Request(rewritten.toString(), { method: "GET", headers: request.headers }),
    );
  }

  // app: continue to normal static asset serving.
  return next();
}
