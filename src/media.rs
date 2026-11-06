//! Analyse et conversion média via ffprobe/ffmpeg (pas de shell, `kill_on_drop`, délais).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde_json::Value;
use tokio::process::Command;

use crate::config::Config;
use crate::error::MediaError;

const PROBE_TIMEOUT: Duration = Duration::from_secs(30);
const ENCODE_TIMEOUT: Duration = Duration::from_secs(900);
/// Débit audio AAC réservé lors de la compression (bits/s).
const AUDIO_BPS: u64 = 128_000;
/// En dessous de ce débit vidéo, l'image est inexploitable : on renonce.
const MIN_VIDEO_BPS: u64 = 80_000;
/// En dessous de ce niveau (dB), la piste est considérée muette.
const SILENCE_DB: f64 = -80.0;

/// Caractéristiques d'un fichier média.
#[derive(Debug, Clone, PartialEq)]
pub struct Probe {
    /// Durée en secondes.
    pub duration: f64,
    /// Présence d'une piste audio.
    pub has_audio: bool,
    /// Présence d'une piste vidéo.
    pub has_video: bool,
    /// Taille en octets.
    pub size: u64,
}

/// Problème d'audio à signaler à l'utilisateur plutôt que d'envoyer en silence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioIssue {
    /// Aucune piste audio dans le fichier.
    NoAudioTrack,
    /// Piste présente mais muette ou vide.
    SilentTrack,
}

/// Sortie d'un process : (succès, stdout, stderr).
type Out = (bool, Vec<u8>, String);

/// Lance `bin args` sans shell ; erreur texte si lancement impossible ou délai dépassé.
async fn run(bin: &Path, args: &[OsString], timeout: Duration) -> Result<Out, String> {
    let mut cmd = Command::new(bin);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let child = cmd
        .spawn()
        .map_err(|e| format!("{} : {e}", bin.display()))?;
    let out = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| {
            format!(
                "{} : délai de {}s dépassé",
                bin.display(),
                timeout.as_secs()
            )
        })?
        .map_err(|e| e.to_string())?;
    let err = String::from_utf8_lossy(&out.stderr).into_owned();
    Ok((out.status.success(), out.stdout, err))
}

fn args<const N: usize>(a: [&str; N]) -> Vec<OsString> {
    a.iter().map(OsString::from).collect()
}

/// Les 300 derniers caractères d'un message d'erreur ffmpeg.
fn tail(s: &str) -> String {
    let t: Vec<char> = s.trim().chars().collect();
    t[t.len().saturating_sub(300)..].iter().collect()
}

/// Analyse un JSON `ffprobe -show_format -show_streams` (pure, testée).
fn parse_probe(json: &[u8], size: u64) -> Result<Probe, MediaError> {
    let v: Value = serde_json::from_slice(json).map_err(|e| MediaError::Probe(e.to_string()))?;
    let has = |kind: &str| {
        v["streams"]
            .as_array()
            .is_some_and(|s| s.iter().any(|x| x["codec_type"] == kind))
    };
    let duration = v["format"]["duration"]
        .as_str()
        .and_then(|d| d.parse::<f64>().ok())
        .or_else(|| v["format"]["duration"].as_f64())
        .filter(|d| d.is_finite() && *d > 0.0)
        .ok_or_else(|| MediaError::Probe("durée absente ou nulle".into()))?;
    Ok(Probe {
        duration,
        has_audio: has("audio"),
        has_video: has("video"),
        size,
    })
}

/// Analyse `path` avec ffprobe.
pub async fn probe(cfg: &Config, path: &Path) -> Result<Probe, MediaError> {
    let size = tokio::fs::metadata(path).await?.len();
    let mut a = args([
        "-v",
        "error",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
    ]);
    a.push(path.into());
    let (ok, out, err) = run(&cfg.ffprobe_path, &a, PROBE_TIMEOUT)
        .await
        .map_err(MediaError::Probe)?;
    if !ok {
        return Err(MediaError::Probe(tail(&err)));
    }
    parse_probe(&out, size)
}

/// Extrait `max_volume` (dB) de la sortie `volumedetect` ; `None` si aucun échantillon décodé.
fn parse_max_volume(stderr: &str) -> Option<f64> {
    let line = stderr.lines().find(|l| l.contains("max_volume:"))?;
    let v = line
        .split("max_volume:")
        .nth(1)?
        .trim()
        .trim_end_matches("dB")
        .trim();
    if v.contains("inf") {
        Some(f64::NEG_INFINITY)
    } else {
        v.parse().ok()
    }
}

/// Signale une vidéo sans piste audio ou une piste muette/vide (`None` = audio présent et audible).
pub async fn audio_issue(cfg: &Config, path: &Path) -> Result<Option<AudioIssue>, MediaError> {
    if !probe(cfg, path).await?.has_audio {
        return Ok(Some(AudioIssue::NoAudioTrack));
    }
    let mut a = args(["-hide_banner", "-nostdin", "-i"]);
    a.push(path.into());
    a.extend(args(["-vn", "-af", "volumedetect", "-f", "null", "-"]));
    let (ok, _, err) = run(&cfg.ffmpeg_path, &a, ENCODE_TIMEOUT)
        .await
        .map_err(MediaError::Ffmpeg)?;
    if !ok {
        return Err(MediaError::Ffmpeg(tail(&err)));
    }
    Ok(match parse_max_volume(&err) {
        Some(db) if db > SILENCE_DB => None,
        _ => Some(AudioIssue::SilentTrack),
    })
}

/// Débit vidéo cible (bits/s) pour tenir sous `limit_bytes` (marge 5 %) ; `None` si trop bas.
fn target_video_bps(limit_bytes: u64, duration: f64, audio_bps: u64) -> Option<u64> {
    let total = limit_bytes as f64 * 0.95 * 8.0 / duration;
    let video = total - audio_bps as f64;
    (video >= MIN_VIDEO_BPS as f64).then_some(video as u64)
}

fn tmp_name(dst: &Path) -> PathBuf {
    let mut s = dst.as_os_str().to_owned();
    s.push(".tmp");
    s.into()
}

/// Recompresse `path` pour passer sous `limit_bytes` ; rend le chemin du fichier produit.
///
/// Rend `path` tel quel s'il est déjà sous la limite. Sinon écrit `<nom>.small.mp4` à côté
/// (via un `.tmp` renommé après succès) : la source n'est jamais touchée.
pub async fn compress_under(
    cfg: &Config,
    path: &Path,
    limit_bytes: u64,
) -> Result<PathBuf, MediaError> {
    let p = probe(cfg, path).await?;
    if p.size <= limit_bytes {
        return Ok(path.to_path_buf());
    }
    let audio_bps = if p.has_audio { AUDIO_BPS } else { 0 };
    let Some(mut vbps) = target_video_bps(limit_bytes, p.duration, audio_bps) else {
        return Err(MediaError::CannotFit { limit_bytes });
    };
    let dst = path.with_extension("small.mp4");
    let tmp = tmp_name(&dst);
    // Passe 1 : résolution d'origine ; passe 2 : 480p max et débit réduit de 20 %.
    for pass in 0..2 {
        if pass == 1 {
            vbps = vbps * 4 / 5;
            if vbps < MIN_VIDEO_BPS {
                break;
            }
        }
        if let Err(e) = encode(cfg, path, &tmp, vbps, p.has_audio, pass == 1).await {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(e);
        }
        let size = tokio::fs::metadata(&tmp).await.map(|m| m.len());
        if size.is_ok_and(|s| s > 0 && s <= limit_bytes) {
            tokio::fs::rename(&tmp, &dst).await?;
            return Ok(dst);
        }
        let _ = tokio::fs::remove_file(&tmp).await;
    }
    Err(MediaError::CannotFit { limit_bytes })
}

async fn encode(
    cfg: &Config,
    src: &Path,
    tmp: &Path,
    vbps: u64,
    has_audio: bool,
    downscale: bool,
) -> Result<(), MediaError> {
    let mut a = args(["-hide_banner", "-nostdin", "-y", "-i"]);
    a.push(src.into());
    a.extend(args(["-c:v", "libx264", "-preset", "veryfast"]));
    if downscale {
        a.extend(args(["-vf", "scale=-2:'min(480,ih)'"]));
    }
    let (b, buf) = (vbps.to_string(), (vbps * 2).to_string());
    a.extend(args([
        "-b:v", &b, "-maxrate", &b, "-bufsize", &buf, "-pix_fmt", "yuv420p",
    ]));
    if has_audio {
        a.extend(args(["-c:a", "aac", "-b:a", "128k"]));
    } else {
        a.push("-an".into());
    }
    a.extend(args(["-movflags", "+faststart", "-f", "mp4"]));
    a.push(tmp.into());
    let (ok, _, err) = run(&cfg.ffmpeg_path, &a, ENCODE_TIMEOUT)
        .await
        .map_err(MediaError::Ffmpeg)?;
    if ok {
        Ok(())
    } else {
        Err(MediaError::Ffmpeg(tail(&err)))
    }
}

/// Convertit `src` en MP3 dans `dst` de façon atomique (fichier temporaire puis renommage).
///
/// Un MP3 partiel n'existe jamais sous le nom final : en cas d'échec le `.tmp` est supprimé.
pub async fn to_mp3_atomic(
    cfg: &Config,
    src: &Path,
    dst: &Path,
    kbps: u64,
) -> Result<PathBuf, MediaError> {
    let tmp = tmp_name(dst);
    let mut a = args(["-hide_banner", "-nostdin", "-y", "-i"]);
    a.push(src.into());
    a.extend(args(["-vn", "-c:a", "libmp3lame", "-b:a"]));
    a.push(format!("{kbps}k").into());
    a.extend(args(["-f", "mp3"]));
    a.push((&tmp).into());
    let res = match run(&cfg.ffmpeg_path, &a, ENCODE_TIMEOUT).await {
        Ok((true, _, _)) => match tokio::fs::metadata(&tmp).await {
            Ok(m) if m.len() > 0 => Ok(()),
            _ => Err(MediaError::Ffmpeg("sortie vide".into())),
        },
        Ok((false, _, err)) => Err(MediaError::Ffmpeg(tail(&err))),
        Err(e) => Err(MediaError::Ffmpeg(e)),
    };
    let res = match res {
        Ok(()) => tokio::fs::rename(&tmp, dst).await.map_err(MediaError::from),
        e => e,
    };
    match res {
        Ok(()) => Ok(dst.to_path_buf()),
        Err(e) => {
            let _ = tokio::fs::remove_file(&tmp).await;
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn cfg(ffmpeg: &str) -> Config {
        let probe_bin = ffmpeg.replace("ffmpeg", "ffprobe");
        let env: HashMap<String, String> = [
            ("BOT_TOKEN", "x"),
            ("FFMPEG_PATH", ffmpeg),
            ("FFPROBE_PATH", probe_bin.as_str()),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        Config::from_map(&env).unwrap()
    }

    /// Config avec le vrai ffmpeg, ou `None` (test ignoré) s'il est introuvable.
    fn real() -> Option<Config> {
        let bin = std::env::var("FFMPEG_PATH").unwrap_or_else(|_| "ffmpeg".into());
        let ok = std::process::Command::new(&bin)
            .arg("-version")
            .output()
            .is_ok_and(|o| o.status.success());
        if !ok {
            eprintln!("ffmpeg introuvable : test ignoré");
            return None;
        }
        Some(cfg(&bin))
    }

    fn gen(c: &Config, out: &Path, extra: &[&str]) {
        let st = std::process::Command::new(&c.ffmpeg_path)
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=size=640x360:rate=25:duration=5",
            ])
            .args(extra)
            .args(["-t", "5", "-pix_fmt", "yuv420p"])
            .arg(out)
            .status()
            .unwrap();
        assert!(st.success());
    }

    #[test]
    fn debit_cible_pur() {
        // 10 Mo sur 100 s : 10e6*0.95*8/100 = 760 000 b/s - 128 000 = 632 000
        assert_eq!(target_video_bps(10_000_000, 100.0, 128_000), Some(632_000));
        assert_eq!(target_video_bps(1_000_000, 3600.0, 128_000), None);
        assert_eq!(target_video_bps(10_000_000, 100.0, 0), Some(760_000));
    }

    #[test]
    fn parse_sondes() {
        let j = br#"{"streams":[{"codec_type":"video"},{"codec_type":"audio"}],"format":{"duration":"12.5"}}"#;
        let p = parse_probe(j, 42).unwrap();
        assert_eq!(
            p,
            Probe {
                duration: 12.5,
                has_audio: true,
                has_video: true,
                size: 42
            }
        );
        assert!(parse_probe(br#"{"streams":[],"format":{}}"#, 1).is_err());
        assert!(parse_probe(b"pas du json", 1).is_err());
        assert_eq!(
            parse_max_volume("x\n[Parsed_volumedetect_0] max_volume: -inf dB\n"),
            Some(f64::NEG_INFINITY)
        );
        assert_eq!(parse_max_volume("max_volume: -12.3 dB"), Some(-12.3));
        assert_eq!(parse_max_volume("rien"), None);
    }

    #[tokio::test]
    async fn mp3_echec_ffmpeg_introuvable_ne_laisse_rien() {
        let d = tempfile::tempdir().unwrap();
        let (src, dst) = (d.path().join("a.mp4"), d.path().join("a.mp3"));
        std::fs::write(&src, b"x").unwrap();
        std::fs::write(tmp_name(&dst), b"reste d'un crash").unwrap(); // .tmp périmé
        let r = to_mp3_atomic(&cfg("ffmpeg-inexistant-wf"), &src, &dst, 192).await;
        assert!(matches!(r, Err(MediaError::Ffmpeg(_))));
        assert!(!dst.exists(), "mp3 final ne doit pas exister");
        assert!(!tmp_name(&dst).exists(), ".tmp doit être supprimé");
    }

    #[tokio::test]
    async fn mp3_echec_ffmpeg_reel_source_corrompue() {
        let Some(c) = real() else { return };
        let d = tempfile::tempdir().unwrap();
        let (src, dst) = (d.path().join("junk.mp4"), d.path().join("junk.mp3"));
        std::fs::write(&src, b"ceci n'est pas une video").unwrap();
        assert!(to_mp3_atomic(&c, &src, &dst, 192).await.is_err());
        assert!(!dst.exists());
        assert!(!tmp_name(&dst).exists());
        let restes = std::fs::read_dir(d.path()).unwrap().count();
        assert_eq!(restes, 1, "seule la source reste");
    }

    #[tokio::test]
    async fn bout_en_bout_sonde_compression_mp3() {
        let Some(c) = real() else { return };
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("t.mp4");
        gen(
            &c,
            &src,
            &[
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=5",
                "-b:v",
                "3M",
                "-c:a",
                "aac",
            ],
        );
        let p = probe(&c, &src).await.unwrap();
        assert!(p.has_audio && p.has_video && (4.5..5.6).contains(&p.duration));
        assert_eq!(audio_issue(&c, &src).await.unwrap(), None);

        // déjà sous la limite : même chemin
        assert_eq!(compress_under(&c, &src, p.size + 1).await.unwrap(), src);
        // compression réelle
        let limit = p.size * 3 / 4;
        let before = std::fs::read(&src).unwrap();
        let out = compress_under(&c, &src, limit).await.unwrap();
        assert_ne!(out, src);
        assert!(std::fs::metadata(&out).unwrap().len() <= limit);
        assert_eq!(std::fs::read(&src).unwrap(), before, "source intacte");
        assert!(!tmp_name(&out).exists());
        // impossible
        assert!(matches!(
            compress_under(&c, &src, 2_000).await,
            Err(MediaError::CannotFit { .. })
        ));

        let mp3 = d.path().join("t.mp3");
        assert_eq!(to_mp3_atomic(&c, &src, &mp3, 192).await.unwrap(), mp3);
        assert!(std::fs::metadata(&mp3).unwrap().len() > 0);
        assert!(!tmp_name(&mp3).exists());
    }

    #[tokio::test]
    async fn garde_fou_audio() {
        let Some(c) = real() else { return };
        let d = tempfile::tempdir().unwrap();
        let (muet, sans) = (d.path().join("muet.mp4"), d.path().join("sans.mp4"));
        gen(
            &c,
            &muet,
            &[
                "-f",
                "lavfi",
                "-i",
                "anullsrc=r=44100:cl=stereo",
                "-c:a",
                "aac",
            ],
        );
        gen(&c, &sans, &[]);
        assert_eq!(
            audio_issue(&c, &muet).await.unwrap(),
            Some(AudioIssue::SilentTrack)
        );
        assert_eq!(
            audio_issue(&c, &sans).await.unwrap(),
            Some(AudioIssue::NoAudioTrack)
        );
    }
}
