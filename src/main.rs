//! Application de révision QCM / QCR / TROU / FLASHCARD pour partiels.
//!
//! Lancement :
//!     cargo run --release            (ou directement l'exécutable revision_qcm)
//!     cargo run --release -- --cli   (force la ligne de commande)
//!     cargo run --release -- --gui   (force l'interface graphique)
//!
//! Les sets de questions sont des fichiers JSON dans le dossier `questions/`.
//! La progression est sauvegardée dans `progression.json`.

// Build « --windowed » : pas de fenêtre console sous Windows (GUI uniquement).
#![cfg_attr(all(windows, feature = "windowed"), windows_subsystem = "windows")]

mod bibliotheque;
mod cli;
mod core;
mod fsrs;
mod gui;
mod langues;
mod math;
mod paths;

use std::io::IsTerminal;

/// Retourne "gui" ou "cli" selon l'argument passé ou le choix de l'utilisateur.
fn choisir_interface() -> &'static str {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--gui") {
        return "gui";
    }
    if args.iter().any(|a| a == "--cli") {
        return "cli";
    }
    // Pas de terminal interactif (ex. : .exe sans console) : on lance la GUI directement.
    if !std::io::stdout().is_terminal() {
        return "gui";
    }
    cli::demander_interface()
}

fn main() {
    if choisir_interface() == "gui" {
        if let Err(e) = gui::lancer_gui() {
            eprintln!("Impossible de lancer l'interface graphique : {e}");
            std::process::exit(1);
        }
    } else {
        cli::main();
    }
}
