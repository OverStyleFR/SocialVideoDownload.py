//! Journalisation tracing et battement de cœur pour le HEALTHCHECK Docker.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::EnvFilter;

/// Âge maximal du battement de cœur pour être déclaré sain.
pub const HEARTBEAT_MAX_AGE: Duration = Duration::from_secs(120);
/// Période d'écriture du battement de cœur.
pub const HEARTBEAT_PERIOD: Duration = Duration::from_secs(30);

/// Initialise tracing : LOG_FORMAT=json ou texte (défaut), filtre RUST_LOG (défaut info), sans ANSI.
/// Ne masque rien : préférer [`init_redacted`] dès que le jeton est connu.
pub fn init() {
    install(std::io::stdout);
}

/// Comme [`init`], mais toute ligne écrite passe par [`redact`] (jeton et motif `/bot<id>:<secret>/`).
pub fn init_redacted(token: &str) {
    init_with_writer(token, std::io::stdout);
}

/// Comme [`init_redacted`] vers un writer au choix (tests : tampon en mémoire). Sans effet si un
/// subscriber global existe déjà.
pub fn init_with_writer<M>(token: &str, w: M)
where
    M: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    install(RedactMake {
        token: token.to_owned(),
        inner: w,
    });
}

type SetLevel = Box<dyn Fn(EnvFilter) -> Result<(), String> + Send + Sync>;
/// Poignée de rechargement du filtre du subscriber installé par ce module.
static RELOAD: std::sync::OnceLock<SetLevel> = std::sync::OnceLock::new();

fn install(w: impl for<'a> MakeWriter<'a> + Send + Sync + 'static) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_writer(w);
    if std::env::var("LOG_FORMAT").is_ok_and(|v| v.eq_ignore_ascii_case("json")) {
        let b = builder.json().with_filter_reloading();
        let h = b.reload_handle();
        if b.try_init().is_ok() {
            let _ = RELOAD.set(Box::new(move |f| h.reload(f).map_err(|e| e.to_string())));
        }
    } else {
        let b = builder.with_filter_reloading();
        let h = b.reload_handle();
        if b.try_init().is_ok() {
            let _ = RELOAD.set(Box::new(move |f| h.reload(f).map_err(|e| e.to_string())));
        }
    }
}

/// Change le niveau des journaux (réglage `sys.log_level`) sans redémarrer le subscriber.
/// RUST_LOG, s'il est défini, garde la priorité. Sans effet si ce module n'a rien installé.
pub fn set_level(level: &str) -> Result<(), String> {
    if std::env::var_os("RUST_LOG").is_some() {
        return Ok(());
    }
    let f = EnvFilter::try_new(level).map_err(|e| e.to_string())?;
    RELOAD.get().map_or(Ok(()), |r| r(f))
}

/// Remplace le jeton exact puis tout motif `/bot<chiffres>:<secret>` (URL de l'API Bot) par `***`.
pub fn redact(text: &str, token: &str) -> String {
    let text = if token.is_empty() {
        text.to_owned()
    } else {
        text.replace(token, "***")
    };
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(i) = rest.find("/bot") {
        let (head, tail) = rest.split_at(i + 4);
        out.push_str(head);
        let digits = tail.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 && tail.as_bytes().get(digits) == Some(&b':') {
            let secret = tail[digits + 1..]
                .find(|c: char| c == '/' || c == '"' || c == '\\' || c.is_whitespace())
                .unwrap_or(tail.len() - digits - 1);
            out.push_str("***");
            rest = &tail[digits + 1 + secret..];
        } else {
            rest = tail;
        }
    }
    out.push_str(rest);
    out
}

struct RedactMake<M> {
    token: String,
    inner: M,
}

impl<'a, M: MakeWriter<'a>> MakeWriter<'a> for RedactMake<M> {
    type Writer = RedactWriter<M::Writer>;
    fn make_writer(&'a self) -> Self::Writer {
        RedactWriter {
            token: self.token.clone(),
            inner: self.inner.make_writer(),
        }
    }
}

/// Writer qui masque le jeton dans chaque écriture (tracing écrit une ligne entière par appel).
struct RedactWriter<W> {
    token: String,
    inner: W,
}

impl<W: Write> Write for RedactWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let clean = redact(&String::from_utf8_lossy(buf), &self.token);
        self.inner.write_all(clean.as_bytes())?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// Lance la tâche qui réécrit périodiquement le fichier battement de cœur.
pub fn spawn_heartbeat(path: PathBuf) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(HEARTBEAT_PERIOD);
        loop {
            tick.tick().await;
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            if let Err(e) = tokio::fs::write(&path, now.to_string()).await {
                tracing::warn!(error = %e, "écriture du heartbeat impossible");
            }
        }
    })
}

/// Vrai si le fichier battement de cœur a été modifié il y a moins de `max_age`.
pub fn heartbeat_fresh(path: &Path, max_age: Duration) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age < max_age)
}
