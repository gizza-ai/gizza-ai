## What this tool does

Expand a **CIDR block** or a **start–end IP range** into the full list of
addresses it covers — or just count them. Works for both IPv4 and IPv6, runs
entirely in your browser (nothing is sent to a server), works offline, and needs
no sign-up. Paste a range, pick **Output**, and copy the result.

## Accepted input

| Form | Example | Expands to |
| --- | --- | --- |
| **CIDR** | `192.168.1.0/24` | `192.168.1.0` … `192.168.1.255` |
| **CIDR (IPv6)** | `2001:db8::/126` | `2001:db8::` … `2001:db8::3` |
| **Range** | `192.168.1.10-192.168.1.20` | `192.168.1.10` … `192.168.1.20` |
| **Range (IPv6)** | `2001:db8::1-2001:db8::5` | `2001:db8::1` … `2001:db8::5` |
| **Range (short end)** | `192.168.1.10-12` | `192.168.1.10`, `.11`, `.12` |

A CIDR expands the **whole block**, including the network and broadcast
addresses. A non-aligned base is normalized to its network address first, so
`192.168.1.130/29` expands the `192.168.1.128`–`192.168.1.135` block.

## Output

- **List** — every address in the range, one per line.
- **Count** — just the total number of addresses. This is exact and unbounded,
  so it works even for an IPv6 `/64` (18 446 744 073 709 551 616 addresses) or a
  whole `/0`.

## Max addresses to list

To keep things fast, **List** mode stops if a range has more than the limit you
set (default **65536**, which is a `/16`). If your range is bigger, the tool
tells you exactly how many addresses it has — raise the limit, narrow the range,
or switch the Output to **Count**.

## Examples

| Input | Output | Result |
| --- | --- | --- |
| `10.0.0.0/30` | List | `10.0.0.0`, `10.0.0.1`, `10.0.0.2`, `10.0.0.3` |
| `192.168.1.0/24` | Count | `256` |
| `10.0.0.1-10.0.0.5` | List | `10.0.0.1` … `10.0.0.5` |
| `2001:db8::/64` | Count | `18446744073709551616` |

## FAQ

<details>
<summary>Does a CIDR include the network and broadcast addresses?</summary>

Yes — this tool
lists the entire block. `10.0.0.0/30` gives all four addresses, including
`10.0.0.0` (network) and `10.0.0.3` (broadcast).

</details>

<details>
<summary>Does it support IPv6?</summary>

Yes. Give an IPv6 CIDR (`2001:db8::/126`) or an IPv6
range (`2001:db8::1-2001:db8::5`). Because IPv6 blocks can be astronomically
large, use **Count** for anything wider than a small prefix.

</details>

<details>
<summary>What if my range is huge?</summary>

**List** mode caps the number of addresses (so
your browser doesn't try to render millions of lines). Use **Count** for the
exact total of any size, or raise the limit for a slightly larger list.

</details>

<details>
<summary>Is it free and private?</summary>

Yes — your input never leaves your device, and the
page keeps working offline once it has loaded.

</details>
