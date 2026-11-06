//! Compteurs de /stats : réussites/échecs par source, durées, erreurs fréquentes, demandes,
//! envois, pic de tâches.
//! En mémoire depuis le démarrage (shortcut : remis à zéro au redémarrage, persister si le besoin se confirme).

use std::collections::{BTreeMap, HashSet, VecDeque};
use std::sync::Mutex;
use std::time::Instant;

/// Nombre de durées gardées pour les percentiles.
const DURATIONS_KEPT: usize = 500;
/// Borne des utilisateurs distincts comptés (shortcut : au-delà, le compte s'arrête ; HyperLogLog si besoin).
const USERS_KEPT: usize = 100_000;

#[derive(Default)]
struct Inner {
    /// source -> (réussites, échecs)
    per_source: BTreeMap<String, (u64, u64)>,
    durations: VecDeque<f64>,
    /// message d'erreur montré à l'utilisateur -> occurrences (jamais d'URL ni d'ID).
    errors: BTreeMap<String, u64>,
    /// Seulement compté, jamais affiché.
    users: HashSet<u64>,
    videos: u64,
    musics: u64,
    sent_files: u64,
    sent_bytes: u64,
    external: u64,
    peak_jobs: usize,
}

/// Instantané lisible, sans verrou.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub uptime_s: u64,
    pub per_source: Vec<(String, u64, u64)>,
    pub p50_s: Option<f64>,
    pub p95_s: Option<f64>,
    /// Les 3 erreurs les plus fréquentes.
    pub top_errors: Vec<(String, u64)>,
    pub users: usize,
    pub videos: u64,
    pub musics: u64,
    /// Fichiers et octets envoyés par Telegram (hors cache).
    pub sent_files: u64,
    pub sent_bytes: u64,
    /// Envois chez l'hébergeur externe réussis.
    pub external: u64,
    pub peak_jobs: usize,
}

pub struct Metrics {
    started: Instant,
    inner: Mutex<Inner>,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            inner: Mutex::default(),
        }
    }
}

/// Percentile (méthode du rang le plus proche) d'une liste triée.
fn percentile(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let rank = ((p * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len());
    Some(sorted[rank - 1])
}

impl Metrics {
    /// Note la fin d'une tâche : `error` = message utilisateur si elle a échoué.
    pub fn record(&self, source: &str, secs: f64, error: Option<&str>) {
        let Ok(mut m) = self.inner.lock() else { return };
        let e = m.per_source.entry(source.to_string()).or_default();
        match error {
            None => e.0 += 1,
            Some(msg) => {
                e.1 += 1;
                *m.errors.entry(msg.to_string()).or_default() += 1;
            }
        }
        if m.durations.len() == DURATIONS_KEPT {
            m.durations.pop_front();
        }
        m.durations.push_back(secs);
    }

    /// Note une demande (vidéo ou musique) d'un utilisateur.
    pub fn note_request(&self, user: u64, music: bool) {
        let Ok(mut m) = self.inner.lock() else { return };
        if m.users.len() < USERS_KEPT {
            m.users.insert(user);
        }
        if music {
            m.musics += 1;
        } else {
            m.videos += 1;
        }
    }

    /// Note un téléchargement qui démarre avec `active` tâches en cours.
    pub fn note_active(&self, active: usize) {
        let Ok(mut m) = self.inner.lock() else { return };
        m.peak_jobs = m.peak_jobs.max(active);
    }

    /// Note un envoi réussi : `Some(octets)` par Telegram, `None` chez l'hébergeur externe.
    pub fn note_sent(&self, bytes: Option<u64>) {
        let Ok(mut m) = self.inner.lock() else { return };
        match bytes {
            Some(b) => {
                m.sent_files += 1;
                m.sent_bytes += b;
            }
            None => m.external += 1,
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let Ok(m) = self.inner.lock() else {
            return Snapshot::default();
        };
        let mut d: Vec<f64> = m.durations.iter().copied().collect();
        d.sort_by(f64::total_cmp);
        let mut errs: Vec<(String, u64)> = m.errors.iter().map(|(k, v)| (k.clone(), *v)).collect();
        errs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        errs.truncate(3);
        Snapshot {
            uptime_s: self.started.elapsed().as_secs(),
            per_source: m
                .per_source
                .iter()
                .map(|(k, (ok, ko))| (k.clone(), *ok, *ko))
                .collect(),
            p50_s: percentile(&d, 0.5),
            p95_s: percentile(&d, 0.95),
            top_errors: errs,
            users: m.users.len(),
            videos: m.videos,
            musics: m.musics,
            sent_files: m.sent_files,
            sent_bytes: m.sent_bytes,
            external: m.external,
            peak_jobs: m.peak_jobs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_percentiles_and_top_errors() {
        let m = Metrics::default();
        for i in 1..=10 {
            m.record("Twitter", f64::from(i), None);
        }
        m.record("YouTube", 20.0, Some("Trop long."));
        m.record("YouTube", 30.0, Some("Trop long."));
        m.record("YouTube", 40.0, Some("Source en panne."));
        let s = m.snapshot();
        assert_eq!(
            s.per_source,
            vec![("Twitter".into(), 10, 0), ("YouTube".into(), 0, 3)]
        );
        assert_eq!(s.p50_s, Some(7.0));
        assert_eq!(s.p95_s, Some(40.0));
        assert_eq!(s.top_errors[0], ("Trop long.".into(), 2));
        assert_eq!(s.top_errors.len(), 2);
    }

    #[test]
    fn users_kinds_sends_and_peak() {
        let m = Metrics::default();
        m.note_request(1, false);
        m.note_request(1, true);
        m.note_request(2, false);
        m.note_active(3);
        m.note_active(1);
        m.note_sent(Some(1_000));
        m.note_sent(Some(500));
        m.note_sent(None);
        let s = m.snapshot();
        assert_eq!((s.users, s.videos, s.musics), (2, 2, 1));
        assert_eq!((s.sent_files, s.sent_bytes, s.external), (2, 1_500, 1));
        assert_eq!(s.peak_jobs, 3);
    }

    #[test]
    fn empty_has_no_percentile_and_ring_is_bounded() {
        let m = Metrics::default();
        assert_eq!(m.snapshot().p50_s, None);
        for _ in 0..DURATIONS_KEPT + 50 {
            m.record("x", 1.0, None);
        }
        assert_eq!(m.inner.lock().unwrap().durations.len(), DURATIONS_KEPT);
    }
}
