## What this tool does

Namespaced XML is correct and almost unusable by hand. A SOAP response, a `.docx` part, an
Atom feed or an XSD-validated payload wraps everything in `xmlns` declarations and `ns:`
prefixes, and from then on `//Item` matches nothing, your XML-to-JSON converter produces keys
like `m:Item`, and every XPath expression needs a namespace map you have to build first.

This tool removes the namespace plumbing and leaves the data. It parses the document
properly — it is a real XML reader, not a search-and-replace over angle brackets — deletes the
`xmlns` and `xmlns:prefix` attributes, rewrites `soap:Envelope` to `Envelope` and `m:id` to
`id`, and writes everything else straight back out.

## Worked example

Input:

```xml
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
  <soap:Body>
    <m:GetPrice xmlns:m="https://example.com/prices">
      <m:Item m:id="1">Apples</m:Item>
    </m:GetPrice>
  </soap:Body>
</soap:Envelope>
```

Output, with every default (remove declarations and prefixes, preserve the layout):

```xml
<Envelope>
  <Body>
    <GetPrice>
      <Item id="1">Apples</Item>
    </GetPrice>
  </Body>
</Envelope>
```

`//Item/@id` now matches. Set **Keep these prefixes** to `soap` and the envelope stays
addressable — `<soap:Envelope xmlns:soap="…">` is kept, declaration included, while the payload
inside it is flattened.

## What is preserved

By default the output is your document minus the namespace plumbing and nothing else. Comments,
processing instructions, CDATA sections, the DOCTYPE, the `<?xml …?>` prolog, text nodes and
attribute values (entities and character references intact) all round-trip unchanged, and so
does the document's own indentation. If a document has no namespaces at all, you get it back
byte for byte.

The reserved `xml:` prefix is always kept, so `xml:lang`, `xml:space` and `xml:id` survive. That
prefix is bound by the XML specification itself and is never declared, so the usual
`local-name()` stylesheet quietly destroys those attributes' meaning; this tool does not.

## Attribute name clashes

Dropping prefixes can collapse two different attributes onto one name — `a:id` next to `b:id`,
or `xsi:type` next to a plain `type`. Duplicate attribute names are not well-formed XML, so a
naive strip produces a document that no longer parses. **Keep both** (the default) gives the
first attribute the bare name and renames each later one to `prefix_name`, so `b:id` becomes
`b_id` and nothing is lost. **Keep the first** drops the later one. **Stop and report** refuses
and names the pair.

Element name clashes are different: two elements from different namespaces that share a local
name (a document mixing two vocabularies that both define `title`) become indistinguishable
after a strip. That is inherent to removing namespaces — check the report before flattening a
document where that matters.

## Limits and behaviour to know about

- Maximum input: **5,000,000 bytes**.
- The XML must be **well-formed**. A mismatched or unclosed tag is reported with its byte
  position instead of being half-processed.
- **Pretty** and **Minify** drop whitespace-only text nodes, so they will reflow mixed content
  such as `<p>hello <b>there</b></p>`. Use **Preserve** (the default) when spacing is part of
  the text.
- Stripping namespaces is lossy on purpose. Elements that were distinguished only by their
  namespace become identical, and the result no longer validates against the original schema.
- Everything runs locally in your browser through WebAssembly. Nothing is uploaded, so
  confidential payloads stay on your machine.

## FAQ

<details>
<summary>Why does my XPath stop matching once a document has namespaces?</summary>

Because `//Item` in XPath 1.0 means "an element named `Item` in **no** namespace". Once the
document declares `xmlns:m="https://example.com/prices"`, the element is `Item` *in that
namespace*, which is a different name. Your options are to register a prefix with whatever
library you are using and write `//m:Item`, to write the clumsy
`//*[local-name()='Item']`, or to strip the namespaces first and write `//Item`. This tool is
the third option.

</details>

<details>
<summary>What is the difference between removing prefixes and removing declarations?</summary>

They are separate edits and the **Remove** dropdown exposes all three useful combinations.
*Declarations and prefixes* (the default) does both: `<m:Item xmlns:m="…">` becomes `<Item>`.
*Prefixes only* rewrites the names but leaves the `xmlns` attributes sitting there, now unused
and harmless. *xmlns attributes only* deletes the declarations and keeps the prefixed names —
what you want when you are pasting a fragment into a document that already declares the same
prefixes, and a second declaration would be redundant.

</details>

<details>
<summary>What are the xsi:schemaLocation attributes and why are they removed by default?</summary>

`xsi:schemaLocation` and `xsi:noNamespaceSchemaLocation` map namespace URIs to `.xsd` files so a
validator knows where to find the schema. Once the namespaces are gone they point at namespaces
the document no longer declares, so they are dangling references rather than data — hence the
default. The match is by *resolved namespace*, not by spelling: an attribute whose prefix merely
happens to be called `xsi` but is bound to some other namespace is treated as data and kept.
Untick the box to keep the hints, with their prefixes stripped like any other attribute.

</details>

<details>
<summary>Can I flatten the payload but keep the SOAP envelope?</summary>

Yes — put `soap` in **Keep these prefixes**. Listed prefixes keep both their names and their own
`xmlns` declarations, so the result is still namespace-well-formed and `soap:Body` remains
addressable while everything inside it is plain. The field takes a comma-separated list
(`soap,wsse`), prefixes are case-sensitive as they are in XML, and the literal token `xmlns`
keeps the default (unprefixed) declaration, which has no prefix of its own to name.

</details>

<details>
<summary>How do I see what a strip would remove before I trust it?</summary>

Switch **Output** to *Report*. You get a `metric,value` CSV — the mode used,
`declarations_removed`, `element_prefixes_stripped`, `attribute_prefixes_stripped`,
`schema_references_removed`, `attribute_clashes_resolved` and the input/output byte counts —
followed by a `prefix,namespace_uri` table naming every declaration that was removed and the
namespace URI it pointed at. It is the dry run for a document you cannot re-fetch.

</details>

<details>
<summary>Is this the same as pretty-printing or minifying XML?</summary>

No. This tool changes names and deletes namespace attributes; it does not reformat unless you
ask it to. The **Layout** dropdown is a convenience so you do not need a second pass:
*Preserve* keeps your whitespace exactly, *Pretty* re-indents with the indent width you choose,
and *Minify* collapses the result onto one line. If formatting is all you need, that is a
different job from this one.

</details>
