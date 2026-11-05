//! Compteurs de /stats : réussites/échecs par source, durées, erreurs fréquentes.
//! En mémoire depuis le démarrage (shortcut : remis à zéro au redémarrage, persister si le besoin se confirme).

use std::collections::{BTreeMap, VecDeque};
use std::sync::Mutex;
use std::time::Instant;

/// Nombre de durées gardées pour les percentiles.
const DURATIONS_KEPT: usize = 500;

#[derive(Default)]
struct Inner {
    /// source -> (réussites, échecs)
    per_source: BTreeMap<String, (u64, u64)>,
    durations: VecDeque<f64>,
    /// message d'erreur montré à l'utilisateur -> occurrences (jamais d'URL ni d'ID).
    errors: BTreeMap<String, u64>,
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
    fn empty_has_no_percentile_and_ring_is_bounded() {
        let m = Metrics::default();
        assert_eq!(m.snapshot().p50_s, None);
        for _ in 0..DURATIONS_KEPT + 50 {
            m.record("x", 1.0, None);
        }
        assert_eq!(m.inner.lock().unwrap().durations.len(), DURATIONS_KEPT);
    }
}
