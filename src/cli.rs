//! Interface en ligne de commande.

use std::io::{self, BufRead, IsTerminal, Write};
use std::sync::{Arc, OnceLock};

mod langues;
mod verifier;

pub use verifier::verifier;

use crate::bibliotheque::{self, Bibliotheque};
use crate::core::{self, Progression, Question, QuestionSet};
use crate::paths;

// ---------------------------------------------------------------------------
// Affichage
// ---------------------------------------------------------------------------
/// Codes couleur ANSI, désactivés automatiquement hors terminal.
#[rustfmt::skip]
mod c {
    use super::*;

    fn actif() -> bool {
        static ACTIF: OnceLock<bool> = OnceLock::new();
        *ACTIF.get_or_init(|| io::stdout().is_terminal())
    }

    fn colorer(code: &str, t: &str) -> String {
        if actif() {
            format!("\x1b[{code}m{t}\x1b[0m")
        } else {
            t.to_string()
        }
    }

    pub fn gras(t: &str) -> String { colorer("1", t) }
    pub fn faible(t: &str) -> String { colorer("2", t) }
    pub fn vert(t: &str) -> String { colorer("92", t) }
    pub fn rouge(t: &str) -> String { colorer("91", t) }
    pub fn jaune(t: &str) -> String { colorer("93", t) }
    pub fn bleu(t: &str) -> String { colorer("96", t) }
}

fn titre(texte: &str) {
    let barre = "=".repeat((texte.chars().count() + 4).max(40));
    println!("\n{}", c::bleu(&barre));
    println!("{}", c::bleu(&c::gras(&format!("  {texte}"))));
    println!("{}", c::bleu(&barre));
}

pub fn barre_progression(pct: f64, largeur: usize) -> String {
    let plein = ((pct / 100.0 * largeur as f64).round() as usize).min(largeur);
    format!(
        "[{}{}] {pct:5.1}%",
        "#".repeat(plein),
        "-".repeat(largeur - plein)
    )
}

/// Lit une ligne au clavier ; fin d'entrée (EOF) équivaut à « q ».
pub fn demander(invite: &str) -> String {
    print!("{invite}");
    let _ = io::stdout().flush();
    let mut ligne = String::new();
    match io::stdin().lock().read_line(&mut ligne) {
        Ok(0) | Err(_) => {
            println!();
            "q".to_string()
        }
        Ok(_) => ligne.trim().to_string(),
    }
}

fn lettre(i: usize) -> char {
    char::from(b'A' + i as u8)
}

fn lettres(indices: &[usize]) -> String {
    indices
        .iter()
        .map(|&i| lettre(i).to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

// ---------------------------------------------------------------------------
// Logique de quiz
// ---------------------------------------------------------------------------
/// Transforme 'a c', 'AC' ou 'a,c' en [0, 2]. Ignore les lettres invalides.
pub fn parser_selection(saisie: &str, n: usize) -> Vec<usize> {
    let mut indices: Vec<usize> = saisie
        .chars()
        .flat_map(char::to_uppercase)
        .filter(|ch| ch.is_alphabetic())
        .filter_map(|ch| (ch as u32).checked_sub(65).map(|i| i as usize))
        .filter(|&i| i < n)
        .collect();
    indices.sort_unstable();
    indices.dedup();
    indices
}

fn afficher_explication(q: &Question) {
    if !q.explication.is_empty() {
        println!("{}", c::faible(&format!("  ⓘ  {}", q.explication)));
    }
}

/// Affiche une question, recueille la réponse, donne le retour.
///
/// Retourne `Some(juste)`, ou `None` si l'utilisateur quitte.
fn poser_question(q: &Question, numero: usize, total: usize) -> Option<bool> {
    println!();
    if q.est_trou() {
        poser_trou(q, numero, total)
    } else if q.est_flashcard() {
        poser_flashcard(q, numero, total)
    } else {
        poser_choix(q, numero, total)
    }
}

/// QCM / QCR : sélection parmi des choix lettrés.
fn poser_choix(q: &Question, numero: usize, total: usize) -> Option<bool> {
    let etiquette = if q.est_qcr() {
        "QCR (plusieurs réponses possibles)"
    } else {
        "QCM (une seule réponse)"
    };
    println!(
        "{}",
        c::faible(&format!("Question {numero}/{total}  ·  {etiquette}"))
    );
    println!("{}", c::gras(&q.enonce));
    for (i, choix) in q.choix.iter().enumerate() {
        println!("   {}. {choix}", c::bleu(&lettre(i).to_string()));
    }

    let selection = loop {
        let saisie = demander("\nVotre réponse (lettres, 'q' pour quitter) : ");
        if saisie.eq_ignore_ascii_case("q") {
            return None;
        }
        let selection = parser_selection(&saisie, q.choix.len());
        if !selection.is_empty() {
            break selection;
        }
        println!(
            "{}",
            c::jaune("  Saisie non reconnue, réessayez (ex. : A, ou 'AC' pour un QCR).")
        );
    };

    let correcte = q.est_correcte(&selection);
    if correcte {
        println!("{}", c::vert("  ✓ Correct !"));
    } else {
        println!(
            "{}",
            c::rouge(&format!(
                "  ✗ Incorrect. Vous avez répondu {}.",
                lettres(&selection)
            ))
        );
        println!(
            "{}",
            c::vert(&format!("  Bonne(s) réponse(s) : {}", lettres(&q.reponses)))
        );
    }
    afficher_explication(q);
    Some(correcte)
}

/// Texte à trous (grammaire) : réponse libre, comparée avec souplesse (casse/accents).
fn poser_trou(q: &Question, numero: usize, total: usize) -> Option<bool> {
    println!(
        "{}",
        c::faible(&format!("Question {numero}/{total}  ·  Texte à trous"))
    );
    println!("{}", c::gras(&q.enonce));

    let saisie = demander("\nVotre réponse ('q' pour quitter) : ");
    if saisie.eq_ignore_ascii_case("q") {
        return None;
    }
    let correcte = q.est_correcte_texte(&saisie);
    if correcte {
        println!("{}", c::vert("  ✓ Correct !"));
    } else {
        println!(
            "{}",
            c::rouge(&format!("  ✗ Incorrect. Vous avez répondu « {saisie} »."))
        );
        println!(
            "{}",
            c::vert(&format!(
                "  Réponse attendue : {}",
                q.reponses_texte.join(" / ")
            ))
        );
    }
    afficher_explication(q);
    Some(correcte)
}

/// Flashcard (vocabulaire) : recto, verso révélé à la demande, auto-évaluation.
fn poser_flashcard(q: &Question, numero: usize, total: usize) -> Option<bool> {
    println!(
        "{}",
        c::faible(&format!("Question {numero}/{total}  ·  Flashcard"))
    );
    println!("{}", c::gras(&q.enonce));

    let saisie = demander("\nAppuyez sur Entrée pour révéler la réponse ('q' pour quitter) : ");
    if saisie.eq_ignore_ascii_case("q") {
        return None;
    }
    println!("{}", c::bleu(&format!("  → {}", q.verso)));
    afficher_explication(q);

    loop {
        let saisie = demander("\nAviez-vous trouvé ? (o/n, 'q' pour quitter) : ").to_lowercase();
        match saisie.as_str() {
            "q" => return None,
            "o" => return Some(true),
            "n" => return Some(false),
            _ => println!("{}", c::jaune("  Répondez par 'o' ou 'n'.")),
        }
    }
}

/// Déroule une session de quiz sur une liste de (set, indice de question).
/// Renvoie (score, nombre de questions répondues).
fn derouler(questions: &[(Arc<QuestionSet>, usize)], prog: &mut Progression) -> (usize, usize) {
    let (mut score, mut repondues) = (0, 0);
    for (i, (s, qi)) in questions.iter().enumerate() {
        let q = &s.questions[*qi];
        let Some(resultat) = poser_question(q, i + 1, questions.len()) else {
            println!(
                "{}",
                c::jaune("\nSession interrompue. Progression enregistrée.")
            );
            break;
        };
        prog.enregistrer_reponse(&s.id, &q.id, resultat);
        sauvegarder(prog); // sauvegarde après chaque réponse (robustesse)
        repondues += 1;
        score += usize::from(resultat);
    }
    (score, repondues)
}

fn sauvegarder(prog: &Progression) {
    if let Err(e) = prog.sauvegarder() {
        println!("{}", c::rouge(&format!("  ⚠ Sauvegarde impossible : {e}")));
    }
}

fn afficher_score(titre_txt: &str, score: usize, repondues: usize, avec_message: bool) {
    if repondues == 0 {
        return;
    }
    let pct = score as f64 / repondues as f64 * 100.0;
    titre(titre_txt);
    println!(
        "  Score : {}  ({pct:.0} %)",
        c::gras(&format!("{score}/{repondues}"))
    );
    println!("  {}", barre_progression(pct, 24));
    if avec_message {
        if pct >= 100.0 {
            println!("{}", c::vert("  Parfait, tout est juste ! ✓"));
        } else if pct >= 60.0 {
            println!(
                "{}",
                c::jaune("  Bon travail, continue de réviser les points faibles.")
            );
        } else {
            println!(
                "{}",
                c::rouge("  À retravailler — relance une révision intelligente bientôt.")
            );
        }
    }
}

fn lancer_session(qset: &Arc<QuestionSet>, indices: Vec<usize>, prog: &mut Progression) {
    if indices.is_empty() {
        println!(
            "{}",
            c::jaune("Aucune question à réviser pour le moment. Bravo !")
        );
        return;
    }
    titre(&format!("{}  ({} questions)", qset.titre, indices.len()));
    let questions: Vec<_> = indices.into_iter().map(|i| (Arc::clone(qset), i)).collect();
    let (score, repondues) = derouler(&questions, prog);
    afficher_score("Résultat de la session", score, repondues, true);
}

// ---------------------------------------------------------------------------
// Écrans / menus
// ---------------------------------------------------------------------------
fn charger_bibliotheque() -> Bibliotheque {
    let b = bibliotheque::charger(&paths::dossier_questions());
    for e in &b.erreurs {
        println!("{}", c::rouge(&format!("  ⚠ {e}")));
    }
    b
}

/// Lit un choix numéroté entre 1 et `n` ; `None` pour « retour » ou une saisie invalide.
fn lire_choix(n: usize) -> Option<usize> {
    let choix = demander("Choix : ");
    if matches!(choix.as_str(), "0" | "q" | "") {
        return None;
    }
    match choix.parse::<usize>() {
        Ok(k) if (1..=n).contains(&k) => Some(k - 1),
        _ => {
            println!("{}", c::jaune("Choix invalide."));
            None
        }
    }
}

/// Demande une catégorie (sous-dossier de questions/) si plusieurs existent.
fn choisir_categorie(sets: &[Arc<QuestionSet>]) -> Option<String> {
    let categories = core::categories(sets);
    if categories.len() <= 1 {
        return categories.into_iter().next();
    }
    println!("\nCatégories disponibles :");
    for (i, cat) in categories.iter().enumerate() {
        let n = sets.iter().filter(|s| &s.categorie == cat).count();
        println!(
            "  {}. {cat}  {}",
            c::bleu(&(i + 1).to_string()),
            c::faible(&format!("({n} sets)"))
        );
    }
    println!("  {}. Retour", c::bleu("0"));
    lire_choix(categories.len()).map(|i| categories[i].clone())
}

fn choisir_set(sets: &[Arc<QuestionSet>]) -> Option<Arc<QuestionSet>> {
    let categorie = choisir_categorie(sets)?;
    let sets_cat: Vec<_> = sets.iter().filter(|s| s.categorie == categorie).collect();

    println!("\nSets disponibles — {categorie} :");
    for (i, s) in sets_cat.iter().enumerate() {
        println!(
            "  {}. {}  {}",
            c::bleu(&(i + 1).to_string()),
            s.titre,
            c::faible(&format!("({} questions)", s.len()))
        );
    }
    println!("  {}. Retour", c::bleu("0"));
    lire_choix(sets_cat.len()).map(|i| Arc::clone(sets_cat[i]))
}

fn menu_session(sets: &[Arc<QuestionSet>], prog: &mut Progression) {
    let Some(qset) = choisir_set(sets) else {
        return;
    };
    println!("\nMode :");
    println!("  {}. Toutes les questions (ordre aléatoire)", c::bleu("1"));
    println!("  {}. Seulement les questions à réviser", c::bleu("2"));
    println!("  {}. Un nombre limité de questions", c::bleu("3"));
    let mode = demander("Choix : ");

    let mut indices: Vec<usize> = (0..qset.len()).collect();
    match mode.as_str() {
        "2" => {
            indices.retain(|&i| prog.est_a_reviser(&qset.id, &qset.questions[i].id));
            indices.sort_by_cached_key(|&i| prog.priorite(&qset.id, &qset.questions[i].id));
        }
        "3" => {
            fastrand::shuffle(&mut indices);
            let n = demander(&format!("Combien de questions (max {}) ? ", indices.len()));
            if let Ok(n) = n.parse::<usize>() {
                indices.truncate(n.max(1));
            }
        }
        _ => fastrand::shuffle(&mut indices),
    }
    lancer_session(&qset, indices, prog);
}

/// Rassemble toutes les questions à réviser, tous sets confondus.
fn menu_revision_intelligente(sets: &[Arc<QuestionSet>], prog: &mut Progression) {
    titre("Révision intelligente");
    let mut a_faire = prog.questions_a_reviser(sets);
    if a_faire.is_empty() {
        println!(
            "{}",
            c::vert("Rien à réviser pour l'instant — reviens plus tard. ✓")
        );
        return;
    }
    println!("{} question(s) à réviser au total.", a_faire.len());
    let n = demander("Combien veux-tu en faire maintenant ? (Entrée = toutes) : ");
    if let Ok(n) = n.parse::<usize>() {
        a_faire.truncate(n.max(1));
    }
    let (score, repondues) = derouler(&a_faire, prog);
    afficher_score("Résultat", score, repondues, false);
}

fn menu_statistiques(sets: &[Arc<QuestionSet>], prog: &Progression) {
    titre("Mes statistiques");
    if sets.is_empty() {
        println!("Aucun set chargé.");
        return;
    }
    for s in sets {
        let r = prog.resume_set(s);
        println!("\n{}", c::gras(&s.titre));
        println!(
            "  Maîtrise globale  {}",
            barre_progression(r.pct_maitrise, 24)
        );
        println!("  Questions vues    {}/{}", r.rencontrees, r.total);
        println!("  À réviser         {}", r.a_reviser);
        if r.rencontrees > 0 {
            println!("  Précision moyenne {:.0} %", r.precision);
        }
        let detail = r
            .repartition
            .iter()
            .enumerate()
            .map(|(b, n)| format!("B{b}:{n}"))
            .collect::<Vec<_>>()
            .join("  ");
        println!(
            "{}",
            c::faible(&format!("  Répartition par boîte de Leitner : {detail}"))
        );
    }
    println!(
        "{}",
        c::faible("\n  (Boîte 0 = à revoir souvent … Boîte 5 = bien maîtrisé)")
    );
}

fn menu_reinitialiser(sets: &[Arc<QuestionSet>], prog: &mut Progression) {
    let Some(qset) = choisir_set(sets) else {
        return;
    };
    let conf = demander(&format!(
        "Effacer toute la progression de « {} » ? (o/N) : ",
        qset.titre
    ));
    if conf.eq_ignore_ascii_case("o") {
        prog.reinitialiser_set(&qset.id);
        sauvegarder(prog);
        println!("{}", c::vert("Progression réinitialisée."));
    } else {
        println!("Annulé.");
    }
}

// ---------------------------------------------------------------------------
// Boucle principale
// ---------------------------------------------------------------------------
pub fn main() {
    let dossier = paths::dossier_questions();
    let _ = std::fs::create_dir_all(&dossier);
    let mut prog = Progression::new(paths::fichier_progression());

    titre("Révision QCM / QCR — Préparation aux partiels");
    loop {
        let bib = charger_bibliotheque();
        let sets = &bib.sets;
        if sets.is_empty() && bib.langues.is_empty() {
            println!(
                "{}",
                c::jaune(&format!(
                    "\nAucun set trouvé dans {}.\nAjoute un fichier .json (voir le README pour le format).",
                    dossier.display()
                ))
            );
        }

        println!("\nMenu principal :");
        println!("  {}. Démarrer une session sur un set", c::bleu("1"));
        println!(
            "  {}. Révision intelligente (questions à réviser)",
            c::bleu("2")
        );
        println!(
            "  {}. Langues (vocabulaire, conjugaison, grammaire)",
            c::bleu("3")
        );
        println!("  {}. Voir mes statistiques", c::bleu("4"));
        println!("  {}. Réinitialiser la progression d'un set", c::bleu("5"));
        println!("  {}. Quitter", c::bleu("6"));
        let choix = demander("Choix : ");

        match choix.as_str() {
            "1" if !sets.is_empty() => menu_session(sets, &mut prog),
            "2" if !sets.is_empty() => menu_revision_intelligente(sets, &mut prog),
            "3" => langues::menu(&bib.langues, &mut prog),
            "4" => menu_statistiques(sets, &prog),
            "5" if !sets.is_empty() => menu_reinitialiser(sets, &mut prog),
            "6" | "q" => {
                println!("À bientôt, et bon courage pour tes partiels ! 👋");
                break;
            }
            _ => println!("{}", c::jaune("Choix invalide.")),
        }
    }
}

/// Menu de choix de l'interface, affiché dans le terminal.
pub fn demander_interface() -> &'static str {
    let barre = "=".repeat(42);
    println!("\n{barre}");
    println!("  Révision QCM — Choisir l'interface");
    println!("{barre}");
    println!("  1. Ligne de commande  (CLI)");
    println!("  2. Interface graphique (GUI)");
    println!("{barre}");
    loop {
        match demander("Votre choix (1/2) : ").as_str() {
            "1" | "q" => return "cli",
            "2" => return "gui",
            _ => println!("{}", c::jaune("Entrez 1 ou 2.")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection() {
        assert_eq!(parser_selection("a c", 4), vec![0, 2]);
        assert_eq!(parser_selection("AC", 4), vec![0, 2]);
        assert_eq!(parser_selection("a,c,z", 4), vec![0, 2]);
        assert_eq!(parser_selection("ca", 4), vec![0, 2]);
        assert!(parser_selection("!?", 4).is_empty());
    }

    #[test]
    fn barre() {
        assert_eq!(barre_progression(50.0, 4), "[##--]  50.0%");
    }
}
