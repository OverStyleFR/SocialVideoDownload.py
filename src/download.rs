//! Téléchargement via yt-dlp.
//!
//! yt-dlp RESTE un binaire externe lancé en sous-processus : Rust ne le remplace pas
//! (des centaines d'extracteurs, mis à jour chaque semaine). On le pilote sans shell
//! (arguments en `Vec`, URL après `--`), avec délai global et `kill_on_drop`.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tokio::process::Command;
use tokio::task::JoinHandle;
use url::Url;

use crate::config::Config;
use crate::error::DownloadError;
use crate::providers;
use crate::retention::JobDir;

/// Délai global d'un fetch (la config n'en porte pas) ; voir [`Downloader::with_timeout`].
const FETCH_TIMEOUT: Duration = Duration::from_secs(600);
/// Délai de la mise à jour de yt-dlp.
const UPDATE_TIMEOUT: Duration = Duration::from_secs(120);

/// Ce qu'on veut récupérer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    Video,
    Audio,
}

/// Résultat d'un téléchargement réussi.
#[derive(Debug, Clone)]
pub struct Downloaded {
    /// Fichier produit, dans le JobDir.
    pub path: PathBuf,
    /// Identifiant du média côté plateforme.
    pub id: String,
    /// Titre du média.
    pub title: String,
    /// Durée en secondes si connue.
    pub duration_s: Option<u64>,
    /// Taille en octets.
    pub size: u64,
}

/// Pilote yt-dlp selon la configuration.
#[derive(Debug, Clone)]
pub struct Downloader {
    /// Configuration partagée.
    pub config: Arc<Config>,
    timeout: Duration,
}

impl Downloader {
    /// Construit un Downloader.
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            config,
            timeout: FETCH_TIMEOUT,
        }
    }

    /// Remplace le délai global d'un fetch.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Télécharge `url` dans `job` selon `mode`, en respectant taille et durée max.
    pub async fn fetch(
        &self,
        url: &Url,
        mode: Mode,
        job: &JobDir,
    ) -> Result<Downloaded, DownloadError> {
        self.fetch_in(url, mode, job.path()).await
    }

    async fn fetch_in(
        &self,
        url: &Url,
        mode: Mode,
        dir: &Path,
    ) -> Result<Downloaded, DownloadError> {
        let args = self.build_args(url, mode, dir);
        let out = run(&self.config.ytdlp_path, &args, dir, self.timeout).await?;
        let stderr = String::from_utf8_lossy(&out.stderr);
        if !out.status.success() {
            return Err(map_stderr(&stderr, &self.config));
        }
        let stdout = String::from_utf8_lossy(&out.stdout);
        // Rien d'imprimé et sortie 0 : yt-dlp a sauté le média (filtre de durée, direct…).
        let Some(line) = stdout
            .lines()
            .rev()
            .find(|l| l.trim_start().starts_with('{'))
        else {
            // yt-dlp écrit « does not pass filter » sur stdout.
            return Err(map_stderr(
                &format!(
                    "{stderr}
{stdout}"
                ),
                &self.config,
            ));
        };
        let info: Info = serde_json::from_str(line)
            .map_err(|e| DownloadError::Tool(format!("sortie yt-dlp illisible : {e}")))?;
        let path = PathBuf::from(&info.filepath);
        // Le chemin vient d'un sous-processus : il doit rester dans le JobDir.
        if !path.starts_with(dir) {
            return Err(DownloadError::Tool(
                "fichier produit hors du répertoire de travail".into(),
            ));
        }
        let size = tokio::fs::metadata(&path)
            .await
            .map_err(|_| DownloadError::Tool("fichier produit introuvable".into()))?
            .len();
        let limit_mb = self.config.max_filesize_mb;
        if size > limit_mb * 1024 * 1024 {
            return Err(DownloadError::TooLarge {
                size_mb: size / (1024 * 1024),
                limit_mb,
            });
        }
        Ok(Downloaded {
            path,
            id: info.id,
            title: info.title.unwrap_or_default(),
            duration_s: info.duration.map(|d| d as u64),
            size,
        })
    }

    fn build_args(&self, url: &Url, mode: Mode, dir: &Path) -> Vec<String> {
        let c = &self.config;
        let mut a: Vec<String> = vec![
            "--no-playlist".into(),
            "--no-progress".into(),
            "--restrict-filenames".into(),
            "--max-filesize".into(),
            format!("{}M", c.max_filesize_mb),
            "--match-filter".into(),
            format!("duration <= {} & !is_live", c.max_duration_s),
            // Nom par identifiant, jamais par titre ; .part reste dans le JobDir.
            "-o".into(),
            format!(
                "{}/%(id)s.%(ext)s",
                dir.display().to_string().replace('\\', "/")
            ),
            "--print".into(),
            "after_move:%(.{id,title,duration,filepath})j".into(),
        ];
        match mode {
            // Audio seul : la conversion (mp3…) est laissée au module média.
            Mode::Audio => a.extend(["-f".into(), "bestaudio".into()]),
            Mode::Video => a.extend([
                "-f".into(),
                format!(
                    "b[ext=mp4][filesize<{}M]/bv*[ext=mp4]+ba[ext=m4a]/b",
                    c.max_filesize_mb
                ),
                "--merge-output-format".into(),
                "mp4".into(),
            ]),
        }
        a.extend(providers::ytdlp_args(providers::detect(url)));
        a.push("--".into());
        a.push(url.as_str().into());
        a
    }

    /// Met à jour yt-dlp (`--update-to stable`). Binaire en lecture seule : simple avertissement.
    pub async fn update_ytdlp(&self) -> Result<(), DownloadError> {
        let args = ["--update-to".to_string(), "stable".to_string()];
        let out = run(
            &self.config.ytdlp_path,
            &args,
            Path::new("."),
            UPDATE_TIMEOUT,
        )
        .await?;
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if out.status.success() {
            tracing::info!(sortie = %text, "yt-dlp mis à jour");
            return Ok(());
        }
        let low = err.to_lowercase();
        if [
            "permission denied",
            "read-only",
            "not writable",
            "installed yt-dlp with pip",
            "access is denied",
        ]
        .iter()
        .any(|k| low.contains(k))
        {
            tracing::warn!(erreur = %err, "mise à jour de yt-dlp impossible (binaire non modifiable)");
            return Ok(());
        }
        Err(DownloadError::Tool(last_line(&err)))
    }
}

/// Lance la mise à jour au démarrage puis toutes les `ytdlp_update_interval` (0 = jamais).
pub fn spawn_updater(dl: Arc<Downloader>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let every = dl.config.ytdlp_update_interval;
        if every.is_zero() {
            return;
        }
        loop {
            if let Err(e) = dl.update_ytdlp().await {
                tracing::warn!(erreur = %e, "échec de la mise à jour de yt-dlp");
            }
            tokio::time::sleep(every).await;
        }
    })
}

#[derive(serde::Deserialize)]
struct Info {
    id: String,
    title: Option<String>,
    duration: Option<f64>,
    filepath: String,
}

/// Lance `bin` sans shell, `kill_on_drop`, avec délai ; le processus est tué si le délai expire.
async fn run(
    bin: &Path,
    args: &[String],
    cwd: &Path,
    timeout: Duration,
) -> Result<std::process::Output, DownloadError> {
    let child = Command::new(bin)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(r) => Ok(r?),
        Err(_) => Err(DownloadError::Timeout), // future abandonnée : child lâché, donc tué
    }
}

fn last_line(s: &str) -> String {
    s.lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("erreur inconnue")
        .trim()
        .to_string()
}

/// Premier entier suivi de « bytes » dans `s`, converti en Mo.
fn first_bytes_mb(s: &str) -> Option<u64> {
    let i = s.find(" bytes")?;
    let digits: String = s[..i]
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect();
    let n: u64 = digits.chars().rev().collect::<String>().parse().ok()?;
    Some(n / (1024 * 1024))
}

fn map_stderr(stderr: &str, c: &Config) -> DownloadError {
    let low = stderr.to_lowercase();
    if low.contains("larger than max-filesize") {
        let size_mb = first_bytes_mb(stderr).unwrap_or(c.max_filesize_mb + 1);
        DownloadError::TooLarge {
            size_mb,
            limit_mb: c.max_filesize_mb,
        }
    } else if low.contains("does not pass filter") {
        // yt-dlp ne donne pas la durée réelle dans ce message.
        DownloadError::TooLong {
            duration_s: c.max_duration_s + 1,
            limit_s: c.max_duration_s,
        }
    } else if low.contains("unsupported url") || low.contains("playlist") {
        DownloadError::Unsupported
    } else {
        // Extracteur cassé ou autre : détail pour les logs, jamais montré tel quel à l'utilisateur.
        DownloadError::Tool(last_line(stderr))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Faux yt-dlp : id = dernier segment de l'URL (dernier argument) ; `sleep` dort, `big` échoue.
    #[cfg(unix)]
    const FAKE: (&str, &str) = (
        "fake-ytdlp.sh",
        r#"#!/bin/sh
for a in "$@"; do last="$a"; done
id="${last##*/}"
printf '%s\n' "$@" > "args-$id.txt"
case "$id" in
  sleep) sleep 30 ;;
  big) echo "ERROR: File is larger than max-filesize (600000000 bytes > 524288000 bytes). Aborting." >&2; exit 1 ;;
  long) echo "[download] x does not pass filter (duration <= 3600 & !is_live), skipping .." ;;
  *) echo data > "$id.mp4"
     printf '{"id":"%s","title":"meme titre","duration":12.5,"filepath":"%s/%s.mp4"}\n' "$id" "$(pwd)" "$id" ;;
esac
"#,
    );
    #[cfg(windows)]
    const FAKE: (&str, &str) = (
        "fake-ytdlp.cmd",
        r#"@echo off
setlocal
for %%a in (%*) do set last=%%a
set last=%last:"=%
for %%a in (%last:/= %) do set id=%%a
echo %*> args-%id%.txt
if "%id%"=="sleep" ping -n 4 127.0.0.1 >nul
if "%id%"=="big" (echo ERROR: File is larger than max-filesize ^(600000000 bytes ^> 524288000 bytes^). Aborting. 1>&2 & exit /b 1)
if "%id%"=="long" (echo [download] x does not pass filter & exit /b 0)
echo data> %id%.mp4
set cwd=%cd:\=/%
echo {"id":"%id%","title":"meme titre","duration":12.5,"filepath":"%cwd%/%id%.mp4"}
"#,
    );

    fn setup(timeout: Duration) -> (tempfile::TempDir, Downloader) {
        let tmp = tempfile::tempdir().unwrap();
        let script = tmp.path().join(FAKE.0);
        std::fs::write(&script, FAKE.1.replace("\r\n", "\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let env: HashMap<String, String> = [("BOT_TOKEN", "123:abc"), ("ALLOW_ALL_USERS", "1")]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let mut cfg = Config::from_map(&env).unwrap();
        cfg.ytdlp_path = script;
        (tmp, Downloader::new(Arc::new(cfg)).with_timeout(timeout))
    }

    fn work(tmp: &tempfile::TempDir, name: &str) -> PathBuf {
        let d = tmp.path().join(name);
        std::fs::create_dir(&d).unwrap();
        d
    }

    fn url(id: &str) -> Url {
        Url::parse(&format!("https://example.com/v/{id}")).unwrap()
    }

    #[tokio::test]
    async fn download_args_et_url_apres_double_tiret() {
        let (tmp, dl) = setup(Duration::from_secs(30));
        let dir = work(&tmp, "j1");
        let r = dl.fetch_in(&url("abc"), Mode::Video, &dir).await.unwrap();
        assert_eq!(
            (r.id.as_str(), r.title.as_str(), r.duration_s),
            ("abc", "meme titre", Some(12))
        );
        assert!(r.size > 0 && r.path.starts_with(&dir));
        let args = std::fs::read_to_string(dir.join("args-abc.txt")).unwrap();
        for k in [
            "--no-playlist",
            "--max-filesize",
            "500M",
            "--match-filter",
            "%(id)s",
            "duration <= 3600",
        ] {
            assert!(args.contains(k), "{k} absent : {args}");
        }
        let i = args
            .find(" -- ")
            .or_else(|| args.find("\n--\n"))
            .expect("séparateur --");
        assert!(args[i..].contains("https://example.com/v/abc"));
        assert!(!args[..i].contains("https://example.com/v/abc"));
    }

    #[tokio::test]
    async fn download_timeout_tue_le_processus() {
        let (tmp, dl) = setup(Duration::from_millis(500));
        let dir = work(&tmp, "j2");
        let t = std::time::Instant::now();
        let e = dl
            .fetch_in(&url("sleep"), Mode::Audio, &dir)
            .await
            .unwrap_err();
        assert!(matches!(e, DownloadError::Timeout), "{e:?}");
        assert!(t.elapsed() < Duration::from_secs(10));
    }

    #[tokio::test]
    async fn download_erreurs_typees() {
        let (tmp, dl) = setup(Duration::from_secs(30));
        let dir = work(&tmp, "j3");
        let e = dl
            .fetch_in(&url("big"), Mode::Video, &dir)
            .await
            .unwrap_err();
        assert!(
            matches!(
                e,
                DownloadError::TooLarge {
                    size_mb: 572,
                    limit_mb: 500
                }
            ),
            "{e:?}"
        );
        let e = dl
            .fetch_in(&url("long"), Mode::Video, &dir)
            .await
            .unwrap_err();
        assert!(
            matches!(e, DownloadError::TooLong { limit_s: 3600, .. }),
            "{e:?}"
        );
    }

    #[tokio::test]
    async fn download_deux_fetch_concurrents_fichiers_distincts() {
        let (tmp, dl) = setup(Duration::from_secs(30));
        let (d1, d2) = (work(&tmp, "a"), work(&tmp, "b"));
        let (u1, u2) = (url("un"), url("deux"));
        let (r1, r2) = tokio::join!(
            dl.fetch_in(&u1, Mode::Video, &d1),
            dl.fetch_in(&u2, Mode::Video, &d2)
        );
        let (r1, r2) = (r1.unwrap(), r2.unwrap());
        assert_eq!(
            (r1.title.as_str(), r2.title.as_str()),
            ("meme titre", "meme titre")
        );
        assert_ne!(r1.path, r2.path);
        assert!(r1.path.exists() && r2.path.exists());
    }

    #[tokio::test]
    async fn download_audio_sans_video_entiere() {
        let (tmp, dl) = setup(Duration::from_secs(30));
        let a = dl.build_args(&url("x"), Mode::Audio, tmp.path());
        assert!(a.windows(2).any(|w| w == ["-f", "bestaudio"]));
        assert_eq!(a[a.len() - 2], "--");
    }
}
