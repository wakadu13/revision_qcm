//! `revision_qcm --verifier <fichier.json | dossier> …` : contrôle des fichiers de
//! questions (par exemple générés par une IA) avant de les utiliser.

use std::collections::HashMap;
use std::path::Path;

use super::c;
use crate::core::{self, QuestionSet, TypeQuestion};
use crate::langues::{self, Forme, Genre, SetLangue};
use crate::math;

/// Avertissements de rendu LaTeX d'un texte, préfixés par `ou`.
fn latex(ou: &str, texte: &str, acc: &mut Vec<String>) {
    for pb in math::diagnostiquer(texte) {
        acc.push(format!("{ou} : {pb}"));
    }
}

fn verifier_set(s: &QuestionSet) -> (String, Vec<String>) {
    let mut avert = Vec::new();
    let mut ids: HashMap<&str, usize> = HashMap::new();
    let mut compte = [0usize; 4];
    let mut toutes_justes = Vec::new();
    for q in &s.questions {
        *ids.entry(q.id.as_str()).or_default() += 1;
        let ou = format!("question {}", q.id);
        compte[q.type_q as usize] += 1;
        for t in std::iter::once(&q.enonce)
            .chain(&q.choix)
            .chain([&q.verso, &q.explication])
        {
            latex(&ou, t, &mut avert);
        }
        if q.type_q == TypeQuestion::Trou && !q.enonce.contains("___") {
            avert.push(format!("{ou} : pas de « ___ » dans la phrase à trous"));
        }
        if q.type_q == TypeQuestion::Qcr && q.reponses.len() == q.choix.len() {
            toutes_justes.push(q.id.as_str());
        }
    }
    if !toutes_justes.is_empty() {
        avert.push(format!(
            "QCR dont toutes les propositions sont justes (à éviter si c'est fréquent : la réponse se devine) : {}",
            toutes_justes.join(", ")
        ));
    }
    let mut doublons: Vec<&str> = ids
        .iter()
        .filter(|(_, n)| **n > 1)
        .map(|(id, _)| *id)
        .collect();
    doublons.sort_unstable();
    for id in doublons {
        avert.push(format!(
            "identifiant « {id} » utilisé plusieurs fois (progression partagée entre ces questions)"
        ));
    }
    let resume = format!(
        "set « {} » : {} questions (QCM {}, QCR {}, TROU {}, FLASHCARD {}) — catégorie « {} »",
        s.titre,
        s.len(),
        compte[0],
        compte[1],
        compte[2],
        compte[3],
        s.categorie
    );
    (resume, avert)
}

fn verifier_langue(s: &SetLangue) -> (String, Vec<String>) {
    let mut avert = Vec::new();
    let mut traductions: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut entrees: HashMap<Genre, std::collections::HashSet<&str>> = HashMap::new();
    for e in &s.exercices {
        entrees.entry(e.genre).or_default().insert(&e.entree);
        let ou = format!("{} {}", e.genre.nom().to_lowercase(), e.entree);
        match &e.forme {
            Forme::Carte { recto, verso } => {
                latex(&ou, recto, &mut avert);
                latex(&ou, verso, &mut avert);
                if e.genre == Genre::Vocabulaire {
                    traductions.entry(verso).or_default().push(&e.entree);
                    let id_ctx = format!("{}#ctx", e.entree);
                    if !s.exercices.iter().any(|x| x.id == id_ctx) {
                        avert.push(format!(
                            "{ou} : pas d'exemple avec le mot entre [crochets] (pas d'exercice en contexte)"
                        ));
                    }
                }
            }
            Forme::Saisie { consigne, .. } => latex(&ou, consigne, &mut avert),
            Forme::Choix { enonce, choix, .. } => {
                latex(&ou, enonce, &mut avert);
                for ch in choix {
                    latex(&ou, ch, &mut avert);
                }
            }
            Forme::Ordre { mots, .. } if mots.len() > 9 => avert.push(format!(
                "{ou} : {} mots à ordonner (9 au plus conseillé, raccourcis clavier limités à 9)",
                mots.len()
            )),
            Forme::Ordre { .. } => {}
        }
    }
    let mut partagees: Vec<_> = traductions.iter().filter(|(_, v)| v.len() > 1).collect();
    partagees.sort();
    for (t, mots) in partagees {
        avert.push(format!(
            "traduction « {t} » commune à {} : l'exercice de production sera ambigu",
            mots.join(", ")
        ));
    }
    let n = |g| entrees.get(&g).map_or(0, |e| e.len());
    let resume = format!(
        "fichier de langue « {} » ({}) : {} mots, {} tableaux de conjugaison, {} exercices de grammaire → {} exercices",
        s.titre,
        s.langue,
        n(Genre::Vocabulaire),
        n(Genre::Conjugaison),
        n(Genre::Grammaire),
        s.exercices.len()
    );
    (resume, avert)
}

/// Vérifie un fichier. Renvoie `false` en cas d'erreur bloquante.
fn verifier_fichier(chemin: &Path, racine: Option<&Path>) -> bool {
    let affiche = chemin.display();
    let resultat = core::lire_fichier(chemin, racine).and_then(|f| {
        if langues::est_fichier_langue(&f.data) {
            langues::set_depuis_json(f).map(|s| verifier_langue(&s))
        } else {
            core::set_depuis_json(f).map(|s| verifier_set(&s))
        }
    });
    match resultat {
        Ok((resume, avert)) => {
            println!("{} {affiche}", c::vert("✓"));
            println!("    {resume}");
            for a in &avert {
                println!("    {}", c::jaune(&format!("⚠ {a}")));
            }
            true
        }
        Err(e) => {
            println!("{} {affiche}", c::rouge("✗"));
            println!("    {}", c::rouge(&e));
            false
        }
    }
}

/// Point d'entrée de `--verifier`. Renvoie le code de sortie du programme.
pub fn verifier(chemins: &[String]) -> i32 {
    if chemins.is_empty() {
        println!("Usage : revision_qcm --verifier <fichier.json | dossier> …");
        return 2;
    }
    let mut ok = true;
    let mut n = 0;
    for c in chemins {
        let p = Path::new(c);
        if p.is_dir() {
            for f in core::lister_sets(p) {
                ok &= verifier_fichier(&f, Some(p));
                n += 1;
            }
        } else {
            ok &= verifier_fichier(p, None);
            n += 1;
        }
    }
    println!();
    if ok {
        println!("{}", c::vert(&format!("{n} fichier(s) valide(s).")));
        0
    } else {
        println!(
            "{}",
            c::rouge(
                "Fichier(s) invalide(s) : ils seraient ignorés par l'application. Recolle l'erreur à l'IA pour qu'elle corrige."
            )
        );
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Chaque prompt de prompts_ia/ contient un exemple JSON qui doit être
    /// accepté tel quel par l'application, sans avertissement.
    #[test]
    fn exemples_des_prompts_ia_valides() {
        let dossier = Path::new(env!("CARGO_MANIFEST_DIR")).join("prompts_ia");
        let mut fichiers: Vec<_> = std::fs::read_dir(&dossier)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.extension().is_some_and(|e| e == "txt") && !p.ends_with("00_LISEZMOI.txt")
            })
            .collect();
        fichiers.sort();
        assert_eq!(fichiers.len(), 10);
        for f in fichiers {
            let texte = std::fs::read_to_string(&f).unwrap();
            let debut = texte
                .find("```json\n")
                .unwrap_or_else(|| panic!("{f:?} : pas d'exemple"))
                + 8;
            let fin = debut + texte[debut..].find("```").unwrap();
            let data: serde_json::Value =
                serde_json::from_str(&texte[debut..fin]).unwrap_or_else(|e| panic!("{f:?} : {e}"));
            let fichier = core::FichierJson {
                nom: "exemple.json".into(),
                stem: "exemple".into(),
                categorie: "test".into(),
                chemin: f.clone(),
                data,
            };
            let (_, avert) = if langues::est_fichier_langue(&fichier.data) {
                verifier_langue(
                    &langues::set_depuis_json(fichier).unwrap_or_else(|e| panic!("{f:?} : {e}")),
                )
            } else {
                verifier_set(
                    &core::set_depuis_json(fichier).unwrap_or_else(|e| panic!("{f:?} : {e}")),
                )
            };
            assert!(avert.is_empty(), "{f:?} : {avert:?}");
        }
    }
}
