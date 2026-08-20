## About this tool

`stack-trace-analyzer` turns a pasted crash log into a readable map of what failed. It understands common stack trace formats from Java/Kotlin/Scala, Python, JavaScript/TypeScript/Node, Go, Ruby, C#/.NET, Rust, and PHP, then reports the detected language, the reported exception, the root cause at the end of the exception chain, and the first frame that looks like your own code.

Use it when a trace is long, nested, or mixed with framework calls. The analyzer normalizes frame order so the throw or panic site appears first by default, marks each frame as **user** or **framework**, and can hide framework frames entirely when you only want the code paths you control. If automatic classification is too broad, add comma-separated **Your code prefixes** such as `com.example`, `/app/src`, or `my_service`.

### Worked example

Input:

```text
Exception in thread "main" com.example.SvcException: could not start
    at com.example.App.start(App.java:42)
    at org.springframework.boot.SpringApplication.run(SpringApplication.java:301)
Caused by: java.net.ConnectException: Connection refused
    at java.base/sun.nio.ch.Net.pollConnect(Native Method)
    at com.example.Db.connect(Db.java:17)
```

Default report highlights:

```text
Language: Java / Kotlin / Scala (auto-detected)
Reported: com.example.SvcException: could not start
Root cause: java.net.ConnectException: Connection refused
First user frame: com.example.Db.connect(Db.java:17)
```

Set **Output** to `table` for a Markdown frame table or `json` when you want a structured exception chain for another script. Enable **Hide framework frames** to drop standard-library, package-manager, runtime, and framework frames from the displayed output.

### Options and limits

- **Language** defaults to `auto`. Force a language for short or truncated traces.
- **Output** chooses readable report, Markdown table, or JSON.
- **Your code prefixes** is an allow-list. When set, only functions or file paths matching those prefixes count as user code.
- **Hide framework frames** removes framework rows from the rendered output but still counts them.
- **List frames in call order** flips the default innermost-first order to outermost-first.
- **Max frames per exception** accepts 1 through 2000; the default is 100.
- Input is capped at 200,000 bytes. Paste the failing trace rather than an entire application log.
- The parser is heuristic. It does not execute code, fetch source maps, symbolicate minified JavaScript, de-obfuscate Java class names, or reconstruct frames missing from the pasted trace.

## FAQ

<details>
<summary>Which stack trace formats does it parse?</summary>

It handles Java-style `at Class.method(File.java:42)` traces including `Caused by`, Python `Traceback` blocks, JavaScript/Node `at function (file:line:col)` traces, Go panics and goroutine frames, Ruby, C#/.NET, Rust panics/backtraces, and PHP fatal-error traces. Use the language selector when a very short snippet cannot be detected reliably.

</details>

<details>
<summary>How does it decide what is my code?</summary>

Without prefixes it uses language-specific framework rules such as Java `java.*`, Node `node_modules` and `node:internal`, Python `site-packages`, Ruby `/gems/`, Rust `/rustc/`, and .NET `System.*`. If you provide **Your code prefixes**, those prefixes become the rule: only matching function or file paths are marked as user code.

</details>

<details>
<summary>What is the root cause?</summary>

The root cause is the innermost exception in a chain: the last `Caused by` in Java, the original exception behind Python `__cause__` / context text, the deepest inner exception in .NET-style traces, or the single reported exception when there is no chain.

</details>

<details>
<summary>Can it symbolicate minified JavaScript or native crash dumps?</summary>

No. It only analyzes the text you paste. Source maps, debug symbols, minidumps, JVM heap dumps, and IDE-specific symbolication are out of scope. If the trace already contains file names, line numbers, and functions, this tool will structure and classify them.

</details>

<details>
<summary>Is the trace uploaded anywhere?</summary>

No. The same Rust parser runs locally in the WebAssembly page and in the CLI. Your stack trace is processed in your browser or terminal.

</details>
