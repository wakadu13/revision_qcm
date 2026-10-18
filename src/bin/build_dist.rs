//! Génère `dist/` : exécutable optimisé + questions/ + manifest.json + version web.
//!
//! Usage :
//!     cargo run --release --bin build_dist              # console (CLI + GUI)
//!     cargo run --release --bin build_dist -- --windowed # GUI uniquement, sans console (Windows)
//!
//! Les éléments générés sont remplacés à chaque build ; `dist/progression.json`
//! (la progression de l'utilisateur) n'est jamais touché.

#[allow(dead_code)]
#[path = "../core.rs"]
mod core;

use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode};

fn copier_dossier(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entree in fs::read_dir(src)? {
        let entree = entree?;
        let cible = dst.join(entree.file_name());
        if entree.file_type()?.is_dir() {
            copier_dossier(&entree.path(), &cible)?;
        } else {
            fs::copy(entree.path(), cible)?;
        }
    }
    Ok(())
}

/// Génère dist/manifest.json : liste des sets (chemins relatifs à questions/,
/// ex. "maths/algebre.json"), pour que revision_qcm.html puisse les charger
/// automatiquement via fetch() — nécessite un serveur HTTP local (pas file://,
/// bloqué par les navigateurs pour des raisons de sécurité).
fn generer_manifest(dossier_questions: &Path, dist: &Path) -> std::io::Result<usize> {
    let manifest: Vec<String> = core::lister_sets(dossier_questions)
        .iter()
        .filter_map(|f| f.strip_prefix(dossier_questions).ok())
        .map(|r| {
            r.components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/")
        })
        .collect();
    let json = serde_json::to_string_pretty(&manifest).map_err(std::io::Error::other)?;
    fs::write(dist.join("manifest.json"), json)?;
    Ok(manifest.len())
}

fn construire(windowed: bool) -> Result<(), String> {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dist = racine.join("dist");
    let barre = "=".repeat(52);
    let mode = if windowed {
        "windowed (GUI uniquement)"
    } else {
        "console (CLI + GUI)"
    };
    println!("\n{barre}\n  Build : {mode}\n{barre}\n");

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut cmd = Command::new(cargo);
    cmd.current_dir(racine)
        .args(["build", "--release", "--bin", "revision_qcm"]);
    if windowed {
        cmd.args(["--features", "windowed"]);
    }
    let statut = cmd
        .status()
        .map_err(|e| format!("cargo introuvable : {e}"))?;
    if !statut.success() {
        return Err("Erreur lors du build.".into());
    }

    let nom_exe = format!("revision_qcm{}", std::env::consts::EXE_SUFFIX);
    let exe_src = racine.join("target").join("release").join(&nom_exe);
    if !exe_src.exists() {
        return Err(format!(
            "Build terminé mais {} introuvable.",
            exe_src.display()
        ));
    }
    fs::create_dir_all(&dist).map_err(|e| e.to_string())?;
    let exe = dist.join(&nom_exe);
    fs::copy(&exe_src, &exe).map_err(|e| e.to_string())?;

    // Copier le dossier questions/ à côté de l'exécutable
    let q_src = racine.join("questions");
    let q_dst = dist.join("questions");
    if q_src.exists() {
        if q_dst.exists() {
            fs::remove_dir_all(&q_dst).map_err(|e| e.to_string())?;
        }
        copier_dossier(&q_src, &q_dst).map_err(|e| e.to_string())?;
        println!("Dossier questions/ copié dans dist/");
        let n = generer_manifest(&q_dst, &dist).map_err(|e| e.to_string())?;
        println!("manifest.json généré ({n} sets)");
    }

    // README, documentation et version web autonome (web/ = code source, pas un artefact)
    for (src, nom) in [
        (racine.join("README.md"), "README.md"),
        (racine.join("architecture.txt"), "architecture.txt"),
        (
            racine.join("web").join("revision_qcm.html"),
            "revision_qcm.html",
        ),
    ] {
        if src.exists() {
            fs::copy(&src, dist.join(nom)).map_err(|e| e.to_string())?;
            println!("{nom} copié dans dist/");
        }
    }

    let taille = fs::metadata(&exe).map(|m| m.len()).unwrap_or(0) as f64 / 1_048_576.0;
    println!(
        "\n{barre}\n  Exécutable généré : {} ({taille:.1} Mo)\n  Lancez-le depuis le dossier dist/\n{barre}\n",
        exe.display()
    );
    Ok(())
}

fn main() -> ExitCode {
    let windowed = std::env::args().any(|a| a == "--windowed");
    match construire(windowed) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("\n{e}");
            ExitCode::FAILURE
        }
    }
}
