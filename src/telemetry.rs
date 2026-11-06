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

/// Initialise tracing : LOG_FORMAT=json ou texte au format du bot Python (défaut), filtre RUST_LOG
/// (défaut : `debug` pour le bot comme le logger Python, `info` pour les bibliothèques), couleurs
/// en console sauf NO_COLOR.
/// Ne masque rien : préférer [`init_redacted`] dès que le jeton est connu.
pub fn init() {
    install(std::io::stdout, None::<DailyFile>);
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
    install(redacted(token, w), None::<DailyFile>);
}

/// Comme le bot Python : console (en couleurs) ET fichier `logs_dir/AAAA-MM-JJ.log`, qui change
/// chaque jour à minuit (rotation automatique). Dossier impossible à créer : console seule.
pub fn init_with_files(token: &str, logs_dir: &Path) {
    match DailyFile::new(logs_dir) {
        Ok(f) => install(redacted(token, std::io::stdout), Some(redacted(token, f))),
        Err(e) => {
            install(
                redacted(token, std::io::stdout),
                None::<RedactMake<DailyFile>>,
            );
            tracing::warn!(error = %e, dir = %logs_dir.display(), "dossier de journaux inutilisable, console seule");
        }
    }
}

fn redacted<M>(token: &str, inner: M) -> RedactMake<M> {
    RedactMake {
        token: token.to_owned(),
        inner,
    }
}

type SetLevel = Box<dyn Fn(EnvFilter) -> Result<(), String> + Send + Sync>;
/// Poignée de rechargement du filtre du subscriber installé par ce module.
static RELOAD: std::sync::OnceLock<SetLevel> = std::sync::OnceLock::new();

type BoxLayer = Box<dyn tracing_subscriber::Layer<tracing_subscriber::Registry> + Send + Sync>;

fn layer<M>(w: M, ansi: bool) -> BoxLayer
where
    M: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    use tracing_subscriber::Layer;
    if std::env::var("LOG_FORMAT").is_ok_and(|v| v.eq_ignore_ascii_case("json")) {
        tracing_subscriber::fmt::layer()
            .json()
            .with_writer(w)
            .boxed()
    } else {
        tracing_subscriber::fmt::layer()
            .event_format(PyFormat)
            .with_ansi(ansi)
            .with_writer(w)
            .boxed()
    }
}

fn install<C, F>(console: C, file: Option<F>)
where
    C: for<'a> MakeWriter<'a> + Send + Sync + 'static,
    F: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    use tracing_subscriber::prelude::*;
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(directive("debug")));
    let (filter, h) = tracing_subscriber::reload::Layer::new(filter);
    // Couleurs en console comme l'original (convention NO_COLOR respectée), jamais dans le fichier.
    let mut layers = vec![layer(console, std::env::var_os("NO_COLOR").is_none())];
    layers.extend(file.map(|f| layer(f, false)));
    if tracing_subscriber::registry()
        .with(layers)
        .with(filter)
        .try_init()
        .is_ok()
    {
        let _ = RELOAD.set(Box::new(move |f| h.reload(f).map_err(|e| e.to_string())));
    }
}

/// Ligne au format du bot Python : `AAAA-MM-JJ HH:MM:SS,mmm - NIVEAU - message`, niveau en
/// couleur (et préfixe `[CATÉGORIE]` dans sa couleur) quand la sortie accepte l'ANSI.
struct PyFormat;

const RESET: &str = "\x1b[0m";
/// Couleurs des catégories, reprises de utils/logger.py.
const CATEGORIES: [(&str, &str); 6] = [
    ("[DOWNLOAD]", "\x1b[34m"),
    ("[MUSIC]", "\x1b[35m"),
    ("[AUTO_DOWNLOAD]", "\x1b[32m"),
    ("[UPLOAD]", "\x1b[33m"),
    ("[CURL UPLOAD PROGRESS]", "\x1b[31m"),
    ("[CURL UPLOAD]", "\x1b[36m"),
];

impl<S, N> tracing_subscriber::fmt::FormatEvent<S, N> for PyFormat
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    N: for<'a> tracing_subscriber::fmt::FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &tracing_subscriber::fmt::FmtContext<'_, S, N>,
        mut w: tracing_subscriber::fmt::format::Writer<'_>,
        ev: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        use tracing::Level;
        let (name, color) = match *ev.metadata().level() {
            Level::ERROR => ("ERROR", "\x1b[31m"),
            Level::WARN => ("WARNING", "\x1b[33m"),
            Level::INFO => ("INFO", "\x1b[32m"),
            _ => ("DEBUG", "\x1b[34m"),
        };
        let mut msg = String::new();
        ctx.field_format()
            .format_fields(tracing_subscriber::fmt::format::Writer::new(&mut msg), ev)?;
        write!(w, "{} - {name} - ", asctime(SystemTime::now()))?;
        if !w.has_ansi_escapes() {
            return writeln!(w, "{msg}");
        }
        match CATEGORIES.iter().find(|(c, _)| msg.starts_with(c)) {
            Some((cat, cc)) => {
                let rest = msg[cat.len()..].trim_start();
                writeln!(w, "{cc}{cat}{RESET} {color}{rest}{RESET}")
            }
            None => writeln!(w, "{color}{msg}{RESET}"),
        }
    }
}

/// (date `AAAA-MM-JJ`, heure `HH:MM:SS,mmm`) en UTC.
/// shortcut: UTC et non l'heure locale de Python ; identique dans un conteneur sans TZ.
fn civil(t: SystemTime) -> (String, String) {
    let d = t.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
    let (days, secs) = ((d.as_secs() / 86_400) as i64, d.as_secs() % 86_400);
    // Algorithme « days from civil » inverse (H. Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (
        format!("{year:04}-{month:02}-{day:02}"),
        format!(
            "{:02}:{:02}:{:02},{:03}",
            secs / 3600,
            secs / 60 % 60,
            secs % 60,
            d.subsec_millis()
        ),
    )
}

fn asctime(t: SystemTime) -> String {
    let (d, h) = civil(t);
    format!("{d} {h}")
}

/// Fichier de journal du jour (`AAAA-MM-JJ.log`, ajout) ; passe au fichier suivant à minuit.
pub struct DailyFile {
    dir: PathBuf,
    cur: std::sync::Mutex<Option<(String, std::fs::File)>>,
}

impl DailyFile {
    /// Crée le dossier si besoin.
    pub fn new(dir: &Path) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        Ok(Self {
            dir: dir.to_path_buf(),
            cur: std::sync::Mutex::new(None),
        })
    }

    fn write_on(&self, day: String, buf: &[u8]) -> std::io::Result<()> {
        let mut cur = self.cur.lock().unwrap_or_else(|e| e.into_inner());
        if cur.as_ref().is_none_or(|(d, _)| *d != day) {
            let f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.dir.join(format!("{day}.log")))?;
            *cur = Some((day, f));
        }
        match cur.as_mut() {
            Some((_, f)) => f.write_all(buf),
            None => Ok(()),
        }
    }
}

/// Écrivain d'une ligne vers [`DailyFile`].
pub struct DailyWriter<'a>(&'a DailyFile);

impl Write for DailyWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write_on(civil(SystemTime::now()).0, buf)?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for DailyFile {
    type Writer = DailyWriter<'a>;
    fn make_writer(&'a self) -> Self::Writer {
        DailyWriter(self)
    }
}

/// Change le niveau des journaux (réglage `sys.log_level`) sans redémarrer le subscriber.
/// RUST_LOG, s'il est défini, garde la priorité. Sans effet si ce module n'a rien installé.
pub fn set_level(level: &str) -> Result<(), String> {
    if std::env::var_os("RUST_LOG").is_some() {
        return Ok(());
    }
    let f = EnvFilter::try_new(directive(level)).map_err(|e| e.to_string())?;
    RELOAD.get().map_or(Ok(()), |r| r(f))
}

/// Filtre d'un niveau : `debug` = débogage du bot seul (logger Python à DEBUG), les bibliothèques
/// (hyper, rustls…) restant à `info`.
fn directive(level: &str) -> &str {
    if level == "debug" {
        "info,socialvideodownload=debug"
    } else {
        level
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_et_heure_au_format_python() {
        let t = SystemTime::UNIX_EPOCH + Duration::from_millis(1_700_000_000_123);
        assert_eq!(asctime(t), "2023-11-14 22:13:20,123");
        assert_eq!(civil(SystemTime::UNIX_EPOCH).0, "1970-01-01");
        let leap = SystemTime::UNIX_EPOCH + Duration::from_secs(951_782_400); // 29/02/2000
        assert_eq!(civil(leap).0, "2000-02-29");
    }

    #[test]
    fn debug_vise_le_bot_seul() {
        assert_eq!(directive("debug"), "info,socialvideodownload=debug");
        assert_eq!(directive("warn"), "warn");
        assert!(EnvFilter::try_new(directive("debug")).is_ok());
    }

    #[test]
    fn couleurs_en_console_jamais_dans_le_fichier() {
        #[derive(Clone, Default)]
        struct Buf(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
        impl Write for Buf {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        impl<'a> MakeWriter<'a> for Buf {
            type Writer = Buf;
            fn make_writer(&'a self) -> Buf {
                self.clone()
            }
        }
        let (console, file) = (Buf::default(), Buf::default());
        // Seul test de la lib à installer le subscriber global (ERROR : passe tout RUST_LOG).
        install(console.clone(), Some(file.clone()));
        tracing::error!("[UPLOAD] essai couleurs");
        let text = |b: &Buf| String::from_utf8(b.0.lock().unwrap().clone()).unwrap();
        let f = text(&file);
        assert!(f.contains(" - ERROR - [UPLOAD] essai couleurs"), "{f:?}");
        assert!(!f.contains('\x1b'), "ANSI dans le fichier : {f:?}");
        if std::env::var_os("NO_COLOR").is_none() {
            let c = text(&console);
            assert!(c.contains("\x1b[33m[UPLOAD]\x1b[0m"), "{c:?}");
        }
    }

    #[test]
    fn un_fichier_par_jour_en_ajout() {
        let d = tempfile::tempdir().unwrap();
        let f = DailyFile::new(&d.path().join("logs")).unwrap();
        f.write_on("2026-11-05".into(), b"a\n").unwrap();
        f.write_on("2026-11-05".into(), b"b\n").unwrap();
        f.write_on("2026-11-06".into(), b"c\n").unwrap();
        let read = |n: &str| std::fs::read_to_string(d.path().join("logs").join(n)).unwrap();
        assert_eq!(read("2026-11-05.log"), "a\nb\n");
        assert_eq!(read("2026-11-06.log"), "c\n");
    }
}
