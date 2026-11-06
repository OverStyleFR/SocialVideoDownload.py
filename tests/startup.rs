//! Démarrage réel : `bot::spawn_background` lance bien le janitor et la mise à jour de yt-dlp.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use socialvideodownload::bot::{self, AppState};
use socialvideodownload::config::Config;

fn state(root: &Path, extra: &[(&str, &str)]) -> Arc<AppState> {
    let ext = if cfg!(windows) { "cmd" } else { "sh" };
    let ytdlp = root.join(format!("fake-ytdlp.{ext}"));
    // Faux yt-dlp : consigne ses arguments, sort en succès.
    let body = if cfg!(windows) {
        "@echo off\r\necho %* >> \"%~dp0calls.txt\"\r\n"
    } else {
        "#!/bin/sh\necho \"$@\" >> \"$(dirname \"$0\")/calls.txt\"\n"
    };
    std::fs::write(&ytdlp, body).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&ytdlp, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let mut env: HashMap<String, String> = [
        ("BOT_TOKEN", "123:abc"),
        ("DOWNLOAD_DIR", root.join("dl").to_str().unwrap()),
        ("YTDLP_PATH", ytdlp.to_str().unwrap()),
    ]
    .into_iter()
    .chain(extra.iter().copied())
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    env.entry("MIN_FREE_DISK_MB".into()).or_insert("0".into());
    std::fs::create_dir_all(root.join("dl")).unwrap();
    AppState::new(Arc::new(Config::from_map(&env).unwrap()))
}

/// Dossier de travail plus vieux de `age`, sans verrou (comme après un crash).
fn old_dir(root: &Path, name: &str, age: Duration) -> std::path::PathBuf {
    let p = root.join("dl").join(name);
    std::fs::create_dir_all(&p).unwrap();
    std::fs::write(p.join("a.mp4"), b"x").unwrap();
    let mut oo = std::fs::OpenOptions::new();
    // Windows : écriture requise pour changer la date ; Linux : un dossier ne s'ouvre qu'en lecture.
    oo.write(cfg!(windows)).read(!cfg!(windows));
    #[cfg(windows)]
    std::os::windows::fs::OpenOptionsExt::custom_flags(&mut oo, 0x0200_0000); // BACKUP_SEMANTICS : ouvrir un dossier
    oo.open(&p)
        .unwrap()
        .set_modified(SystemTime::now() - age)
        .unwrap();
    p
}

async fn until(mut cond: impl FnMut() -> bool) -> bool {
    for _ in 0..100 {
        if cond() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    cond()
}

// (a) RETENTION_HOURS : un vieux dossier disparaît dès le démarrage, un récent reste.
#[tokio::test(flavor = "multi_thread")]
async fn a_janitor_removes_expired_job_dir() {
    let root = tempfile::tempdir().unwrap();
    let st = state(root.path(), &[("RETENTION_HOURS", "1")]);
    let old = old_dir(root.path(), "vieux", Duration::from_secs(3 * 3600));
    let fresh = old_dir(root.path(), "recent", Duration::ZERO);
    let _h = bot::spawn_background(&st);
    assert!(
        until(|| !old.exists()).await,
        "vieux dossier jamais nettoyé"
    );
    assert!(fresh.exists(), "le récent doit survivre");
}

// (b) YTDLP_UPDATE_INTERVAL_H>0 : yt-dlp reçoit la mise à jour ; =0 : jamais appelé.
#[tokio::test(flavor = "multi_thread")]
async fn b_updater_calls_ytdlp_only_when_enabled() {
    let root = tempfile::tempdir().unwrap();
    let st = state(root.path(), &[("YTDLP_UPDATE_INTERVAL_H", "24")]);
    let calls = root.path().join("calls.txt");
    let _h = bot::spawn_background(&st);
    assert!(until(|| calls.exists()).await, "yt-dlp jamais appelé");
    let text = std::fs::read_to_string(&calls).unwrap();
    assert!(text.contains("--update-to stable"), "{text}");

    let root0 = tempfile::tempdir().unwrap();
    let st0 = state(root0.path(), &[("YTDLP_UPDATE_INTERVAL_H", "0")]);
    let _h0 = bot::spawn_background(&st0);
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(!root0.path().join("calls.txt").exists());
}

// (c) MIN_FREE_DISK_MB arrive jusqu'au nettoyage : seuil énorme = même un récent part ; 0 = il reste.
#[tokio::test(flavor = "multi_thread")]
async fn c_min_free_disk_reaches_cleanup() {
    let root = tempfile::tempdir().unwrap();
    let st = state(root.path(), &[("MIN_FREE_DISK_MB", "99999999")]);
    let fresh = old_dir(root.path(), "recent", Duration::ZERO);
    let _h = bot::spawn_background(&st);
    assert!(until(|| !fresh.exists()).await, "seuil ignoré");

    let root0 = tempfile::tempdir().unwrap();
    let st0 = state(root0.path(), &[("MIN_FREE_DISK_MB", "0")]);
    let keep = old_dir(root0.path(), "recent", Duration::ZERO);
    let _h0 = bot::spawn_background(&st0);
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(keep.exists());
}
