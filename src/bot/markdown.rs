//! Échappement MarkdownV2 de Telegram.

/// Caractères réservés de MarkdownV2, antislash compris.
const RESERVED: &str = "\\_*[]()~`>#+-=|{}.!";

/// Échappe tous les caractères réservés de MarkdownV2 (et l'antislash).
pub fn escape_md_v2(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        if RESERVED.contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::escape_md_v2;

    #[test]
    fn escapes_every_reserved_character() {
        let all = "_*[]()~`>#+-=|{}.!";
        assert_eq!(all.chars().count(), 18);
        for c in all.chars() {
            assert_eq!(
                escape_md_v2(&c.to_string()),
                format!("\\{c}"),
                "caractère {c}"
            );
        }
        assert_eq!(escape_md_v2("\\"), "\\\\");
    }

    #[test]
    fn escapes_file_name_and_keeps_plain_text() {
        assert_eq!(escape_md_v2("a_b*[c].mp4"), "a\\_b\\*\\[c\\]\\.mp4");
        assert_eq!(escape_md_v2("vidéo 42 😀"), "vidéo 42 😀");
        assert_eq!(escape_md_v2(""), "");
    }
}
