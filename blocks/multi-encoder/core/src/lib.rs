//! gizza-ai/multi-encoder core — encode/decode text across Base64, hex, binary,
//! URL percent-encoding, ROT13, and Morse code. No wafer/wasm-bindgen deps
//! beyond base64 + percent-encoding (both proven wasm-safe).

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use percent_encoding::{percent_decode_str, utf8_percent_encode, NON_ALPHANUMERIC};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Base64,
    Hex,
    Binary,
    Url,
    Rot13,
    Morse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Encode,
    Decode,
}

pub fn parse_encoding(s: &str) -> Result<Encoding, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "base64" | "b64" => Ok(Encoding::Base64),
        "hex" => Ok(Encoding::Hex),
        "binary" | "bin" => Ok(Encoding::Binary),
        "url" | "urlencode" | "percent" => Ok(Encoding::Url),
        "rot13" => Ok(Encoding::Rot13),
        "morse" => Ok(Encoding::Morse),
        other => Err(format!("encoding {other:?} not supported (base64|hex|binary|url|rot13|morse)")),
    }
}

pub fn parse_direction(s: &str) -> Result<Direction, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "encode" => Ok(Direction::Encode),
        "decode" => Ok(Direction::Decode),
        other => Err(format!("direction {other:?} not supported (encode|decode)")),
    }
}

fn rot13(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='z' => (((c as u8 - b'a' + 13) % 26) + b'a') as char,
            'A'..='Z' => (((c as u8 - b'A' + 13) % 26) + b'A') as char,
            _ => c,
        })
        .collect()
}

// Morse table for A-Z, 0-9, and common punctuation.
const MORSE: &[(char, &str)] = &[
    ('A', ".-"), ('B', "-..."), ('C', "-.-."), ('D', "-.."), ('E', "."), ('F', "..-."),
    ('G', "--."), ('H', "...."), ('I', ".."), ('J', ".---"), ('K', "-.-"), ('L', ".-.."),
    ('M', "--"), ('N', "-."), ('O', "---"), ('P', ".--."), ('Q', "--.-"), ('R', ".-."),
    ('S', "..."), ('T', "-"), ('U', "..-"), ('V', "...-"), ('W', ".--"), ('X', "-..-"),
    ('Y', "-.--"), ('Z', "--.."),
    ('0', "-----"), ('1', ".----"), ('2', "..---"), ('3', "...--"), ('4', "....-"),
    ('5', "....."), ('6', "-...."), ('7', "--..."), ('8', "---.."), ('9', "----."),
    ('.', ".-.-.-"), (',', "--..--"), ('?', "..--.."), ('\'', ".----."), ('!', "-.-.--"),
    ('/', "-..-."), ('(', "-.--."), (')', "-.--.-"), ('&', ".-..."), (':', "---..."),
    (';', "-.-.-."), ('=', "-...-"), ('+', ".-.-."), ('-', "-....-"), ('_', "..--.-"),
    ('"', ".-..-."), ('$', "...-..-"), ('@', ".--.-."),
];

fn morse_encode(s: &str) -> Result<String, String> {
    let mut words: Vec<String> = Vec::new();
    for word in s.split_whitespace() {
        let mut letters = Vec::new();
        for ch in word.chars() {
            let up = ch.to_ascii_uppercase();
            match MORSE.iter().find(|(c, _)| *c == up) {
                Some((_, code)) => letters.push(code.to_string()),
                None => return Err(format!("character '{ch}' has no Morse representation")),
            }
        }
        words.push(letters.join(" "));
    }
    Ok(words.join(" / "))
}

fn morse_decode(s: &str) -> Result<String, String> {
    let mut out = Vec::new();
    for word in s.trim().split('/') {
        let mut letters = String::new();
        for code in word.split_whitespace() {
            match MORSE.iter().find(|(_, m)| *m == code) {
                Some((c, _)) => letters.push(*c),
                None => return Err(format!("'{code}' is not valid Morse")),
            }
        }
        if !letters.is_empty() {
            out.push(letters);
        }
    }
    Ok(out.join(" "))
}

/// Transform `text` with `enc` in `dir`.
pub fn transform(text: &str, enc: Encoding, dir: Direction) -> Result<String, String> {
    match (enc, dir) {
        (Encoding::Base64, Direction::Encode) => Ok(B64.encode(text.as_bytes())),
        (Encoding::Base64, Direction::Decode) => {
            let bytes = B64.decode(text.trim()).map_err(|e| format!("invalid base64: {e}"))?;
            String::from_utf8(bytes).map_err(|_| "decoded bytes are not valid UTF-8".to_string())
        }
        (Encoding::Hex, Direction::Encode) => {
            Ok(text.as_bytes().iter().map(|b| format!("{b:02x}")).collect())
        }
        (Encoding::Hex, Direction::Decode) => {
            let clean: String = text.chars().filter(|c| !c.is_whitespace()).collect();
            if clean.len() % 2 != 0 {
                return Err("hex input has an odd number of digits".into());
            }
            let mut bytes = Vec::with_capacity(clean.len() / 2);
            let chars: Vec<char> = clean.chars().collect();
            for pair in chars.chunks(2) {
                let s: String = pair.iter().collect();
                bytes.push(u8::from_str_radix(&s, 16).map_err(|_| format!("invalid hex byte '{s}'"))?);
            }
            String::from_utf8(bytes).map_err(|_| "decoded bytes are not valid UTF-8".to_string())
        }
        (Encoding::Binary, Direction::Encode) => {
            Ok(text.as_bytes().iter().map(|b| format!("{b:08b}")).collect::<Vec<_>>().join(" "))
        }
        (Encoding::Binary, Direction::Decode) => {
            let mut bytes = Vec::new();
            for tok in text.split_whitespace() {
                bytes.push(u8::from_str_radix(tok, 2).map_err(|_| format!("invalid binary byte '{tok}'"))?);
            }
            String::from_utf8(bytes).map_err(|_| "decoded bytes are not valid UTF-8".to_string())
        }
        (Encoding::Url, Direction::Encode) => {
            Ok(utf8_percent_encode(text, NON_ALPHANUMERIC).to_string())
        }
        (Encoding::Url, Direction::Decode) => percent_decode_str(text)
            .decode_utf8()
            .map(|c| c.into_owned())
            .map_err(|_| "decoded bytes are not valid UTF-8".to_string()),
        (Encoding::Rot13, _) => Ok(rot13(text)), // symmetric
        (Encoding::Morse, Direction::Encode) => morse_encode(text),
        (Encoding::Morse, Direction::Decode) => morse_decode(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enc(t: &str, e: Encoding) -> String {
        transform(t, e, Direction::Encode).unwrap()
    }
    fn dec(t: &str, e: Encoding) -> String {
        transform(t, e, Direction::Decode).unwrap()
    }

    #[test]
    fn base64_round_trip() {
        assert_eq!(enc("hi", Encoding::Base64), "aGk=");
        assert_eq!(dec("aGk=", Encoding::Base64), "hi");
    }

    #[test]
    fn hex_round_trip() {
        assert_eq!(enc("hi", Encoding::Hex), "6869");
        assert_eq!(dec("68 69", Encoding::Hex), "hi"); // whitespace tolerated
    }

    #[test]
    fn binary_round_trip() {
        assert_eq!(enc("A", Encoding::Binary), "01000001");
        assert_eq!(dec("01000001 01000010", Encoding::Binary), "AB");
    }

    #[test]
    fn url_round_trip() {
        assert_eq!(enc("a b&c", Encoding::Url), "a%20b%26c");
        assert_eq!(dec("a%20b%26c", Encoding::Url), "a b&c");
    }

    #[test]
    fn rot13_is_symmetric() {
        assert_eq!(enc("Hello, World!", Encoding::Rot13), "Uryyb, Jbeyq!");
        assert_eq!(dec("Uryyb, Jbeyq!", Encoding::Rot13), "Hello, World!");
    }

    #[test]
    fn morse_round_trip() {
        assert_eq!(enc("SOS", Encoding::Morse), "... --- ...");
        assert_eq!(enc("HI ALL", Encoding::Morse), ".... .. / .- .-.. .-..");
        assert_eq!(dec(".... .. / .- .-.. .-..", Encoding::Morse), "HI ALL");
    }

    #[test]
    fn errors() {
        assert!(transform("not!base64", Encoding::Base64, Direction::Decode).is_err());
        assert!(transform("abc", Encoding::Hex, Direction::Decode).is_err()); // odd length
        assert!(transform("012", Encoding::Binary, Direction::Decode).is_err()); // not 8-bit/binary
        assert!(transform("§", Encoding::Morse, Direction::Encode).is_err());
        assert!(parse_encoding("rot47").is_err());
        assert!(parse_direction("sideways").is_err());
    }
}
