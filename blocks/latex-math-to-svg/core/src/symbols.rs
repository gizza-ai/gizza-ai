//! LaTeX command → Unicode glyph table + spacing categories, for the math
//! renderer. Pure data; no deps.

/// Spacing/atom category used to pick inter-atom spacing.
#[derive(Clone, Copy)]
pub enum Cat {
    Ord,
    Bin,
    Rel,
    Punct,
    Open,
}

/// Functions that render as upright multi-letter operators (\sin, \log, ...).
pub fn function_name(name: &str) -> Option<&'static str> {
    Some(match name {
        "sin" => "sin",
        "cos" => "cos",
        "tan" => "tan",
        "cot" => "cot",
        "sec" => "sec",
        "csc" => "csc",
        "arcsin" => "arcsin",
        "arccos" => "arccos",
        "arctan" => "arctan",
        "sinh" => "sinh",
        "cosh" => "cosh",
        "tanh" => "tanh",
        "coth" => "coth",
        "log" => "log",
        "ln" => "ln",
        "lg" => "lg",
        "exp" => "exp",
        "det" => "det",
        "dim" => "dim",
        "ker" => "ker",
        "deg" => "deg",
        "gcd" => "gcd",
        "hom" => "hom",
        "arg" => "arg",
        "mod" => "mod",
        "bmod" => "mod",
        "min" => "min",
        "max" => "max",
        "sup" => "sup",
        "inf" => "inf",
        "lim" => "lim",
        "limsup" => "lim sup",
        "liminf" => "lim inf",
        "Pr" => "Pr",
        _ => return None,
    })
}

/// Big operators that can carry above/below limits.
pub fn is_big_op(name: &str) -> bool {
    matches!(
        name,
        "sum"
            | "prod"
            | "coprod"
            | "int"
            | "iint"
            | "iiint"
            | "oint"
            | "bigcup"
            | "bigcap"
            | "bigvee"
            | "bigwedge"
            | "bigoplus"
            | "bigotimes"
            | "bigodot"
            | "biguplus"
            | "bigsqcup"
    )
}

/// Lookup the Unicode glyph for a LaTeX command name (no backslash).
pub fn lookup_symbol(name: &str) -> Option<String> {
    let g = match name {
        // lowercase Greek
        "alpha" => "α",
        "beta" => "β",
        "gamma" => "γ",
        "delta" => "δ",
        "epsilon" => "ϵ",
        "varepsilon" => "ε",
        "zeta" => "ζ",
        "eta" => "η",
        "theta" => "θ",
        "vartheta" => "ϑ",
        "iota" => "ι",
        "kappa" => "κ",
        "lambda" => "λ",
        "mu" => "μ",
        "nu" => "ν",
        "xi" => "ξ",
        "omicron" => "ο",
        "pi" => "π",
        "varpi" => "ϖ",
        "rho" => "ρ",
        "varrho" => "ϱ",
        "sigma" => "σ",
        "varsigma" => "ς",
        "tau" => "τ",
        "upsilon" => "υ",
        "phi" => "ϕ",
        "varphi" => "φ",
        "chi" => "χ",
        "psi" => "ψ",
        "omega" => "ω",
        // uppercase Greek
        "Gamma" => "Γ",
        "Delta" => "Δ",
        "Theta" => "Θ",
        "Lambda" => "Λ",
        "Xi" => "Ξ",
        "Pi" => "Π",
        "Sigma" => "Σ",
        "Upsilon" => "Υ",
        "Phi" => "Φ",
        "Psi" => "Ψ",
        "Omega" => "Ω",
        // big operators
        "sum" => "∑",
        "prod" => "∏",
        "coprod" => "∐",
        "int" => "∫",
        "iint" => "∬",
        "iiint" => "∭",
        "oint" => "∮",
        "bigcup" => "⋃",
        "bigcap" => "⋂",
        "bigvee" => "⋁",
        "bigwedge" => "⋀",
        "bigoplus" => "⨁",
        "bigotimes" => "⨂",
        "bigodot" => "⨀",
        "biguplus" => "⨄",
        "bigsqcup" => "⨆",
        // relations
        "leq" | "le" => "≤",
        "geq" | "ge" => "≥",
        "neq" | "ne" => "≠",
        "equiv" => "≡",
        "approx" => "≈",
        "cong" => "≅",
        "sim" => "∼",
        "simeq" => "≃",
        "propto" => "∝",
        "ll" => "≪",
        "gg" => "≫",
        "subset" => "⊂",
        "supset" => "⊃",
        "subseteq" => "⊆",
        "supseteq" => "⊇",
        "in" => "∈",
        "notin" => "∉",
        "ni" => "∋",
        "mid" => "∣",
        "parallel" => "∥",
        "perp" => "⊥",
        "models" => "⊨",
        "vdash" => "⊢",
        "doteq" => "≐",
        "prec" => "≺",
        "succ" => "≻",
        "preceq" => "⪯",
        "succeq" => "⪰",
        "asymp" => "≍",
        // arrows (treated as relations)
        "to" | "rightarrow" => "→",
        "leftarrow" | "gets" => "←",
        "leftrightarrow" => "↔",
        "Rightarrow" | "implies" => "⇒",
        "Leftarrow" => "⇐",
        "Leftrightarrow" | "iff" => "⇔",
        "mapsto" => "↦",
        "uparrow" => "↑",
        "downarrow" => "↓",
        "longrightarrow" => "⟶",
        "longleftarrow" => "⟵",
        "hookrightarrow" => "↪",
        // binary operators
        "times" => "×",
        "div" => "÷",
        "pm" => "±",
        "mp" => "∓",
        "cdot" => "⋅",
        "ast" => "∗",
        "star" => "⋆",
        "circ" => "∘",
        "bullet" => "∙",
        "oplus" => "⊕",
        "ominus" => "⊖",
        "otimes" => "⊗",
        "oslash" => "⊘",
        "odot" => "⊙",
        "cap" => "∩",
        "cup" => "∪",
        "uplus" => "⊎",
        "sqcap" => "⊓",
        "sqcup" => "⊔",
        "wedge" | "land" => "∧",
        "vee" | "lor" => "∨",
        "setminus" => "∖",
        "wr" => "≀",
        "dagger" => "†",
        "ddagger" => "‡",
        "amalg" => "⨿",
        "triangleleft" => "◁",
        "triangleright" => "▷",
        // miscellaneous ordinary symbols
        "infty" => "∞",
        "partial" => "∂",
        "nabla" => "∇",
        "forall" => "∀",
        "exists" => "∃",
        "nexists" => "∄",
        "emptyset" | "varnothing" => "∅",
        "neg" | "lnot" => "¬",
        "angle" => "∠",
        "triangle" => "△",
        "square" => "□",
        "Box" => "□",
        "diamond" => "◇",
        "ell" => "ℓ",
        "hbar" => "ℏ",
        "Re" => "ℜ",
        "Im" => "ℑ",
        "aleph" => "ℵ",
        "wp" => "℘",
        "prime" => "′",
        "dots" | "ldots" => "…",
        "cdots" => "⋯",
        "vdots" => "⋮",
        "ddots" => "⋱",
        "surd" => "√",
        "top" => "⊤",
        "bot" => "⊥",
        "flat" => "♭",
        "sharp" => "♯",
        "natural" => "♮",
        "clubsuit" => "♣",
        "diamondsuit" => "♢",
        "heartsuit" => "♡",
        "spadesuit" => "♠",
        "checkmark" => "✓",
        "degree" => "°",
        "S" => "§",
        "P" => "¶",
        "copyright" => "©",
        "pounds" => "£",
        "euro" => "€",
        "complement" => "∁",
        "mathbb" => "", // handled? fall through as text otherwise
        // blackboard-ish common letters via dedicated commands
        "N" => "ℕ",
        "Z" => "ℤ",
        "Q" => "ℚ",
        "R" => "ℝ",
        "C" => "ℂ",
        "H" => "ℍ",
        // punctuation
        "colon" => ":",
        "ldotp" => ".",
        "cdotp" => "·",
        _ => return None,
    };
    if g.is_empty() {
        return None;
    }
    Some(g.to_string())
}

/// Spacing category for a known symbol name.
pub fn category(name: &str) -> Cat {
    match name {
        // relations
        "leq" | "le" | "geq" | "ge" | "neq" | "ne" | "equiv" | "approx" | "cong" | "sim"
        | "simeq" | "propto" | "ll" | "gg" | "subset" | "supset" | "subseteq" | "supseteq"
        | "in" | "notin" | "ni" | "mid" | "parallel" | "perp" | "models" | "vdash" | "doteq"
        | "prec" | "succ" | "preceq" | "succeq" | "asymp" | "to" | "rightarrow" | "leftarrow"
        | "gets" | "leftrightarrow" | "Rightarrow" | "implies" | "Leftarrow" | "Leftrightarrow"
        | "iff" | "mapsto" | "longrightarrow" | "longleftarrow" | "hookrightarrow"
        | "uparrow" | "downarrow" => Cat::Rel,
        // binary
        "times" | "div" | "pm" | "mp" | "cdot" | "ast" | "star" | "circ" | "bullet" | "oplus"
        | "ominus" | "otimes" | "oslash" | "odot" | "cap" | "cup" | "uplus" | "sqcap" | "sqcup"
        | "wedge" | "land" | "vee" | "lor" | "setminus" | "wr" | "dagger" | "ddagger" | "amalg"
        | "triangleleft" | "triangleright" => Cat::Bin,
        // punctuation
        "colon" | "ldotp" | "cdotp" | "dots" | "ldots" | "cdots" => Cat::Punct,
        _ => Cat::Ord,
    }
}
