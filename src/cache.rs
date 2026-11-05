//! Cache des file_id Telegram par URL normalisée et mode (TTL + borne de taille).

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use url::Url;

use crate::download::Mode;

/// Clé du cache : URL normalisée + mode.
pub type CacheKey = (String, Mode);

/// Paramètres de suivi sans effet sur le contenu (en plus des `utm_*`).
const TRACKING: [&str; 4] = ["fbclid", "gclid", "igshid", "si"];

/// Cache en mémoire, seul mécanisme de réutilisation des envois.
// shortcut: mémoire seule, pas de persistance disque ; à ajouter si les redémarrages coûtent des renvois.
#[derive(Debug)]
pub struct ResultCache {
    ttl: Duration,
    max_entries: usize,
    entries: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    map: HashMap<CacheKey, (String, Instant)>,
    /// Ordre d'insertion (éviction du plus ancien) ; peut garder des clés déjà expirées.
    order: VecDeque<CacheKey>,
}

impl ResultCache {
    /// Crée un cache de durée de vie `ttl` et d'au plus `max_entries` entrées.
    pub fn new(ttl: Duration, max_entries: usize) -> Self {
        Self {
            ttl,
            max_entries,
            entries: Mutex::new(Inner::default()),
        }
    }

    /// Normalise une URL pour la clé (hôte minuscule, fragment et paramètres de suivi retirés).
    pub fn normalize(url: &Url) -> String {
        let mut u = url.clone(); // `Url` met déjà schéma et hôte en minuscules
        u.set_fragment(None);
        let kept: Vec<(String, String)> = u
            .query_pairs()
            .filter(|(k, _)| !k.starts_with("utm_") && !TRACKING.contains(&&**k))
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        if kept.is_empty() {
            u.set_query(None);
        } else {
            u.query_pairs_mut().clear().extend_pairs(kept);
        }
        u.into()
    }

    /// Rend le file_id encore valide pour (url, mode).
    pub fn get(&self, url: &Url, mode: Mode) -> Option<String> {
        self.get_at(url, mode, Instant::now())
    }

    /// Enregistre le file_id pour (url, mode), en évinçant si la borne est atteinte.
    pub fn put(&self, url: &Url, mode: Mode, file_id: String) {
        self.put_at(url, mode, file_id, Instant::now());
    }

    fn get_at(&self, url: &Url, mode: Mode, now: Instant) -> Option<String> {
        let key = (Self::normalize(url), mode);
        let mut g = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        match g.map.get(&key) {
            Some((id, at)) if now.duration_since(*at) < self.ttl => Some(id.clone()),
            Some(_) => {
                g.map.remove(&key); // la clé reste dans `order`, purgée plus tard
                None
            }
            None => None,
        }
    }

    fn put_at(&self, url: &Url, mode: Mode, file_id: String, now: Instant) {
        if self.max_entries == 0 {
            return;
        }
        let key = (Self::normalize(url), mode);
        let mut g = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        if g.map.insert(key.clone(), (file_id, now)).is_none() {
            g.order.push_back(key);
        }
        while g.map.len() > self.max_entries {
            if let Some(old) = g.order.pop_front() {
                g.map.remove(&old);
            }
        }
        if g.order.len() > 2 * self.max_entries {
            let Inner { map, order } = &mut *g;
            order.retain(|k| map.contains_key(k));
        }
    }

    /// Nombre d'entrées (y compris expirées non encore purgées).
    pub fn len(&self) -> usize {
        self.entries.lock().map_or(0, |g| g.map.len())
    }

    /// Vrai si le cache est vide.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Url {
        Url::parse(s).expect("url")
    }

    #[test]
    fn hit_and_normalized_key() {
        let c = ResultCache::new(Duration::from_secs(60), 10);
        c.put(
            &u("https://X.com/v/1?utm_source=a#frag"),
            Mode::Video,
            "f1".into(),
        );
        assert_eq!(
            c.get(&u("https://x.com/v/1"), Mode::Video).as_deref(),
            Some("f1")
        );
        assert_eq!(c.get(&u("https://x.com/v/2"), Mode::Video), None);
    }

    #[test]
    fn video_differs_from_audio() {
        let c = ResultCache::new(Duration::from_secs(60), 10);
        c.put(&u("https://x.com/v/1"), Mode::Video, "f1".into());
        assert_eq!(c.get(&u("https://x.com/v/1"), Mode::Audio), None);
    }

    #[test]
    fn expires_after_ttl() {
        let c = ResultCache::new(Duration::from_secs(60), 10);
        let t0 = Instant::now();
        let url = u("https://x.com/a");
        c.put_at(&url, Mode::Video, "f".into(), t0);
        let at = |s| c.get_at(&url, Mode::Video, t0 + Duration::from_secs(s));
        assert!(at(59).is_some());
        assert!(at(61).is_none());
        assert!(c.is_empty());
    }

    #[test]
    fn evicts_oldest_at_capacity() {
        let c = ResultCache::new(Duration::from_secs(60), 2);
        for (i, p) in ["a", "b", "c"].iter().enumerate() {
            c.put(
                &u(&format!("https://x.com/{p}")),
                Mode::Video,
                format!("f{i}"),
            );
        }
        assert_eq!(c.len(), 2);
        assert_eq!(c.get(&u("https://x.com/a"), Mode::Video), None);
        assert!(c.get(&u("https://x.com/c"), Mode::Video).is_some());
    }
}
