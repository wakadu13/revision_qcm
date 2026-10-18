//! Emplacement des données : `questions/` et `progression.json`.

use std::path::{Path, PathBuf};

/// Racine des données :
/// - en développement (`cargo run`, exécutable dans `target/`) : la racine du projet ;
/// - sinon : le dossier de l'exécutable (ex. `dist/`).
pub fn racine() -> PathBuf {
    let projet = Path::new(env!("CARGO_MANIFEST_DIR"));
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.canonicalize().ok())
        .and_then(|p| p.parent().map(Path::to_path_buf));
    match exe_dir {
        Some(dir) if dir.starts_with(projet.join("target")) => projet.to_path_buf(),
        Some(dir) => dir,
        None => projet.to_path_buf(),
    }
}

pub fn dossier_questions() -> PathBuf {
    racine().join("questions")
}

pub fn fichier_progression() -> PathBuf {
    racine().join("progression.json")
}
