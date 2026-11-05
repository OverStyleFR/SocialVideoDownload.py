//! Chiffres système pour /stats (équivalent de psutil et shutil.disk_usage du bot Python).
//! Linux : lus dans /proc ; ailleurs, les valeurs sont absentes (affichées « n/a »).

use std::path::Path;
use std::time::Duration;

/// Mémoire vive en octets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Memory {
    pub used: u64,
    pub total: u64,
}

impl Memory {
    pub fn percent(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            100.0 * self.used as f64 / self.total as f64
        }
    }
}

/// Temps écoulé depuis le démarrage de la machine.
pub fn uptime() -> Option<Duration> {
    let s = std::fs::read_to_string("/proc/uptime").ok()?;
    s.split_whitespace()
        .next()?
        .parse::<f64>()
        .ok()
        .map(Duration::from_secs_f64)
}

/// RAM utilisée = total − disponible (comme psutil `used` à peu près).
pub fn memory() -> Option<Memory> {
    parse_meminfo(&std::fs::read_to_string("/proc/meminfo").ok()?)
}

fn parse_meminfo(s: &str) -> Option<Memory> {
    let kb = |key: &str| -> Option<u64> {
        s.lines()
            .find_map(|l| l.strip_prefix(key))?
            .trim_start_matches(':')
            .split_whitespace()
            .next()?
            .parse()
            .ok()
    };
    let total = kb("MemTotal")? * 1024;
    let available = kb("MemAvailable")? * 1024;
    Some(Memory {
        used: total.saturating_sub(available),
        total,
    })
}

/// CPU en % sur ~100 ms (comme `psutil.cpu_percent(interval=0.1)`). Bloquant.
pub fn cpu_percent() -> Option<f64> {
    let a = cpu_times()?;
    std::thread::sleep(Duration::from_millis(100));
    let b = cpu_times()?;
    let (busy, total) = (b.0.saturating_sub(a.0), b.1.saturating_sub(a.1));
    (total > 0).then(|| 100.0 * busy as f64 / total as f64)
}

/// (temps actif, temps total) cumulés de la ligne `cpu` de /proc/stat.
fn cpu_times() -> Option<(u64, u64)> {
    parse_cpu_line(&std::fs::read_to_string("/proc/stat").ok()?)
}

fn parse_cpu_line(s: &str) -> Option<(u64, u64)> {
    let v: Vec<u64> = s
        .lines()
        .next()?
        .strip_prefix("cpu")?
        .split_whitespace()
        .filter_map(|x| x.parse().ok())
        .collect();
    let total: u64 = v.iter().take(8).sum(); // user nice system idle iowait irq softirq steal
    let idle = v.get(3)? + v.get(4).copied().unwrap_or(0);
    Some((total.saturating_sub(idle), total))
}

/// Espace libre (octets) du disque qui porte `path` (ou `.` si absent).
#[cfg(unix)]
pub fn free_bytes(path: &Path) -> Option<u64> {
    let probe = if path.exists() { path } else { Path::new(".") };
    let st = rustix::fs::statvfs(probe).ok()?;
    Some(st.f_bavail * st.f_frsize)
}

#[cfg(not(unix))]
pub fn free_bytes(_path: &Path) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meminfo_et_cpu_lus_comme_psutil() {
        let m = parse_meminfo(
            "MemTotal:       8000000 kB\nMemFree: 1 kB\nMemAvailable:   2000000 kB\n",
        )
        .unwrap();
        assert_eq!((m.total, m.used), (8_000_000 * 1024, 6_000_000 * 1024));
        assert!((m.percent() - 75.0).abs() < 1e-9);
        // user=10 nice=0 system=10 idle=70 iowait=10 → actif 20 sur 100
        assert_eq!(
            parse_cpu_line("cpu  10 0 10 70 10 0 0 0 0 0\ncpu0 1 1 1 1\n"),
            Some((20, 100))
        );
    }

    #[cfg(unix)]
    #[test]
    fn espace_libre_du_dossier_courant() {
        assert!(free_bytes(Path::new(".")).is_some_and(|b| b > 0));
    }
}
