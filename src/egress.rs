//! Repli d'hébergement externe pour les fichiers trop gros pour Telegram.

use std::path::Path;

use url::Url;

use crate::config::Config;
use crate::error::EgressError;

/// Envoie `path` vers EXTERNAL_UPLOAD_URL et rend le lien public ; `Disabled` si non configuré.
pub async fn upload_external(cfg: &Config, path: &Path) -> Result<Url, EgressError> {
    let _ = (cfg, path);
    todo!("egress::upload_external")
}

/// Texte demandant le consentement avant d'envoyer un fichier de `size_mb` Mo hors de Telegram.
pub fn consent_text(size_mb: u64) -> String {
    let _ = size_mb;
    todo!("egress::consent_text")
}
