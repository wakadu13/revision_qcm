//! Module Langues en ligne de commande.

use std::sync::Arc;

use super::{c, demander, lettre, lettres, lire_choix, parser_selection, sauvegarder, titre};
use crate::core::Progression;
use crate::fsrs::Note;
use crate::langues::{self, ExoSession, Forme, Genre, Mode, SetLangue, Verdict};

const TAILLE_SESSION: usize = 20;

/// Réponse attendue, différences avec la saisie surlignées.
fn diff_colore(saisie: &str, attendu: &str) -> String {
    let mut res = String::new();
    let mut tampon = String::new();
    let mut etat_ok = true;
    let vider = |res: &mut String, tampon: &mut String, ok: bool| {
        if !tampon.is_empty() {
            res.push_str(&if ok {
                c::vert(tampon)
            } else {
                c::jaune(&c::gras(tampon))
            });
            tampon.clear();
        }
    };
    for (ch, ok) in langues::diff(saisie, attendu) {
        if ok != etat_ok {
            vider(&mut res, &mut tampon, etat_ok);
            etat_ok = ok;
        }
        tampon.push(ch);
    }
    vider(&mut res, &mut tampon, etat_ok);
    res
}

fn afficher_details(e: &ExoSession) {
    for d in &e.exercice().details {
        println!("{}", c::faible(&format!("  ⓘ  {d}")));
    }
}

fn afficher_tableau(t: &langues::Tableau) {
    println!("{}", c::faible(&format!("  ┌ {}", t.titre)));
    let largeur = t
        .lignes
        .iter()
        .map(|(p, _)| p.chars().count())
        .max()
        .unwrap_or(0);
    for (k, (p, f)) in t.lignes.iter().enumerate() {
        let ligne = format!("  │ {p:<largeur$}  {f}");
        println!(
            "{}",
            if k == t.surligne {
                c::bleu(&c::gras(&ligne))
            } else {
                c::faible(&ligne)
            }
        );
    }
}

fn afficher_verdict(v: Verdict) {
    let m = v.message();
    println!(
        "{}",
        match v {
            Verdict::Juste => c::vert(&format!("  {m}")),
            Verdict::Faux => c::rouge(&format!("  {m}")),
            _ => c::jaune(&format!("  {m}")),
        }
    );
}

/// Pose un exercice. Renvoie la note, ou `None` si l'utilisateur quitte.
fn poser(e: &ExoSession, numero: usize, total: usize, prog: &Progression) -> Option<Note> {
    let exo = e.exercice();
    let set = &e.set;
    let mut entete = format!("Exercice {numero}/{total}  ·  {}", exo.etiquette);
    if e.nouveau {
        entete.push_str("  ·  nouveau");
    }
    if e.tentative > 0 {
        entete.push_str("  ·  nouvelle tentative");
    }
    println!("\n{}", c::faible(&entete));

    match &exo.forme {
        Forme::Carte { recto, verso } => {
            println!("{}", c::gras(recto));
            if demander("\nEntrée pour retourner la carte ('q' pour quitter) : ")
                .eq_ignore_ascii_case("q")
            {
                return None;
            }
            println!("{}", c::bleu(&format!("  → {verso}")));
            afficher_details(e);
            let notes = [Note::ARevoir, Note::Difficile, Note::Bien, Note::Facile];
            let libelles = ["À revoir", "Difficile", "Bien", "Facile"];
            let choix = notes
                .iter()
                .zip(libelles)
                .enumerate()
                .map(|(i, (&n, l))| {
                    let d = langues::texte_delai(langues::prevision(prog, &set.id, &exo.id, n));
                    format!("{} {l} ({d})", c::bleu(&(i + 1).to_string()))
                })
                .collect::<Vec<_>>()
                .join("  ·  ");
            println!("\n  {choix}");
            loop {
                match demander("Ta note (1-4, 'q' pour quitter) : ").as_str() {
                    "q" | "Q" => return None,
                    "1" => return Some(Note::ARevoir),
                    "2" => return Some(Note::Difficile),
                    "3" => return Some(Note::Bien),
                    "4" => return Some(Note::Facile),
                    _ => println!("{}", c::jaune("  Réponds par 1, 2, 3 ou 4.")),
                }
            }
        }
        Forme::Saisie {
            consigne,
            invite,
            reponses,
            tolerant,
            prerempli,
            tableau,
        } => {
            println!("{}", c::gras(consigne));
            let invite = if !invite.is_empty() {
                format!("\n{} → ", c::bleu(invite))
            } else if !prerempli.is_empty() {
                "\nPhrase corrigée ('q' pour quitter) : ".to_string()
            } else {
                "\nTa réponse ('q' pour quitter) : ".to_string()
            };
            let saisie = demander(&invite);
            if saisie == "q" || saisie == "Q" {
                return None;
            }
            let (v, attendu) = langues::noter_saisie(&saisie, reponses, *tolerant, &set.particules);
            afficher_verdict(v);
            if v != Verdict::Juste {
                println!("  Réponse attendue : {}", diff_colore(&saisie, &attendu));
                let autres: Vec<&String> = reponses.iter().filter(|r| **r != attendu).collect();
                if !autres.is_empty() {
                    let autres: Vec<&str> = autres.iter().map(|s| s.as_str()).collect();
                    println!(
                        "{}",
                        c::faible(&format!("  Aussi accepté : {}", autres.join(" / ")))
                    );
                }
            }
            if let Some(t) = tableau {
                afficher_tableau(t);
            }
            afficher_details(e);
            Some(v.note())
        }
        Forme::Choix {
            enonce,
            choix,
            reponses,
            multiple,
        } => {
            println!("{}", c::gras(enonce));
            for (i, ch) in choix.iter().enumerate() {
                println!("   {}. {ch}", c::bleu(&lettre(i).to_string()));
            }
            let indication = if *multiple {
                "plusieurs lettres possibles"
            } else {
                "une lettre"
            };
            let selection = loop {
                let s = demander(&format!("\nTa réponse ({indication}, 'q' pour quitter) : "));
                if s.eq_ignore_ascii_case("q") {
                    return None;
                }
                let sel = parser_selection(&s, choix.len());
                if !sel.is_empty() {
                    break sel;
                }
                println!("{}", c::jaune("  Saisie non reconnue."));
            };
            let juste = selection == *reponses;
            afficher_verdict(if juste { Verdict::Juste } else { Verdict::Faux });
            if !juste {
                println!(
                    "{}",
                    c::vert(&format!("  Bonne(s) réponse(s) : {}", lettres(reponses)))
                );
            }
            afficher_details(e);
            Some(if juste { Note::Bien } else { Note::ARevoir })
        }
        Forme::Ordre {
            consigne,
            mots,
            reponses,
        } => {
            println!("{}", c::gras(consigne));
            let tuiles: Vec<&String> = e.melange.iter().map(|&i| &mots[i]).collect();
            let affichage = tuiles
                .iter()
                .enumerate()
                .map(|(i, m)| format!("{} {m}", c::bleu(&(i + 1).to_string())))
                .collect::<Vec<_>>()
                .join("   ");
            println!("   {affichage}");
            let s = demander("\nTa phrase (ou les numéros dans l'ordre, 'q' pour quitter) : ");
            if s.eq_ignore_ascii_case("q") {
                return None;
            }
            let numeros: Option<Vec<usize>> = s
                .split(|c: char| c.is_whitespace() || c == ',')
                .filter(|t| !t.is_empty())
                .map(|t| {
                    t.parse::<usize>()
                        .ok()
                        .filter(|&k| (1..=tuiles.len()).contains(&k))
                })
                .collect();
            let phrase = match numeros {
                Some(n) if !n.is_empty() => n
                    .iter()
                    .map(|&k| tuiles[k - 1].as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
                _ => s.clone(),
            };
            let (v, attendu) = langues::noter_ordre(&phrase, reponses);
            afficher_verdict(v);
            if v != Verdict::Juste {
                println!("  Ta phrase : {phrase}");
                println!("  Réponse   : {}", c::vert(&attendu));
            }
            afficher_details(e);
            Some(v.note())
        }
    }
}

fn lancer(sets: &[Arc<SetLangue>], prog: &mut Progression, mode: Mode) {
    let n = demander(&format!(
        "Combien d'exercices ? (Entrée = {TAILLE_SESSION}) : "
    ));
    let n = n
        .parse::<usize>()
        .ok()
        .filter(|&n| n > 0)
        .unwrap_or(TAILLE_SESSION);
    let mut file = langues::construire_session(sets, prog, mode, n);
    if file.is_empty() {
        let msg = if mode == Mode::PointsFaibles {
            "Aucun point faible pour l'instant. ✓"
        } else {
            "Rien à réviser ni à découvrir pour l'instant — reviens plus tard. ✓"
        };
        println!("{}", c::vert(msg));
        return;
    }
    titre(&format!("{}  ({} exercices)", mode.nom(), file.len()));
    let (mut score, mut premiers) = (0, 0);
    let mut i = 0;
    while i < file.len() {
        let total = file.len();
        let Some(note) = poser(&file[i], i + 1, total, prog) else {
            println!(
                "{}",
                c::jaune("\nSession interrompue. Progression enregistrée.")
            );
            break;
        };
        let e = &file[i];
        if e.tentative == 0 {
            langues::enregistrer(prog, &e.set.id, &e.exercice().id, note);
            sauvegarder(prog);
            premiers += 1;
            score += usize::from(note.reussie());
        }
        if note == Note::ARevoir {
            langues::reprogrammer(&mut file, i);
        }
        i += 1;
    }
    super::afficher_score("Résultat de la session", score, premiers, true);
}

fn statistiques(langue: &str, sets: &[Arc<SetLangue>], prog: &Progression) {
    titre(&format!("Statistiques — {langue}"));
    let r = langues::resume(sets, prog);
    for (g, rg) in &r.genres {
        if rg.total == 0 {
            continue;
        }
        println!("\n{}", c::gras(g.nom()));
        let vus = rg.total - rg.nouveaux;
        println!("  Appris           {vus}/{}", rg.total);
        println!("  Maîtrisés (≥ 3 semaines) {}", rg.maitrises);
        println!("  À revoir         {}", rg.a_revoir);
        if let Some(ret) = rg.retention {
            println!(
                "  Rétention estimée {}",
                super::barre_progression(ret * 100.0, 24)
            );
        }
    }
    println!("\nÀ revoir dans les 7 prochains jours : {}", r.sept_jours);
    let fichiers: Vec<&str> = sets.iter().map(|s| s.titre.as_str()).collect();
    println!(
        "{}",
        c::faible(&format!("Contenu : {}", fichiers.join(" · ")))
    );
    if !r.difficiles.is_empty() {
        println!("\n{}", c::gras("Éléments les plus souvent oubliés :"));
        for (txt, n) in &r.difficiles {
            println!("  {}  {}", c::rouge(&format!("×{n}")), txt);
        }
    }
}

/// Menu du module Langues.
pub fn menu(tous: &[Arc<SetLangue>], prog: &mut Progression) {
    let noms = langues::langues(tous);
    if noms.is_empty() {
        println!(
            "{}",
            c::jaune(
                "Aucun fichier de langue dans questions/ (voir le README, « Module Langues »)."
            )
        );
        return;
    }
    let langue = if noms.len() == 1 {
        noms[0].clone()
    } else {
        println!("\nLangues disponibles :");
        for (i, l) in noms.iter().enumerate() {
            println!("  {}. {l}", c::bleu(&(i + 1).to_string()));
        }
        println!("  {}. Retour", c::bleu("0"));
        match lire_choix(noms.len()) {
            Some(i) => noms[i].clone(),
            None => return,
        }
    };
    let sets = langues::sets_de(tous, &langue);

    loop {
        let r = langues::resume(&sets, prog);
        titre(&format!("Langues — {langue}"));
        println!(
            "{}",
            c::faible(&format!(
                "  {} à revoir · {} nouveaux disponibles",
                r.a_revoir, r.nouveaux_disponibles
            ))
        );
        println!(
            "  {}. Session du jour (recommandé : tout mélangé)",
            c::bleu("1")
        );
        println!("  {}. Vocabulaire", c::bleu("2"));
        println!("  {}. Conjugaison", c::bleu("3"));
        println!("  {}. Grammaire", c::bleu("4"));
        println!("  {}. Points faibles", c::bleu("5"));
        println!("  {}. Statistiques", c::bleu("6"));
        println!(
            "  {}. Réinitialiser la progression de cette langue",
            c::bleu("7")
        );
        println!("  {}. Retour", c::bleu("0"));
        match lire_choix(7) {
            Some(0) => lancer(&sets, prog, Mode::Quotidien),
            Some(1) => lancer(&sets, prog, Mode::Genre(Genre::Vocabulaire)),
            Some(2) => lancer(&sets, prog, Mode::Genre(Genre::Conjugaison)),
            Some(3) => lancer(&sets, prog, Mode::Genre(Genre::Grammaire)),
            Some(4) => lancer(&sets, prog, Mode::PointsFaibles),
            Some(5) => statistiques(&langue, &sets, prog),
            Some(_) => {
                let conf = demander(&format!(
                    "Effacer toute la progression en {langue} ? (o/N) : "
                ));
                if conf.eq_ignore_ascii_case("o") {
                    for s in &sets {
                        prog.reinitialiser_set(&s.id);
                    }
                    sauvegarder(prog);
                    println!("{}", c::vert("Progression réinitialisée."));
                }
            }
            None => return,
        }
    }
}
