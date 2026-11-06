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
use crate::providers::router::{Decision, Router};
use crate::providers::source::{Http, Provider, Resolved};
use crate::providers::{bluesky::Bluesky, tiktok_api::TikTokApi, x_api::XApi};
use crate::retention::JobDir;
use crate::settings::SettingsStore;

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
#[derive(Clone)]
pub struct Downloader {
    /// Configuration partagée.
    pub config: Arc<Config>,
    timeout: Duration,
    settings: Option<SettingsStore>,
    api_base: Option<Url>,
}

impl std::fmt::Debug for Downloader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Downloader")
            .field("timeout", &self.timeout)
            .field("routage", &self.settings.is_some())
            .finish_non_exhaustive()
    }
}

impl Downloader {
    /// Construit un Downloader.
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            config,
            timeout: FETCH_TIMEOUT,
            settings: None,
            api_base: None,
        }
    }

    /// Active le routage par providers d'API, lu à chaque job dans les réglages (à chaud).
    pub fn with_settings(mut self, settings: SettingsStore) -> Self {
        self.settings = Some(settings);
        self
    }

    /// Remplace la base d'API des trois providers (serveur factice des tests).
    pub fn with_api_base(mut self, base: Url) -> Self {
        self.api_base = Some(base);
        self
    }

    /// Routeur construit depuis les réglages courants ; `None` sans réglages.
    fn router(&self) -> Option<Router> {
        let s = self.settings.clone()?;
        let http = Http::new(self.config.unsafe_allow_private_hosts);
        let providers = match &self.api_base {
            Some(b) => vec![
                Provider::X(XApi::new(http.clone(), b.clone())),
                Provider::Bluesky(Bluesky::new(http.clone(), b.clone())),
                Provider::TikTok(TikTokApi::new(http, b.clone())),
            ],
            None => vec![
                Provider::X(XApi::with_default_base(http.clone())),
                Provider::Bluesky(Bluesky::with_default_base(http.clone())),
                Provider::TikTok(TikTokApi::with_default_base(http)),
            ],
        };
        let order = s.get_order("providers.order").unwrap_or_default();
        let fallback = s.get_bool("providers.ytdlp_fallback").unwrap_or(true);
        let s2 = s.clone();
        Some(Router::new(
            providers,
            order,
            move |id| {
                s2.get_bool(&format!("providers.{id}.enabled"))
                    .unwrap_or(false)
            },
            fallback,
        ))
    }

    /// Remplace le délai global d'un fetch.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Taille (Mo) et durée (s) max effectives : `/settings` à chaud, sinon la config.
    fn limits(&self) -> (u64, u64) {
        let s = self.settings.as_ref();
        (
            s.and_then(|s| s.get_u64("dl.max_filesize_mb"))
                .unwrap_or(self.config.max_filesize_mb),
            s.and_then(|s| s.get_u64("dl.max_duration_s"))
                .unwrap_or(self.config.max_duration_s),
        )
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
        if mode == Mode::Video {
            if let Some(router) = self.router() {
                match router.route(url).await {
                    // HLS : yt-dlp/ffmpeg sait l'assembler, pas un simple GET.
                    Decision::Direct(r, id) if !r.media_url.path().ends_with(".m3u8") => {
                        match self.fetch_direct(url, &r, dir).await {
                            Ok(d) => return Ok(d),
                            Err(
                                e
                                @ (DownloadError::TooLarge { .. } | DownloadError::TooLong { .. }),
                            ) => return Err(e),
                            Err(e) => {
                                tracing::warn!(provider = %id, error = %e, "téléchargement direct en échec");
                                let fb = self
                                    .settings
                                    .as_ref()
                                    .and_then(|s| s.get_bool("providers.ytdlp_fallback"))
                                    .unwrap_or(true);
                                if !fb {
                                    return Err(e);
                                }
                            }
                        }
                    }
                    Decision::Refused(e) => return Err(DownloadError::Tool(e.to_string())),
                    _ => {}
                }
            }
        }
        let args = self.build_args(url, mode, dir);
        let out = run(&self.config.ytdlp_path, &args, dir, self.timeout).await?;
        let stderr = String::from_utf8_lossy(&out.stderr);
        if !out.status.success() {
            return Err(map_stderr(&stderr, self.limits()));
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
                self.limits(),
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
        let (limit_mb, _) = self.limits();
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

    /// Télécharge `r.media_url` en flux borné par `max_filesize_mb` (le provider a déjà validé l'URL).
    async fn fetch_direct(
        &self,
        url: &Url,
        r: &Resolved,
        dir: &Path,
    ) -> Result<Downloaded, DownloadError> {
        use tokio::io::AsyncWriteExt;
        let (limit_mb, max_s) = self.limits();
        let limit = limit_mb * 1024 * 1024;
        if let Some(d) = r.duration_s.filter(|d| *d > max_s) {
            return Err(DownloadError::TooLong {
                duration_s: d,
                limit_s: max_s,
            });
        }
        let too_large = |n: u64| DownloadError::TooLarge {
            size_mb: n / (1024 * 1024),
            limit_mb,
        };
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(self.timeout)
            .build()
            .map_err(|e| DownloadError::Tool(e.without_url().to_string()))?;
        let tool = |e: reqwest::Error| DownloadError::Tool(e.without_url().to_string());
        let mut resp = client.get(r.media_url.clone()).send().await.map_err(tool)?;
        if !resp.status().is_success() {
            return Err(DownloadError::Tool(format!(
                "statut HTTP {}",
                resp.status()
            )));
        }
        if let Some(n) = resp.content_length().filter(|n| *n > limit) {
            return Err(too_large(n));
        }
        // Nom par identifiant du lien (alphanumérique seulement), jamais par titre.
        let id: String = url
            .path_segments()
            .and_then(|mut s| s.next_back().map(str::to_string))
            .unwrap_or_default()
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect();
        let id = if id.is_empty() {
            "media".to_string()
        } else {
            id
        };
        let path = dir.join(format!("{id}.mp4"));
        let mut f = tokio::fs::File::create(&path).await?;
        let mut size = 0u64;
        while let Some(chunk) = resp.chunk().await.map_err(tool)? {
            size += chunk.len() as u64;
            if size > limit {
                drop(f);
                let _ = tokio::fs::remove_file(&path).await;
                return Err(too_large(size));
            }
            f.write_all(&chunk).await?;
        }
        f.flush().await?;
        Ok(Downloaded {
            path,
            id,
            title: r.title.clone(),
            duration_s: r.duration_s,
            size,
        })
    }

    fn build_args(&self, url: &Url, mode: Mode, dir: &Path) -> Vec<String> {
        let (max_mb, max_s) = self.limits();
        let mut a: Vec<String> = vec![
            "--no-playlist".into(),
            // Une seule vidéo par lien (la 1re d'un carrousel ou d'une playlist) : le bot envoie un fichier.
            "--playlist-items".into(),
            "1".into(),
            "--no-progress".into(),
            "--restrict-filenames".into(),
            "--max-filesize".into(),
            format!("{max_mb}M"),
            "--match-filter".into(),
            format!("duration <=? {max_s} & !is_live"),
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
                video_format(max_mb, self.max_height()),
                "--merge-output-format".into(),
                "mp4".into(),
            ]),
        }
        a.extend(providers::ytdlp_args(providers::detect(url)));
        a.push("--".into());
        a.push(url.as_str().into());
        a
    }

    /// Hauteur max de la vidéo (`dl.quality`) ; `None` = la meilleure.
    fn max_height(&self) -> Option<u32> {
        self.settings
            .as_ref()
            .and_then(|s| s.get_choice("dl.quality"))
            .and_then(|q| q.parse().ok())
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
            // Dernière ligne de yt-dlp : « yt-dlp is up to date (…) » ou « Updated yt-dlp to … ».
            tracing::info!("[YTDLP] {}", text.lines().last().unwrap_or_default());
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

/// Lance la mise à jour au démarrage puis toutes les `sys.ytdlp_update_h` heures (0 = jamais, lu au démarrage).
pub fn spawn_updater(dl: Arc<Downloader>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let every = dl
            .settings
            .as_ref()
            .and_then(|s| s.get_u64("sys.ytdlp_update_h"))
            .map_or(dl.config.ytdlp_update_interval, |h| {
                Duration::from_secs(h * 3600)
            });
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

/// Format vidéo : mp4 sous la taille max ; `height` borne la hauteur (`<=?` garde les formats
/// sans hauteur connue) et retombe sur la meilleure plutôt que d'échouer.
fn video_format(max_mb: u64, height: Option<u32>) -> String {
    let Some(h) = height else {
        return format!("b[ext=mp4][filesize<{max_mb}M]/bv*[ext=mp4]+ba[ext=m4a]/b");
    };
    let h = format!("[height<=?{h}]");
    format!("b[ext=mp4]{h}[filesize<{max_mb}M]/bv*[ext=mp4]{h}+ba[ext=m4a]/b{h}/b")
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

fn map_stderr(stderr: &str, (max_mb, max_s): (u64, u64)) -> DownloadError {
    let low = stderr.to_lowercase();
    if low.contains("larger than max-filesize") {
        let size_mb = first_bytes_mb(stderr).unwrap_or(max_mb + 1);
        DownloadError::TooLarge {
            size_mb,
            limit_mb: max_mb,
        }
    } else if low.contains("does not pass filter") {
        // yt-dlp ne donne pas la durée réelle dans ce message.
        DownloadError::TooLong {
            duration_s: max_s + 1,
            limit_s: max_s,
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
  sleep) sleep 2; echo x > done-sleep ;;
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
if "%id%"=="sleep" (ping -n 4 127.0.0.1 >nul & echo x> done-sleep)
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
        let text = FAKE.1.replace("\r\n", "\n");
        // Écrit par un `sh` enfant sous Unix : si ce processus tenait le script ouvert en écriture, un
        // test parallèle qui lance un programme au même moment hériterait du descripteur jusqu'à son
        // exec, et lancer le script échouerait en ETXTBSY (« Text file busy », vu en CI).
        #[cfg(unix)]
        {
            use std::io::Write;
            let mut sh = std::process::Command::new("sh")
                .args(["-c", "cat > \"$0\" && chmod 755 \"$0\""])
                .arg(&script)
                .stdin(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            sh.stdin.take().unwrap().write_all(text.as_bytes()).unwrap();
            assert!(sh.wait().unwrap().success());
        }
        #[cfg(windows)]
        std::fs::write(&script, text).unwrap();
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
            "duration <=? 3600",
        ] {
            assert!(args.contains(k), "{k} absent : {args}");
        }
        // sans `?`, yt-dlp rejette les URL sans durée connue (bug f13)
        assert!(!args.contains("duration <= "), "opérateur sans ? : {args}");
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
        // Le faux yt-dlp écrit un témoin après son attente : absent = processus tué.
        tokio::time::sleep(Duration::from_secs(5)).await;
        assert!(!dir.join("done-sleep").exists(), "processus non tué");
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

    #[test]
    fn reglages_appliques_a_yt_dlp() {
        let (tmp, dl) = setup(Duration::from_secs(30));
        let s = SettingsStore::load(&tmp.path().join("settings.json"), &dl.config).unwrap();
        let a = dl
            .clone()
            .with_settings(s.clone())
            .build_args(&url("x"), Mode::Video, tmp.path())
            .join(" ");
        // défauts : format d'origine, une seule vidéo par lien
        for k in [
            "--playlist-items 1",
            "-f b[ext=mp4][filesize<500M]/bv*[ext=mp4]+ba[ext=m4a]/b --",
        ] {
            assert!(a.contains(k), "{k} absent : {a}");
        }
        for (k, v) in [
            ("dl.max_filesize_mb", "50"),
            ("dl.max_duration_s", "60"),
            ("dl.quality", "720"),
        ] {
            s.set(k, v, 1).unwrap();
        }
        let dl = dl.with_settings(s);
        let a = dl.build_args(&url("x"), Mode::Video, tmp.path()).join(" ");
        for k in [
            "--max-filesize 50M",
            "duration <=? 60 & !is_live",
            "-f b[ext=mp4][height<=?720][filesize<50M]/bv*[ext=mp4][height<=?720]+ba[ext=m4a]/b[height<=?720]/b --",
        ] {
            assert!(a.contains(k), "{k} absent : {a}");
        }
        assert!(matches!(
            map_stderr("x does not pass filter", dl.limits()),
            DownloadError::TooLong { limit_s: 60, .. }
        ));
    }
}
