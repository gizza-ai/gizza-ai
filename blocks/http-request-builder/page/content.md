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
