//! Configuration typée, chargée une seule fois depuis l'environnement.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use url::Url;

pub use crate::error::ConfigError;

/// Configuration complète du bot (aucun autre module ne lit l'environnement).
#[derive(Clone)]
pub struct Config {
    /// BOT_TOKEN : obligatoire, jamais loggué (Debug le masque).
    pub bot_token: String,
    /// TELEGRAM_API_URL : défaut https://api.telegram.org (surchargé par le faux Telegram des tests).
    pub telegram_api_url: Url,
    /// ALLOWED_USER_IDS : liste d'IDs séparés par des virgules ; vide = personne sauf ALLOW_ALL_USERS=1.
    pub allowed_user_ids: HashSet<u64>,
    /// ALLOW_ALL_USERS : défaut false.
    pub allow_all_users: bool,
    /// ADMIN_USER_IDS : IDs numériques autorisés à /stats ; défaut vide.
    pub admin_user_ids: HashSet<u64>,
    /// RATE_LIMIT_PER_MIN : requêtes par utilisateur et par minute ; défaut 5.
    pub rate_limit_per_min: u32,
    /// MAX_CONCURRENT_DOWNLOADS : téléchargements simultanés ; défaut 3.
    pub max_concurrent_downloads: usize,
    /// MAX_FILESIZE_MB : taille max téléchargée ; défaut 500.
    pub max_filesize_mb: u64,
    /// MAX_DURATION_S : durée max d'un média ; défaut 3600.
    pub max_duration_s: u64,
    /// TELEGRAM_UPLOAD_LIMIT_MB : limite d'envoi de l'API Bot ; défaut 50.
    pub telegram_upload_limit_mb: u64,
    /// DOWNLOAD_DIR : racine des répertoires de travail ; défaut ./downloads.
    pub download_dir: PathBuf,
    /// RETENTION_HOURS : durée de conservation des fichiers ; défaut 24.
    pub retention: Duration,
    /// MIN_FREE_DISK_MB : espace libre minimal visé par le nettoyage ; défaut 500.
    pub min_free_disk_mb: u64,
    /// YTDLP_PATH : défaut yt-dlp.
    pub ytdlp_path: PathBuf,
    /// FFMPEG_PATH : défaut ffmpeg.
    pub ffmpeg_path: PathBuf,
    /// FFPROBE_PATH : défaut ffprobe.
    pub ffprobe_path: PathBuf,
    /// YTDLP_UPDATE_INTERVAL_H : intervalle de mise à jour de yt-dlp ; défaut 24 (0 = jamais).
    pub ytdlp_update_interval: Duration,
    /// EXTERNAL_UPLOAD_URL : repli d'hébergement externe ; absent = désactivé.
    pub external_upload_url: Option<Url>,
    /// EXTERNAL_UPLOAD_TIMEOUT_S : délai de l'envoi externe ; défaut 300.
    pub external_upload_timeout: Duration,
    /// CACHE_TTL_H : durée de vie du cache file_id ; défaut 24.
    pub cache_ttl: Duration,
    /// HEARTBEAT_FILE : fichier battement de cœur du healthcheck ; défaut /tmp/svd-heartbeat.
    pub heartbeat_file: PathBuf,
    /// UNSAFE_ALLOW_PRIVATE_HOSTS : autorise les IP privées (bancs/tests seulement) ; défaut false.
    pub unsafe_allow_private_hosts: bool,
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("bot_token", &"<masqué>")
            .field("telegram_api_url", &self.telegram_api_url.as_str())
            .field("allowed_user_ids", &self.allowed_user_ids.len())
            .field("allow_all_users", &self.allow_all_users)
            .field("admin_user_ids", &self.admin_user_ids.len())
            .field("rate_limit_per_min", &self.rate_limit_per_min)
            .field("max_concurrent_downloads", &self.max_concurrent_downloads)
            .field("max_filesize_mb", &self.max_filesize_mb)
            .field("max_duration_s", &self.max_duration_s)
            .field("telegram_upload_limit_mb", &self.telegram_upload_limit_mb)
            .field("download_dir", &self.download_dir)
            .field("retention", &self.retention)
            .field("min_free_disk_mb", &self.min_free_disk_mb)
            .field("external_upload", &self.external_upload_url.is_some())
            .field(
                "unsafe_allow_private_hosts",
                &self.unsafe_allow_private_hosts,
            )
            .finish_non_exhaustive()
    }
}

impl Config {
    /// Charge la configuration depuis les variables d'environnement du processus.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_map(&std::env::vars().collect())
    }

    /// Charge la configuration depuis une table clé/valeur (testable).
    pub fn from_map(env: &HashMap<String, String>) -> Result<Self, ConfigError> {
        let get = |k: &str| env.get(k).map(|v| v.trim()).filter(|v| !v.is_empty());
        let num = |k: &'static str, def: u64| -> Result<u64, ConfigError> {
            get(k).map_or(Ok(def), |v| {
                v.parse().map_err(|_| ConfigError::Invalid {
                    key: k,
                    reason: "entier positif attendu".into(),
                })
            })
        };
        let flag = |k: &'static str| -> Result<bool, ConfigError> {
            match get(k) {
                None => Ok(false),
                Some("1" | "true" | "yes" | "on") => Ok(true),
                Some("0" | "false" | "no" | "off") => Ok(false),
                Some(_) => Err(ConfigError::Invalid {
                    key: k,
                    reason: "booléen attendu (1/0)".into(),
                }),
            }
        };
        let ids = |k: &'static str| -> Result<HashSet<u64>, ConfigError> {
            get(k)
                .unwrap_or("")
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| {
                    s.parse().map_err(|_| ConfigError::Invalid {
                        key: k,
                        reason: "IDs numériques séparés par des virgules attendus".into(),
                    })
                })
                .collect()
        };
        let url = |k: &'static str, v: &str| -> Result<Url, ConfigError> {
            Url::parse(v).map_err(|_| ConfigError::Invalid {
                key: k,
                reason: "URL invalide".into(),
            })
        };
        let path = |k: &str, def: &str| PathBuf::from(get(k).unwrap_or(def));
        let hours = |h: u64| Duration::from_secs(h * 3600);

        let bot_token = get("BOT_TOKEN")
            .ok_or(ConfigError::Missing("BOT_TOKEN"))?
            .to_owned();
        let max_concurrent = num("MAX_CONCURRENT_DOWNLOADS", 3)?;
        if max_concurrent == 0 {
            return Err(ConfigError::Invalid {
                key: "MAX_CONCURRENT_DOWNLOADS",
                reason: "doit être >= 1".into(),
            });
        }
        Ok(Self {
            bot_token,
            telegram_api_url: url(
                "TELEGRAM_API_URL",
                get("TELEGRAM_API_URL").unwrap_or("https://api.telegram.org"),
            )?,
            allowed_user_ids: ids("ALLOWED_USER_IDS")?,
            allow_all_users: flag("ALLOW_ALL_USERS")?,
            admin_user_ids: ids("ADMIN_USER_IDS")?,
            rate_limit_per_min: u32::try_from(num("RATE_LIMIT_PER_MIN", 5)?).map_err(|_| {
                ConfigError::Invalid {
                    key: "RATE_LIMIT_PER_MIN",
                    reason: "trop grand".into(),
                }
            })?,
            max_concurrent_downloads: max_concurrent as usize,
            max_filesize_mb: num("MAX_FILESIZE_MB", 500)?,
            max_duration_s: num("MAX_DURATION_S", 3600)?,
            telegram_upload_limit_mb: num("TELEGRAM_UPLOAD_LIMIT_MB", 50)?,
            download_dir: path("DOWNLOAD_DIR", "./downloads"),
            retention: hours(num("RETENTION_HOURS", 24)?),
            min_free_disk_mb: num("MIN_FREE_DISK_MB", 500)?,
            ytdlp_path: path("YTDLP_PATH", "yt-dlp"),
            ffmpeg_path: path("FFMPEG_PATH", "ffmpeg"),
            ffprobe_path: path("FFPROBE_PATH", "ffprobe"),
            ytdlp_update_interval: hours(num("YTDLP_UPDATE_INTERVAL_H", 24)?),
            external_upload_url: get("EXTERNAL_UPLOAD_URL")
                .map(|v| url("EXTERNAL_UPLOAD_URL", v))
                .transpose()?,
            external_upload_timeout: Duration::from_secs(num("EXTERNAL_UPLOAD_TIMEOUT_S", 300)?),
            cache_ttl: hours(num("CACHE_TTL_H", 24)?),
            heartbeat_file: path("HEARTBEAT_FILE", "/tmp/svd-heartbeat"),
            unsafe_allow_private_hosts: flag("UNSAFE_ALLOW_PRIVATE_HOSTS")?,
        })
    }

    /// Limite d'envoi Telegram en octets.
    pub fn upload_limit_bytes(&self) -> u64 {
        self.telegram_upload_limit_mb * 1024 * 1024
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn config_defauts() {
        let c = Config::from_map(&map(&[("BOT_TOKEN", "x")])).unwrap();
        assert_eq!(c.telegram_api_url.as_str(), "https://api.telegram.org/");
        assert!(c.allowed_user_ids.is_empty());
        assert!(!c.allow_all_users);
        assert_eq!(c.telegram_upload_limit_mb, 50);
        assert_eq!(c.retention, Duration::from_secs(24 * 3600));
        assert!(c.external_upload_url.is_none());
        assert!(!c.unsafe_allow_private_hosts);
        assert!(!format!("{c:?}").contains("\"x\""));
    }

    #[test]
    fn config_liste_ids() {
        let c = Config::from_map(&map(&[
            ("BOT_TOKEN", "x"),
            ("ALLOWED_USER_IDS", " 1, 22 ,333,"),
            ("ADMIN_USER_IDS", "22"),
        ]))
        .unwrap();
        assert_eq!(c.allowed_user_ids, HashSet::from([1, 22, 333]));
        assert_eq!(c.admin_user_ids, HashSet::from([22]));
    }

    #[test]
    fn config_id_non_numerique_refuse() {
        let e = Config::from_map(&map(&[("BOT_TOKEN", "x"), ("ADMIN_USER_IDS", "@pseudo")]))
            .unwrap_err();
        assert!(matches!(
            e,
            ConfigError::Invalid {
                key: "ADMIN_USER_IDS",
                ..
            }
        ));
    }

    #[test]
    fn config_token_absent_refuse() {
        assert!(matches!(
            Config::from_map(&map(&[])),
            Err(ConfigError::Missing("BOT_TOKEN"))
        ));
        assert!(matches!(
            Config::from_map(&map(&[("BOT_TOKEN", "  ")])),
            Err(ConfigError::Missing("BOT_TOKEN"))
        ));
    }
}
