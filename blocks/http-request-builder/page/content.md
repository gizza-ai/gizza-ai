## About this tool

**HTTP Request Builder** composes a well-formed **raw HTTP/1.1 request message**
from a method, URL, headers, and body — the exact bytes you'd put on the wire.
**Nothing is sent**; you get the text to copy.

- The **request-target** (path + query) and the **`Host`** header are derived
  from the URL.
- **Headers** go one `Name: Value` per line; if you don't include `Host`, it's
  added for you.
- Provide a **body** and a `Content-Length` header is added automatically.
- Lines are **CRLF**-terminated per the HTTP spec.

Everything runs **locally in your browser** via WebAssembly — nothing is
uploaded and no request is made.

### Handy for

- Pasting into `nc` / `openssl s_client` to send a request by hand.
- Teaching or learning how HTTP requests are framed.
- Building a request fixture for tests or documentation.

> To actually **send** a request and see the response, use the **HTTP Request**
> tool instead.

## FAQ

<details>
<summary>Does this actually fire the request at the server?</summary>

No — it only assembles the raw HTTP/1.1 message text; no network call is ever
made. Copy the output into `nc`, `openssl s_client`, or a test fixture to use
it. If you want to send a request and inspect the response, that's the
separate HTTP Request tool.

</details>

<details>
<summary>What if I add my own Host or Content-Length header?</summary>

Yours wins. The tool only injects `Host` (from the URL) when your header lines
don't already include one, and only adds `Content-Length` when you provided a
body without one — it never emits a duplicate or overrides your value.

</details>

<details>
<summary>How is Content-Length computed for the body?</summary>

It's the body's size in bytes (UTF-8), not characters — so `{"a":1}` yields
`Content-Length: 7`, and multi-byte characters count as more than one. It's
added only when the body field is non-empty.

</details>

<details>
<summary>When does the Host header include a port?</summary>

Only when the URL carries an explicit non-default port — `http://localhost:8080/`
produces `Host: localhost:8080`, while `https://example.com/` gives just
`Host: example.com` because the scheme's default port is implied.

</details>
