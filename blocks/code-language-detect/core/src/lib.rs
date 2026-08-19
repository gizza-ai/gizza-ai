//! code-language-detect core — pure compute, shared by the chat skill block and the web page.
//!
//! Deterministic, GitHub-Linguist-flavoured detection: every language owns a table of weighted
//! signals (keywords, operators, line shapes, structural checks). Each signal that fires adds its
//! weight; the highest total wins. A supplied filename or a shebang line is a strong extra hint,
//! exactly as Linguist checks the filename before it reads the bytes. No model, no network, no
//! randomness — the same snippet always produces the same ranking.

use serde_json::json;

/// Largest snippet accepted, in bytes.
pub const MAX_CODE_BYTES: usize = 1_048_576;
/// Largest accepted `top_k`.
pub const MAX_TOP_K: usize = 30;
/// Accepted `output` values.
pub const OUTPUTS: [&str; 3] = ["report", "json", "language"];
/// Score added when the filename extension or a shebang names a language.
const HINT_WEIGHT: f64 = 10.0;
/// How many evidence lines the readable report prints.
const MAX_EVIDENCE_SHOWN: usize = 8;

/// Everything the caller can tune.
#[derive(Clone, Debug)]
pub struct Options {
    /// Optional filename (e.g. `main.rs`, `Dockerfile`) used as an extension hint.
    pub filename: String,
    /// Comma-separated allowlist of language ids; empty means "consider all".
    pub candidates: String,
    /// Restrict the candidate set to the mainstream languages.
    pub common_only: bool,
    /// How many ranked candidates to list; 0 lists every language that scored.
    pub top_k: usize,
    /// Include the matched signals that drove the decision.
    pub explain: bool,
    /// `report`, `json` or `language`.
    pub output: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            filename: String::new(),
            candidates: String::new(),
            common_only: false,
            top_k: 3,
            explain: true,
            output: "report".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Signal vocabulary
// ---------------------------------------------------------------------------

/// A structural check that looks at the shape of the whole snippet, not one token.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sig {
    /// The whole snippet parses as a JSON object or array.
    JsonDocument,
    /// `selector { … }` blocks holding `property: value;` declarations.
    CssRuleBlocks,
    /// Two or more `key: value` lines with no braces or semicolons anywhere.
    YamlMapping,
    /// A `[table]` header line plus at least one `key = value` line.
    TomlTable,
    /// A `target:` line immediately followed by a tab-indented recipe line.
    MakeRecipe,
    /// A `$name: value` declaration line (Sass/SCSS variable).
    ScssVariable,
    /// A line ending in `:` followed by a more deeply indented line.
    IndentedColonBlock,
}

/// One matchable pattern.
#[derive(Clone, Copy)]
enum Pat {
    /// Case-sensitive substring.
    Sub(&'static str),
    /// Case-insensitive substring (write the needle lower-case).
    SubCi(&'static str),
    /// Case-insensitive whole word (write the needle lower-case).
    WordCi(&'static str),
    /// A line whose trimmed start begins with this.
    LineStart(&'static str),
    /// A line that trims to exactly this.
    LineEq(&'static str),
    /// A line whose trimmed form starts with `.0` and ends with `.1`.
    LineStartEnd(&'static str, &'static str),
    /// A structural check.
    Struct(Sig),
}

struct Rule {
    pat: Pat,
    w: f64,
    label: &'static str,
}

const fn r(pat: Pat, w: f64, label: &'static str) -> Rule {
    Rule { pat, w, label }
}

struct Lang {
    id: &'static str,
    name: &'static str,
    /// Part of the curated "common languages" subset.
    common: bool,
    /// Filename extensions (lower-case, no dot) and exact filenames that name this language.
    exts: &'static [&'static str],
    rules: &'static [Rule],
}

// ---------------------------------------------------------------------------
// Language table
// ---------------------------------------------------------------------------

const LANGS: &[Lang] = &[
    Lang {
        id: "rust",
        name: "Rust",
        common: true,
        exts: &["rs"],
        rules: &[
            r(Pat::Sub("fn "), 4.0, "`fn` function definition"),
            r(Pat::Sub("let mut "), 6.0, "`let mut` binding"),
            r(Pat::Sub("impl "), 5.0, "`impl` block"),
            r(Pat::Sub("println!"), 6.0, "`println!` macro"),
            r(Pat::Sub(".unwrap()"), 5.0, "`.unwrap()` call"),
            r(Pat::Sub("#["), 4.0, "`#[…]` attribute"),
            r(Pat::Sub("&str"), 5.0, "`&str` type"),
            r(Pat::Sub("Vec<"), 4.0, "`Vec<…>` type"),
            r(Pat::Sub("Option<"), 4.0, "`Option<…>` type"),
            r(Pat::Sub("Result<"), 4.0, "`Result<…>` type"),
            r(Pat::Sub("crate::"), 5.0, "`crate::` path"),
            r(Pat::Sub("::"), 1.0, "`::` path separator"),
            r(Pat::Sub("->"), 1.0, "`->` return arrow"),
            r(Pat::LineStart("use "), 2.0, "`use` import"),
            r(Pat::LineStart("pub "), 3.0, "`pub` visibility"),
            r(Pat::Sub("match "), 2.0, "`match` expression"),
            r(Pat::Sub("<?php"), -12.0, "not PHP"),
        ],
    },
    Lang {
        id: "go",
        name: "Go",
        common: true,
        exts: &["go"],
        rules: &[
            r(Pat::Sub("package main"), 8.0, "`package main` declaration"),
            r(Pat::Sub(":="), 6.0, "`:=` short variable declaration"),
            r(Pat::Sub("func "), 4.0, "`func` keyword"),
            r(Pat::Sub("fmt."), 6.0, "`fmt` package call"),
            r(Pat::Sub("err != nil"), 8.0, "`err != nil` check"),
            r(Pat::Sub("import ("), 5.0, "grouped `import (` block"),
            r(Pat::Sub(" struct {"), 4.0, "`struct {` type"),
            r(Pat::Sub("defer "), 4.0, "`defer` statement"),
            r(Pat::Sub("go func"), 5.0, "`go func` goroutine"),
            r(Pat::Sub("interface{}"), 4.0, "empty `interface{}`"),
            r(Pat::Sub("nil"), 1.0, "`nil` literal"),
        ],
    },
    Lang {
        id: "python",
        name: "Python",
        common: true,
        exts: &["py", "pyw"],
        rules: &[
            r(Pat::LineStartEnd("def ", ":"), 7.0, "`def …:` function"),
            r(
                Pat::LineStartEnd("class ", ":"),
                5.0,
                "`class …:` definition",
            ),
            r(Pat::LineStart("elif "), 6.0, "`elif` branch"),
            r(Pat::LineStart("import "), 3.0, "`import` statement"),
            r(Pat::LineStart("from "), 2.0, "`from …` import"),
            r(Pat::Sub(" import "), 3.0, "`import` clause"),
            r(Pat::Sub("self."), 5.0, "`self.` attribute access"),
            r(Pat::Sub("__init__"), 6.0, "`__init__` method"),
            r(Pat::Sub("__name__"), 6.0, "`__name__` guard"),
            r(Pat::Sub("print("), 2.0, "`print(` call"),
            r(Pat::Sub("None"), 3.0, "`None` literal"),
            r(Pat::Sub("True"), 1.0, "`True` literal"),
            r(Pat::Sub("lambda "), 3.0, "`lambda` expression"),
            r(
                Pat::Struct(Sig::IndentedColonBlock),
                5.0,
                "colon-and-indent block structure",
            ),
            r(Pat::Sub("<?php"), -12.0, "not PHP"),
        ],
    },
    Lang {
        id: "ruby",
        name: "Ruby",
        common: true,
        exts: &["rb", "rake", "gemspec"],
        rules: &[
            r(Pat::Sub("puts "), 6.0, "`puts` output"),
            r(Pat::LineEq("end"), 4.0, "bare `end` line"),
            r(Pat::Sub("do |"), 6.0, "`do |x|` block"),
            r(Pat::Sub("elsif"), 6.0, "`elsif` branch"),
            r(Pat::Sub("attr_accessor"), 7.0, "`attr_accessor` macro"),
            r(Pat::LineStart("require "), 3.0, "`require` statement"),
            r(Pat::Sub(".each do"), 6.0, "`.each do` iteration"),
            r(Pat::LineStart("def "), 2.0, "`def` method"),
            r(Pat::Sub("unless "), 4.0, "`unless` guard"),
            r(Pat::Sub("nil"), 1.0, "`nil` literal"),
            r(Pat::Sub("@"), 0.5, "`@ivar` sigil"),
            r(Pat::Sub("@@"), -1.0, "not Ruby"),
        ],
    },
    Lang {
        id: "php",
        name: "PHP",
        common: true,
        exts: &["php", "phtml"],
        rules: &[
            r(Pat::Sub("<?php"), 14.0, "`<?php` open tag"),
            r(Pat::Sub("$this->"), 8.0, "`$this->` member access"),
            r(Pat::Sub("echo "), 4.0, "`echo` statement"),
            r(Pat::Sub("public function"), 6.0, "`public function` method"),
            r(Pat::Sub("$_"), 4.0, "superglobal `$_…`"),
            r(Pat::Sub("::class"), 4.0, "`::class` constant"),
            r(Pat::Sub("array("), 3.0, "`array(` literal"),
            r(Pat::Sub("->"), 1.0, "`->` member arrow"),
            r(Pat::Sub("=>"), 1.0, "`=>` array arrow"),
        ],
    },
    Lang {
        id: "perl",
        name: "Perl",
        common: false,
        exts: &["pl", "pm", "t"],
        rules: &[
            r(Pat::Sub("my $"), 8.0, "`my $var` declaration"),
            r(Pat::Sub("use strict;"), 8.0, "`use strict;` pragma"),
            r(Pat::Sub("=~"), 6.0, "`=~` binding operator"),
            r(Pat::Sub("@ARGV"), 8.0, "`@ARGV` array"),
            r(Pat::Sub("qw("), 6.0, "`qw(` word list"),
            r(Pat::LineStart("sub "), 4.0, "`sub` definition"),
            r(Pat::Sub("$_"), 3.0, "`$_` topic variable"),
            r(Pat::Sub("->"), 1.0, "`->` dereference arrow"),
        ],
    },
    Lang {
        id: "lua",
        name: "Lua",
        common: false,
        exts: &["lua"],
        rules: &[
            r(Pat::Sub("local "), 7.0, "`local` declaration"),
            r(Pat::LineEq("end"), 3.0, "bare `end` line"),
            r(Pat::Sub("elseif"), 6.0, "`elseif` branch"),
            r(Pat::Sub("~="), 6.0, "`~=` inequality"),
            r(Pat::Sub("ipairs"), 6.0, "`ipairs` iterator"),
            r(Pat::Sub("pairs("), 4.0, "`pairs(` iterator"),
            r(Pat::Sub("--[["), 6.0, "`--[[` long comment"),
            r(Pat::Sub(" then"), 3.0, "`then` keyword"),
            r(Pat::Sub("function "), 2.0, "`function` keyword"),
            r(Pat::Sub("nil"), 1.0, "`nil` literal"),
        ],
    },
    Lang {
        id: "r",
        name: "R",
        common: false,
        exts: &["r"],
        rules: &[
            r(Pat::Sub("<-"), 7.0, "`<-` assignment"),
            r(Pat::Sub("library("), 8.0, "`library(` load"),
            r(Pat::Sub("data.frame"), 7.0, "`data.frame` call"),
            r(Pat::Sub("%>%"), 8.0, "`%>%` pipe"),
            r(Pat::Sub("ggplot"), 6.0, "`ggplot` call"),
            r(Pat::Sub("function("), 2.0, "`function(` literal"),
            r(Pat::Sub("NULL"), 2.0, "`NULL` literal"),
            r(Pat::Sub("TRUE"), 2.0, "`TRUE` literal"),
        ],
    },
    Lang {
        id: "java",
        name: "Java",
        common: true,
        exts: &["java"],
        rules: &[
            r(Pat::Sub("public class "), 7.0, "`public class` declaration"),
            r(
                Pat::Sub("public static void main"),
                9.0,
                "`public static void main` entry point",
            ),
            r(
                Pat::Sub("System.out.print"),
                9.0,
                "`System.out.print…` call",
            ),
            r(Pat::LineStart("import java"), 8.0, "`import java.…` import"),
            r(Pat::Sub("@Override"), 6.0, "`@Override` annotation"),
            r(Pat::Sub("String[] "), 5.0, "`String[]` array type"),
            r(
                Pat::LineStartEnd("package ", ";"),
                5.0,
                "`package …;` header",
            ),
            r(Pat::Sub("extends "), 2.0, "`extends` keyword"),
            r(Pat::Sub("implements "), 2.0, "`implements` keyword"),
            r(Pat::Sub("private "), 1.0, "`private` modifier"),
            r(Pat::Sub("new "), 1.0, "`new` allocation"),
            r(Pat::Sub("Console.WriteLine"), -8.0, "not C#"),
        ],
    },
    Lang {
        id: "kotlin",
        name: "Kotlin",
        common: true,
        exts: &["kt", "kts"],
        rules: &[
            r(Pat::Sub("fun "), 6.0, "`fun` function"),
            r(Pat::Sub("val "), 5.0, "`val` binding"),
            r(Pat::Sub("data class"), 8.0, "`data class` declaration"),
            r(Pat::Sub("companion object"), 8.0, "`companion object`"),
            r(Pat::Sub("println("), 4.0, "`println(` call"),
            r(Pat::LineStart("import kotlin"), 8.0, "`import kotlin.…`"),
            r(Pat::Sub("?:"), 4.0, "`?:` elvis operator"),
            r(Pat::Sub("?."), 3.0, "`?.` safe call"),
            r(Pat::Sub("listOf("), 5.0, "`listOf(` builder"),
            r(Pat::Sub(": String"), 2.0, "`: String` type annotation"),
        ],
    },
    Lang {
        id: "scala",
        name: "Scala",
        common: false,
        exts: &["scala", "sc"],
        rules: &[
            r(Pat::Sub("case class"), 8.0, "`case class` declaration"),
            r(Pat::LineStart("object "), 5.0, "`object` singleton"),
            r(Pat::Sub("implicit "), 7.0, "`implicit` modifier"),
            r(Pat::LineStart("import scala"), 8.0, "`import scala.…`"),
            r(Pat::Sub("trait "), 4.0, "`trait` declaration"),
            r(Pat::Sub("val "), 3.0, "`val` binding"),
            r(Pat::Sub("def "), 2.0, "`def` method"),
            r(Pat::Sub("=>"), 2.0, "`=>` lambda arrow"),
            r(Pat::Sub("Seq("), 4.0, "`Seq(` collection"),
        ],
    },
    Lang {
        id: "csharp",
        name: "C#",
        common: true,
        exts: &["cs", "csx"],
        rules: &[
            r(
                Pat::LineStart("using System"),
                9.0,
                "`using System…` directive",
            ),
            r(
                Pat::Sub("Console.WriteLine"),
                9.0,
                "`Console.WriteLine` call",
            ),
            r(
                Pat::Sub("public static void Main"),
                8.0,
                "`public static void Main` entry point",
            ),
            r(Pat::Sub("get; set;"), 8.0, "auto-property `get; set;`"),
            r(Pat::Sub("namespace "), 4.0, "`namespace` declaration"),
            r(Pat::Sub("async Task"), 6.0, "`async Task` method"),
            r(Pat::Sub("string[] "), 5.0, "`string[]` array type"),
            r(Pat::Sub("#region"), 6.0, "`#region` directive"),
            r(Pat::Sub("public class"), 2.0, "`public class` declaration"),
            r(Pat::Sub("var "), 1.0, "`var` local"),
            r(Pat::Sub("System.out.print"), -8.0, "not Java"),
        ],
    },
    Lang {
        id: "cpp",
        name: "C++",
        common: true,
        exts: &["cpp", "cc", "cxx", "hpp", "hh", "hxx"],
        rules: &[
            r(
                Pat::Sub("#include <iostream>"),
                10.0,
                "`#include <iostream>`",
            ),
            r(Pat::Sub("std::"), 9.0, "`std::` namespace"),
            r(Pat::Sub("cout <<"), 9.0, "`cout <<` stream write"),
            r(Pat::Sub("nullptr"), 7.0, "`nullptr` literal"),
            r(Pat::Sub("template<"), 7.0, "`template<` declaration"),
            r(Pat::Sub("template <"), 7.0, "`template <` declaration"),
            r(Pat::Sub("public:"), 6.0, "`public:` access label"),
            r(Pat::Sub("#include"), 3.0, "`#include` directive"),
            r(Pat::Sub("namespace "), 2.0, "`namespace` declaration"),
            r(Pat::Sub("::"), 1.0, "`::` scope operator"),
        ],
    },
    Lang {
        id: "c",
        name: "C",
        common: true,
        exts: &["c", "h"],
        rules: &[
            r(Pat::Sub("#include <stdio.h>"), 10.0, "`#include <stdio.h>`"),
            r(Pat::Sub("printf("), 7.0, "`printf(` call"),
            r(Pat::Sub("int main("), 6.0, "`int main(` entry point"),
            r(Pat::Sub("malloc("), 7.0, "`malloc(` allocation"),
            r(Pat::Sub("typedef struct"), 7.0, "`typedef struct`"),
            r(Pat::Sub("sizeof("), 4.0, "`sizeof(` operator"),
            r(Pat::Sub("char *"), 5.0, "`char *` pointer"),
            r(Pat::Sub("#include"), 3.0, "`#include` directive"),
            r(Pat::Sub("NULL"), 2.0, "`NULL` macro"),
            r(Pat::Sub("std::"), -9.0, "not C++"),
            r(Pat::Sub("cout"), -6.0, "not C++"),
            r(Pat::Sub("nullptr"), -6.0, "not C++"),
        ],
    },
    Lang {
        id: "swift",
        name: "Swift",
        common: true,
        exts: &["swift"],
        rules: &[
            r(Pat::Sub("import Foundation"), 9.0, "`import Foundation`"),
            r(Pat::Sub("guard let"), 8.0, "`guard let` unwrap"),
            r(Pat::Sub("if let "), 6.0, "`if let` unwrap"),
            r(Pat::Sub("-> Void"), 6.0, "`-> Void` return type"),
            r(Pat::Sub("@objc"), 6.0, "`@objc` attribute"),
            r(Pat::Sub(": Codable"), 6.0, "`: Codable` conformance"),
            r(Pat::Sub("func "), 3.0, "`func` function"),
            r(Pat::Sub("?? "), 3.0, "`??` nil-coalescing"),
            r(Pat::Sub("let "), 2.0, "`let` binding"),
            r(Pat::Sub("print("), 1.0, "`print(` call"),
        ],
    },
    Lang {
        id: "dart",
        name: "Dart",
        common: false,
        exts: &["dart"],
        rules: &[
            r(Pat::Sub("import 'package:"), 9.0, "`import 'package:…'`"),
            r(Pat::Sub("Widget build("), 9.0, "`Widget build(` method"),
            r(Pat::Sub("void main()"), 6.0, "`void main()` entry point"),
            r(Pat::Sub("@override"), 6.0, "`@override` annotation"),
            r(Pat::Sub("late "), 4.0, "`late` modifier"),
            r(Pat::Sub("final "), 2.0, "`final` binding"),
            r(Pat::Sub("List<"), 2.0, "`List<…>` type"),
        ],
    },
    Lang {
        id: "elixir",
        name: "Elixir",
        common: false,
        exts: &["ex", "exs"],
        rules: &[
            r(Pat::Sub("defmodule "), 10.0, "`defmodule` declaration"),
            r(Pat::Sub("|>"), 7.0, "`|>` pipe operator"),
            r(Pat::Sub("IO.puts"), 8.0, "`IO.puts` call"),
            r(Pat::Sub("defp "), 7.0, "`defp` private function"),
            r(Pat::Sub("@moduledoc"), 7.0, "`@moduledoc` attribute"),
            r(Pat::Sub("%{"), 6.0, "`%{}` map literal"),
            r(Pat::LineEq("end"), 2.0, "bare `end` line"),
            r(Pat::Sub("def "), 2.0, "`def` function"),
        ],
    },
    Lang {
        id: "haskell",
        name: "Haskell",
        common: false,
        exts: &["hs", "lhs"],
        rules: &[
            r(Pat::Sub("putStrLn"), 9.0, "`putStrLn` output"),
            r(Pat::LineStart("import Data."), 8.0, "`import Data.…`"),
            r(Pat::Sub(">>="), 7.0, "`>>=` bind operator"),
            r(Pat::Sub("<$>"), 7.0, "`<$>` fmap operator"),
            r(Pat::Sub("deriving "), 7.0, "`deriving` clause"),
            r(Pat::LineStart("module "), 5.0, "`module` header"),
            r(Pat::Sub(" where"), 4.0, "`where` clause"),
            r(Pat::Sub(" :: "), 4.0, "`::` type signature"),
            r(Pat::Sub("Maybe "), 4.0, "`Maybe` type"),
        ],
    },
    Lang {
        id: "javascript",
        name: "JavaScript",
        common: true,
        exts: &["js", "mjs", "cjs", "jsx"],
        rules: &[
            r(Pat::Sub("console.log"), 8.0, "`console.log` call"),
            r(
                Pat::Sub("module.exports"),
                8.0,
                "`module.exports` assignment",
            ),
            r(Pat::Sub("==="), 6.0, "`===` strict equality"),
            r(Pat::Sub("require("), 5.0, "`require(` import"),
            r(Pat::Sub("document."), 5.0, "`document.` DOM access"),
            r(Pat::Sub("const "), 4.0, "`const` binding"),
            r(Pat::Sub("=>"), 3.0, "`=>` arrow function"),
            r(Pat::Sub("function "), 3.0, "`function` keyword"),
            r(Pat::Sub("undefined"), 4.0, "`undefined` literal"),
            r(Pat::Sub("var "), 2.0, "`var` binding"),
            r(Pat::Sub("let "), 2.0, "`let` binding"),
            r(Pat::Sub("${"), 2.0, "`${…}` template literal"),
            r(Pat::Sub(": string"), -6.0, "not TypeScript"),
            r(Pat::Sub(": number"), -6.0, "not TypeScript"),
            r(Pat::Sub("interface "), -4.0, "not TypeScript"),
        ],
    },
    Lang {
        id: "typescript",
        name: "TypeScript",
        common: true,
        exts: &["ts", "tsx", "mts", "cts"],
        rules: &[
            r(Pat::Sub(": string"), 8.0, "`: string` type annotation"),
            r(Pat::Sub(": number"), 8.0, "`: number` type annotation"),
            r(Pat::Sub(": boolean"), 7.0, "`: boolean` type annotation"),
            r(Pat::Sub("interface "), 7.0, "`interface` declaration"),
            r(Pat::Sub("readonly "), 6.0, "`readonly` modifier"),
            r(Pat::Sub("<T>"), 5.0, "`<T>` generic parameter"),
            r(Pat::Sub("?: "), 4.0, "`?:` optional property"),
            r(Pat::Sub("enum "), 4.0, "`enum` declaration"),
            r(Pat::Sub("implements "), 3.0, "`implements` clause"),
            r(Pat::Sub("private "), 2.0, "`private` modifier"),
            r(Pat::Sub("const "), 2.0, "`const` binding"),
            r(Pat::Sub("=>"), 2.0, "`=>` arrow function"),
            r(Pat::Sub("console.log"), 2.0, "`console.log` call"),
        ],
    },
    Lang {
        id: "html",
        name: "HTML",
        common: true,
        exts: &["html", "htm", "xhtml"],
        rules: &[
            r(
                Pat::SubCi("<!doctype html"),
                14.0,
                "`<!DOCTYPE html>` declaration",
            ),
            r(Pat::SubCi("<html"), 9.0, "`<html>` element"),
            r(Pat::SubCi("</div>"), 8.0, "`</div>` element"),
            r(Pat::SubCi("<body"), 7.0, "`<body>` element"),
            r(Pat::SubCi("<head"), 5.0, "`<head>` element"),
            r(Pat::SubCi("<script"), 5.0, "`<script>` element"),
            r(Pat::SubCi("<meta"), 4.0, "`<meta>` element"),
            r(Pat::Sub("class=\""), 4.0, "`class=\"…\"` attribute"),
            r(Pat::Sub("href="), 3.0, "`href=` attribute"),
            r(Pat::SubCi("<p>"), 3.0, "`<p>` element"),
            r(Pat::Sub("<?xml"), -10.0, "not XML"),
        ],
    },
    Lang {
        id: "css",
        name: "CSS",
        common: true,
        exts: &["css"],
        rules: &[
            r(
                Pat::Struct(Sig::CssRuleBlocks),
                9.0,
                "`selector { … }` rule blocks",
            ),
            r(Pat::Sub("@media"), 7.0, "`@media` query"),
            r(Pat::Sub("!important"), 6.0, "`!important` flag"),
            r(Pat::Sub("px;"), 5.0, "`px;` length value"),
            r(Pat::Sub("::before"), 6.0, "`::before` pseudo-element"),
            r(Pat::Sub("color:"), 4.0, "`color:` declaration"),
            r(Pat::Sub("margin:"), 4.0, "`margin:` declaration"),
            r(Pat::Sub("padding:"), 4.0, "`padding:` declaration"),
            r(Pat::Sub("display:"), 4.0, "`display:` declaration"),
            r(Pat::Sub("rgba("), 3.0, "`rgba(` colour"),
            r(Pat::Sub("function "), -5.0, "not a script"),
        ],
    },
    Lang {
        id: "scss",
        name: "SCSS",
        common: false,
        exts: &["scss", "sass"],
        rules: &[
            r(Pat::Sub("@mixin"), 10.0, "`@mixin` definition"),
            r(Pat::Sub("@include"), 9.0, "`@include` call"),
            r(Pat::Sub("@extend"), 9.0, "`@extend` directive"),
            r(
                Pat::Struct(Sig::ScssVariable),
                8.0,
                "`$variable:` declaration",
            ),
            r(Pat::Sub("&:"), 6.0, "`&:` parent selector"),
            r(Pat::Sub("@use "), 6.0, "`@use` module import"),
            r(
                Pat::Struct(Sig::CssRuleBlocks),
                3.0,
                "`selector { … }` rule blocks",
            ),
            r(Pat::Sub("px;"), 2.0, "`px;` length value"),
        ],
    },
    Lang {
        id: "sql",
        name: "SQL",
        common: true,
        exts: &["sql"],
        rules: &[
            r(Pat::SubCi("create table"), 10.0, "`CREATE TABLE` statement"),
            r(Pat::SubCi("insert into"), 9.0, "`INSERT INTO` statement"),
            r(Pat::SubCi("group by"), 8.0, "`GROUP BY` clause"),
            r(Pat::SubCi("inner join"), 8.0, "`INNER JOIN` clause"),
            r(Pat::SubCi("primary key"), 8.0, "`PRIMARY KEY` constraint"),
            r(Pat::SubCi("order by"), 7.0, "`ORDER BY` clause"),
            r(Pat::WordCi("varchar"), 7.0, "`VARCHAR` column type"),
            r(Pat::WordCi("select"), 5.0, "`SELECT` keyword"),
            r(Pat::WordCi("where"), 4.0, "`WHERE` keyword"),
            r(Pat::WordCi("from"), 3.0, "`FROM` keyword"),
            r(Pat::WordCi("values"), 3.0, "`VALUES` keyword"),
            r(Pat::Sub("function "), -5.0, "not a script"),
            r(Pat::Sub("{"), -2.0, "no brace blocks in SQL"),
        ],
    },
    Lang {
        id: "shell",
        name: "Shell",
        common: true,
        exts: &["sh", "bash", "zsh", "ksh"],
        rules: &[
            r(Pat::LineEq("esac"), 9.0, "`esac` case terminator"),
            r(Pat::LineEq("fi"), 8.0, "`fi` if terminator"),
            r(Pat::Sub("if [ "), 8.0, "`if [ … ]` test"),
            r(Pat::Sub("if [[ "), 8.0, "`if [[ … ]]` test"),
            r(Pat::Sub("2>&1"), 7.0, "`2>&1` redirection"),
            r(Pat::LineEq("done"), 6.0, "`done` loop terminator"),
            r(Pat::Sub("$1"), 5.0, "`$1` positional parameter"),
            r(Pat::Sub("$("), 5.0, "`$(…)` command substitution"),
            r(Pat::Sub("${"), 4.0, "`${…}` parameter expansion"),
            r(Pat::Sub("echo "), 4.0, "`echo` command"),
            r(Pat::LineStart("export "), 4.0, "`export` statement"),
            r(Pat::Sub("; then"), 4.0, "`; then` clause"),
        ],
    },
    Lang {
        id: "powershell",
        name: "PowerShell",
        common: false,
        exts: &["ps1", "psm1", "psd1"],
        rules: &[
            r(
                Pat::Sub("[CmdletBinding()]"),
                10.0,
                "`[CmdletBinding()]` attribute",
            ),
            r(Pat::Sub("Write-Host"), 9.0, "`Write-Host` cmdlet"),
            r(
                Pat::Sub("$PSVersionTable"),
                9.0,
                "`$PSVersionTable` variable",
            ),
            r(Pat::Sub("Where-Object"), 8.0, "`Where-Object` cmdlet"),
            r(Pat::Sub("Get-"), 6.0, "`Get-…` cmdlet"),
            r(Pat::Sub("$_."), 6.0, "`$_.` pipeline variable"),
            r(Pat::Sub("-eq "), 6.0, "`-eq` comparison"),
            r(Pat::Sub("$true"), 5.0, "`$true` literal"),
            r(Pat::Sub("param("), 4.0, "`param(` block"),
        ],
    },
    Lang {
        id: "json",
        name: "JSON",
        common: true,
        exts: &["json", "jsonc", "geojson"],
        rules: &[
            r(
                Pat::Struct(Sig::JsonDocument),
                22.0,
                "the whole snippet parses as JSON",
            ),
            r(Pat::Sub("\": "), 2.0, "`\"key\": value` pair"),
            r(Pat::Sub("//"), -4.0, "JSON has no comments"),
            r(Pat::Sub(";"), -3.0, "JSON has no statements"),
        ],
    },
    Lang {
        id: "yaml",
        name: "YAML",
        common: true,
        exts: &["yaml", "yml"],
        rules: &[
            r(
                Pat::Struct(Sig::YamlMapping),
                10.0,
                "`key: value` mapping lines",
            ),
            r(Pat::LineStart("---"), 6.0, "`---` document marker"),
            r(Pat::LineStart("- "), 3.0, "`- ` sequence item"),
            r(Pat::Sub("{"), -3.0, "braces are unusual in YAML"),
            r(Pat::Sub(";"), -3.0, "YAML has no statement terminator"),
            r(Pat::Sub("()"), -3.0, "YAML has no call syntax"),
        ],
    },
    Lang {
        id: "toml",
        name: "TOML",
        common: false,
        exts: &["toml"],
        rules: &[
            r(
                Pat::Struct(Sig::TomlTable),
                11.0,
                "`[table]` header plus `key = value`",
            ),
            r(Pat::Sub("[dependencies]"), 9.0, "`[dependencies]` table"),
            r(Pat::LineStart("[["), 6.0, "`[[array-of-tables]]` header"),
            r(Pat::Sub(" = \""), 5.0, "`key = \"value\"` assignment"),
            r(Pat::Sub(";"), -3.0, "TOML has no statement terminator"),
        ],
    },
    Lang {
        id: "xml",
        name: "XML",
        common: false,
        exts: &["xml", "xsd", "xsl", "svg", "plist"],
        rules: &[
            r(Pat::Sub("<?xml"), 14.0, "`<?xml …?>` prolog"),
            r(Pat::Sub("xmlns"), 8.0, "`xmlns` namespace"),
            r(Pat::Sub("</"), 3.0, "closing tag"),
            r(Pat::Sub("/>"), 3.0, "self-closing tag"),
            r(Pat::SubCi("<!doctype html"), -12.0, "not HTML"),
            r(Pat::SubCi("<div"), -5.0, "not HTML"),
        ],
    },
    Lang {
        id: "markdown",
        name: "Markdown",
        common: true,
        exts: &["md", "markdown", "mdown"],
        rules: &[
            r(Pat::Sub("```"), 9.0, "fenced code block"),
            r(Pat::LineStart("## "), 7.0, "`## ` heading"),
            r(Pat::LineStart("# "), 6.0, "`# ` heading"),
            r(Pat::Sub("]("), 6.0, "`[text](link)` link"),
            r(Pat::Sub("**"), 4.0, "`**bold**` emphasis"),
            r(Pat::LineStart("- "), 3.0, "`- ` bullet"),
            r(Pat::LineStart("> "), 3.0, "`> ` block quote"),
            r(Pat::Sub("|---"), 5.0, "table separator row"),
        ],
    },
    Lang {
        id: "dockerfile",
        name: "Dockerfile",
        common: false,
        exts: &["dockerfile", "containerfile"],
        rules: &[
            r(Pat::LineStart("FROM "), 11.0, "`FROM` base image"),
            r(Pat::LineStart("RUN "), 9.0, "`RUN` instruction"),
            r(Pat::LineStart("WORKDIR "), 9.0, "`WORKDIR` instruction"),
            r(
                Pat::LineStart("ENTRYPOINT"),
                9.0,
                "`ENTRYPOINT` instruction",
            ),
            r(Pat::LineStart("CMD "), 8.0, "`CMD` instruction"),
            r(Pat::LineStart("COPY "), 8.0, "`COPY` instruction"),
            r(Pat::LineStart("EXPOSE "), 8.0, "`EXPOSE` instruction"),
            r(Pat::LineStart("ENV "), 5.0, "`ENV` instruction"),
        ],
    },
    Lang {
        id: "makefile",
        name: "Makefile",
        common: false,
        exts: &["makefile", "mk", "gnumakefile"],
        rules: &[
            r(
                Pat::Struct(Sig::MakeRecipe),
                11.0,
                "`target:` with a tab-indented recipe",
            ),
            r(Pat::Sub(".PHONY"), 10.0, "`.PHONY` target"),
            r(Pat::Sub("$@"), 8.0, "`$@` automatic variable"),
            r(Pat::Sub("$<"), 8.0, "`$<` automatic variable"),
            r(Pat::Sub("$("), 3.0, "`$(VAR)` expansion"),
        ],
    },
];

/// Shebang interpreter name → language id.
const SHEBANGS: &[(&str, &str)] = &[
    ("python", "python"),
    ("node", "javascript"),
    ("deno", "typescript"),
    ("ruby", "ruby"),
    ("perl", "perl"),
    ("php", "php"),
    ("lua", "lua"),
    ("elixir", "elixir"),
    ("rscript", "r"),
    ("pwsh", "powershell"),
    ("bash", "shell"),
    ("zsh", "shell"),
    ("ksh", "shell"),
    ("dash", "shell"),
    ("sh", "shell"),
];

// ---------------------------------------------------------------------------
// Matching
// ---------------------------------------------------------------------------

struct Ctx<'a> {
    code: &'a str,
    lower: String,
    lines: Vec<&'a str>,
}

impl<'a> Ctx<'a> {
    fn new(code: &'a str) -> Self {
        Ctx {
            code,
            lower: code.to_lowercase(),
            lines: code.lines().collect(),
        }
    }
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn count_word(hay: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    let bytes = hay.as_bytes();
    let mut n = 0;
    for (i, _) in hay.match_indices(needle) {
        let before = i == 0 || !is_word_byte(bytes[i - 1]);
        let end = i + needle.len();
        let after = end >= bytes.len() || !is_word_byte(bytes[end]);
        if before && after {
            n += 1;
        }
    }
    n
}

impl Pat {
    fn hits(&self, ctx: &Ctx) -> usize {
        match self {
            Pat::Sub(s) => ctx.code.matches(s).count(),
            Pat::SubCi(s) => ctx.lower.matches(s).count(),
            Pat::WordCi(s) => count_word(&ctx.lower, s),
            Pat::LineStart(s) => ctx
                .lines
                .iter()
                .filter(|l| l.trim_start().starts_with(s))
                .count(),
            Pat::LineEq(s) => ctx.lines.iter().filter(|l| l.trim() == *s).count(),
            Pat::LineStartEnd(a, b) => ctx
                .lines
                .iter()
                .filter(|l| {
                    let t = l.trim();
                    t.starts_with(a) && t.ends_with(b) && t.len() > a.len()
                })
                .count(),
            Pat::Struct(sig) => usize::from(structural(*sig, ctx)),
        }
    }
}

fn structural(sig: Sig, ctx: &Ctx) -> bool {
    match sig {
        Sig::JsonDocument => {
            let t = ctx.code.trim();
            (t.starts_with('{') || t.starts_with('['))
                && serde_json::from_str::<serde_json::Value>(t).is_ok()
        }
        Sig::CssRuleBlocks => {
            let mut selectors = 0usize;
            let mut decls = 0usize;
            for line in &ctx.lines {
                let t = line.trim();
                if t.ends_with('{') && !t.contains('(') && !t.starts_with('@') {
                    selectors += 1;
                } else if t.ends_with(';') {
                    if let Some((k, v)) = t.trim_end_matches(';').split_once(':') {
                        let k = k.trim();
                        if !k.is_empty()
                            && !v.trim().is_empty()
                            && k.chars()
                                .all(|c| c.is_ascii_lowercase() || c == '-' || c.is_ascii_digit())
                        {
                            decls += 1;
                        }
                    }
                }
            }
            selectors >= 1 && decls >= 2
        }
        Sig::YamlMapping => {
            if ctx.code.contains('{') || ctx.code.contains(';') {
                return false;
            }
            let mut maps = 0usize;
            for line in &ctx.lines {
                let t = line.trim_start();
                if t.starts_with('#') {
                    continue;
                }
                if let Some((k, v)) = t.split_once(':') {
                    let key_ok = !k.is_empty()
                        && k.chars()
                            .all(|c| c.is_ascii_alphanumeric() || "_-. ".contains(c));
                    if key_ok && (v.is_empty() || v.starts_with(' ')) {
                        maps += 1;
                    }
                }
            }
            maps >= 2
        }
        Sig::TomlTable => {
            let mut table = false;
            let mut assign = false;
            for line in &ctx.lines {
                let t = line.trim();
                if t.starts_with('[') && t.ends_with(']') && t.len() > 2 {
                    table = true;
                } else if let Some((k, v)) = t.split_once('=') {
                    let k = k.trim();
                    if !k.is_empty()
                        && !v.trim().is_empty()
                        && k.chars()
                            .all(|c| c.is_ascii_alphanumeric() || "_-.\"".contains(c))
                    {
                        assign = true;
                    }
                }
            }
            table && assign
        }
        Sig::MakeRecipe => {
            for (i, line) in ctx.lines.iter().enumerate() {
                let t = line.trim_end();
                if t.starts_with(' ') || t.starts_with('\t') || t.is_empty() {
                    continue;
                }
                let Some((target, _)) = t.split_once(':') else {
                    continue;
                };
                if target.is_empty() || target.contains(' ') && target.contains('=') {
                    continue;
                }
                if ctx
                    .lines
                    .get(i + 1)
                    .is_some_and(|next| next.starts_with('\t'))
                {
                    return true;
                }
            }
            false
        }
        Sig::ScssVariable => ctx.lines.iter().any(|l| {
            let t = l.trim_start();
            t.starts_with('$')
                && t.split_once(':')
                    .is_some_and(|(k, v)| k.len() > 1 && !v.trim().is_empty())
        }),
        Sig::IndentedColonBlock => {
            for (i, line) in ctx.lines.iter().enumerate() {
                let t = line.trim_end();
                if !t.ends_with(':') || t.trim().is_empty() {
                    continue;
                }
                let indent = t.len() - t.trim_start().len();
                if let Some(next) = ctx.lines.get(i + 1) {
                    let next_indent = next.len() - next.trim_start().len();
                    if !next.trim().is_empty() && next_indent > indent {
                        return true;
                    }
                }
            }
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

/// One matched signal, kept for the evidence list.
#[derive(Clone, Debug)]
pub struct Evidence {
    pub label: String,
    pub weight: f64,
    pub hits: usize,
}

/// One language's total score.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub id: String,
    pub name: String,
    pub score: f64,
    pub share: f64,
    pub evidence: Vec<Evidence>,
}

/// The full detection result.
#[derive(Clone, Debug)]
pub struct Detection {
    pub language: String,
    pub language_name: String,
    pub confidence: f64,
    pub confidence_level: String,
    pub candidates: Vec<Candidate>,
    pub notes: Vec<String>,
    pub lines: usize,
    pub characters: usize,
    pub considered: usize,
}

fn lang_by_id(id: &str) -> Option<&'static Lang> {
    LANGS.iter().find(|l| l.id == id)
}

fn all_ids() -> String {
    LANGS.iter().map(|l| l.id).collect::<Vec<_>>().join(", ")
}

/// Resolve `filename` to a language id via its extension or a well-known bare name.
fn filename_hint(filename: &str) -> Option<(&'static str, String)> {
    let base = filename
        .trim()
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .trim();
    if base.is_empty() {
        return None;
    }
    let lower = base.to_lowercase();
    // A bare, extension-less name such as `Dockerfile` or `Makefile`.
    let stem_key = lower.split('.').next().unwrap_or("").to_string();
    for l in LANGS {
        if l.exts.contains(&lower.as_str())
            || l.exts.contains(&stem_key.as_str()) && !lower.contains('.')
        {
            return Some((l.id, format!("filename `{base}` names {}", l.name)));
        }
    }
    let ext = lower.rsplit_once('.').map(|(_, e)| e.to_string())?;
    for l in LANGS {
        if l.exts.contains(&ext.as_str()) {
            return Some((l.id, format!("filename extension `.{ext}`")));
        }
    }
    None
}

/// Resolve a `#!` first line to a language id.
fn shebang_hint(code: &str) -> Option<(&'static str, String)> {
    let first = code.lines().next()?.trim();
    if !first.starts_with("#!") {
        return None;
    }
    let lower = first.to_lowercase();
    for (needle, id) in SHEBANGS {
        if lower.contains(needle) {
            return Some((id, format!("shebang line names `{needle}`")));
        }
    }
    None
}

fn parse_candidates(spec: &str) -> Result<Vec<&'static str>, String> {
    let mut out: Vec<&'static str> = Vec::new();
    for raw in spec.split([',', ';', '\n', ' ', '\t']) {
        let t = raw.trim().to_lowercase();
        if t.is_empty() {
            continue;
        }
        match lang_by_id(&t) {
            Some(l) => {
                if !out.contains(&l.id) {
                    out.push(l.id);
                }
            }
            None => {
                return Err(format!(
                    "unknown language id `{t}` in candidates: expected one of {}",
                    all_ids()
                ))
            }
        }
    }
    Ok(out)
}

/// Score a snippet and return the ranked result.
pub fn detect(code: &str, opts: &Options) -> Result<Detection, String> {
    if code.len() > MAX_CODE_BYTES {
        return Err(format!(
            "code is too large: {} bytes, maximum is {MAX_CODE_BYTES} bytes (1 MiB)",
            code.len()
        ));
    }
    if code.trim().is_empty() {
        return Err("code is empty: paste at least one line of source code to detect".into());
    }
    if opts.top_k > MAX_TOP_K {
        return Err(format!(
            "top_k is too large: {}, maximum is {MAX_TOP_K} (use 0 to list every scoring language)",
            opts.top_k
        ));
    }
    if !OUTPUTS.contains(&opts.output.as_str()) {
        return Err(format!(
            "unknown output `{}`: expected one of {}",
            opts.output,
            OUTPUTS.join(", ")
        ));
    }

    let allow = parse_candidates(&opts.candidates)?;
    let pool: Vec<&Lang> = LANGS
        .iter()
        .filter(|l| {
            if !allow.is_empty() {
                allow.contains(&l.id)
            } else if opts.common_only {
                l.common
            } else {
                true
            }
        })
        .collect();
    if pool.is_empty() {
        return Err(
            "no candidate languages left: clear the candidates list or the common-only filter"
                .into(),
        );
    }

    let ctx = Ctx::new(code);
    let ext = filename_hint(&opts.filename);
    let sheb = shebang_hint(code);

    let mut scored: Vec<Candidate> = Vec::with_capacity(pool.len());
    for l in &pool {
        let mut score = 0.0f64;
        let mut evidence: Vec<Evidence> = Vec::new();
        for rule in l.rules {
            let hits = rule.pat.hits(&ctx);
            if hits == 0 {
                continue;
            }
            // Repeats reinforce a positive signal (with a cap); penalties apply once.
            let contribution = if rule.w > 0.0 {
                let repeats = hits.min(4) as f64;
                rule.w * (1.0 + 0.2 * (repeats - 1.0))
            } else {
                rule.w
            };
            score += contribution;
            evidence.push(Evidence {
                label: rule.label.to_string(),
                weight: round2(contribution),
                hits,
            });
        }
        for hint in [&ext, &sheb].into_iter().flatten() {
            if hint.0 == l.id {
                score += HINT_WEIGHT;
                evidence.push(Evidence {
                    label: hint.1.clone(),
                    weight: HINT_WEIGHT,
                    hits: 1,
                });
            }
        }
        evidence.sort_by(|a, b| {
            b.weight
                .partial_cmp(&a.weight)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.label.cmp(&b.label))
        });
        scored.push(Candidate {
            id: l.id.to_string(),
            name: l.name.to_string(),
            score: round2(score),
            share: 0.0,
            evidence,
        });
    }

    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });

    let total: f64 = scored.iter().map(|c| c.score.max(0.0)).sum();
    if total > 0.0 {
        for c in scored.iter_mut() {
            c.share = round4(c.score.max(0.0) / total);
        }
    }

    let top = scored.first().map(|c| c.score).unwrap_or(0.0);
    let second = scored.get(1).map(|c| c.score.max(0.0)).unwrap_or(0.0);
    let lines = ctx.lines.iter().filter(|l| !l.trim().is_empty()).count();
    let characters = code.chars().count();

    let mut notes: Vec<String> = Vec::new();
    if lines < 3 {
        notes.push(format!(
            "Only {} non-blank line{} of code. Three to five lines detect far more reliably.",
            lines,
            if lines == 1 { "" } else { "s" }
        ));
    }
    let (language, language_name, confidence) = if top <= 0.0 {
        notes.push(
            "No language signal matched. Paste more of the file, or pass a filename such as `main.rs` as a hint."
                .into(),
        );
        ("unknown".to_string(), "Unknown".to_string(), 0.0)
    } else {
        let margin = ((top - second) / top).clamp(0.0, 1.0);
        let strength = (top / 15.0).min(1.0);
        let conf = (0.35 + 0.40 * margin + 0.25 * strength).clamp(0.0, 1.0);
        let first = &scored[0];
        if margin < 0.20 {
            if let Some(runner_up) = scored.get(1) {
                notes.push(format!(
                    "Close call: {} and {} score within {}% of each other — treat the result as a guess.",
                    first.name,
                    runner_up.name,
                    (margin * 100.0).round() as i64
                ));
            }
        }
        if lines < 3 {
            notes.push(format!(
                "Only {} non-blank line{} of code. Three to five lines detect far more reliably.",
                lines,
                if lines == 1 { "" } else { "s" }
            ));
        }
        if top < 8.0 {
            notes.push(
                "Weak overall evidence: few signals matched. A filename hint would help.".into(),
            );
        }
        (first.id.clone(), first.name.clone(), round4(conf))
    };

    if !allow.is_empty() {
        notes.push(format!(
            "Restricted to {} candidate language{}.",
            allow.len(),
            if allow.len() == 1 { "" } else { "s" }
        ));
    } else if opts.common_only {
        notes.push(format!(
            "Restricted to the {} common languages.",
            pool.len()
        ));
    }

    let level = if confidence >= 0.75 {
        "high"
    } else if confidence >= 0.5 {
        "medium"
    } else if confidence > 0.0 {
        "low"
    } else {
        "none"
    };

    let keep = if opts.top_k == 0 {
        scored.iter().filter(|c| c.score > 0.0).count().max(1)
    } else {
        opts.top_k
    };
    let candidates: Vec<Candidate> = scored
        .into_iter()
        .filter(|c| c.score > 0.0)
        .take(keep)
        .collect();

    Ok(Detection {
        language,
        language_name,
        confidence,
        confidence_level: level.to_string(),
        candidates,
        notes,
        lines,
        characters,
        considered: pool.len(),
    })
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn round4(v: f64) -> f64 {
    (v * 10000.0).round() / 10000.0
}

/// Detect and render in the requested `output` format.
pub fn detect_to_string(code: &str, opts: &Options) -> Result<String, String> {
    let d = detect(code, opts)?;
    Ok(match opts.output.as_str() {
        "language" => d.language,
        "json" => render_json(&d, opts),
        _ => render_report(&d, opts),
    })
}

fn render_json(d: &Detection, opts: &Options) -> String {
    let candidates: Vec<serde_json::Value> = d
        .candidates
        .iter()
        .map(|c| {
            let mut v = json!({
                "language": c.id,
                "name": c.name,
                "score": c.score,
                "share": c.share,
            });
            if opts.explain {
                v["evidence"] = json!(c
                    .evidence
                    .iter()
                    .map(|e| json!({ "signal": e.label, "weight": e.weight, "hits": e.hits }))
                    .collect::<Vec<_>>());
            }
            v
        })
        .collect();
    let out = json!({
        "language": d.language,
        "language_name": d.language_name,
        "confidence": d.confidence,
        "confidence_level": d.confidence_level,
        "candidates": candidates,
        "notes": d.notes,
        "stats": {
            "lines": d.lines,
            "characters": d.characters,
            "languages_considered": d.considered,
        },
    });
    serde_json::to_string_pretty(&out).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

fn render_report(d: &Detection, opts: &Options) -> String {
    let mut s = String::new();
    if d.language == "unknown" {
        s.push_str("Detected language: unknown\n");
    } else {
        s.push_str(&format!(
            "Detected language: {} ({})\n",
            d.language_name, d.language
        ));
    }
    s.push_str(&format!(
        "Confidence: {}% ({})\n",
        (d.confidence * 100.0).round() as i64,
        d.confidence_level
    ));
    s.push_str(&format!(
        "Analyzed: {} non-blank line{}, {} characters, {} languages considered\n",
        d.lines,
        if d.lines == 1 { "" } else { "s" },
        d.characters,
        d.considered
    ));

    if !d.candidates.is_empty() {
        s.push_str("\nTop candidates:\n");
        for (i, c) in d.candidates.iter().enumerate() {
            s.push_str(&format!(
                "  {}. {} ({}) — score {:.1}, {}% of total\n",
                i + 1,
                c.name,
                c.id,
                c.score,
                (c.share * 100.0).round() as i64
            ));
        }
    }

    if opts.explain {
        if let Some(top) = d.candidates.first() {
            if !top.evidence.is_empty() {
                s.push_str(&format!("\nEvidence for {}:\n", top.name));
                for e in top.evidence.iter().take(MAX_EVIDENCE_SHOWN) {
                    let times = if e.hits > 1 {
                        format!(" (x{})", e.hits)
                    } else {
                        String::new()
                    };
                    s.push_str(&format!("  {:+.1}  {}{}\n", e.weight, e.label, times));
                }
                if top.evidence.len() > MAX_EVIDENCE_SHOWN {
                    s.push_str(&format!(
                        "  … and {} more signals\n",
                        top.evidence.len() - MAX_EVIDENCE_SHOWN
                    ));
                }
            }
        }
    }

    if !d.notes.is_empty() {
        s.push_str("\nNotes:\n");
        for n in &d.notes {
            s.push_str(&format!("  - {n}\n"));
        }
    }
    s.trim_end().to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn id_of(code: &str) -> String {
        let opts = Options {
            output: "language".into(),
            ..Options::default()
        };
        detect_to_string(code, &opts).unwrap()
    }

    const RUST: &str = r#"
use std::collections::HashMap;

pub fn tally(words: &[&str]) -> HashMap<String, usize> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for w in words {
        *counts.entry(w.to_string()).or_insert(0) += 1;
    }
    counts
}
"#;

    const PYTHON: &str = r#"
import sys

def tally(words):
    counts = {}
    for w in words:
        counts[w] = counts.get(w, 0) + 1
    return counts

if __name__ == "__main__":
    print(tally(sys.argv[1:]))
"#;

    #[test]
    fn detects_rust() {
        assert_eq!(id_of(RUST), "rust");
    }

    #[test]
    fn detects_python() {
        assert_eq!(id_of(PYTHON), "python");
    }

    #[test]
    fn detects_common_languages() {
        let cases: &[(&str, &str)] = &[
            (
                "go",
                "package main\n\nimport \"fmt\"\n\nfunc main() {\n\tv, err := run()\n\tif err != nil {\n\t\treturn\n\t}\n\tfmt.Println(v)\n}\n",
            ),
            (
                "javascript",
                "const items = [1, 2, 3];\nfunction total(list) {\n  return list.reduce((a, b) => a + b, 0);\n}\nconsole.log(total(items));\nmodule.exports = { total };\n",
            ),
            (
                "typescript",
                "interface User {\n  name: string;\n  age: number;\n}\n\nexport function greet(u: User): string {\n  return `hi ${u.name}`;\n}\n",
            ),
            (
                "java",
                "package demo;\n\nimport java.util.List;\n\npublic class Main {\n    public static void main(String[] args) {\n        System.out.println(\"hi\");\n    }\n}\n",
            ),
            (
                "csharp",
                "using System;\n\nnamespace Demo {\n    public class Program {\n        public static void Main(string[] args) {\n            Console.WriteLine(\"hi\");\n        }\n    }\n}\n",
            ),
            (
                "c",
                "#include <stdio.h>\n\nint main(void) {\n    char *msg = \"hi\";\n    printf(\"%s\\n\", msg);\n    return 0;\n}\n",
            ),
            (
                "cpp",
                "#include <iostream>\n\nint main() {\n    std::string msg = \"hi\";\n    std::cout << msg << std::endl;\n    return 0;\n}\n",
            ),
            (
                "ruby",
                "require 'json'\n\ndef tally(words)\n  words.each do |w|\n    puts w\n  end\nend\n",
            ),
            (
                "php",
                "<?php\n\nclass Greeter {\n    public function greet($name) {\n        echo \"hi $name\";\n    }\n}\n",
            ),
            (
                "shell",
                "#!/bin/bash\nset -eu\nif [ -z \"$1\" ]; then\n  echo \"usage: $0 NAME\"\n  exit 1\nfi\necho \"hi $1\"\n",
            ),
            (
                "sql",
                "SELECT name, count(*) AS n\nFROM users\nWHERE active = 1\nGROUP BY name\nORDER BY n DESC;\n",
            ),
            (
                "css",
                ".card {\n  display: flex;\n  padding: 12px;\n  color: #333;\n}\n\n@media (max-width: 600px) {\n  .card {\n    padding: 4px;\n  }\n}\n",
            ),
            (
                "html",
                "<!DOCTYPE html>\n<html lang=\"en\">\n<head><meta charset=\"utf-8\"></head>\n<body>\n<div class=\"wrap\"><p>hi</p></div>\n</body>\n</html>\n",
            ),
            (
                "json",
                "{\n  \"name\": \"demo\",\n  \"version\": \"1.0.0\",\n  \"tags\": [\"a\", \"b\"]\n}\n",
            ),
            (
                "yaml",
                "name: build\non:\n  push:\n    branches:\n      - main\njobs:\n  test:\n    runs-on: ubuntu-latest\n",
            ),
            (
                "toml",
                "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n",
            ),
            (
                "markdown",
                "# Title\n\nSome text with a [link](https://example.com) and **bold**.\n\n- one\n- two\n\n```rust\nlet x = 1;\n```\n",
            ),
            (
                "dockerfile",
                "FROM rust:1.82\nWORKDIR /app\nCOPY . .\nRUN cargo build --release\nCMD [\"./target/release/app\"]\n",
            ),
            (
                "makefile",
                ".PHONY: build\n\nbuild: main.o\n\tcc -o app $<\n\nmain.o: main.c\n\tcc -c main.c\n",
            ),
            (
                "kotlin",
                "import kotlin.math.max\n\ndata class User(val name: String, val age: Int)\n\nfun main() {\n    val u = User(\"a\", 3)\n    println(max(u.age, 1))\n}\n",
            ),
            (
                "swift",
                "import Foundation\n\nfunc greet(_ name: String?) -> Void {\n    guard let n = name else { return }\n    print(\"hi \\(n)\")\n}\n",
            ),
            (
                "lua",
                "local function tally(words)\n  local counts = {}\n  for _, w in ipairs(words) do\n    counts[w] = (counts[w] or 0) + 1\n  end\n  return counts\nend\n",
            ),
            (
                "perl",
                "use strict;\nuse warnings;\n\nmy $name = shift @ARGV;\nif ($name =~ /^a/) {\n    print \"hi $name\\n\";\n}\n",
            ),
            (
                "elixir",
                "defmodule Greeter do\n  @moduledoc \"greets\"\n\n  def greet(name) do\n    name |> String.upcase() |> IO.puts()\n  end\nend\n",
            ),
            (
                "haskell",
                "module Main where\n\nimport Data.List (sort)\n\nmain :: IO ()\nmain = putStrLn (show (sort [3, 1, 2]))\n",
            ),
            (
                "powershell",
                "[CmdletBinding()]\nparam($Name)\n\nGet-ChildItem | Where-Object { $_.Length -eq 0 }\nWrite-Host \"hi $Name\"\n",
            ),
            (
                "xml",
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<catalog xmlns=\"urn:demo\">\n  <book id=\"1\"><title>Hi</title></book>\n</catalog>\n",
            ),
            (
                "scss",
                "$brand: #036;\n\n@mixin pad($n) {\n  padding: $n;\n}\n\n.card {\n  @include pad(4px);\n  &:hover {\n    color: $brand;\n  }\n}\n",
            ),
            (
                "r",
                "library(dplyr)\n\nsummary <- data.frame(x = c(1, 2, 3))\nresult <- summary %>% filter(x > 1)\nprint(result)\n",
            ),
            (
                "dart",
                "import 'package:flutter/material.dart';\n\nclass Hi extends StatelessWidget {\n  @override\n  Widget build(BuildContext context) {\n    return const Text('hi');\n  }\n}\n",
            ),
            (
                "scala",
                "import scala.collection.mutable\n\ncase class User(name: String)\n\nobject Main {\n  implicit val ord: Ordering[User] = Ordering.by(_.name)\n  def run(users: Seq[User]) = users.sorted\n}\n",
            ),
        ];
        for (want, code) in cases {
            let got = id_of(code);
            assert_eq!(&got, want, "expected {want} for snippet:\n{code}");
        }
    }

    #[test]
    fn filename_extension_breaks_a_tie() {
        // `x = 1` alone is close to meaningless; the filename decides it.
        let opts = Options {
            filename: "notes.md".into(),
            output: "language".into(),
            ..Options::default()
        };
        assert_eq!(detect_to_string("x = 1\n", &opts).unwrap(), "markdown");
    }

    #[test]
    fn bare_dockerfile_name_is_a_hint() {
        let opts = Options {
            filename: "Dockerfile".into(),
            output: "language".into(),
            ..Options::default()
        };
        assert_eq!(detect_to_string("ARG x=1\n", &opts).unwrap(), "dockerfile");
    }

    #[test]
    fn shebang_alone_identifies_the_interpreter() {
        assert_eq!(id_of("#!/usr/bin/env python3\nx = compute()\n"), "python");
        assert_eq!(id_of("#!/usr/bin/env node\nrun()\n"), "javascript");
    }

    #[test]
    fn candidates_restrict_the_pool() {
        let opts = Options {
            candidates: "ruby, lua".into(),
            output: "language".into(),
            ..Options::default()
        };
        // Real Python, but Python is not on the list, so the best of ruby/lua wins.
        let got = detect_to_string(PYTHON, &opts).unwrap();
        assert!(got == "ruby" || got == "lua", "got {got}");
    }

    #[test]
    fn common_only_drops_niche_languages() {
        let d = detect(
            "defmodule A do\n  def b, do: 1\nend\n",
            &Options {
                common_only: true,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(d.candidates.iter().all(|c| c.id != "elixir"));
        assert!(d.notes.iter().any(|n| n.contains("common languages")));
    }

    #[test]
    fn top_k_zero_lists_every_scoring_language() {
        let d = detect(
            RUST,
            &Options {
                top_k: 0,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(d.candidates.len() > 3, "got {}", d.candidates.len());
        assert!(d.candidates.iter().all(|c| c.score > 0.0));
    }

    #[test]
    fn ambiguous_snippet_is_flagged_not_hidden() {
        let d = detect("x = 1", &Options::default()).unwrap();
        assert!(d.confidence < 0.75, "confidence {}", d.confidence);
        assert!(
            d.notes.iter().any(|n| n.contains("non-blank line")),
            "notes: {:?}",
            d.notes
        );
    }

    #[test]
    fn unmatched_snippet_is_unknown_not_an_error() {
        let d = detect("???\n@@@\n", &Options::default()).unwrap();
        assert_eq!(d.language, "unknown");
        assert_eq!(d.confidence, 0.0);
        assert_eq!(d.confidence_level, "none");
        assert!(d.notes.iter().any(|n| n.contains("No language signal")));
    }

    #[test]
    fn report_lists_candidates_and_evidence() {
        let out = detect_to_string(RUST, &Options::default()).unwrap();
        assert!(out.starts_with("Detected language: Rust (rust)"), "{out}");
        assert!(out.contains("Top candidates:"), "{out}");
        assert!(out.contains("Evidence for Rust:"), "{out}");
        assert!(out.contains("`let mut` binding"), "{out}");
    }

    #[test]
    fn explain_off_hides_the_evidence_section() {
        let out = detect_to_string(
            RUST,
            &Options {
                explain: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(!out.contains("Evidence for"), "{out}");
        assert!(out.contains("Top candidates:"), "{out}");
    }

    #[test]
    fn json_output_is_machine_readable() {
        let out = detect_to_string(
            PYTHON,
            &Options {
                output: "json".into(),
                ..Options::default()
            },
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["language"], "python");
        assert_eq!(v["language_name"], "Python");
        assert!(v["confidence"].as_f64().unwrap() > 0.5);
        assert!(v["candidates"][0]["evidence"].is_array());
        assert_eq!(v["stats"]["languages_considered"], LANGS.len());
    }

    #[test]
    fn empty_code_is_an_error() {
        let err = detect("   \n\t\n", &Options::default()).unwrap_err();
        assert!(err.contains("code is empty"), "{err}");
    }

    #[test]
    fn oversized_code_is_an_error() {
        let big = "a".repeat(MAX_CODE_BYTES + 1);
        let err = detect(&big, &Options::default()).unwrap_err();
        assert!(err.contains("too large"), "{err}");
    }

    #[test]
    fn unknown_candidate_id_is_an_error_that_lists_the_choices() {
        let err = detect(
            RUST,
            &Options {
                candidates: "rust, klingon".into(),
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("klingon"), "{err}");
        assert!(err.contains("typescript"), "{err}");
    }

    #[test]
    fn unknown_output_is_an_error() {
        let err = detect_to_string(
            RUST,
            &Options {
                output: "yaml".into(),
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(
            err.contains("expected one of report, json, language"),
            "{err}"
        );
    }

    #[test]
    fn top_k_above_the_cap_is_an_error() {
        let err = detect(
            RUST,
            &Options {
                top_k: MAX_TOP_K + 1,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("top_k is too large"), "{err}");
    }

    #[test]
    fn language_ids_are_unique() {
        let mut ids: Vec<&str> = LANGS.iter().map(|l| l.id).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len(), "duplicate language id in the table");
    }
}
