//! Schéma typé des réglages : LA liste unique (menus, validation et persistance en dérivent).

use std::collections::BTreeMap;
use std::fmt;

use crate::config::Config;

/// Rubrique d'un réglage (une entrée du menu racine).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    Telechargement,
    Sources,
    Audio,
    Envoi,
    Stockage,
    Acces,
    Systeme,
}

impl Section {
    /// Toutes les rubriques, dans l'ordre d'affichage.
    pub const ALL: [Section; 7] = [
        Section::Telechargement,
        Section::Sources,
        Section::Audio,
        Section::Envoi,
        Section::Stockage,
        Section::Acces,
        Section::Systeme,
    ];

    /// Libellé français affiché.
    pub fn label(self) -> &'static str {
        match self {
            Section::Telechargement => "Téléchargement",
            Section::Sources => "Sources",
            Section::Audio => "Audio",
            Section::Envoi => "Envoi",
            Section::Stockage => "Stockage",
            Section::Acces => "Accès",
            Section::Systeme => "Système",
        }
    }

    /// Identifiant court utilisé dans les données de rappel.
    pub fn id(self) -> &'static str {
        match self {
            Section::Telechargement => "dl",
            Section::Sources => "src",
            Section::Audio => "aud",
            Section::Envoi => "env",
            Section::Stockage => "sto",
            Section::Acces => "acc",
            Section::Systeme => "sys",
        }
    }

    /// Inverse de [`Section::id`].
    pub fn from_id(id: &str) -> Option<Section> {
        Section::ALL.into_iter().find(|s| s.id() == id)
    }
}

/// Type d'un réglage et ses contraintes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Booléen (saisie : on/off, true/false, 1/0, oui/non).
    Bool,
    /// Entier borné inclus ; `step` = pas des boutons +/−.
    U64 { min: u64, max: u64, step: u64 },
    /// Une valeur parmi la liste.
    Choice(&'static [&'static str]),
    /// Liste d'IDs Telegram numériques (jamais de pseudo `@x`).
    IdList,
    /// Permutation ordonnée de la liste.
    Order(&'static [&'static str]),
    /// Affiché seulement, jamais modifiable.
    ReadOnly,
}

/// Valeur typée d'un réglage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Bool(bool),
    U64(u64),
    /// Choice et ReadOnly.
    Text(String),
    Ids(Vec<u64>),
    /// Order.
    List(Vec<String>),
}

impl fmt::Display for Value {
    /// Forme canonique : relue telle quelle par la validation (c'est aussi la forme persistée).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Bool(b) => f.write_str(if *b { "on" } else { "off" }),
            Value::U64(n) => write!(f, "{n}"),
            Value::Text(s) => f.write_str(s),
            Value::Ids(v) => {
                let s: Vec<String> = v.iter().map(u64::to_string).collect();
                f.write_str(&s.join(","))
            }
            Value::List(v) => f.write_str(&v.join(",")),
        }
    }
}

/// Valeurs courantes de tous les réglages, par clé.
pub type Values = BTreeMap<&'static str, Value>;

/// Description d'un réglage.
#[derive(Clone, Copy, Debug)]
pub struct Spec {
    /// Clé stable (persistée, < 30 octets pour tenir dans un callback).
    pub key: &'static str,
    pub section: Section,
    /// Nom court affiché sur les boutons.
    pub label: &'static str,
    pub kind: Kind,
    /// Défaut, calculé depuis la configuration d'environnement.
    pub default: fn(&Config) -> Value,
    /// Explication en français.
    pub description: &'static str,
    /// true = appliqué à chaud ; false = effet au prochain redémarrage.
    pub hot: bool,
    /// Avertissement affiché sur l'écran du réglage.
    pub warning: Option<&'static str>,
}

/// Fournisseurs directs (hors yt-dlp) ordonnables.
pub const PROVIDERS: &[&str] = &["x", "bluesky", "tiktok"];

fn ids(set: &std::collections::HashSet<u64>) -> Value {
    let mut v: Vec<u64> = set.iter().copied().collect();
    v.sort_unstable();
    Value::Ids(v)
}

/// Tous les réglages. Aucun secret (BOT_TOKEN, EXTERNAL_UPLOAD_URL exclus).
pub static SCHEMA: &[Spec] = &[
    // Téléchargement
    Spec {
        key: "dl.max_filesize_mb",
        section: Section::Telechargement,
        label: "Taille max (Mo)",
        kind: Kind::U64 {
            min: 1,
            max: 4000,
            step: 50,
        },
        default: |c| Value::U64(c.max_filesize_mb),
        description: "Taille maximale d'un fichier téléchargé, en Mo.",
        hot: true,
        warning: None,
    },
    Spec {
        key: "dl.max_duration_s",
        section: Section::Telechargement,
        label: "Durée max (s)",
        kind: Kind::U64 {
            min: 10,
            max: 86_400,
            step: 300,
        },
        default: |c| Value::U64(c.max_duration_s),
        description: "Durée maximale d'un média, en secondes.",
        hot: true,
        warning: None,
    },
    Spec {
        key: "dl.quality",
        section: Section::Telechargement,
        label: "Qualité",
        kind: Kind::Choice(&["best", "1080", "720", "480"]),
        default: |_| Value::Text("best".into()),
        description: "Hauteur maximale de la vidéo prise par yt-dlp (best = la meilleure ; à défaut, la meilleure disponible).",
        hot: true,
        warning: None,
    },
    // Sources
    Spec {
        key: "providers.order",
        section: Section::Sources,
        label: "Ordre des sources",
        kind: Kind::Order(PROVIDERS),
        default: |_| Value::List(PROVIDERS.iter().map(|s| s.to_string()).collect()),
        description: "Ordre d'essai des sources directes activées, avant yt-dlp.",
        hot: true,
        warning: None,
    },
    Spec {
        key: "providers.x.enabled",
        section: Section::Sources,
        label: "Source X",
        kind: Kind::Bool,
        default: |_| Value::Bool(false),
        description: "Récupération directe des vidéos X (sinon yt-dlp seul).",
        hot: true,
        warning: None,
    },
    Spec {
        key: "providers.bluesky.enabled",
        section: Section::Sources,
        label: "Source Bluesky",
        kind: Kind::Bool,
        default: |_| Value::Bool(false),
        description: "Récupération directe des vidéos Bluesky (sinon yt-dlp seul).",
        hot: true,
        warning: None,
    },
    Spec {
        key: "providers.tiktok.enabled",
        section: Section::Sources,
        label: "Source TikTok",
        kind: Kind::Bool,
        default: |_| Value::Bool(false),
        description: "Récupération TikTok par une API publique (sinon yt-dlp seul).",
        hot: true,
        warning: Some("API publique tierce : la vidéo passe par un service externe"),
    },
    Spec {
        key: "providers.ytdlp_fallback",
        section: Section::Sources,
        label: "Repli yt-dlp",
        kind: Kind::Bool,
        default: |_| Value::Bool(true),
        description: "Essayer yt-dlp quand les sources directes échouent.",
        hot: true,
        warning: None,
    },
    // Audio
    Spec {
        key: "audio.music_enabled",
        section: Section::Audio,
        label: "/music",
        kind: Kind::Bool,
        default: |_| Value::Bool(true),
        description: "Commande /music (extraction audio) activée.",
        hot: true,
        warning: None,
    },
    Spec {
        key: "audio.bitrate_kbps",
        section: Section::Audio,
        label: "Débit (kbit/s)",
        kind: Kind::U64 {
            min: 64,
            max: 320,
            step: 32,
        },
        default: |_| Value::U64(192),
        description: "Débit de l'audio extrait, en kbit/s.",
        hot: true,
        warning: None,
    },
    // Envoi
    Spec {
        key: "send.telegram_limit_mb",
        section: Section::Envoi,
        label: "Limite Telegram (Mo)",
        kind: Kind::U64 {
            min: 1,
            max: 2000,
            step: 10,
        },
        default: |c| Value::U64(c.telegram_upload_limit_mb),
        description: "Taille maximale envoyée directement sur Telegram, en Mo.",
        hot: true,
        warning: None,
    },
    Spec {
        key: "send.external_fallback",
        section: Section::Envoi,
        label: "Repli externe",
        kind: Kind::Bool,
        default: |c| Value::Bool(c.external_upload_url.is_some()),
        description: "Au-delà de la limite, envoyer chez l'hébergeur externe (accord demandé si EXTERNAL_UPLOAD_CONSENT).",
        hot: true,
        warning: Some("Le fichier part vers un hébergeur tiers (curl.libriciel.fr par défaut, comme le bot Python)"),
    },
    Spec {
        key: "send.compress",
        section: Section::Envoi,
        label: "Compression",
        kind: Kind::Bool,
        default: |_| Value::Bool(false),
        description: "Compresser les vidéos trop lourdes avant l'envoi.",
        hot: true,
        warning: None,
    },
    // Stockage
    Spec {
        key: "sto.retention_small_h",
        section: Section::Stockage,
        label: "Rétention petits (h)",
        kind: Kind::U64 {
            min: 1,
            max: 720,
            step: 6,
        },
        default: |c| Value::U64(c.retention_small.as_secs() / 3600),
        description: "Conservation des mp3 et des fichiers sous SMALL_FILE_SIZE_MB, en heures.",
        hot: true,
        warning: None,
    },
    Spec {
        key: "sto.retention_large_h",
        section: Section::Stockage,
        label: "Rétention gros (h)",
        kind: Kind::U64 {
            min: 1,
            max: 720,
            step: 6,
        },
        default: |c| Value::U64(c.retention_large.as_secs() / 3600),
        description: "Conservation des fichiers à partir de SMALL_FILE_SIZE_MB, en heures.",
        hot: true,
        warning: None,
    },
    Spec {
        key: "sto.min_free_mb",
        section: Section::Stockage,
        label: "Espace libre min (Mo)",
        kind: Kind::U64 {
            min: 0,
            max: 1_000_000,
            step: 100,
        },
        default: |c| Value::U64(c.min_free_disk_mb),
        description: "Espace disque libre visé par le nettoyage, en Mo.",
        hot: true,
        warning: None,
    },
    Spec {
        key: "sto.cleanup_min",
        section: Section::Stockage,
        label: "Nettoyage (min)",
        kind: Kind::U64 {
            min: 1,
            max: 10080,
            step: 5,
        },
        // CLEANUP_INTERVAL_HOURS (24 h par défaut, comme le bot Python).
        default: |c| Value::U64((c.cleanup_interval.as_secs() / 60).clamp(1, 10080)),
        description: "Intervalle entre deux nettoyages, en minutes.",
        hot: false,
        warning: None,
    },
    // Accès
    Spec {
        key: "acc.allowed_users",
        section: Section::Acces,
        label: "Utilisateurs autorisés",
        kind: Kind::IdList,
        default: |c| ids(&c.allowed_user_ids),
        description: "IDs Telegram numériques autorisés (vide = bot ouvert à tous, comme le bot Python).",
        hot: true,
        warning: None,
    },
    Spec {
        key: "acc.rate_limit_per_min",
        section: Section::Acces,
        label: "Limite / minute",
        kind: Kind::U64 {
            min: 1,
            max: 1000,
            step: 1,
        },
        default: |c| Value::U64(c.rate_limit_per_min.into()),
        description: "Requêtes par utilisateur et par minute.",
        hot: true,
        warning: None,
    },
    Spec {
        key: "acc.admins",
        section: Section::Acces,
        label: "Administrateurs",
        kind: Kind::IdList,
        default: |c| ids(&c.admin_user_ids),
        description: "IDs Telegram numériques des administrateurs (jamais vide).",
        hot: true,
        warning: Some("Un ID retiré perd aussitôt l'accès à /settings"),
    },
    // Système
    Spec {
        key: "sys.version",
        section: Section::Systeme,
        label: "Version",
        kind: Kind::ReadOnly,
        default: |c| Value::Text(c.version.clone()),
        description: "Version du bot, la même que /help (VERSION, lecture seule).",
        hot: true,
        warning: None,
    },
    Spec {
        key: "sys.ytdlp_update_h",
        section: Section::Systeme,
        label: "MAJ yt-dlp (h)",
        kind: Kind::U64 {
            min: 0,
            max: 720,
            step: 6,
        },
        default: |c| Value::U64(c.ytdlp_update_interval.as_secs() / 3600),
        description: "Intervalle de mise à jour de yt-dlp, en heures (0 = jamais).",
        hot: false,
        warning: None,
    },
    Spec {
        key: "sys.log_level",
        section: Section::Systeme,
        label: "Journaux",
        kind: Kind::Choice(&["error", "warn", "info", "debug"]),
        default: |_| Value::Text("debug".into()),
        description: "Niveau de détail des journaux du bot (debug par défaut, comme le bot Python).",
        hot: false,
        warning: None,
    },
];

/// Spec d'une clé, `None` si inconnue.
pub fn spec(key: &str) -> Option<&'static Spec> {
    SCHEMA.iter().find(|s| s.key == key)
}

/// Valeurs par défaut de tous les réglages, dérivées de la configuration d'environnement.
pub fn defaults_from(cfg: &Config) -> Values {
    SCHEMA.iter().map(|s| (s.key, (s.default)(cfg))).collect()
}

/// Taille max d'une saisie (anti-abus).
const MAX_RAW: usize = 4096;

/// Valide STRICTEMENT une saisie brute pour `spec` ; renvoie la raison du refus en français.
pub fn parse(spec: &Spec, raw: &str) -> Result<Value, String> {
    if raw.len() > MAX_RAW {
        return Err("saisie trop longue".into());
    }
    let raw = raw.trim();
    match spec.kind {
        Kind::Bool => match raw.to_ascii_lowercase().as_str() {
            "on" | "true" | "1" | "oui" => Ok(Value::Bool(true)),
            "off" | "false" | "0" | "non" => Ok(Value::Bool(false)),
            _ => Err("booléen attendu (on/off)".into()),
        },
        Kind::U64 { min, max, .. } => {
            if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
                return Err("entier positif attendu".into());
            }
            let n: u64 = raw.parse().map_err(|_| "nombre trop grand".to_string())?;
            if n < min || n > max {
                return Err(format!("valeur hors limites ({min} à {max})"));
            }
            Ok(Value::U64(n))
        }
        Kind::Choice(opts) => opts
            .iter()
            .find(|o| **o == raw)
            .map(|o| Value::Text((*o).into()))
            .ok_or_else(|| format!("valeur attendue parmi : {}", opts.join(", "))),
        Kind::IdList => {
            let mut v = Vec::new();
            for tok in raw.split([',', ' ', '\n']).filter(|t| !t.is_empty()) {
                if !tok.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(
                        "IDs numériques attendus (pas de pseudo @…), séparés par des virgules"
                            .into(),
                    );
                }
                match tok.parse::<u64>() {
                    Ok(id) if id > 0 => v.push(id),
                    _ => return Err(format!("ID invalide : {tok}")),
                }
            }
            v.sort_unstable();
            v.dedup();
            if spec.key == "acc.admins" && v.is_empty() {
                return Err("il faut au moins un administrateur".into());
            }
            Ok(Value::Ids(v))
        }
        Kind::Order(opts) => {
            let v: Vec<String> = raw.split(',').map(|s| s.trim().to_string()).collect();
            let mut sorted: Vec<&str> = v.iter().map(String::as_str).collect();
            sorted.sort_unstable();
            let mut want = opts.to_vec();
            want.sort_unstable();
            if sorted != want {
                return Err(format!(
                    "ordre attendu : chacun une fois parmi {}",
                    opts.join(", ")
                ));
            }
            Ok(Value::List(v))
        }
        Kind::ReadOnly => Err("réglage en lecture seule".into()),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::HashMap;

    pub(crate) fn cfg() -> Config {
        let m: HashMap<String, String> = [
            ("BOT_TOKEN", "SECRET-TOKEN-zz9"),
            ("ADMIN_USER_IDS", "111111111"),
            ("EXTERNAL_UPLOAD_URL", "https://secret-host.invalid/up"),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .collect();
        Config::from_map(&m).unwrap()
    }

    #[test]
    fn schema_cles_uniques_et_courtes() {
        let mut keys: Vec<&str> = SCHEMA.iter().map(|s| s.key).collect();
        keys.sort_unstable();
        let n = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), n, "clé en double");
        for s in Section::ALL {
            assert!(SCHEMA.iter().any(|x| x.section == s), "{s:?} vide");
            assert_eq!(Section::from_id(s.id()), Some(s));
        }
    }

    #[test]
    fn defauts_providers_tous_desactives() {
        let d = defaults_from(&cfg());
        for k in [
            "providers.x.enabled",
            "providers.bluesky.enabled",
            "providers.tiktok.enabled",
        ] {
            assert_eq!(d[k], Value::Bool(false), "{k}");
        }
        assert_eq!(d["providers.ytdlp_fallback"], Value::Bool(true));
        assert!(spec("providers.tiktok.enabled")
            .unwrap()
            .warning
            .unwrap()
            .contains("API publique tierce"));
    }

    #[test]
    fn defauts_viennent_de_config_et_sans_secret() {
        let c = cfg();
        let d = defaults_from(&c);
        assert_eq!(d["dl.max_filesize_mb"], Value::U64(500));
        assert_eq!(d["acc.admins"], Value::Ids(vec![111_111_111]));
        assert_eq!(d["send.external_fallback"], Value::Bool(true));
        // chaque défaut est accepté par sa propre validation (sauf lecture seule)
        for s in SCHEMA.iter().filter(|s| s.kind != Kind::ReadOnly) {
            assert_eq!(
                parse(s, &d[s.key].to_string()).as_ref(),
                Ok(&d[s.key]),
                "{}",
                s.key
            );
        }
        let all: String = d.values().map(|v| v.to_string()).collect();
        assert!(!all.contains("SECRET") && !all.contains("secret-host"));
    }

    #[test]
    fn borne_hors_limites_refusee() {
        let s = spec("dl.max_filesize_mb").unwrap();
        assert!(parse(s, "0").is_err());
        assert!(parse(s, "4001").is_err());
        assert!(parse(s, "-5").is_err());
        assert!(parse(s, "99999999999999999999999").is_err());
        assert_eq!(parse(s, " 4000 "), Ok(Value::U64(4000)));
        assert!(parse(spec("dl.quality").unwrap(), "4k").is_err());
        assert!(parse(spec("providers.order").unwrap(), "x,x,tiktok").is_err());
        assert!(parse(spec("sys.version").unwrap(), "9").is_err());
    }

    #[test]
    fn pseudo_refuse() {
        let s = spec("acc.allowed_users").unwrap();
        assert!(parse(s, "@quelquun").is_err());
        assert!(parse(s, "111111111,@x").is_err());
        assert!(parse(s, "-1").is_err());
        assert!(parse(s, "0").is_err());
        assert_eq!(
            parse(s, "22, 111111111 22"),
            Ok(Value::Ids(vec![22, 111_111_111]))
        );
        assert_eq!(parse(s, ""), Ok(Value::Ids(vec![])));
        assert!(parse(spec("acc.admins").unwrap(), "").is_err());
    }
}
