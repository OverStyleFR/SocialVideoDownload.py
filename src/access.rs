//! Contrôle d'accès (liste blanche, admins) et limitation de débit par utilisateur.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::Instant;

use crate::config::Config;
use crate::error::AccessError;
use crate::settings::SettingsStore;

/// Au-delà de ce nombre d'utilisateurs suivis, on oublie ceux dont le seau est plein (mémoire bornée).
const PURGE_AT: usize = 1024;

/// Seau de jetons d'un utilisateur : jetons restants à l'instant `last`.
#[derive(Debug, Clone, Copy)]
struct Bucket {
    tokens: f64,
    last: Instant,
}

/// Décisions d'accès d'après la configuration, ou d'après les réglages (/settings, lus à chaque
/// appel donc appliqués à chaud) quand [`AccessControl::with_settings`] a été appelé.
pub struct AccessControl {
    allowed: HashSet<u64>,
    allow_all: bool,
    admins: HashSet<u64>,
    rate_per_min: u32,
    burst: u32,
    settings: Option<SettingsStore>,
    buckets: Mutex<HashMap<u64, Bucket>>,
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
            burst: cfg.rate_limit_burst,
            settings: None,
            buckets: Mutex::new(HashMap::new()),
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

    /// Comportement du bot Python : ouvert à tous. Seule une ALLOWED_USER_IDS renseignée (ajout
    /// optionnel de la refonte) restreint l'accès ; ADMIN_USER_IDS ne réserve que /stats et /settings.
    pub fn is_open(&self) -> bool {
        self.allow_all || self.list_empty("acc.allowed_users", &self.allowed)
    }

    fn setting(&self, key: &str, fixed: u32) -> u64 {
        self.settings
            .as_ref()
            .and_then(|s| s.get_u64(key))
            .unwrap_or(fixed.into())
            .max(1)
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

    /// Compte une requête (seau de jetons) : jusqu'à RATE_LIMIT_BURST liens d'affilée, puis
    /// RATE_LIMIT_PER_MIN par minute ; `RateLimited` quand le seau est vide.
    pub fn check_rate(&self, user_id: u64) -> Result<(), AccessError> {
        self.check_rate_at(user_id, Instant::now())
    }

    fn check_rate_at(&self, user_id: u64, now: Instant) -> Result<(), AccessError> {
        let rate = self.setting("acc.rate_limit_per_min", self.rate_per_min) as f64;
        let burst = self.setting("acc.rate_limit_burst", self.burst) as f64;
        // Multiplier avant de diviser : 6 s à 10/min donnent exactement 1 jeton.
        let level = |b: &Bucket| {
            (b.tokens + now.saturating_duration_since(b.last).as_secs_f64() * rate / 60.0)
                .min(burst)
        };
        let mut buckets = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        if buckets.len() >= PURGE_AT {
            buckets.retain(|_, b| level(b) < burst);
        }
        let b = buckets.entry(user_id).or_insert(Bucket {
            tokens: burst,
            last: now,
        });
        let tokens = level(b);
        if tokens < 1.0 {
            return Err(AccessError::RateLimited {
                retry_after_s: ((1.0 - tokens) * 60.0 / rate).ceil() as u64,
            });
        }
        *b = Bucket {
            tokens: tokens - 1.0,
            last: now,
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn ctrl(rate: u32, burst: u32, allow_all: bool) -> AccessControl {
        let cfg = Config::from_map(&HashMap::from([
            ("BOT_TOKEN".to_string(), "t".to_string()),
            ("ALLOWED_USER_IDS".into(), "111".into()),
            ("ADMIN_USER_IDS".into(), "222".into()),
            ("RATE_LIMIT_PER_MIN".into(), rate.to_string()),
            ("RATE_LIMIT_BURST".into(), burst.to_string()),
            ("ALLOW_ALL_USERS".into(), allow_all.to_string()),
        ]))
        .expect("config");
        AccessControl::new(&cfg)
    }

    #[test]
    fn user_outside_list_refused() {
        let a = ctrl(5, 5, false);
        assert!(a.is_allowed(111));
        assert!(!a.is_allowed(333));
        // 222 est administrateur mais absent de ALLOWED_USER_IDS : il a quand même accès.
        assert!(a.is_allowed(222));
        assert!(ctrl(5, 5, true).is_allowed(333));
    }

    #[test]
    fn open_by_default_without_any_list_like_the_python_bot() {
        let cfg = Config::from_map(&HashMap::from([("BOT_TOKEN".to_string(), "t".to_string())]))
            .expect("config");
        let a = AccessControl::new(&cfg);
        assert!(a.is_open() && a.is_allowed(5_092_023_723));
        // ADMIN_USER_IDS seul (cas de l'original : /stats réservé) ne ferme PAS le bot.
        let cfg = Config::from_map(&HashMap::from([
            ("BOT_TOKEN".to_string(), "t".to_string()),
            ("ADMIN_USER_IDS".into(), "1".into()),
        ]))
        .expect("config");
        let a = AccessControl::new(&cfg);
        assert!(
            a.is_open() && a.is_allowed(1) && a.is_allowed(2) && a.is_admin(1) && !a.is_admin(2)
        );
        // ALLOWED_USER_IDS renseignée : restreint.
        let cfg = Config::from_map(&HashMap::from([
            ("BOT_TOKEN".to_string(), "t".to_string()),
            ("ALLOWED_USER_IDS".into(), "1".into()),
        ]))
        .expect("config");
        let a = AccessControl::new(&cfg);
        assert!(!a.is_open() && a.is_allowed(1) && !a.is_allowed(2));
    }

    #[test]
    fn admin_by_numeric_id_only() {
        let a = ctrl(5, 5, false);
        assert!(a.is_admin(222));
        assert!(!a.is_admin(111));
    }

    #[test]
    fn burst_then_steady_rate() {
        // Défauts demandés : 10 par minute, 20 d'affilée.
        let a = ctrl(10, 20, false);
        let t0 = Instant::now();
        for _ in 0..20 {
            assert!(a.check_rate_at(111, t0).is_ok());
        }
        let err = a.check_rate_at(111, t0).unwrap_err();
        // Un jeton toutes les 6 s.
        assert!(
            matches!(err, AccessError::RateLimited { retry_after_s: 6 }),
            "{err:?}"
        );
        assert!(a.check_rate_at(222, t0).is_ok(), "autre utilisateur intact");
        let t6 = t0 + Duration::from_secs(6);
        assert!(a.check_rate_at(111, t6).is_ok());
        assert!(a.check_rate_at(111, t6).is_err());
        // Après une minute calme : 10 jetons rechargés, pas plus.
        let t66 = t6 + Duration::from_secs(60);
        for _ in 0..10 {
            assert!(a.check_rate_at(111, t66).is_ok());
        }
        assert!(a.check_rate_at(111, t66).is_err());
    }

    #[test]
    fn bucket_capped_at_burst() {
        let a = ctrl(10, 3, false);
        let t0 = Instant::now();
        assert!(a.check_rate_at(111, t0).is_ok());
        // Une heure d'inactivité ne donne pas plus que la réserve.
        let t1 = t0 + Duration::from_secs(3600);
        for _ in 0..3 {
            assert!(a.check_rate_at(111, t1).is_ok());
        }
        assert!(a.check_rate_at(111, t1).is_err());
    }

    #[test]
    fn inactive_users_purged() {
        let a = ctrl(1, 1, false);
        let t0 = Instant::now();
        for id in 0..PURGE_AT as u64 {
            a.check_rate_at(id, t0).expect("ok");
        }
        a.check_rate_at(9999, t0 + Duration::from_secs(120))
            .expect("ok");
        assert_eq!(a.buckets.lock().expect("lock").len(), 1);
    }
}
