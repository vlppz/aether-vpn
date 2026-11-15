//! Best-effort country detection from human-written server names such as
//! `🇩🇪 Frankfurt #2`, `[NL] Amsterdam` or `Финляндия`.

const NAMES: &[(&str, &[&str])] = &[
    ("US", &["united states", "usa", "america", "сша", "new york", "los angeles", "miami"]),
    ("GB", &["united kingdom", "england", "london", "великобритания", "лондон"]),
    ("DE", &["germany", "frankfurt", "berlin", "германия", "франкфурт"]),
    ("NL", &["netherlands", "amsterdam", "нидерланды", "голландия", "амстердам"]),
    ("FI", &["finland", "helsinki", "финляндия", "хельсинки"]),
    ("FR", &["france", "paris", "франция", "париж"]),
    ("SE", &["sweden", "stockholm", "швеция"]),
    ("PL", &["poland", "warsaw", "польша"]),
    ("RU", &["russia", "moscow", "россия", "москва"]),
    ("KZ", &["kazakhstan", "almaty", "казахстан"]),
    ("TR", &["turkey", "türkiye", "istanbul", "турция", "стамбул"]),
    ("JP", &["japan", "tokyo", "япония", "токио"]),
    ("SG", &["singapore", "сингапур"]),
    ("HK", &["hong kong", "гонконг"]),
    ("CA", &["canada", "toronto", "канада"]),
    ("CH", &["switzerland", "zurich", "швейцария"]),
    ("AT", &["austria", "vienna", "австрия"]),
    ("LV", &["latvia", "riga", "латвия"]),
    ("LT", &["lithuania", "vilnius", "литва"]),
    ("EE", &["estonia", "tallinn", "эстония"]),
    ("AE", &["emirates", "dubai", "uae", "оаэ", "дубай"]),
    ("IN", &["india", "mumbai", "индия"]),
    ("KR", &["korea", "seoul", "корея"]),
    ("AM", &["armenia", "yerevan", "армения"]),
    ("GE", &["georgia", "tbilisi", "грузия"]),
    ("ES", &["spain", "madrid", "испания"]),
    ("IT", &["italy", "milan", "италия"]),
    ("BG", &["bulgaria", "sofia", "болгария"]),
    ("RS", &["serbia", "belgrade", "сербия"]),
    ("CZ", &["czech", "prague", "чехия"]),
];

/// Returns an uppercase ISO 3166-1 alpha-2 code, if one can be inferred.
pub fn detect(name: &str) -> Option<String> {
    from_flag(name)
        .or_else(|| from_tag(name))
        .or_else(|| from_names(name))
}

/// Decodes the first pair of regional-indicator symbols (a flag emoji).
fn from_flag(name: &str) -> Option<String> {
    const BASE: u32 = 0x1F1E6;
    let chars: Vec<u32> = name.chars().map(|c| c as u32).collect();
    chars.windows(2).find_map(|w| {
        let is_ri = |c: u32| (BASE..BASE + 26).contains(&c);
        (is_ri(w[0]) && is_ri(w[1])).then(|| {
            [w[0], w[1]]
                .iter()
                .map(|c| char::from_u32(c - BASE + 'A' as u32).unwrap())
                .collect()
        })
    })
}

/// Matches standalone two-letter tags like `[DE]`, `DE-1`, `nl_ams`.
fn from_tag(name: &str) -> Option<String> {
    name.split(|c: char| !c.is_ascii_alphabetic())
        .filter(|w| w.len() == 2 && w.chars().all(|c| c.is_ascii_uppercase()))
        .find(|w| NAMES.iter().any(|(code, _)| code == w) || *w == "UK")
        .map(|w| if w == "UK" { "GB".into() } else { w.to_string() })
}

fn from_names(name: &str) -> Option<String> {
    let lower = name.to_lowercase();
    NAMES
        .iter()
        .find(|(_, words)| words.iter().any(|w| lower.contains(w)))
        .map(|(code, _)| code.to_string())
}

/// Converts an ISO code back into a flag emoji.
pub fn flag(code: &str) -> String {
    code.to_ascii_uppercase()
        .chars()
        .filter_map(|c| char::from_u32(0x1F1E6 + (c as u32).wrapping_sub('A' as u32)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_flags_tags_and_names() {
        assert_eq!(detect("🇩🇪 Frankfurt #2").as_deref(), Some("DE"));
        assert_eq!(detect("Fast 🇳🇱").as_deref(), Some("NL"));
        assert_eq!(detect("[FI] Helsinki").as_deref(), Some("FI"));
        assert_eq!(detect("UK-London-1").as_deref(), Some("GB"));
        assert_eq!(detect("Финляндия Premium").as_deref(), Some("FI"));
        assert_eq!(detect("random node").as_deref(), None);
        // two uppercase letters that are not a known code must not match
        assert_eq!(detect("VPN XX node"), None);
    }

    #[test]
    fn flag_round_trip() {
        assert_eq!(flag("de"), "🇩🇪");
        assert_eq!(detect(&flag("JP")).as_deref(), Some("JP"));
    }
}
