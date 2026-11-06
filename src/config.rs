//! Configuration typée, chargée une seule fois depuis l'environnement.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use url::Url;

pub use crate::error::ConfigError;

/// Valeur du modèle `.env` du bot Python : vaut jeton absent.
pub const TOKEN_PLACEHOLDER: &str = "YOUR_TELEGRAM_BOT_TOKEN_HERE";

/// Paires `CLÉ=valeur` d'un `.env` : commentaires `#`, préfixe `export`, guillemets simples ou doubles.
// shortcut: ni échappements ni valeurs multilignes ; passer à dotenvy si un .env en contient.
pub fn parse_dotenv(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim();
            let l = l.strip_prefix("export ").unwrap_or(l);
            if l.starts_with('#') {
                return None;
            }
            let (k, v) = l.split_once('=')?;
            let v = v.trim();
            let v = match v.chars().next() {
                Some(q @ ('"' | '\'')) => v[1..].split(q).next().unwrap_or(""),
                _ => v.split(" #").next().unwrap_or("").trim(),
            };
            Some((k.trim().to_string(), v.to_string()))
        })
        .filter(|(k, _)| !k.is_empty())
        .collect()
}

/// Hébergeur externe du bot Python (utils/curl_uploader.py).
pub const DEFAULT_EXTERNAL_UPLOAD_URL: &str = "https://curl.libriciel.fr/";

/// Configuration complète du bot (aucun autre module ne lit l'environnement).
#[derive(Clone)]
pub struct Config {
    /// BOT_TOKEN : obligatoire, jamais loggué (Debug le masque).
    pub bot_token: String,
    /// TELEGRAM_API_URL : défaut https://api.telegram.org (surchargé par le faux Telegram des tests).
    pub telegram_api_url: Url,
    /// ALLOWED_USER_IDS : liste d'IDs séparés par des virgules ; vide = bot ouvert à tous, comme le bot Python.
    pub allowed_user_ids: HashSet<u64>,
    /// ALLOW_ALL_USERS : défaut false.
    pub allow_all_users: bool,
    /// ADMIN_USER_IDS : IDs numériques autorisés à /stats et /settings (ne restreint pas le bot) ; défaut vide.
    pub admin_user_ids: HashSet<u64>,
    /// RATE_LIMIT_PER_MIN : liens rechargés par utilisateur et par minute ; défaut 10.
    pub rate_limit_per_min: u32,
    /// RATE_LIMIT_BURST : liens qu'un utilisateur peut envoyer d'affilée ; défaut 20.
    pub rate_limit_burst: u32,
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
    /// RETENTION_SMALL_HOURS (nom du bot Python) : conservation des mp3 et petits fichiers ; défaut RETENTION_HOURS ou 24.
    pub retention_small: Duration,
    /// RETENTION_LARGE_HOURS (nom du bot Python) : conservation des gros fichiers ; défaut RETENTION_HOURS ou 2.
    pub retention_large: Duration,
    /// SMALL_FILE_SIZE_MB (nom du bot Python) : seuil petit/gros fichier ; défaut 12.
    pub small_file_size_mb: u64,
    /// CLEANUP_INTERVAL_HOURS (nom du bot Python) : intervalle du nettoyage ; défaut 24.
    pub cleanup_interval: Duration,
    /// MIN_FREE_DISK_MB, ou MIN_FREE_SPACE_MB (nom du bot Python) : espace libre minimal visé ; défaut 500.
    pub min_free_disk_mb: u64,
    /// VERSION (nom du bot Python) : affichée par /help ; défaut « unknown ».
    pub version: String,
    /// DEVELOPED_BY (nom du bot Python) : affiché par /help ; défaut « Tom V. | OverStyleFR ».
    pub developed_by: String,
    /// YTDLP_PATH : défaut yt-dlp.
    pub ytdlp_path: PathBuf,
    /// FFMPEG_PATH : défaut ffmpeg.
    pub ffmpeg_path: PathBuf,
    /// FFPROBE_PATH : défaut ffprobe.
    pub ffprobe_path: PathBuf,
    /// YTDLP_UPDATE_INTERVAL_H : intervalle de mise à jour de yt-dlp ; défaut 24 (0 = jamais).
    pub ytdlp_update_interval: Duration,
    /// EXTERNAL_UPLOAD_URL : hébergeur du repli externe ; défaut curl.libriciel.fr (bot Python), vide ou « off » = coupé.
    pub external_upload_url: Option<Url>,
    /// EXTERNAL_UPLOAD_CONSENT : demander Oui/Non avant l'envoi externe (ajout) ; défaut true.
    /// « Accord permanent » dans /settings (ou 0 ici) envoie directement, comme le bot Python.
    pub external_upload_consent: bool,
    /// EXTERNAL_UPLOAD_TIMEOUT_S : délai de l'envoi externe ; défaut 300.
    pub external_upload_timeout: Duration,
    /// CACHE_TTL_H : durée de vie du cache file_id ; défaut 24.
    pub cache_ttl: Duration,
    /// SETTINGS_FILE : réglages modifiés par /settings ; défaut <DOWNLOAD_DIR>/../settings.json
    /// (./data/settings.json si DOWNLOAD_DIR n'a pas de parent).
    pub settings_file: PathBuf,
    /// LOGS_DIR : dossier des journaux (par défaut `logs` à côté du dossier de téléchargement).
    pub logs_dir: PathBuf,
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
            .field("rate_limit_burst", &self.rate_limit_burst)
            .field("max_concurrent_downloads", &self.max_concurrent_downloads)
            .field("max_filesize_mb", &self.max_filesize_mb)
            .field("max_duration_s", &self.max_duration_s)
            .field("telegram_upload_limit_mb", &self.telegram_upload_limit_mb)
            .field("download_dir", &self.download_dir)
            .field("settings_file", &self.settings_file)
            .field("retention_small", &self.retention_small)
            .field("retention_large", &self.retention_large)
            .field("small_file_size_mb", &self.small_file_size_mb)
            .field("cleanup_interval", &self.cleanup_interval)
            .field("min_free_disk_mb", &self.min_free_disk_mb)
            .field("version", &self.version)
            .field("ffmpeg_path", &self.ffmpeg_path)
            .field("external_upload", &self.external_upload_url.is_some())
            .field(
                "unsafe_allow_private_hosts",
                &self.unsafe_allow_private_hosts,
            )
            .finish_non_exhaustive()
    }
}

/// FFMPEG_PATH du bot Python (`ffmpeg/ffmpeg-7.0.2-amd64-static/ffmpeg`) : s'il n'existe pas, on
/// retombe sur `ffmpeg` du PATH, comme l'original.
fn ffmpeg_path(v: Option<&str>) -> PathBuf {
    match v {
        Some(p) if p.contains('/') && !std::path::Path::new(p).exists() => PathBuf::from("ffmpeg"),
        Some(p) => PathBuf::from(p),
        None => PathBuf::from("ffmpeg"),
    }
}

impl Config {
    /// Charge l'environnement du processus, complété par le `.env` du dossier courant sans rien
    /// écraser (`load_dotenv(".env", override=False)`) ; une variable vide ou le jeton modèle
    /// cèdent la place au fichier, comme `token_loader.py`.
    pub fn from_env() -> Result<Self, ConfigError> {
        let mut env: HashMap<String, String> = std::env::vars().collect();
        if let Ok(text) = std::fs::read_to_string(".env") {
            for (k, v) in parse_dotenv(&text) {
                if env
                    .get(&k)
                    .is_none_or(|old| old.trim().is_empty() || old == TOKEN_PLACEHOLDER)
                {
                    env.insert(k, v);
                }
            }
        }
        Self::from_map(&env)
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
        let flag_or = |k: &'static str, def: bool| -> Result<bool, ConfigError> {
            match get(k) {
                None => Ok(def),
                Some("1" | "true" | "yes" | "on") => Ok(true),
                Some("0" | "false" | "no" | "off") => Ok(false),
                Some(_) => Err(ConfigError::Invalid {
                    key: k,
                    reason: "booléen attendu (1/0)".into(),
                }),
            }
        };
        let flag = |k: &'static str| flag_or(k, false);
        let num32 = |k: &'static str, def: u64| -> Result<u32, ConfigError> {
            u32::try_from(num(k, def)?).map_err(|_| ConfigError::Invalid {
                key: k,
                reason: "trop grand".into(),
            })
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
        // Nouveau nom d'abord, nom du bot Python ensuite.
        let num2 = |k: &'static str, old: &'static str, def: u64| -> Result<u64, ConfigError> {
            if get(k).is_some() {
                num(k, def)
            } else {
                num(old, def)
            }
        };
        let hours = |h: u64| Duration::from_secs(h * 3600);

        let bot_token = get("BOT_TOKEN")
            .filter(|t| *t != TOKEN_PLACEHOLDER)
            .ok_or(ConfigError::Missing("BOT_TOKEN"))?
            .to_owned();
        let max_concurrent = num("MAX_CONCURRENT_DOWNLOADS", 3)?;
        if max_concurrent == 0 {
            return Err(ConfigError::Invalid {
                key: "MAX_CONCURRENT_DOWNLOADS",
                reason: "doit être >= 1".into(),
            });
        }
        // Absolu : la garde « fichier produit dans le JobDir » compare au chemin rendu par yt-dlp.
        let download_dir = path("DOWNLOAD_DIR", "./downloads");
        let download_dir = std::path::absolute(&download_dir).unwrap_or(download_dir);
        let (small_def, large_def) = if get("RETENTION_HOURS").is_some() {
            let h = num("RETENTION_HOURS", 24)?;
            (h, h)
        } else {
            (24, 2)
        };
        let logs_dir = get("LOGS_DIR").map_or_else(
            || {
                download_dir
                    .parent()
                    .map_or_else(|| PathBuf::from("logs"), |p| p.join("logs"))
            },
            PathBuf::from,
        );
        let settings_file = get("SETTINGS_FILE").map_or_else(
            || match download_dir.parent() {
                Some(p) if !p.as_os_str().is_empty() => p.join("settings.json"),
                _ => PathBuf::from("./data/settings.json"),
            },
            PathBuf::from,
        );
        Ok(Self {
            bot_token,
            telegram_api_url: url(
                "TELEGRAM_API_URL",
                get("TELEGRAM_API_URL").unwrap_or("https://api.telegram.org"),
            )?,
            allowed_user_ids: ids("ALLOWED_USER_IDS")?,
            allow_all_users: flag("ALLOW_ALL_USERS")?,
            admin_user_ids: ids("ADMIN_USER_IDS")?,
            rate_limit_per_min: num32("RATE_LIMIT_PER_MIN", 10)?,
            rate_limit_burst: num32("RATE_LIMIT_BURST", 20)?,
            max_concurrent_downloads: max_concurrent as usize,
            max_filesize_mb: num("MAX_FILESIZE_MB", 500)?,
            max_duration_s: num("MAX_DURATION_S", 3600)?,
            telegram_upload_limit_mb: num("TELEGRAM_UPLOAD_LIMIT_MB", 50)?,
            download_dir,
            logs_dir,
            settings_file,
            retention: hours(num("RETENTION_HOURS", 24)?),
            retention_small: hours(num("RETENTION_SMALL_HOURS", small_def)?),
            retention_large: hours(num("RETENTION_LARGE_HOURS", large_def)?),
            small_file_size_mb: num("SMALL_FILE_SIZE_MB", 12)?,
            cleanup_interval: hours(num("CLEANUP_INTERVAL_HOURS", 24)?.max(1)),
            min_free_disk_mb: num2("MIN_FREE_DISK_MB", "MIN_FREE_SPACE_MB", 500)?,
            version: get("VERSION").unwrap_or("unknown").to_owned(),
            developed_by: get("DEVELOPED_BY")
                .unwrap_or("Tom V. | OverStyleFR")
                .to_owned(),
            ytdlp_path: path("YTDLP_PATH", "yt-dlp"),
            ffmpeg_path: ffmpeg_path(get("FFMPEG_PATH")),
            ffprobe_path: path("FFPROBE_PATH", "ffprobe"),
            ytdlp_update_interval: hours(num("YTDLP_UPDATE_INTERVAL_H", 24)?),
            // curl.libriciel.fr par défaut comme le bot Python ; « off » (ou vide) coupe l'envoi externe.
            // Lu brut : une valeur vide coupe l'envoi externe au lieu de rendre le défaut.
            external_upload_url: match env.get("EXTERNAL_UPLOAD_URL").map(|v| v.trim()) {
                None => Some(url("EXTERNAL_UPLOAD_URL", DEFAULT_EXTERNAL_UPLOAD_URL)?),
                Some("" | "off" | "false" | "0" | "no") => None,
                Some(v) => Some(url("EXTERNAL_UPLOAD_URL", v)?),
            },
            external_upload_consent: flag_or("EXTERNAL_UPLOAD_CONSENT", true)?,
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
        assert_eq!(c.retention_small, Duration::from_secs(24 * 3600));
        assert_eq!(
            c.external_upload_url.as_ref().map(Url::as_str),
            Some("https://curl.libriciel.fr/")
        );
        assert!(c.external_upload_consent);
        assert_eq!((c.rate_limit_per_min, c.rate_limit_burst), (10, 20));
        assert!(!c.unsafe_allow_private_hosts);
        assert!(!format!("{c:?}").contains("\"x\""));
    }

    #[test]
    fn config_noms_du_bot_python() {
        let c = Config::from_map(&map(&[
            ("BOT_TOKEN", "x"),
            ("VERSION", "1.2.3"),
            ("DEVELOPED_BY", "Moi"),
            ("MIN_FREE_SPACE_MB", "900"),
            ("SMALL_FILE_SIZE_MB", "20"),
            ("RETENTION_SMALL_HOURS", "10"),
            ("RETENTION_LARGE_HOURS", "3"),
            ("CLEANUP_INTERVAL_HOURS", "6"),
            ("FFMPEG_PATH", "absent/ffmpeg-7.0.2-amd64-static/ffmpeg"),
        ]))
        .unwrap();
        assert_eq!(c.version, "1.2.3");
        assert_eq!(c.developed_by, "Moi");
        assert_eq!(c.min_free_disk_mb, 900);
        assert_eq!(c.small_file_size_mb, 20);
        assert_eq!(c.retention_small, Duration::from_secs(10 * 3600));
        assert_eq!(c.retention_large, Duration::from_secs(3 * 3600));
        assert_eq!(c.cleanup_interval, Duration::from_secs(6 * 3600));
        assert_eq!(
            c.ffmpeg_path,
            PathBuf::from("ffmpeg"),
            "chemin absent : repli PATH"
        );
        let d = Config::from_map(&map(&[("BOT_TOKEN", "x")])).unwrap();
        assert_eq!(d.retention_small, Duration::from_secs(86400));
        assert_eq!(d.retention_large, Duration::from_secs(7200));
        assert_eq!(d.developed_by, "Tom V. | OverStyleFR");
    }

    #[test]
    fn download_dir_relatif_devient_absolu() {
        // Cas de Tom (DOWNLOAD_DIR=./downloads) : yt-dlp rend un chemin absolu, la garde compare avec.
        let c =
            Config::from_map(&map(&[("BOT_TOKEN", "x"), ("DOWNLOAD_DIR", "./downloads")])).unwrap();
        assert!(c.download_dir.is_absolute(), "{:?}", c.download_dir);
        assert!(c.download_dir.ends_with("downloads"));
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

    #[test]
    fn dotenv_comme_python_dotenv() {
        let v = parse_dotenv(
            "# c\nBOT_TOKEN=123:abc\nexport A = 'x y'\nB=\"z # pas un commentaire\"\nC=1 # note\n\n=x\nsans_egal\n",
        );
        let m: HashMap<_, _> = v.into_iter().collect();
        assert_eq!(m["BOT_TOKEN"], "123:abc");
        assert_eq!(m["A"], "x y");
        assert_eq!(m["B"], "z # pas un commentaire");
        assert_eq!(m["C"], "1");
        assert_eq!(m.len(), 4);
    }

    #[test]
    fn jeton_modele_et_url_externe_vide() {
        assert!(matches!(
            Config::from_map(&map(&[("BOT_TOKEN", TOKEN_PLACEHOLDER)])),
            Err(ConfigError::Missing("BOT_TOKEN"))
        ));
        let c = Config::from_map(&map(&[("BOT_TOKEN", "x"), ("EXTERNAL_UPLOAD_URL", "")])).unwrap();
        assert!(c.external_upload_url.is_none(), "vide = coupé");
    }
}
