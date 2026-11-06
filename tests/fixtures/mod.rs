//! Faux outils des tests (yt-dlp, ffmpeg, ffprobe) posés dans le dossier temporaire d'un test.

use std::path::{Path, PathBuf};

/// Copie le faux outil `name` de tests/fixtures (`.sh`, ou `.cmd` sous Windows) dans `dir`.
pub fn tool(dir: &Path, name: &str) -> PathBuf {
    let ext = if cfg!(windows) { "cmd" } else { "sh" };
    let src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(format!("{name}.{ext}"));
    let dst = dir.join(format!("{name}.{ext}"));
    let text = std::fs::read_to_string(src).unwrap().replace("\r\n", "\n");
    write_exe(&dst, &text);
    dst
}

/// Écrit un script exécutable.
///
/// Sous Unix, c'est un `sh` enfant qui l'écrit : si ce processus tenait le fichier ouvert en écriture,
/// un test parallèle qui lance un programme au même moment hériterait du descripteur jusqu'à son exec,
/// et lancer le script échouerait en ETXTBSY (« Text file busy »).
pub fn write_exe(path: &Path, text: &str) {
    #[cfg(unix)]
    {
        use std::io::Write;
        let mut sh = std::process::Command::new("sh")
            .args(["-c", "cat > \"$0\" && chmod 755 \"$0\""])
            .arg(path)
            .stdin(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        sh.stdin.take().unwrap().write_all(text.as_bytes()).unwrap();
        assert!(
            sh.wait().unwrap().success(),
            "écriture de {}",
            path.display()
        );
    }
    #[cfg(windows)]
    std::fs::write(path, text).unwrap();
}
