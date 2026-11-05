//! Texte de /stats : mise en page du bot Python (Markdown historique de Telegram), puis les
//! compteurs ajoutés par la refonte en fin de message.

use std::path::Path;

use super::AppState;
use crate::cache::CacheStats;
use crate::metrics::Snapshot;
use crate::retention::{self, RetentionStats};
use crate::system::{self, Memory};

const MO: f64 = 1_048_576.0;

/// Données affichées par /stats.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StatsData {
    pub version: String,
    pub developed_by: String,
    pub cache: CacheStats,
    /// Politique de rétention affichée : seuil (Mo), heures petits, heures gros.
    pub policy: (u64, u64, u64),
    pub retention: RetentionStats,
    /// Espace libre du disque de téléchargement (octets).
    pub free_bytes: Option<u64>,
    /// Fichiers et octets du dossier de téléchargement (récursif).
    pub files: u64,
    pub bytes: u64,
    /// URLs enregistrées (équivalent de `hashes.txt`).
    pub urls: usize,
    pub log_files: u64,
    pub log_bytes: u64,
    pub uptime_s: Option<u64>,
    pub cpu: Option<f64>,
    pub memory: Option<Memory>,
    // --- ajouts de la refonte ---
    pub metrics: Snapshot,
    pub active_jobs: usize,
    pub max_jobs: usize,
    pub pending_consents: usize,
    pub biggest: Option<(String, u64)>,
}

/// Décimale à virgule française (`_fmt_fr` du bot Python).
fn fr(v: f64, decimals: usize) -> String {
    format!("{v:.decimals$}").replace('.', ",")
}

/// Échappe ce qui casse le Markdown historique en dehors des blocs de code.
fn esc(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('_', "\\_")
        .replace('*', "\\*")
        .replace('`', "\\`")
        .replace('[', "\\[")
}

/// Mise en forme du bot Python, ligne pour ligne.
pub fn render_stats(d: &StatsData) -> String {
    let na = || "n/a".to_string();
    let next = match &d.retention.next {
        Some((name, left_s)) => format!("`Prochain:` {} dans {} min", esc(name), left_s / 60),
        None => "`Prochain:` aucun fichier".to_string(),
    };
    let c = &d.cache;
    let (small_mb, small_h, large_h) = d.policy;
    let r = &d.retention;
    let mut t = format!(
        "📊 *Statistiques du Bot*\n\
         `Version:` {}\n\
         `Développé par:` {}\n\n\
         🗂️ *Cache (session)*\n\
         `Entrées totales:` {}\n\
         `Entrées valides:` {}\n\
         `Entrées expirées:` {}\n\
         `Petits fichiers (≤5Mo):` {}\n\
         `Gros fichiers (>5Mo):` {}\n\
         `Taille totale cache:` {:.2} Mo\n\
         `Hits cache:` {} ({:.2} Mo économisés)\n\n\
         💾 *Disque*\n\
         `Espace libre:` {}\n\
         `Fichiers downloads:` {} ({:.2} Mo)\n\
         `URLs enregistrées:` {}\n\
         \n⏳ *Rétention*\n\
         `Politique:` petits ≤{} Mo → {}h, gros → {}h\n\
         `Fichiers:` {} ({} Mo)\n\
         {}\n\
         `Libérable 2h:` {} Mo ({} fichiers)\n\
         `Libérable 24h:` {} Mo ({} fichiers)\n\
         📝 *Logs*\n\
         `Fichiers logs:` {}\n\
         `Taille totale logs:` {:.2} Mo\n\n\
         🖥️ *Système*\n\
         `Uptime:` {}\n\
         `CPU:` {}\n\
         `RAM:` {}\n",
        esc(&d.version),
        esc(&d.developed_by),
        c.total,
        c.valid,
        c.expired,
        c.small,
        c.large,
        c.total_size as f64 / MO,
        c.hits,
        c.bytes_saved as f64 / MO,
        d.free_bytes
            .map_or_else(na, |b| format!("{} Go", fr(b as f64 / 1_073_741_824.0, 2))),
        d.files,
        d.bytes as f64 / MO,
        d.urls,
        small_mb,
        small_h,
        large_h,
        r.files,
        fr(r.bytes as f64 / MO, 2),
        next,
        fr(r.freed_2h.1 as f64 / MO, 2),
        r.freed_2h.0,
        fr(r.freed_24h.1 as f64 / MO, 2),
        r.freed_24h.0,
        d.log_files,
        d.log_bytes as f64 / MO,
        d.uptime_s
            .map_or_else(na, |s| format!("{}h {}m", s / 3600, s % 3600 / 60)),
        d.cpu.map_or_else(na, |p| format!("{p:.1}%")),
        d.memory.map_or_else(na, |m| format!(
            "{}/{} Go ({:.1}%)",
            fr(m.used as f64 / 1_073_741_824.0, 1),
            fr(m.total as f64 / 1_073_741_824.0, 1),
            m.percent()
        )),
    );
    t.push_str(&activity(d));
    t
}

/// Bloc ajouté par la refonte (taux de réussite, p50/p95, erreurs, tâches), après le texte d'origine.
fn activity(d: &StatsData) -> String {
    let m = &d.metrics;
    let (ok, ko) = m
        .per_source
        .iter()
        .fold((0, 0), |a, (_, o, k)| (a.0 + o, a.1 + k));
    let mut l = vec![
        format!("\n📈 *Activité (depuis {})*", duration(m.uptime_s)),
        format!("`Tâches actives:` {}/{}", d.active_jobs, d.max_jobs),
        format!("`Envois en attente d'accord:` {}", d.pending_consents),
    ];
    if let Some((name, size)) = &d.biggest {
        l.push(format!(
            "`Plus gros fichier:` {} ({:.1} Mo)",
            esc(name),
            *size as f64 / MO
        ));
    }
    if ok + ko == 0 {
        l.push("Aucun téléchargement pour l'instant.".into());
    } else {
        l.push(format!(
            "`Tâches:` {ok} réussies, {ko} en échec ({:.0} % de réussite)",
            100.0 * ok as f64 / (ok + ko) as f64
        ));
        for (src, o, k) in &m.per_source {
            l.push(format!("`{}:` {o} ok, {k} échecs", src.replace('`', "")));
        }
        if let (Some(p50), Some(p95)) = (m.p50_s, m.p95_s) {
            l.push(format!("`Durée médiane / p95:` {p50:.1} s / {p95:.1} s"));
        }
        for (msg, n) in &m.top_errors {
            l.push(format!("`Erreur fréquente:` {} (×{n})", esc(msg)));
        }
    }
    l.join("\n")
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

/// Fichiers (non récursif) et octets du dossier des journaux.
fn logs_usage(dir: &Path) -> (u64, u64) {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.metadata().ok().filter(|m| m.is_file()))
        .fold((0, 0), |a, m| (a.0 + 1, a.1 + m.len()))
}

/// Collecte les chiffres puis met en forme.
pub async fn stats_text(state: &AppState) -> String {
    let cfg = state.config.clone();
    let tiers = retention::current_tiers(&cfg, Some(&state.settings));
    let (root, logs) = (cfg.download_dir.clone(), cfg.logs_dir.clone());
    let sys = tokio::task::spawn_blocking(move || {
        let (files, bytes, biggest) = usage(&root);
        let (log_files, log_bytes) = logs_usage(&logs);
        StatsData {
            retention: retention::stats(&root, tiers),
            free_bytes: system::free_bytes(&root),
            files,
            bytes,
            biggest,
            log_files,
            log_bytes,
            uptime_s: system::uptime().map(|d| d.as_secs()),
            cpu: system::cpu_percent(),
            memory: system::memory(),
            ..StatsData::default()
        }
    })
    .await
    .unwrap_or_default();
    let max = cfg.max_concurrent_downloads;
    let cache = state.cache.stats();
    render_stats(&StatsData {
        version: cfg.version.clone(),
        developed_by: cfg.developed_by.clone(),
        urls: cache.total,
        cache,
        policy: (
            tiers.small_bytes >> 20,
            tiers.small.as_secs() / 3600,
            tiers.large.as_secs() / 3600,
        ),
        metrics: state.metrics.snapshot(),
        active_jobs: max.saturating_sub(state.semaphore.available_permits()),
        max_jobs: max,
        pending_consents: state.pending.lock().map_or(0, |p| p.len()),
        ..sys
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exemple() -> StatsData {
        StatsData {
            version: "V10.0.0".into(),
            developed_by: "Tom V. | OverStyleFR".into(),
            cache: CacheStats {
                total: 3,
                valid: 2,
                expired: 1,
                small: 1,
                large: 1,
                total_size: 21_000_000,
                hits: 2,
                bytes_saved: 2_000_000,
            },
            policy: (12, 24, 2),
            retention: RetentionStats {
                files: 2,
                bytes: 3_145_728,
                next: Some(("a_b.mp4".into(), 7_199)),
                freed_2h: (1, 1_048_576),
                freed_24h: (2, 3_145_728),
            },
            free_bytes: Some(53_687_091_200),
            files: 2,
            bytes: 3_145_728,
            urls: 3,
            log_files: 4,
            log_bytes: 2_097_152,
            uptime_s: Some(3 * 3600 + 25 * 60),
            cpu: Some(12.34),
            memory: Some(Memory {
                used: 4_294_967_296,
                total: 8_589_934_592,
            }),
            ..Default::default()
        }
    }

    #[test]
    fn mise_en_page_identique_au_bot_python() {
        let t = render_stats(&exemple());
        // Texte d'origine (commands/stats.py) : tout ce qui précède l'ajout de la refonte.
        let attendu = "📊 *Statistiques du Bot*\n\
`Version:` V10.0.0\n\
`Développé par:` Tom V. | OverStyleFR\n\n\
🗂️ *Cache (session)*\n\
`Entrées totales:` 3\n\
`Entrées valides:` 2\n\
`Entrées expirées:` 1\n\
`Petits fichiers (≤5Mo):` 1\n\
`Gros fichiers (>5Mo):` 1\n\
`Taille totale cache:` 20.03 Mo\n\
`Hits cache:` 2 (1.91 Mo économisés)\n\n\
💾 *Disque*\n\
`Espace libre:` 50,00 Go\n\
`Fichiers downloads:` 2 (3.00 Mo)\n\
`URLs enregistrées:` 3\n\
\n⏳ *Rétention*\n\
`Politique:` petits ≤12 Mo → 24h, gros → 2h\n\
`Fichiers:` 2 (3,00 Mo)\n\
`Prochain:` a\\_b.mp4 dans 119 min\n\
`Libérable 2h:` 1,00 Mo (1 fichiers)\n\
`Libérable 24h:` 3,00 Mo (2 fichiers)\n\
📝 *Logs*\n\
`Fichiers logs:` 4\n\
`Taille totale logs:` 2.00 Mo\n\n\
🖥️ *Système*\n\
`Uptime:` 3h 25m\n\
`CPU:` 12.3%\n\
`RAM:` 4,0/8,0 Go (50.0%)\n";
        assert!(t.starts_with(attendu), "{t}");
    }

    #[test]
    fn sans_donnees_systeme_ni_fichier_affiche_na_et_aucun_fichier() {
        let t = render_stats(&StatsData::default());
        assert!(t.contains("`Prochain:` aucun fichier"), "{t}");
        assert!(t.contains("`Espace libre:` n/a"), "{t}");
        assert!(t.contains("`CPU:` n/a") && t.contains("`RAM:` n/a"), "{t}");
    }

    #[test]
    fn ajouts_de_la_refonte_en_fin_de_message() {
        let t = render_stats(&StatsData {
            metrics: Snapshot {
                uptime_s: 7_500,
                per_source: vec![("Twitter".into(), 3, 1)],
                p50_s: Some(2.0),
                p95_s: Some(9.5),
                top_errors: vec![("Trop gros (max 500 Mo).".into(), 1)],
            },
            ..exemple()
        });
        assert!(t.contains("📈 *Activité (depuis 2 h 5 min)*"), "{t}");
        assert!(
            t.contains("3 réussies, 1 en échec (75 % de réussite)"),
            "{t}"
        );
        assert!(t.contains("`Durée médiane / p95:` 2.0 s / 9.5 s"), "{t}");
        assert!(t.find("🖥️").unwrap() < t.find("📈").unwrap());
    }

    #[test]
    fn nom_de_fichier_echappe_pour_le_markdown() {
        assert_eq!(esc("a_b*[c]`d"), "a\\_b\\*\\[c]\\`d");
    }
}
