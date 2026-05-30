// Pure host→path mapping for gizza.ai. Unit-tested independently of Cloudflare.
// apex/www/localhost/pages.dev → main app; <sub>.gizza.ai → /tools/<sub>/...

const APEX = "gizza.ai";

/**
 * @param {string} host - request Host header (may include :port)
 * @param {string} pathname - request path (starts with "/")
 * @returns {{type:"app"|"tool"|"redirect", path:string, location?:string}}
 */
export function resolve(host, pathname) {
  const h = (host || "").split(":")[0].toLowerCase();

  // Non-production hosts: always the app.
  if (h === "localhost" || h === "127.0.0.1" || h.endsWith(".pages.dev")) {
    return { type: "app", path: pathname };
  }

  if (h === APEX || h === `www.${APEX}`) {
    return { type: "app", path: pathname };
  }

  if (h.endsWith(`.${APEX}`)) {
    const sub = h.slice(0, -1 * `.${APEX}`.length);
    // Single-label subdomains only (calculator, clock).
    if (sub && !sub.includes(".")) {
      const tail = pathname === "/" ? "/index.html" : pathname;
      return { type: "tool", path: `/tools/${sub}${tail}` };
    }
  }

  // Unknown host → send to apex.
  return { type: "redirect", path: pathname, location: `https://${APEX}/` };
}
