//! Texte de /stats en MarkdownV2 (toute valeur dynamique est échappée).

use std::path::Path;

use super::markdown::escape_md_v2 as esc;
use super::AppState;
use crate::metrics::Snapshot;

/// Données affichées par /stats.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StatsData {
    /// Réussites, échecs, durées et erreurs depuis le démarrage.
    pub metrics: Snapshot,
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
    let m = &d.metrics;
    let (ok, ko) = m
        .per_source
        .iter()
        .fold((0, 0), |a, (_, o, k)| (a.0 + o, a.1 + k));
    lines.push(String::new());
    lines.push(format!(
        "*{}*",
        esc(&format!("Depuis le démarrage ({})", duration(m.uptime_s)))
    ));
    if ok + ko == 0 {
        lines.push(esc("Aucun téléchargement pour l'instant."));
    } else {
        lines.push(line(
            "Tâches",
            format!(
                "{ok} réussies, {ko} en échec ({:.0} % de réussite)",
                100.0 * ok as f64 / (ok + ko) as f64
            ),
        ));
        for (src, o, k) in &m.per_source {
            lines.push(line(&format!("  {src}"), format!("{o} ok, {k} échecs")));
        }
        if let (Some(p50), Some(p95)) = (m.p50_s, m.p95_s) {
            lines.push(line(
                "Durée médiane / p95",
                format!("{p50:.1} s / {p95:.1} s"),
            ));
        }
        for (msg, n) in &m.top_errors {
            lines.push(line("Erreur fréquente", format!("{msg} (×{n})")));
        }
    }
    lines.join("\n")
}

/// « 3 j 4 h », « 2 h 5 min », « 40 s ».
fn duration(s: u64) -> String {
    match (s / 86_400, s % 86_400 / 3_600, s % 3_600 / 60) {
        (0, 0, 0) => format!("{s} s"),
        (0, 0, m) => format!("{m} min"),
        (0, h, m) => format!("{h} h {m} min"),
        (d, h, _) => format!("{d} j {h} h"),
    }
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
        metrics: state.metrics.snapshot(),
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
        assert!(t.contains("Aucun téléchargement"), "{t}");
    }

    #[test]
    fn shows_success_rate_sources_and_escaped_errors() {
        let t = render_stats(&StatsData {
            metrics: Snapshot {
                uptime_s: 7_500,
                per_source: vec![("Twitter".into(), 3, 1)],
                p50_s: Some(2.0),
                p95_s: Some(9.5),
                top_errors: vec![("Trop gros (max 500 Mo).".into(), 1)],
            },
            ..Default::default()
        });
        assert!(t.contains(r"Depuis le démarrage \(2 h 5 min\)"), "{t}");
        assert!(
            t.contains(r"3 réussies, 1 en échec \(75 % de réussite\)"),
            "{t}"
        );
        assert!(t.contains(r"Durée médiane / p95 : 2\.0 s / 9\.5 s"), "{t}");
        assert!(t.contains(r"Trop gros \(max 500 Mo\)\. \(×1\)"), "{t}");
    }
}
