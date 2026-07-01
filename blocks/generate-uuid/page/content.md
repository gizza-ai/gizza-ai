## About this UUID generator

A UUID (Universally Unique Identifier), also called a GUID, is a 128-bit value
written as 32 hexadecimal digits in the canonical `8-4-4-4-12` form, for example
`f47ac10b-58cc-4372-a567-0e02b2c3d479`. UUIDs let independent systems mint
identifiers that are effectively unique without coordinating with a central
authority. This tool generates them locally in your browser — no value is sent
to a server.

### Which version should I use?

- **v4 (random)** — 122 random bits. The default, and the right choice for most
  identifiers (database keys, request ids, session tokens) when you just need a
  collision-resistant unique value.
- **v7 (time-ordered)** — a 48-bit Unix-millisecond timestamp followed by random
  bits. Because the timestamp comes first, v7 values sort chronologically as
  plain strings, which makes them far friendlier as database primary keys than v4
  (better index locality). Recommended for new systems that want sortable ids.
- **v1 (time + node)** — a gregorian timestamp plus a node identifier. This tool
  randomizes the node (with the multicast bit set) so it never leaks a real MAC
  address.
- **v5 (namespace, SHA-1)** and **v3 (namespace, MD5)** — *deterministic*: the
  same namespace and name always produce the same UUID. Use these to derive a
  stable id from a string (a URL, a filename, a record key). Pick the namespace
  (`dns`, `url`, `oid`, `x500`, or any UUID) and a name. Prefer v5 over v3.
- **nil / max** — the all-zero and all-one sentinel UUIDs from RFC 9562.

### Bulk and formatting

Set **How many** to generate a batch (up to 1000). For v5/v3 you can paste
multiple names separated by commas or newlines to get one deterministic UUID per
name. Formatting toggles let you switch to **uppercase**, drop the **hyphens**
(32-char form), wrap each value in **{braces}** (the Microsoft registry GUID
form), or add the **urn:uuid:** prefix (RFC 4122 URN form).

UUIDs are generated client-side with a cryptographically secure random source;
nothing is uploaded.

## FAQ

<details>
<summary>Should I use v4 or v7 for database primary keys?</summary>

Prefer **v7** for new systems: its leading 48-bit Unix-millisecond timestamp means
values sort chronologically as plain strings, giving much better index locality
than fully random v4 keys. v4 remains the right default when you only need a
collision-resistant opaque identifier and ordering doesn't matter.

</details>

<details>
<summary>Why do v5 and v3 always give me the same UUID?</summary>

That's by design — they hash a namespace plus a name (SHA-1 for v5, MD5 for v3),
so the same inputs always yield the same UUID. Use them to derive a stable id from
a string. Pick `dns`, `url`, `oid`, `x500`, or any UUID as the namespace, and
paste several names separated by commas or newlines to get one UUID per name.

</details>

<details>
<summary>Does generating a v1 UUID expose my MAC address?</summary>

No. Classic v1 embeds a node identifier that historically was the machine's MAC
address, but this tool always randomizes the node and sets the multicast bit (the
RFC-sanctioned way to mark a random node), so no hardware address ever appears in
the output.

</details>

<details>
<summary>Is there a limit on how many UUIDs I can generate?</summary>

Yes — 1 to 1000 per run; a count outside that range is rejected with an error.
Formatting options (uppercase, no hyphens, {braces}, urn:uuid: prefix) apply to
the whole batch, and everything is generated locally with a cryptographically
secure RNG.

</details>
