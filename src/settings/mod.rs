//! Réglages modifiables à chaud par les administrateurs (future commande `/settings`).
//!
//! - [`schema`] : la liste unique des réglages ([`schema::SCHEMA`]), leurs types, bornes et défauts
//!   (tirés de [`crate::config::Config`], jamais de secret).
//! - [`store`] : [`store::SettingsStore`], fichier JSON persistant (écriture atomique, 0600),
//!   journal d'audit JSONL, abonnement aux changements (`subscribe`).
//! - [`menu`] : écrans Telegram (texte + clavier inline) générés depuis le schéma et
//!   [`menu::parse_callback`] pour décoder les boutons (préfixe `s:`).
//!
//! Parcours type côté bot : `store.snapshot()` → `menu::root()/section()/item()` → bouton →
//! `menu::parse_callback` → `store.set(key, value, user_id)` (qui revalide tout) → réafficher `item`.
//! Les messages s'envoient SANS `parse_mode` (texte brut, aucune valeur n'est échappée).

pub mod menu;
pub mod schema;
pub mod store;

pub use menu::{parse_callback, Action};
pub use schema::{defaults_from, spec, Kind, Section, Spec, Value, Values, SCHEMA};
pub use store::{Change, SettingsError, SettingsStore};
