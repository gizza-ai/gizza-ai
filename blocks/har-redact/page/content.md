## Share HAR captures without leaking session secrets

Browser HAR files are useful for debugging failed requests because they preserve URLs, methods, status codes, timings, headers, cookies, and response metadata. They can also contain live session cookies, bearer tokens, API keys, OAuth codes, and copied response bodies. Paste a HAR here to replace those sensitive **values** with a placeholder while keeping the capture's shape intact, so the redacted file still opens in HAR viewers and remains useful for support or bug reports.

### Worked example

Paste a HAR exported from DevTools after reproducing a login issue. Leave the default options on to redact cookie values, Authorization/API-key headers, sensitive query parameters such as `token` or `client_secret`, and response bodies. The output keeps request URLs, paths, status codes, timings, and header names, but turns secrets such as cookie strings, bearer tokens, and `?token=...` values into `[REDACTED]`. Choose **Summary** first if you want a dry-run count before copying the sanitized HAR.

### What is redacted

- Request and response `cookies[].value`, plus `Cookie` and `Set-Cookie` header values.
- Authorization, Proxy-Authorization, and common API-key/token header values.
- Sensitive query-string parameter values in both `request.queryString[]` and `request.url`.
- Response bodies by default; request bodies too when **Redact bodies** is set to `Both` or `Request bodies only`.
- Extra header names or parameter names you provide as comma-separated lists.

### Limits and edge cases

The input must be a JSON HAR object with `log.entries`. Up to 10,000 entries are processed per run. Values are substituted in place rather than removed, so the output remains valid JSON/HAR; however, no automated redactor can know every private field name in a custom API. Add project-specific header or parameter names when your capture contains organization-specific secrets.

## FAQ

<details>
<summary>Does this remove requests from the HAR?</summary>

No. It keeps the waterfall structure, request and response objects, URLs, timings, status codes, and names of headers/cookies/parameters. Only selected sensitive values are replaced with the placeholder.

</details>

<details>
<summary>Should I redact request bodies too?</summary>

If a request body can contain passwords, form fields, tokens, or customer data, choose `Both` or `Request bodies only`. The default redacts response bodies because API responses commonly echo session data, while preserving request bodies unless you opt in.

</details>

<details>
<summary>Can I add my own secret field names?</summary>

Yes. Use **Extra header names** for headers such as `x-tenant-secret`, and **Extra param names** for query or form parameter names such as `account_id` or `tenant`. Names are matched case-insensitively.

</details>

<details>
<summary>Is this a guarantee that the HAR is safe to publish publicly?</summary>

No automated redactor can guarantee that. Review the output before publishing, especially URLs, path segments, custom headers, and JSON bodies you chose not to redact. This tool is meant to reduce common HAR leaks before private sharing and review.

</details>
