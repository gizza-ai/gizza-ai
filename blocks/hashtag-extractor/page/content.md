## About this tool

Most hashtag tools do one of two jobs. Extractors pull out the `#tags` a post already contains. Generators turn prose into new tags. This one does both in a single pass: the hashtags you already wrote are kept verbatim and listed first, then the text's strongest keywords are scored and formatted as tags behind them.

Ranking is deliberately simple enough to explain. A candidate's score is how often it occurs, multiplied by an earliness bonus (a word in the first sentence outranks the same word buried at the end), multiplied by how many words the phrase has. So the order you get is relevance order, not alphabetical order and not raw frequency.

Everything runs in your browser. No account, no daily quota, and the text never leaves your machine.

### Worked example

Paste this caption:

```
Remote work is changing how teams build software. Async communication and clear documentation keep remote teams shipping fast.
```

With the defaults (10 tags, no platform cap, lowercase, one word per tag, minimum word length 3), the result is:

```
#remote #teams #work #changing #build #software #async #communication #clear #documentation

10 hashtags · 91 characters · 13 candidates found
```

`#remote` and `#teams` lead because each appears twice; `#work` follows because it appears early. The footer counts characters as well as tags, because platforms budget captions by character.

Now set **Platform cap** to Instagram, **Casing** to PascalCase, **Words per hashtag** to 2 and **Maximum hashtags** to 0, and the same text gives a caption-ready set instead:

```
#Remote #RemoteWork #Teams #TeamsBuild #BuildSoftware

5 hashtags · 53 characters · 13 candidates found
```

Multi-word tags need capitals: `#RemoteWork` is announced as two words by a screen reader, `#remotework` is not.

### Limits and edge cases

- **Maximum hashtags** accepts 0–100; 0 means no limit of your own. If you also pick a platform, the tighter of the two numbers wins.
- **Platform caps** are 2026 usage recommendations — Instagram 5, TikTok 5, LinkedIn 5, Facebook 3, X 2 — not the hard maxima the networks technically accept. They reflect the shift away from 30-tag blocks toward a handful of relevant tags.
- **Words per hashtag** goes up to 4. When a longer phrase fully contains a shorter candidate and both occur equally often, the shorter one is dropped, so you do not get `#content`, `#marketing` and `#contentmarketing` all at once.
- **Minimum word length** goes up to 20 characters. Numbers-only tokens are always dropped, because a digits-only hashtag is just a number to the platforms.
- The stop-word list is **English only**. Tokenisation is Unicode-aware, so Greek, Cyrillic, or CJK text still produces tags — but common words in those languages are not filtered out.
- Duplicates are removed case-insensitively, so a written `#Remote` and a generated `#remote` collapse into one tag.
- There is no trend, volume or reach data here. Scoring is based on your text alone; nothing is fetched from any social network.

## FAQ

<details>
<summary>Does it keep the hashtags I already wrote, or only generate new ones?</summary>

Both, by default. Tags already present in the text are emitted exactly as you typed them — casing included — and listed first, ahead of the generated ones. Turn off **Keep hashtags already in the text** if you only want fresh suggestions. Note that the words inside those tags are still ordinary words in the text, so a prominent `#DevLog` may come back as a generated `#devlog`.

</details>

<details>
<summary>How many hashtags should I actually use?</summary>

Fewer than the old advice suggested. The platform presets here use 5 for Instagram, TikTok and LinkedIn, 3 for Facebook and 2 for X. Those are current usage recommendations rather than hard limits — the networks accept more, but large blocks of loosely related tags no longer help reach. Set **Platform cap** to "No platform cap" and use **Maximum hashtags** if you want your own number.

</details>

<details>
<summary>Why is a word from my text missing?</summary>

Three filters run before scoring: English stop words (the, and, with, …) are dropped, words shorter than **Minimum word length** are dropped, and digits-only tokens are dropped. After that, only the top-scoring candidates survive the tag limit — the footer tells you how many candidates were found in total, so you can raise **Maximum hashtags** to see more.

</details>

<details>
<summary>Which casing should I choose for multi-word tags?</summary>

PascalCase. `#RemoteWork` is read aloud as two words by screen readers, while `#remotework` is read as one run of letters. Platforms treat hashtags case-insensitively, so capitals cost you nothing in matching. Use "Preserve" when the text already contains the exact brand spelling you want to keep, such as an all-caps product name.

</details>

<details>
<summary>Can I use it on a non-English caption?</summary>

Yes, with one caveat. Words are split using Unicode rules, so Greek, Cyrillic, Arabic and CJK text tokenises and produces hashtags correctly. The built-in stop-word list is English only, so common words in other languages will not be filtered automatically. Raising **Minimum word length** is a reasonable workaround, since most function words are short.

</details>

<details>
<summary>Does it show search volume or trending hashtags?</summary>

No. Ranking comes entirely from the text you paste — occurrence count, position and phrase length. Trend and volume data would require a live connection to each social network, an account and an API key. This page makes no network requests at all, which is also why it works offline and why your draft copy stays on your machine.

</details>
