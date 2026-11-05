//! Contrôle d'accès (liste blanche, admins) et limitation de débit par utilisateur.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::error::AccessError;
use crate::settings::SettingsStore;

const WINDOW: Duration = Duration::from_secs(60);
/// Au-delà de ce nombre d'utilisateurs suivis, on oublie les inactifs (mémoire bornée).
const PURGE_AT: usize = 1024;

/// Décisions d'accès d'après la configuration, ou d'après les réglages (/settings, lus à chaque
/// appel donc appliqués à chaud) quand [`AccessControl::with_settings`] a été appelé.
pub struct AccessControl {
    allowed: HashSet<u64>,
    allow_all: bool,
    admins: HashSet<u64>,
    rate_per_min: u32,
    settings: Option<SettingsStore>,
    hits: Mutex<HashMap<u64, VecDeque<Instant>>>,
}

impl std::fmt::Debug for AccessControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccessControl")
            .field("allow_all", &self.allow_all)
            .field("settings", &self.settings.is_some())
            .finish_non_exhaustive()
    }
}

impl AccessControl {
    /// Construit le contrôle d'accès depuis la configuration.
    pub fn new(cfg: &Config) -> Self {
        Self {
            allowed: cfg.allowed_user_ids.clone(),
            allow_all: cfg.allow_all_users,
            admins: cfg.admin_user_ids.clone(),
            rate_per_min: cfg.rate_limit_per_min,
            settings: None,
            hits: Mutex::new(HashMap::new()),
        }
    }

    /// Lit liste blanche, admins et débit dans `settings` à chaque appel (valeurs à chaud).
    pub fn with_settings(mut self, settings: SettingsStore) -> Self {
        self.settings = Some(settings);
        self
    }

    fn ids(&self, key: &str, fixed: &HashSet<u64>, id: u64) -> bool {
        match self.settings.as_ref().and_then(|s| s.get_ids(key)) {
            Some(ids) => ids.contains(&id),
            None => fixed.contains(&id),
        }
    }

    fn list_empty(&self, key: &str, fixed: &HashSet<u64>) -> bool {
        match self.settings.as_ref().and_then(|s| s.get_ids(key)) {
            Some(ids) => ids.is_empty(),
            None => fixed.is_empty(),
        }
    }

    /// Comportement du bot Python : sans aucune liste (ni ALLOWED_USER_IDS ni ADMIN_USER_IDS),
    /// le bot est ouvert à tous. Dès qu'une liste existe, l'accès est restreint.
    pub fn is_open(&self) -> bool {
        self.allow_all
            || (self.list_empty("acc.allowed_users", &self.allowed)
                && self.list_empty("acc.admins", &self.admins))
    }

    fn rate_per_min(&self) -> u64 {
        self.settings
            .as_ref()
            .and_then(|s| s.get_u64("acc.rate_limit_per_min"))
            .unwrap_or(self.rate_per_min.into())
    }

    /// Vrai si l'utilisateur peut utiliser le bot : bot ouvert (voir [`AccessControl::is_open`]),
    /// liste blanche, ou administrateur (toujours admis).
    pub fn is_allowed(&self, user_id: u64) -> bool {
        self.is_open()
            || self.ids("acc.allowed_users", &self.allowed, user_id)
            || self.is_admin(user_id)
    }

    /// Vrai si l'utilisateur est administrateur (par ID numérique, jamais par pseudo).
    pub fn is_admin(&self, user_id: u64) -> bool {
        self.ids("acc.admins", &self.admins, user_id)
    }

    /// Compte une requête ; `RateLimited` au-delà de RATE_LIMIT_PER_MIN sur une minute glissante.
    pub fn check_rate(&self, user_id: u64) -> Result<(), AccessError> {
        self.check_rate_at(user_id, Instant::now())
    }

    fn check_rate_at(&self, user_id: u64, now: Instant) -> Result<(), AccessError> {
        let mut hits = self.hits.lock().unwrap_or_else(|e| e.into_inner());
        if hits.len() >= PURGE_AT {
            hits.retain(|_, q| q.back().is_some_and(|t| now.duration_since(*t) < WINDOW));
        }
        let q = hits.entry(user_id).or_default();
        while q.front().is_some_and(|t| now.duration_since(*t) >= WINDOW) {
            q.pop_front();
        }
        if q.len() as u64 >= self.rate_per_min() {
            let wait = q
                .front()
                .map_or(WINDOW, |t| WINDOW.saturating_sub(now.duration_since(*t)));
            return Err(AccessError::RateLimited {
                retry_after_s: wait.as_secs() + 1,
            });
        }
        q.push_back(now);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctrl(rate: u32, allow_all: bool) -> AccessControl {
        let cfg = Config::from_map(&HashMap::from([
            ("BOT_TOKEN".to_string(), "t".to_string()),
            ("ALLOWED_USER_IDS".into(), "111".into()),
            ("ADMIN_USER_IDS".into(), "222".into()),
            ("RATE_LIMIT_PER_MIN".into(), rate.to_string()),
            ("ALLOW_ALL_USERS".into(), allow_all.to_string()),
        ]))
        .expect("config");
        AccessControl::new(&cfg)
    }

    #[test]
    fn user_outside_list_refused() {
        let a = ctrl(5, false);
        assert!(a.is_allowed(111));
        assert!(!a.is_allowed(333));
        // 222 est administrateur mais absent de ALLOWED_USER_IDS : il a quand même accès.
        assert!(a.is_allowed(222));
        assert!(ctrl(5, true).is_allowed(333));
    }

    #[test]
    fn open_by_default_without_any_list_like_the_python_bot() {
        let cfg = Config::from_map(&HashMap::from([("BOT_TOKEN".to_string(), "t".to_string())]))
            .expect("config");
        let a = AccessControl::new(&cfg);
        assert!(a.is_open() && a.is_allowed(5_092_023_723));
        // Une seule liste suffit à restreindre.
        let cfg = Config::from_map(&HashMap::from([
            ("BOT_TOKEN".to_string(), "t".to_string()),
            ("ADMIN_USER_IDS".into(), "1".into()),
        ]))
        .expect("config");
        let a = AccessControl::new(&cfg);
        assert!(!a.is_open() && a.is_allowed(1) && !a.is_allowed(2));
    }

    #[test]
    fn admin_by_numeric_id_only() {
        let a = ctrl(5, false);
        assert!(a.is_admin(222));
        assert!(!a.is_admin(111));
    }

    #[test]
    fn rate_limit_at_n_plus_1_then_reset_after_window() {
        let a = ctrl(3, false);
        let t0 = Instant::now();
        for _ in 0..3 {
            assert!(a.check_rate_at(111, t0).is_ok());
        }
        let err = a
            .check_rate_at(111, t0 + Duration::from_secs(10))
            .unwrap_err();
        assert!(
            matches!(err, AccessError::RateLimited { retry_after_s: 51 }),
            "{err:?}"
        );
        assert!(a.check_rate_at(222, t0).is_ok(), "autre utilisateur intact");
        assert!(a.check_rate_at(111, t0 + Duration::from_secs(61)).is_ok());
    }

    #[test]
    fn inactive_users_purged() {
        let a = ctrl(1, false);
        let t0 = Instant::now();
        for id in 0..PURGE_AT as u64 {
            a.check_rate_at(id, t0).expect("ok");
        }
        a.check_rate_at(9999, t0 + Duration::from_secs(120))
            .expect("ok");
        assert_eq!(a.hits.lock().expect("lock").len(), 1);
    }
}
