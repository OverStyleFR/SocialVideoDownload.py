//! Texte de /stats en MarkdownV2 (toute valeur dynamique est échappée).

use std::path::Path;

use super::markdown::escape_md_v2 as esc;
use super::AppState;

/// Données affichées par /stats.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatsData {
    pub version: String,
    pub active_jobs: usize,
    pub max_jobs: usize,
    pub cache_entries: usize,
    pub pending_consents: usize,
    pub files: u64,
    pub bytes: u64,
    /// Plus gros fichier du dossier de travail : (nom, octets).
    pub biggest: Option<(String, u64)>,
}

/// Mise en forme MarkdownV2 : seuls les `*` du titre sont des balises.
pub fn render_stats(d: &StatsData) -> String {
    let line = |label: &str, value: String| format!("{} : {}", esc(label), esc(&value));
    let mut lines = vec![
        format!("*{}*", esc("Statistiques")),
        line("Version", d.version.clone()),
        line(
            "Tâches actives",
            format!("{}/{}", d.active_jobs, d.max_jobs),
        ),
        line("Entrées du cache", d.cache_entries.to_string()),
        line(
            "Envois en attente de consentement",
            d.pending_consents.to_string(),
        ),
        line(
            "Dossier de travail",
            format!(
                "{} fichiers, {:.1} Mo",
                d.files,
                d.bytes as f64 / 1_048_576.0
            ),
        ),
    ];
    if let Some((name, size)) = &d.biggest {
        lines.push(line(
            "Plus gros fichier",
            format!("{name} ({:.1} Mo)", *size as f64 / 1_048_576.0),
        ));
    }
    lines.join("\n")
}

/// Parcourt `root` : (nombre de fichiers, octets, plus gros fichier).
fn usage(root: &Path) -> (u64, u64, Option<(String, u64)>) {
    let (mut files, mut bytes, mut biggest) = (0, 0, None::<(String, u64)>);
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let Ok(md) = e.metadata() else { continue };
            if md.is_dir() {
                stack.push(e.path());
            } else {
                files += 1;
                bytes += md.len();
                if biggest.as_ref().is_none_or(|b| md.len() > b.1) {
                    biggest = Some((e.file_name().to_string_lossy().into_owned(), md.len()));
                }
            }
        }
    }
    (files, bytes, biggest)
}

/// Collecte les chiffres puis met en forme.
pub async fn stats_text(state: &AppState) -> String {
    let root = state.config.download_dir.clone();
    let (files, bytes, biggest) = tokio::task::spawn_blocking(move || usage(&root))
        .await
        .unwrap_or_default();
    let max = state.config.max_concurrent_downloads;
    render_stats(&StatsData {
        version: env!("CARGO_PKG_VERSION").into(),
        active_jobs: max.saturating_sub(state.semaphore.available_permits()),
        max_jobs: max,
        cache_entries: state.cache.len(),
        pending_consents: state.pending.lock().map_or(0, |p| p.len()),
        files,
        bytes,
        biggest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_and_values_are_escaped() {
        let t = render_stats(&StatsData {
            version: "0.1.0".into(),
            files: 2,
            bytes: 1_048_576,
            biggest: Some(("a_b*[c].mp4".into(), 2_097_152)),
            ..Default::default()
        });
        assert!(t.starts_with("*Statistiques*"));
        assert!(t.contains(r"a\_b\*\[c\]\.mp4 \(2\.0 Mo\)"), "{t}");
        assert!(t.contains(r"Version : 0\.1\.0"), "{t}");
        assert!(!t.contains("a_b*"), "{t}");
    }
}
