//! Erreurs par domaine ; chacune a un message utilisateur en français.

use thiserror::Error;

/// Erreur de chargement de la configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Variable obligatoire absente.
    #[error("variable obligatoire absente : {0}")]
    Missing(&'static str),
    /// Valeur invalide.
    #[error("valeur invalide pour {key} : {reason}")]
    Invalid { key: &'static str, reason: String },
}

/// Erreur de validation d'URL (anti-SSRF).
#[derive(Debug, Error)]
pub enum UrlError {
    /// Texte non analysable comme URL.
    #[error("URL mal formée")]
    Malformed,
    /// Schéma autre que http/https.
    #[error("schéma refusé : {0}")]
    Scheme(String),
    /// URL sans hôte.
    #[error("hôte absent")]
    NoHost,
    /// Résolution DNS en échec.
    #[error("résolution DNS impossible : {0}")]
    Dns(String),
    /// Hôte résolu vers une adresse interdite (privée, loopback…).
    #[error("adresse interdite : {0}")]
    Forbidden(std::net::IpAddr),
}

/// Erreur de téléchargement (yt-dlp).
#[derive(Debug, Error)]
pub enum DownloadError {
    /// Plateforme ou contenu non pris en charge.
    #[error("contenu non pris en charge")]
    Unsupported,
    /// Fichier trop gros.
    #[error("fichier trop volumineux ({size_mb} Mo > {limit_mb} Mo)")]
    TooLarge { size_mb: u64, limit_mb: u64 },
    /// Média trop long.
    #[error("média trop long ({duration_s} s > {limit_s} s)")]
    TooLong { duration_s: u64, limit_s: u64 },
    /// yt-dlp a échoué.
    #[error("yt-dlp a échoué : {0}")]
    Tool(String),
    /// Délai dépassé.
    #[error("délai dépassé")]
    Timeout,
    /// Erreur d'entrée/sortie.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Erreur de traitement média (ffmpeg/ffprobe).
#[derive(Debug, Error)]
pub enum MediaError {
    /// ffprobe a échoué ou sortie illisible.
    #[error("analyse du média impossible : {0}")]
    Probe(String),
    /// ffmpeg a échoué.
    #[error("conversion impossible : {0}")]
    Ffmpeg(String),
    /// Impossible d'atteindre la taille visée.
    #[error("compression insuffisante pour {limit_bytes} octets")]
    CannotFit { limit_bytes: u64 },
    /// Erreur d'entrée/sortie.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Erreur d'envoi vers l'hébergement externe.
#[derive(Debug, Error)]
pub enum EgressError {
    /// Repli externe non configuré.
    #[error("hébergement externe désactivé")]
    Disabled,
    /// Erreur HTTP.
    #[error("échec HTTP : {0}")]
    Http(String),
    /// Réponse inattendue.
    #[error("réponse invalide : {0}")]
    BadResponse(String),
    /// Erreur d'entrée/sortie.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Refus d'accès.
#[derive(Debug, Error)]
pub enum AccessError {
    /// Utilisateur non autorisé.
    #[error("utilisateur non autorisé")]
    NotAllowed,
    /// Commande réservée aux admins.
    #[error("réservé aux administrateurs")]
    NotAdmin,
    /// Limite de débit atteinte.
    #[error("limite de débit, réessayer dans {retry_after_s} s")]
    RateLimited { retry_after_s: u64 },
}

/// Erreur applicative agrégée.
#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Url(#[from] UrlError),
    #[error(transparent)]
    Download(#[from] DownloadError),
    #[error(transparent)]
    Media(#[from] MediaError),
    #[error(transparent)]
    Egress(#[from] EgressError),
    #[error(transparent)]
    Access(#[from] AccessError),
}

impl AppError {
    /// Message destiné à l'utilisateur Telegram (jamais de détail interne).
    pub fn user_message(&self) -> String {
        match self {
            Self::Config(_) => "Le bot est mal configuré, préviens l'administrateur.".into(),
            Self::Url(UrlError::Forbidden(_)) => {
                "Ce lien pointe vers une adresse interdite.".into()
            }
            Self::Url(UrlError::Scheme(_)) => "Seuls les liens http(s) sont acceptés.".into(),
            Self::Url(_) => "Ce lien est invalide ou injoignable.".into(),
            Self::Download(DownloadError::Unsupported) => {
                "Ce contenu n'est pas pris en charge.".into()
            }
            Self::Download(DownloadError::TooLarge { limit_mb, .. }) => {
                format!("Fichier trop volumineux (limite {limit_mb} Mo).")
            }
            Self::Download(DownloadError::TooLong { limit_s, .. }) if *limit_s < 60 => {
                format!("Média trop long (limite {limit_s} s).")
            }
            Self::Download(DownloadError::TooLong { limit_s, .. }) => {
                format!("Média trop long (limite {} min).", limit_s / 60)
            }
            Self::Download(DownloadError::Timeout) => {
                "Le téléchargement a pris trop de temps.".into()
            }
            Self::Download(_) => "Le téléchargement a échoué, réessaie plus tard.".into(),
            Self::Media(MediaError::CannotFit { .. }) => {
                "Impossible de réduire le fichier sous la limite de Telegram.".into()
            }
            Self::Media(_) => "Le traitement du fichier a échoué.".into(),
            Self::Egress(EgressError::Disabled) => {
                "Fichier trop gros pour Telegram et l'envoi vers un hébergeur externe est désactivé."
                    .into()
            }
            Self::Egress(_) => "L'envoi vers l'hébergement externe a échoué.".into(),
            Self::Access(AccessError::NotAllowed) => {
                "Tu n'es pas autorisé à utiliser ce bot.".into()
            }
            Self::Access(AccessError::NotAdmin) => "Commande réservée aux administrateurs.".into(),
            Self::Access(AccessError::RateLimited { retry_after_s }) => {
                format!("Trop de demandes, réessaie dans {retry_after_s} s.")
            }
        }
    }
}
