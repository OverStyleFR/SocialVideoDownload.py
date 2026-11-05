//! Détection de plateforme et arguments yt-dlp spécifiques (un fichier par plateforme).

use url::Url;

pub mod generic;
pub mod instagram;
pub mod tiktok;
pub mod twitter;
pub mod youtube;

/// User-Agent Chrome repris de l'ancienne configuration Python.
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36";

/// Plateforme reconnue d'après l'hôte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    TikTok,
    Instagram,
    Twitter,
    YouTube,
    Generic,
}

/// Vrai si `host` est `domain` ou un sous-domaine de `domain`.
pub(crate) fn host_is(host: &str, domain: &str) -> bool {
    let host = host.to_ascii_lowercase();
    host == domain || host.ends_with(&format!(".{domain}"))
}

/// Détecte la plateforme d'une URL.
pub fn detect(url: &Url) -> Platform {
    let Some(host) = url.host_str() else {
        return Platform::Generic;
    };
    if tiktok::matches(host) {
        Platform::TikTok
    } else if instagram::matches(host) {
        Platform::Instagram
    } else if twitter::matches(host) {
        Platform::Twitter
    } else if youtube::matches(host) {
        Platform::YouTube
    } else {
        Platform::Generic
    }
}

fn is_tracking(platform: Platform, name: &str) -> bool {
    name.starts_with("utm_")
        || matches!(name, "fbclid" | "gclid")
        || match platform {
            Platform::TikTok => tiktok::is_tracking(name),
            Platform::Instagram => instagram::is_tracking(name),
            Platform::Twitter => twitter::is_tracking(name),
            Platform::YouTube => youtube::is_tracking(name),
            Platform::Generic => generic::is_tracking(name),
        }
}

/// Forme canonique de l'URL (sans suivi ni fragment) : sert de clé de cache.
pub fn normalize(url: &Url) -> Url {
    let platform = detect(url);
    let mut out = url.clone();
    out.set_fragment(None);
    let kept: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(k, _)| !is_tracking(platform, k))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    if kept.is_empty() {
        out.set_query(None);
    } else {
        out.query_pairs_mut().clear().extend_pairs(kept);
    }
    out
}

/// Arguments yt-dlp propres à la plateforme (User-Agent, Referer…).
pub fn ytdlp_args(platform: Platform) -> Vec<String> {
    let mut args = vec!["--user-agent".to_string(), USER_AGENT.to_string()];
    let referer = match platform {
        Platform::TikTok => Some(tiktok::REFERER),
        Platform::Instagram => Some(instagram::REFERER),
        Platform::Twitter => Some(twitter::REFERER),
        Platform::YouTube => Some(youtube::REFERER),
        Platform::Generic => None,
    };
    if let Some(r) = referer {
        args.push("--referer".into());
        args.push(r.into());
    }
    args
}

/// Extrait les URL http(s) d'un texte libre, dans l'ordre, sans doublon.
pub fn extract_urls(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for tok in text.split(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"')) {
        let Some(start) = tok.find("http://").or_else(|| tok.find("https://")) else {
            continue;
        };
        let u = tok[start..].trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '\'']);
        if Url::parse(u).is_ok() && !out.iter().any(|x| x == u) {
            out.push(u.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn detects_five_platforms() {
        assert_eq!(detect(&u("https://vm.tiktok.com/ZM123/")), Platform::TikTok);
        assert_eq!(
            detect(&u("https://www.tiktok.com/@a/video/1")),
            Platform::TikTok
        );
        assert_eq!(
            detect(&u("https://www.instagram.com/reel/abc/")),
            Platform::Instagram
        );
        assert_eq!(detect(&u("https://x.com/a/status/1")), Platform::Twitter);
        assert_eq!(
            detect(&u("https://twitter.com/a/status/1")),
            Platform::Twitter
        );
        assert_eq!(detect(&u("https://youtu.be/abc")), Platform::YouTube);
        assert_eq!(
            detect(&u("https://m.youtube.com/watch?v=a")),
            Platform::YouTube
        );
        assert_eq!(detect(&u("https://example.com/v")), Platform::Generic);
        assert_eq!(detect(&u("https://nottiktok.com/v")), Platform::Generic);
        assert_eq!(
            detect(&u("https://tiktok.com.evil.test/v")),
            Platform::Generic
        );
    }

    #[test]
    fn normalizes_tracking() {
        let n = |s: &str| normalize(&u(s)).to_string();
        assert_eq!(
            n("https://www.instagram.com/reel/a/?igsh=xyz&utm_source=ig"),
            "https://www.instagram.com/reel/a/"
        );
        assert_eq!(
            n("https://youtu.be/abc?si=zzz&t=30"),
            "https://youtu.be/abc?t=30"
        );
        assert_eq!(
            n("https://x.com/a/status/1?s=20&t=k#frag"),
            "https://x.com/a/status/1"
        );
        assert_eq!(
            n("https://www.tiktok.com/@a/video/1?_t=1&_r=1&is_from_webapp=1"),
            "https://www.tiktok.com/@a/video/1"
        );
        assert_eq!(
            n("https://www.youtube.com/watch?v=a&feature=share"),
            "https://www.youtube.com/watch?v=a"
        );
        assert_eq!(
            n("https://example.com/v?id=3&utm_medium=x"),
            "https://example.com/v?id=3"
        );
    }

    #[test]
    fn args_per_platform() {
        let a = ytdlp_args(Platform::TikTok);
        assert!(a.contains(&"https://www.tiktok.com/".to_string()));
        assert!(a.iter().any(|x| x.contains("Chrome/128")));
        assert!(!ytdlp_args(Platform::Generic).contains(&"--referer".to_string()));
        assert!(ytdlp_args(Platform::Twitter).contains(&"https://x.com/".to_string()));
    }

    #[test]
    fn extracts_urls_from_mixed_message() {
        let t = "Regarde ça https://vm.tiktok.com/ZM1/, et (https://youtu.be/abc). Aussi http://example.com/a?b=1! doublon https://youtu.be/abc ftp://x.y/z";
        assert_eq!(
            extract_urls(t),
            vec![
                "https://vm.tiktok.com/ZM1/",
                "https://youtu.be/abc",
                "http://example.com/a?b=1"
            ]
        );
        assert!(extract_urls("rien ici").is_empty());
    }
}
