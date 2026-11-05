//! Cache des file_id Telegram par URL normalisée et mode (TTL + borne de taille).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use url::Url;

use crate::download::Mode;

/// Clé du cache : URL normalisée + mode.
pub type CacheKey = (String, Mode);

/// Cache en mémoire, seul mécanisme de réutilisation des envois.
#[derive(Debug)]
pub struct ResultCache {
    ttl: Duration,
    max_entries: usize,
    entries: Mutex<HashMap<CacheKey, (String, Instant)>>,
}

impl ResultCache {
    /// Crée un cache de durée de vie `ttl` et d'au plus `max_entries` entrées.
    pub fn new(ttl: Duration, max_entries: usize) -> Self {
        Self {
            ttl,
            max_entries,
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// Normalise une URL pour la clé (hôte minuscule, fragment et paramètres de suivi retirés).
    pub fn normalize(url: &Url) -> String {
        let _ = url;
        todo!("ResultCache::normalize")
    }

    /// Rend le file_id encore valide pour (url, mode).
    pub fn get(&self, url: &Url, mode: Mode) -> Option<String> {
        let _ = (url, mode, self.ttl, &self.entries);
        todo!("ResultCache::get")
    }

    /// Enregistre le file_id pour (url, mode), en évinçant si la borne est atteinte.
    pub fn put(&self, url: &Url, mode: Mode, file_id: String) {
        let _ = (url, mode, file_id, self.max_entries);
        todo!("ResultCache::put")
    }

    /// Nombre d'entrées (y compris expirées non encore purgées).
    pub fn len(&self) -> usize {
        self.entries.lock().map_or(0, |m| m.len())
    }

    /// Vrai si le cache est vide.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
