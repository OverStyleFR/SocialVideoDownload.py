//! Contrôle d'accès (liste blanche, admins) et limitation de débit par utilisateur.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;
use std::time::Instant;

use crate::config::Config;
use crate::error::AccessError;

/// Décisions d'accès d'après la configuration.
#[derive(Debug)]
pub struct AccessControl {
    allowed: HashSet<u64>,
    allow_all: bool,
    admins: HashSet<u64>,
    rate_per_min: u32,
    hits: Mutex<HashMap<u64, VecDeque<Instant>>>,
}

impl AccessControl {
    /// Construit le contrôle d'accès depuis la configuration.
    pub fn new(cfg: &Config) -> Self {
        Self {
            allowed: cfg.allowed_user_ids.clone(),
            allow_all: cfg.allow_all_users,
            admins: cfg.admin_user_ids.clone(),
            rate_per_min: cfg.rate_limit_per_min,
            hits: Mutex::new(HashMap::new()),
        }
    }

    /// Vrai si l'utilisateur peut utiliser le bot.
    pub fn is_allowed(&self, user_id: u64) -> bool {
        let _ = (user_id, &self.allowed, self.allow_all);
        todo!("AccessControl::is_allowed")
    }

    /// Vrai si l'utilisateur est administrateur.
    pub fn is_admin(&self, user_id: u64) -> bool {
        let _ = (user_id, &self.admins);
        todo!("AccessControl::is_admin")
    }

    /// Compte une requête ; `RateLimited` au-delà de RATE_LIMIT_PER_MIN sur une minute glissante.
    pub fn check_rate(&self, user_id: u64) -> Result<(), AccessError> {
        let _ = (user_id, self.rate_per_min, &self.hits);
        todo!("AccessControl::check_rate")
    }
}
