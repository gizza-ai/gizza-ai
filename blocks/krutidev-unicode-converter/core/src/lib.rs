//! core — pure compute, shared by the chat skill block and the web page.

pub mod mappings;

pub fn is_unicode(text: &str) -> bool {
    text.chars().any(|c| {
        let u = c as u32;
        u >= 0x0900 && u <= 0x097f
    })
}

fn is_unicode_vowel_sign(c: char) -> bool {
    match c {
        '\u{0901}'..='\u{0903}' | // ँ, ं, ः
        '\u{093e}'..='\u{094c}' => true,
        _ => false,
    }
}

fn is_unicode_consonant(c: char) -> bool {
    let u = c as u32;
    (u >= 0x0915 && u <= 0x0939) || // क to ह
    (u >= 0x0958 && u <= 0x095f) || // क़ to य़
    u == 0x0929 || u == 0x0931 || u == 0x0933 || u == 0x0934
}

pub fn krutidev_to_unicode(input: &str) -> Result<String, String> {
    let mut text = input.to_string();
    
    // 1. Initial space + conjunct alignments
    text = text.replace(" \u{00aa}", "\u{00aa}");
    text = text.replace(" ~j", "~j");
    text = text.replace(" z", "z");
    
    // 2. Main mappings (sort by length descending)
    let mut mappings = mappings::K2U_MAPPING.to_vec();
    mappings.sort_by_key(|&(kru, _)| std::cmp::Reverse(kru.len()));
    for &(kru, uni) in &mappings {
        text = text.replace(kru, uni);
    }
    
    // 3. Special replacements from Python script
    text = text.replace("\u{00b1}", "Z\u{0902}"); // ± -> Zं
    text = text.replace("\u{00c6}", "\u{0930}\u{094df}"); // Æ -> र्f
    
    // 4. Misplaced f correction:
    // f + character -> character + ि
    while let Some(f_idx) = text.find('f') {
        let rest = &text[f_idx + 1..];
        if let Some(next_char) = rest.chars().next() {
            let next_char_str = next_char.to_string();
            let target = format!("f{}", next_char_str);
            let replacement = format!("{}\u{093f}", next_char_str);
            text = text.replacen(&target, &replacement, 1);
        } else {
            text = text.replacen("f", "\u{093f}", 1);
        }
    }
    
    // 5. Special replacements
    text = text.replace("\u{00c7}", "fa"); // Ç -> fa
    text = text.replace("\u{00af}", "fa"); // ¯ -> fa
    text = text.replace("\u{00c9}", "\u{0930}\u{094dfa}"); // É -> र्fa
    
    // 6. fa + character -> character + िं
    while let Some(fa_idx) = text.find("fa") {
        let rest = &text[fa_idx + 2..];
        if let Some(next_char) = rest.chars().next() {
            let next_char_str = next_char.to_string();
            let target = format!("fa{}", next_char_str);
            let replacement = format!("{}\u{093f}\u{0902}", next_char_str);
            text = text.replacen(&target, &replacement, 1);
        } else {
            text = text.replacen("fa", "\u{093f}\u{0902}", 1);
        }
    }
    
    text = text.replace("\u{00ca}", "\u{0940}Z"); // Ê -> ीZ
    
    // 7. ि् + character -> ् + character + ि
    while let Some(idx) = text.find("\u{093f}\u{094d}") {
        let rest = &text[idx + 6..];
        if let Some(next_char) = rest.chars().next() {
            let next_char_str = next_char.to_string();
            let target = format!("\u{093f}\u{094d}{}", next_char_str);
            let replacement = format!("\u{094d}{}\u{093f}", next_char_str);
            text = text.replacen(&target, &replacement, 1);
        } else {
            break;
        }
    }
    
    text = text.replace("\u{094d}Z", "Z"); // ् + Z -> Z
    
    // 8. Reordering Z (reph):
    // For each 'Z' in the text, move it to the beginning of its consonant cluster and replace with 'र्' (\u0930\u094d).
    let mut chars: Vec<char> = text.chars().collect();
    while let Some(z_idx) = chars.iter().position(|&c| c == 'Z') {
        let mut idx = z_idx;
        
        // Step 1: Skip vowel signs
        while idx > 0 && is_unicode_vowel_sign(chars[idx - 1]) {
            idx -= 1;
        }
        
        // Step 2: Skip one consonant
        if idx > 0 && is_unicode_consonant(chars[idx - 1]) {
            idx -= 1;
        }
        
        // Step 3: Skip preceding (halant + consonant) pairs
        while idx > 1 && chars[idx - 1] == '\u{094d}' && is_unicode_consonant(chars[idx - 2]) {
            idx -= 2;
        }
        
        // Remove 'Z' and insert 'र्' at idx
        chars.remove(z_idx);
        chars.insert(idx, '\u{094d}');
        chars.insert(idx, '\u{0930}');
    }
    text = chars.into_iter().collect();
    
    // 9. Standard cleanups:
    // Illegal characters before matra
    let unattached_vowel_signs = [
        "\u{093e}", "\u{093f}", "\u{0940}", "\u{0941}", "\u{0942}", "\u{0943}",
        "\u{0947}", "\u{0948}", "\u{094b}", "\u{094c}", "\u{0902}", "\u{0903}",
        "\u{0901}", "\u{0945}"
    ];
    for &matra in &unattached_vowel_signs {
        text = text.replace(&format!(" {}", matra), matra);
        text = text.replace(&format!(",{}", matra), &format!("{},", matra));
        text = text.replace(&format!("\u{094d}{}", matra), matra);
    }
    
    text = text.replace("\u{094d}\u{094d}\u{0930}", "\u{094d}\u{0930}");
    text = text.replace("\u{094d}\u{0930}\u{094d}", "\u{0930}\u{094d}");
    text = text.replace("\u{094d}\u{094d}", "\u{094d}");
    text = text.replace("\u{094d} ", " ");
    
    Ok(text)
}

pub fn unicode_to_krutidev(input: &str) -> Result<String, String> {
    let mut text = input.to_string();
    
    // 1. Reorder 'ि' (chhoti-i): move to the front of the cluster as 'f'
    let mut chars: Vec<char> = text.chars().collect();
    while let Some(i_idx) = chars.iter().position(|&c| c == '\u{093f}') {
        let mut idx = i_idx;
        
        // Step 1: Skip preceding consonant
        if idx > 0 && is_unicode_consonant(chars[idx - 1]) {
            idx -= 1;
        }
        
        // Step 2: Skip preceding (halant + consonant) pairs
        while idx > 1 && chars[idx - 1] == '\u{094d}' && is_unicode_consonant(chars[idx - 2]) {
            idx -= 2;
        }
        
        // Replace 'ि' with 'f' at the start of the cluster
        chars.remove(i_idx);
        chars.insert(idx, 'f');
    }
    
    // 2. Reorder 'र्' (reph): move to the end of the cluster as 'Z'
    while let Some(r_idx) = find_sequence_chars(&chars, &['\u{0930}', '\u{094d}']) {
        let mut idx = r_idx + 2;
        
        // Step 1: Skip one consonant
        if idx < chars.len() && is_unicode_consonant(chars[idx]) {
            idx += 1;
            
            // Step 2: Skip preceding halant + consonant pairs
            while idx + 1 < chars.len() && chars[idx] == '\u{094d}' && is_unicode_consonant(chars[idx + 1]) {
                idx += 2;
            }
        }
        
        // Step 3: Skip vowel signs/modifiers
        while idx < chars.len() && is_unicode_vowel_sign(chars[idx]) {
            idx += 1;
        }
        
        // Replace '\u{0930}\u{094d}' with 'Z' at the end of the cluster
        chars.remove(r_idx + 1); // remove '\u{094d}'
        chars.remove(r_idx);     // remove '\u{0930}'
        chars.insert(idx - 2, 'Z');
    }
    text = chars.into_iter().collect();
    
    // 3. Perform character replacements from Unicode to Krutidev mapping
    let mut mappings = mappings::U2K_MAPPING.to_vec();
    mappings.sort_by_key(|&(uni, _)| std::cmp::Reverse(uni.len()));
    for &(uni, kru) in &mappings {
        text = text.replace(uni, kru);
    }
    
    Ok(text)
}

fn find_sequence_chars(chars: &[char], seq: &[char]) -> Option<usize> {
    if seq.is_empty() { return None; }
    for i in 0..=chars.len().saturating_sub(seq.len()) {
        if &chars[i..i+seq.len()] == seq {
            return Some(i);
        }
    }
    None
}

pub fn run(input: &str) -> Result<String, String> {
    if is_unicode(input) {
        unicode_to_krutidev(input)
    } else {
        krutidev_to_unicode(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_unicode() {
        assert!(is_unicode("नमस्ते"));
        assert!(!is_unicode("uesrs"));
    }

    #[test]
    fn test_krutidev_to_unicode() {
        // "मेरा नाम नेहल है।" in Krutidev: "esjk uke usgy gSA"
        let res = krutidev_to_unicode("esjk uke usgy gSA").unwrap();
        assert_eq!(res, "मेरा नाम नेहल है।");
    }

    #[test]
    fn test_unicode_to_krutidev() {
        let res = unicode_to_krutidev("मेरा नाम नेहल है।").unwrap();
        assert_eq!(res, "esjk uke usgy gSA");
    }

    #[test]
    fn test_reph_reordering() {
        // "धर्म" in Unicode -> "/keZ" in Kruti Dev
        let to_uni = krutidev_to_unicode("/keZ").unwrap();
        assert_eq!(to_uni, "धर्म");

        let to_kru = unicode_to_krutidev("धर्म").unwrap();
        assert_eq!(to_kru, "/keZ");
    }

    #[test]
    fn test_chhoti_e_reordering() {
        // "स्थिर" in Unicode -> "fLFkj" in Kruti Dev
        let to_uni = krutidev_to_unicode("fLFkj").unwrap();
        assert_eq!(to_uni, "स्थिर");

        let to_kru = unicode_to_krutidev("स्थिर").unwrap();
        assert_eq!(to_kru, "fLFkj");
    }
}
