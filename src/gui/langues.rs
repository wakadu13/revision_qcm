//! Écrans du module Langues (vocabulaire, conjugaison, grammaire).

use eframe::egui::text::{LayoutJob, TextFormat};

use super::*;
use crate::fsrs::Note;
use crate::langues::{self as lg, ExoSession, Forme, Genre, Verdict};

const TAILLE_SESSION: usize = 20;

/// Session d'exercices de langue en cours.
pub(super) struct QuizLangue {
    langue: String,
    file: Vec<ExoSession>,
    index: usize,
    score: usize,
    /// Exercices faits au premier essai (dénominateur du score).
    premiers: usize,
    saisie: String,
    selection: Vec<usize>,
    /// Remise en ordre : indices des mots placés, dans l'ordre choisi.
    places: Vec<usize>,
    revele: bool,
    verdict: Option<Verdict>,
    /// Réponse de référence affichée à la correction.
    attendu: String,
    note: Option<Note>,
    enregistre: bool,
    focus: bool,
}

impl QuizLangue {
    fn new(langue: String, file: Vec<ExoSession>) -> Self {
        let mut q = Self {
            langue,
            file,
            index: 0,
            score: 0,
            premiers: 0,
            saisie: String::new(),
            selection: Vec::new(),
            places: Vec::new(),
            revele: false,
            verdict: None,
            attendu: String::new(),
            note: None,
            enregistre: false,
            focus: true,
        };
        q.preparer();
        q
    }

    /// Réinitialise l'état pour l'exercice courant.
    fn preparer(&mut self) {
        self.saisie = match &self.file[self.index].exercice().forme {
            Forme::Saisie { prerempli, .. } => prerempli.clone(),
            _ => String::new(),
        };
        self.selection.clear();
        self.places.clear();
        self.revele = false;
        self.verdict = None;
        self.attendu.clear();
        self.note = None;
        self.enregistre = false;
        self.focus = true;
    }
}

#[derive(PartialEq)]
enum Act {
    Rien,
    Basculer(usize),
    Valider,
    Reveler,
    Noter(Note),
    Facile,
    Placer(usize),
    Retirer(usize),
    RetirerDernier,
    Suivante,
    Quitter,
}

/// Texte de la réponse attendue, caractères différents de la saisie surlignés.
fn job_diff(saisie: &str, attendu: &str) -> LayoutJob {
    let mut job = LayoutJob::default();
    for (c, ok) in lg::diff(saisie, attendu) {
        let format = TextFormat {
            font_id: if ok {
                FontId::proportional(F_LARGE)
            } else {
                gras(F_LARGE)
            },
            color: if ok { GREEN } else { YELLOW },
            underline: if ok {
                Stroke::NONE
            } else {
                Stroke::new(2.0, YELLOW)
            },
            ..Default::default()
        };
        job.append(&c.to_string(), 0.0, format);
    }
    job
}

/// Tuile de mot (remise en ordre), avec son numéro de raccourci.
fn tuile(ui: &mut Ui, numero: Option<usize>, mot: &str, fond: Color32) -> egui::Response {
    let mut job = LayoutJob::default();
    if let Some(n) = numero {
        job.append(
            &format!("{n} "),
            0.0,
            TextFormat {
                font_id: FontId::proportional(F_SMALL),
                color: MUTED,
                ..Default::default()
            },
        );
    }
    job.append(
        mot,
        0.0,
        TextFormat {
            font_id: FontId::proportional(F_LARGE),
            color: FG,
            ..Default::default()
        },
    );
    ui.add(
        egui::Button::new(job)
            .fill(fond)
            .corner_radius(4)
            .min_size(vec2(0.0, 38.0)),
    )
    .on_hover_cursor(CursorIcon::PointingHand)
}

/// Insère `texte` à la position du curseur du champ `id` (à la fin à défaut).
fn inserer_au_curseur(ctx: &egui::Context, id: Option<egui::Id>, saisie: &mut String, texte: &str) {
    use egui::text::{CCursor, CCursorRange};
    let etat = id.and_then(|id| egui::TextEdit::load_state(ctx, id).map(|e| (id, e)));
    let n = saisie.chars().count();
    let pos = etat
        .as_ref()
        .and_then(|(_, e)| e.cursor.char_range())
        .map_or(n, |r| usize::from(r.primary.index).min(n));
    let octet = saisie
        .char_indices()
        .nth(pos)
        .map_or(saisie.len(), |(i, _)| i);
    saisie.insert_str(octet, texte);
    if let Some((id, mut e)) = etat {
        let apres = CCursor::new(pos + texte.chars().count());
        e.cursor.set_char_range(Some(CCursorRange::one(apres)));
        e.store(ctx, id);
    }
}

fn badge(ui: &mut Ui, texte: &str, couleur: Color32) {
    Frame::NONE
        .fill(couleur)
        .corner_radius(3)
        .inner_margin(Margin::symmetric(6, 1))
        .show(ui, |ui| {
            ui.label(RichText::new(texte).size(F_SMALL - 2.0).color(BG));
        });
}

impl RevisionGui {
    // -----------------------------------------------------------------------
    // Navigation
    // -----------------------------------------------------------------------
    pub(super) fn aller_langues(&mut self) {
        self.charger_sets();
        let noms = lg::langues(&self.langues);
        match noms.len() {
            0 => self.info(
                "Aucune langue",
                "Aucun fichier de langue dans questions/.\nVoir le README, section « Module Langues ».",
            ),
            1 => self.aller_langue(noms[0].clone()),
            _ => self.ecran = Ecran::Langues { sel: 0 },
        }
    }

    pub(super) fn aller_langue(&mut self, langue: String) {
        self.ecran = Ecran::LangueMenu { langue, sel: 0 };
    }

    fn retour_langues(&mut self) {
        if lg::langues(&self.langues).len() > 1 {
            self.ecran = Ecran::Langues { sel: 0 };
        } else {
            self.aller_menu();
        }
    }

    pub(super) fn lancer_session_langue(&mut self, langue: String, mode: Mode, n: usize) {
        let sets = lg::sets_de(&self.langues, &langue);
        let file = lg::construire_session(&sets, &self.prog, mode, n);
        if file.is_empty() {
            self.info("Rien à faire", "Aucun exercice disponible pour l'instant.");
            return;
        }
        self.ecran = Ecran::QuizLangue(Box::new(QuizLangue::new(langue, file)));
    }

    fn choisir_mode(&mut self, langue: String, mode: Mode) {
        let sets = lg::sets_de(&self.langues, &langue);
        let dispo = lg::construire_session(&sets, &self.prog, mode, usize::MAX).len();
        if dispo == 0 {
            let msg = if mode == Mode::PointsFaibles {
                "Aucun point faible pour l'instant. ✓"
            } else {
                "Rien à réviser ni à découvrir pour l'instant — reviens plus tard. ✓"
            };
            self.info("Rien à faire", msg);
            return;
        }
        self.dialogue = Some(Dialogue::Nombre {
            max: dispo,
            saisie: dispo.min(TAILLE_SESSION).to_string(),
            action: ActionNombre::Langue(langue, mode),
            init: true,
        });
    }

    // -----------------------------------------------------------------------
    // Menus
    // -----------------------------------------------------------------------
    pub(super) fn ecran_langues(&mut self, ui: &mut Ui, actif: bool) {
        let Ecran::Langues { sel } = &mut self.ecran else {
            return;
        };
        let noms = lg::langues(&self.langues);
        let items: Vec<String> = noms
            .iter()
            .map(|l| {
                let r = lg::resume(&lg::sets_de(&self.langues, l), &self.prog);
                format!(
                    "{l}  ({} à revoir · {} nouveaux)",
                    r.a_revoir, r.nouveaux_disponibles
                )
            })
            .collect();
        Self::pied(ui, "↑↓ Naviguer   Entrée Sélectionner   Échap Retour");
        let mut nav = Nav::Rien;
        centre().show(ui, |ui| {
            Self::entete(ui, "Langues");
            nav = Self::liste_nav(ui, &items, sel, actif);
        });
        match nav {
            Nav::Choix(i) => self.aller_langue(noms[i].clone()),
            Nav::Retour => self.aller_menu(),
            Nav::Rien => {}
        }
    }

    pub(super) fn ecran_langue_menu(&mut self, ui: &mut Ui, actif: bool) {
        let Ecran::LangueMenu { langue, sel } = &mut self.ecran else {
            return;
        };
        let langue = langue.clone();
        let r = lg::resume(&lg::sets_de(&self.langues, &langue), &self.prog);
        let a_revoir = |g: Genre| {
            r.genres
                .iter()
                .find(|(x, _)| *x == g)
                .map_or(0, |(_, rg)| rg.a_revoir)
        };
        let items = vec![
            "Session du jour — tout mélangé (recommandé)".to_string(),
            format!("Vocabulaire  ({} à revoir)", a_revoir(Genre::Vocabulaire)),
            format!("Conjugaison  ({} à revoir)", a_revoir(Genre::Conjugaison)),
            format!("Grammaire  ({} à revoir)", a_revoir(Genre::Grammaire)),
            "Points faibles".to_string(),
            "Statistiques".to_string(),
            "Réinitialiser la progression de cette langue".to_string(),
        ];
        Self::pied(ui, "↑↓ Naviguer   Entrée Sélectionner   Échap Retour");
        let mut nav = Nav::Rien;
        centre().show(ui, |ui| {
            Self::entete(ui, &format!("Langues — {langue}"));
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new(format!(
                        "{} à revoir aujourd'hui · {} nouveaux à découvrir · {} dans les 7 prochains jours",
                        r.a_revoir, r.nouveaux_disponibles, r.sept_jours
                    ))
                    .size(F_SMALL)
                    .color(MUTED),
                );
            });
            nav = Self::liste_nav(ui, &items, sel, actif);
        });
        match nav {
            Nav::Choix(0) => self.choisir_mode(langue, Mode::Quotidien),
            Nav::Choix(1) => self.choisir_mode(langue, Mode::Genre(Genre::Vocabulaire)),
            Nav::Choix(2) => self.choisir_mode(langue, Mode::Genre(Genre::Conjugaison)),
            Nav::Choix(3) => self.choisir_mode(langue, Mode::Genre(Genre::Grammaire)),
            Nav::Choix(4) => self.choisir_mode(langue, Mode::PointsFaibles),
            Nav::Choix(5) => self.ecran = Ecran::StatsLangue { langue },
            Nav::Choix(_) => {
                self.dialogue = Some(Dialogue::Confirmer {
                    titre: "Confirmer la réinitialisation".into(),
                    message: format!(
                        "Effacer toute la progression en {langue} ?\n\nCette action est irréversible."
                    ),
                    action: ActionConfirmee::ReinitialiserLangue(langue),
                });
            }
            Nav::Retour => self.retour_langues(),
            Nav::Rien => {}
        }
    }

    pub(super) fn ecran_stats_langue(&mut self, ui: &mut Ui, actif: bool) {
        let Ecran::StatsLangue { langue } = &self.ecran else {
            return;
        };
        let langue = langue.clone();
        let sets = lg::sets_de(&self.langues, &langue);
        let r = lg::resume(&sets, &self.prog);
        let fichiers = sets
            .iter()
            .map(|s| s.titre.as_str())
            .collect::<Vec<_>>()
            .join(" · ");
        let mut retour = actif && ui.input_mut(|i| i.consume_key(Default::default(), Key::Escape));
        centre().show(ui, |ui| {
            Self::entete(ui, &format!("Statistiques — {langue}"));
            Frame::NONE.inner_margin(Margin { left: 20, right: 20, top: 8, bottom: 0 }).show(ui, |ui| {
                if petit_bouton(ui, "← Retour  (Échap)").clicked() {
                    retour = true;
                }
            });
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                Frame::NONE.inner_margin(Margin::symmetric(20, 6)).show(ui, |ui| {
                    for (g, rg) in r.genres.iter().filter(|(_, rg)| rg.total > 0) {
                        Frame::NONE.fill(BG2).inner_margin(Margin::symmetric(18, 10)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(RichText::new(g.nom()).font(gras(17.0)).color(ACCENT));
                            let appris = rg.total - rg.nouveaux;
                            let ligne = |ui: &mut Ui, nom: &str, frac: f32, texte: String, couleur: Color32| {
                                ui.horizontal(|ui| {
                                    ui.allocate_ui_with_layout(
                                        vec2(170.0, 20.0),
                                        egui::Layout::left_to_right(Align::Center),
                                        |ui| ui.label(RichText::new(nom).size(F_MED).color(FG)),
                                    );
                                    barre(ui, 240.0, 10.0, frac, couleur);
                                    ui.add_space(8.0);
                                    ui.label(RichText::new(texte).size(F_MED).color(couleur));
                                });
                            };
                            ligne(ui, "Exercices appris", appris as f32 / rg.total as f32, format!("{appris}/{}", rg.total), ACCENT);
                            ligne(
                                ui,
                                "Maîtrisés",
                                rg.maitrises as f32 / rg.total as f32,
                                format!("{}/{}", rg.maitrises, rg.total),
                                GREEN,
                            );
                            if let Some(ret) = rg.retention {
                                let c = if ret >= 0.85 { GREEN } else if ret >= 0.7 { YELLOW } else { RED };
                                ligne(ui, "Rétention estimée", ret as f32, format!("{:.0} %", ret * 100.0), c);
                            }
                            ui.label(
                                RichText::new(format!("À revoir maintenant : {}", rg.a_revoir)).size(F_SMALL).color(MUTED),
                            );
                        });
                        ui.add_space(10.0);
                    }
                    ui.label(
                        RichText::new(format!("À revoir dans les 7 prochains jours : {}", r.sept_jours))
                            .size(F_MED)
                            .color(FG),
                    );
                    ui.label(RichText::new(format!("Contenu : {fichiers}")).size(F_SMALL).color(MUTED));
                    ui.label(
                        RichText::new("« Maîtrisé » : souvenir stable au moins 3 semaines (algorithme FSRS).")
                            .size(F_SMALL)
                            .color(MUTED),
                    );
                    if !r.difficiles.is_empty() {
                        ui.add_space(12.0);
                        ui.label(RichText::new("Éléments les plus souvent oubliés").font(gras(F_MED)).color(RED));
                        for (txt, n) in &r.difficiles {
                            ui.label(RichText::new(format!("×{n}   {txt}")).size(F_MED).color(FG));
                        }
                    }
                });
            });
        });
        if retour {
            self.aller_langue(langue);
        }
    }

    // -----------------------------------------------------------------------
    // Exercices
    // -----------------------------------------------------------------------
    pub(super) fn ecran_quiz_langue(&mut self, ui: &mut Ui, actif: bool) {
        let Ecran::QuizLangue(quiz) = &mut self.ecran else {
            return;
        };
        let e = quiz.file[quiz.index].clone();
        let exo = e.exercice();
        let repondu = quiz.verdict.is_some();
        let n_mots = match &exo.forme {
            Forme::Ordre { mots, .. } => mots.len(),
            _ => 0,
        };

        // --- Clavier ---
        let mut act = Act::Rien;
        if actif {
            ui.input_mut(|i| {
                let entree = i.consume_key(Default::default(), Key::Enter);
                let espace = i.key_pressed(Key::Space);
                let echap = i.consume_key(Default::default(), Key::Escape);
                let texte: Vec<char> = i
                    .events
                    .iter()
                    .filter_map(|ev| match ev {
                        egui::Event::Text(t) => t.chars().next(),
                        _ => None,
                    })
                    .filter(|c| c.is_alphanumeric())
                    .flat_map(char::to_uppercase)
                    .collect();
                let lettre_q = texte.contains(&'Q');
                if repondu {
                    if entree || espace {
                        act = Act::Suivante;
                    } else if texte.contains(&'F') {
                        act = Act::Facile;
                    } else if lettre_q || echap {
                        act = Act::Quitter;
                    }
                    return;
                }
                match &exo.forme {
                    Forme::Carte { .. } => {
                        if !quiz.revele && (espace || entree) {
                            act = Act::Reveler;
                        } else if quiz.revele {
                            act = match texte.first() {
                                Some('1') => Act::Noter(Note::ARevoir),
                                Some('2') => Act::Noter(Note::Difficile),
                                Some('3') => Act::Noter(Note::Bien),
                                Some('4') => Act::Noter(Note::Facile),
                                _ if espace || entree => Act::Noter(Note::Bien),
                                _ => Act::Rien,
                            };
                        }
                        if act == Act::Rien && (lettre_q || echap) {
                            act = Act::Quitter;
                        }
                    }
                    Forme::Saisie { .. } => {
                        if echap {
                            act = Act::Quitter;
                        } else if entree {
                            act = Act::Valider;
                        }
                    }
                    Forme::Choix { choix, .. } => {
                        if entree || espace {
                            act = Act::Valider;
                        } else if lettre_q || echap {
                            act = Act::Quitter;
                        } else if let Some(&l) = texte.first() {
                            let idx = (l as u32).wrapping_sub(65) as usize;
                            if idx < choix.len() {
                                act = Act::Basculer(idx);
                            }
                        }
                    }
                    Forme::Ordre { .. } => {
                        if entree {
                            act = Act::Valider;
                        } else if i.consume_key(Default::default(), Key::Backspace) {
                            act = Act::RetirerDernier;
                        } else if lettre_q || echap {
                            act = Act::Quitter;
                        } else if let Some(d) = texte.first().and_then(|c| c.to_digit(10)) {
                            let pos = d as usize;
                            if (1..=n_mots).contains(&pos) {
                                act = Act::Placer(pos - 1);
                            }
                        }
                    }
                }
            });
        }

        // --- Barre du haut ---
        let total = quiz.file.len();
        let num = quiz.index + 1;
        egui::Panel::top("progression_langue")
            .frame(
                Frame::NONE
                    .fill(BG2)
                    .inner_margin(Margin::symmetric(20, 10)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let pct = quiz.index as f32 / total as f32;
                    ui.label(
                        RichText::new(format!("Exercice {num}/{total}"))
                            .size(F_MED)
                            .color(ACCENT),
                    );
                    ui.add_space(10.0);
                    barre(ui, 200.0, 10.0, pct, ACCENT);
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!(
                                "Score : {}/{}     {}",
                                quiz.score, quiz.premiers, quiz.langue
                            ))
                            .size(F_SMALL)
                            .color(MUTED),
                        );
                    });
                });
            });

        let aide = match (&exo.forme, repondu) {
            (_, true) if quiz.verdict == Some(Verdict::Juste) => {
                "Entrée Suivant   F C'était trop facile   Q Quitter"
            }
            (_, true) => "Entrée / Espace → Suivant     Q Quitter",
            (Forme::Carte { .. }, _) if quiz.revele => {
                "1 À revoir   2 Difficile   3 Bien (Espace)   4 Facile"
            }
            (Forme::Carte { .. }, _) => "Espace Retourner la carte   Q Quitter",
            (Forme::Saisie { .. }, _) => "Tape ta réponse   Entrée Valider   Échap Quitter",
            (Forme::Choix { .. }, _) => "A–D Choisir   Entrée Valider   Q Quitter",
            (Forme::Ordre { .. }, _) => {
                "Clique les mots (ou tape leur numéro)   ⌫ Retirer   Entrée Valider"
            }
        };
        Self::pied(ui, aide);

        let prog = &self.prog;
        centre().show(ui, |ui| {
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    Frame::NONE
                        .inner_margin(Margin {
                            left: 40,
                            right: 40,
                            top: 16,
                            bottom: 16,
                        })
                        .show(ui, |ui| {
                            ui.set_max_width(LARGEUR_CONTENU.min(ui.available_width()));
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&exo.etiquette).size(F_SMALL).color(MUTED));
                                if e.nouveau {
                                    badge(ui, "NOUVEAU", ACCENT);
                                }
                                if e.tentative > 0 {
                                    badge(ui, "NOUVELLE TENTATIVE", YELLOW);
                                }
                            });
                            ui.add_space(8.0);

                            match &exo.forme {
                                Forme::Carte { recto, verso } => {
                                    math::etiquette(ui, recto, F_TITLE, WHITE);
                                    ui.add_space(14.0);
                                    if !quiz.revele {
                                        if bouton(ui, "Retourner la carte  (Espace)", ACCENT)
                                            .clicked()
                                            && actif
                                        {
                                            act = Act::Reveler;
                                        }
                                    } else {
                                        math::etiquette(ui, verso, F_LARGE + 2.0, ACCENT);
                                        ui.add_space(8.0);
                                        for d in &exo.details {
                                            math::etiquette(ui, &format!("ℹ  {d}"), F_SMALL, MUTED);
                                        }
                                        ui.add_space(14.0);
                                        ui.label(
                                            RichText::new("Tu t'en souvenais ?")
                                                .size(F_SMALL)
                                                .color(MUTED),
                                        );
                                        ui.horizontal(|ui| {
                                            let notes = [
                                                (Note::ARevoir, "1 · À revoir", RED),
                                                (Note::Difficile, "2 · Difficile", YELLOW),
                                                (Note::Bien, "3 · Bien", GREEN),
                                                (Note::Facile, "4 · Facile", ACCENT),
                                            ];
                                            for (note, libelle, couleur) in notes {
                                                let delai = lg::texte_delai(lg::prevision(
                                                    prog, &e.set.id, &exo.id, note,
                                                ));
                                                let txt =
                                                    RichText::new(format!("{libelle}\n{delai}"))
                                                        .size(F_MED)
                                                        .color(BG);
                                                let b = egui::Button::new(txt)
                                                    .fill(couleur)
                                                    .corner_radius(4)
                                                    .min_size(vec2(118.0, 52.0));
                                                if ui
                                                    .add(b)
                                                    .on_hover_cursor(CursorIcon::PointingHand)
                                                    .clicked()
                                                    && actif
                                                {
                                                    act = Act::Noter(note);
                                                }
                                            }
                                        });
                                    }
                                }
                                Forme::Saisie {
                                    consigne,
                                    invite,
                                    reponses,
                                    tableau,
                                    ..
                                } => {
                                    math::etiquette(ui, consigne, F_LARGE, WHITE);
                                    ui.add_space(12.0);
                                    let mut valider = false;
                                    let mut id_champ = None;
                                    ui.horizontal(|ui| {
                                        if !invite.is_empty() {
                                            ui.label(
                                                RichText::new(invite).size(F_LARGE).color(ACCENT),
                                            );
                                            ui.add_space(6.0);
                                        }
                                        let edit = egui::TextEdit::singleline(&mut quiz.saisie)
                                            .font(FontId::proportional(F_LARGE))
                                            .text_color(FG)
                                            .background_color(BG2)
                                            .margin(Margin::symmetric(10, 10))
                                            .desired_width(f32::INFINITY);
                                        let resp = ui.add_enabled(!repondu, edit);
                                        id_champ = Some(resp.id);
                                        if quiz.focus && actif && !repondu {
                                            resp.request_focus();
                                            quiz.focus = false;
                                        }
                                        valider = !repondu
                                            && resp.lost_focus()
                                            && ui.input(|i| i.key_pressed(Key::Enter));
                                    });
                                    if valider {
                                        act = Act::Valider;
                                    }
                                    if !repondu && !e.set.caracteres_speciaux.is_empty() {
                                        ui.add_space(4.0);
                                        ui.horizontal_wrapped(|ui| {
                                            for c in &e.set.caracteres_speciaux {
                                                if petit_bouton(ui, c).clicked() {
                                                    inserer_au_curseur(
                                                        ui.ctx(),
                                                        id_champ,
                                                        &mut quiz.saisie,
                                                        c,
                                                    );
                                                    quiz.focus = true;
                                                }
                                            }
                                        });
                                    }
                                    if let Some(v) = quiz.verdict {
                                        ui.add_space(10.0);
                                        let couleur = match v {
                                            Verdict::Juste => GREEN,
                                            Verdict::Faux => RED,
                                            _ => YELLOW,
                                        };
                                        ui.label(
                                            RichText::new(v.message())
                                                .font(gras(17.5))
                                                .color(couleur),
                                        );
                                        if v != Verdict::Juste {
                                            ui.horizontal_wrapped(|ui| {
                                                ui.label(
                                                    RichText::new("Réponse attendue :")
                                                        .size(F_MED)
                                                        .color(FG),
                                                );
                                                ui.label(job_diff(
                                                    quiz.saisie.trim(),
                                                    &quiz.attendu,
                                                ));
                                            });
                                            let autres: Vec<&str> = reponses
                                                .iter()
                                                .filter(|r| **r != quiz.attendu)
                                                .map(String::as_str)
                                                .collect();
                                            if !autres.is_empty() {
                                                ui.label(
                                                    RichText::new(format!(
                                                        "Aussi accepté : {}",
                                                        autres.join(" / ")
                                                    ))
                                                    .size(F_SMALL)
                                                    .color(MUTED),
                                                );
                                            }
                                        }
                                        if let Some(t) = tableau {
                                            ui.add_space(8.0);
                                            Frame::NONE
                                                .fill(BG2)
                                                .corner_radius(4)
                                                .inner_margin(Margin::symmetric(14, 8))
                                                .show(ui, |ui| {
                                                    ui.label(
                                                        RichText::new(&t.titre)
                                                            .font(gras(F_MED))
                                                            .color(ACCENT),
                                                    );
                                                    egui::Grid::new("tableau_conjugaison")
                                                        .spacing(vec2(24.0, 4.0))
                                                        .show(ui, |ui| {
                                                            for (k, (p, f)) in
                                                                t.lignes.iter().enumerate()
                                                            {
                                                                let c = if k == t.surligne {
                                                                    YELLOW
                                                                } else {
                                                                    FG
                                                                };
                                                                ui.label(
                                                                    RichText::new(p)
                                                                        .size(F_MED)
                                                                        .color(
                                                                            if k == t.surligne {
                                                                                YELLOW
                                                                            } else {
                                                                                MUTED
                                                                            },
                                                                        ),
                                                                );
                                                                ui.label(
                                                                    RichText::new(f)
                                                                        .size(F_MED)
                                                                        .color(c),
                                                                );
                                                                ui.end_row();
                                                            }
                                                        });
                                                });
                                        }
                                    }
                                }
                                Forme::Choix {
                                    enonce,
                                    choix,
                                    reponses,
                                    ..
                                } => {
                                    math::etiquette(ui, enonce, F_LARGE, WHITE);
                                    ui.add_space(14.0);
                                    for (i, ch) in choix.iter().enumerate() {
                                        let sel = quiz.selection.contains(&i);
                                        let (fond, texte) = if repondu {
                                            if reponses.contains(&i) {
                                                (GREEN, BG)
                                            } else if sel {
                                                (RED, BG)
                                            } else {
                                                (BG2, FG)
                                            }
                                        } else if sel {
                                            (ACCENT, BG)
                                        } else {
                                            (BG2, FG)
                                        };
                                        if carte_choix(ui, lettre(i), ch, fond, texte, !repondu)
                                            .clicked()
                                            && actif
                                        {
                                            act = Act::Basculer(i);
                                        }
                                        ui.add_space(6.0);
                                    }
                                    if let Some(v) = quiz.verdict {
                                        ui.add_space(6.0);
                                        let couleur = if v == Verdict::Juste { GREEN } else { RED };
                                        ui.label(
                                            RichText::new(v.message())
                                                .font(gras(17.5))
                                                .color(couleur),
                                        );
                                    }
                                }
                                Forme::Ordre { consigne, mots, .. } => {
                                    math::etiquette(ui, consigne, F_LARGE, WHITE);
                                    ui.add_space(12.0);
                                    ui.label(
                                        RichText::new("Ta phrase :").size(F_SMALL).color(MUTED),
                                    );
                                    Frame::NONE
                                        .fill(BG2)
                                        .corner_radius(4)
                                        .inner_margin(Margin::same(8))
                                        .show(ui, |ui| {
                                            ui.set_min_height(42.0);
                                            ui.set_width(ui.available_width());
                                            ui.horizontal_wrapped(|ui| {
                                                for (k, &m) in quiz.places.iter().enumerate() {
                                                    if tuile(ui, None, &mots[m], BG3).clicked()
                                                        && actif
                                                        && !repondu
                                                    {
                                                        act = Act::Retirer(k);
                                                    }
                                                }
                                            });
                                        });
                                    ui.add_space(10.0);
                                    if !repondu {
                                        ui.horizontal_wrapped(|ui| {
                                            for (pos, &m) in e.melange.iter().enumerate() {
                                                if quiz.places.contains(&m) {
                                                    continue;
                                                }
                                                let numero = (mots.len() <= 9).then_some(pos + 1);
                                                if tuile(ui, numero, &mots[m], BG2).clicked()
                                                    && actif
                                                {
                                                    act = Act::Placer(pos);
                                                }
                                            }
                                        });
                                        ui.add_space(10.0);
                                        ui.horizontal(|ui| {
                                            if petit_bouton(ui, "⌫ Retirer le dernier").clicked()
                                                && actif
                                            {
                                                act = Act::RetirerDernier;
                                            }
                                            let complet = quiz.places.len() == mots.len();
                                            if complet
                                                && bouton(ui, "Valider  (Entrée)", ACCENT).clicked()
                                                && actif
                                            {
                                                act = Act::Valider;
                                            }
                                        });
                                    }
                                    if let Some(v) = quiz.verdict {
                                        ui.add_space(6.0);
                                        let couleur = if v == Verdict::Juste { GREEN } else { RED };
                                        ui.label(
                                            RichText::new(v.message())
                                                .font(gras(17.5))
                                                .color(couleur),
                                        );
                                        if v != Verdict::Juste {
                                            ui.label(
                                                RichText::new(format!(
                                                    "Réponse : {}",
                                                    quiz.attendu
                                                ))
                                                .size(F_LARGE)
                                                .color(GREEN),
                                            );
                                        }
                                    }
                                }
                            }

                            // Détails (exemple, règle…) après la correction.
                            if repondu
                                && !matches!(exo.forme, Forme::Carte { .. })
                                && !exo.details.is_empty()
                            {
                                ui.add_space(6.0);
                                for d in &exo.details {
                                    math::etiquette(ui, &format!("ℹ  {d}"), F_SMALL, MUTED);
                                }
                            }
                            if quiz.note == Some(Note::Facile) && repondu {
                                ui.label(
                                    RichText::new("Noté « trop facile » : il reviendra plus tard.")
                                        .size(F_SMALL)
                                        .color(ACCENT),
                                );
                            }
                        });
                });
        });

        // --- Application de l'action ---
        let mut a_enregistrer: Option<Note> = None;
        let mut fin = None;
        let mut quitter = false;
        match act {
            Act::Rien => {}
            Act::Basculer(i) if !repondu => {
                if let Forme::Choix { multiple: true, .. } = exo.forme {
                    if let Some(p) = quiz.selection.iter().position(|&s| s == i) {
                        quiz.selection.remove(p);
                    } else {
                        quiz.selection.push(i);
                    }
                } else {
                    quiz.selection = vec![i];
                }
            }
            Act::Placer(pos) if !repondu => {
                let m = e.melange[pos];
                if !quiz.places.contains(&m) {
                    quiz.places.push(m);
                }
            }
            Act::Retirer(k) if !repondu && k < quiz.places.len() => {
                quiz.places.remove(k);
            }
            Act::RetirerDernier if !repondu => {
                quiz.places.pop();
            }
            Act::Reveler => quiz.revele = true,
            Act::Valider if !repondu => {
                let resultat = match &exo.forme {
                    Forme::Saisie {
                        reponses, tolerant, ..
                    } if !quiz.saisie.trim().is_empty() => Some(lg::noter_saisie(
                        quiz.saisie.trim(),
                        reponses,
                        *tolerant,
                        &e.set.particules,
                    )),
                    Forme::Choix { reponses, .. } if !quiz.selection.is_empty() => {
                        let mut s = quiz.selection.clone();
                        s.sort_unstable();
                        let v = if s == *reponses {
                            Verdict::Juste
                        } else {
                            Verdict::Faux
                        };
                        Some((v, String::new()))
                    }
                    Forme::Ordre { mots, reponses, .. } if quiz.places.len() == mots.len() => {
                        let phrase = quiz
                            .places
                            .iter()
                            .map(|&m| mots[m].as_str())
                            .collect::<Vec<_>>()
                            .join(" ");
                        Some(lg::noter_ordre(&phrase, reponses))
                    }
                    _ => None,
                };
                if let Some((v, attendu)) = resultat {
                    quiz.verdict = Some(v);
                    quiz.attendu = attendu;
                    quiz.note = Some(v.note());
                    if e.tentative == 0 {
                        quiz.premiers += 1;
                        quiz.score += usize::from(v.note().reussie());
                    }
                }
            }
            Act::Noter(note) if quiz.revele && !repondu => {
                quiz.note = Some(note);
                if e.tentative == 0 {
                    quiz.premiers += 1;
                    quiz.score += usize::from(note.reussie());
                }
                act = Act::Suivante;
            }
            Act::Facile if quiz.verdict == Some(Verdict::Juste) => quiz.note = Some(Note::Facile),
            Act::Quitter => quitter = true,
            _ => {}
        }
        // La révision est enregistrée en quittant l'exercice (la touche F a pu
        // entre-temps la passer en « Facile »).
        if (act == Act::Suivante || quitter)
            && let Some(note) = quiz.note
            && !quiz.enregistre
        {
            quiz.enregistre = true;
            if e.tentative == 0 {
                a_enregistrer = Some(note);
            }
            if note == Note::ARevoir && act == Act::Suivante {
                lg::reprogrammer(&mut quiz.file, quiz.index);
            }
        }
        if act == Act::Suivante && quiz.note.is_some() {
            if quiz.index + 1 >= quiz.file.len() {
                fin = Some(Ecran::Resultat {
                    score: quiz.score,
                    total: quiz.premiers,
                    langue: Some(quiz.langue.clone()),
                });
            } else {
                quiz.index += 1;
                quiz.preparer();
            }
        }
        if quitter {
            self.dialogue = Some(Dialogue::Confirmer {
                titre: "Quitter la session".into(),
                message:
                    "Quitter la session en cours ?\nLes réponses déjà données sont enregistrées."
                        .into(),
                action: ActionConfirmee::QuitterQuizLangue(quiz.langue.clone()),
            });
        }
        if act != Act::Rien {
            ui.ctx().request_repaint();
        }
        if let Some(note) = a_enregistrer {
            lg::enregistrer(&mut self.prog, &e.set.id, &exo.id, note);
            self.sauvegarder();
        }
        if let Some(ecran) = fin {
            self.ecran = ecran;
        }
    }
}
