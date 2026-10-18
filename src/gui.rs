//! Interface graphique (egui) pour Révision QCM / QCR.
//!
//! Rendu immédiat accéléré par le GPU : l'écran n'est redessiné que lorsqu'un
//! événement survient (clavier, souris, redimensionnement) — 0 % CPU au repos.
//! Les formules LaTeX (`$...$`) sont rendues nativement par [`crate::math`].

use std::sync::Arc;

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, CursorIcon, FontData, FontDefinitions, FontFamily,
    FontId, Frame, Key, Margin, Modal, Pos2, Rect, RichText, ScrollArea, Sense, Stroke, Ui, Vec2,
    ViewportCommand, pos2, vec2,
};

mod langues;

use crate::bibliotheque;
use crate::core::{self, Progression, Question, QuestionSet, TypeQuestion};
use crate::langues::{Mode, SetLangue};
use crate::math;
use crate::paths;

// --- Palette sombre (Catppuccin Mocha) ---
const BG: Color32 = Color32::from_rgb(0x1e, 0x1e, 0x2e);
const BG2: Color32 = Color32::from_rgb(0x31, 0x32, 0x44);
const BG3: Color32 = Color32::from_rgb(0x45, 0x47, 0x5a);
const FG: Color32 = Color32::from_rgb(0xcd, 0xd6, 0xf4);
const ACCENT: Color32 = Color32::from_rgb(0x89, 0xb4, 0xfa);
const GREEN: Color32 = Color32::from_rgb(0xa6, 0xe3, 0xa1);
const RED: Color32 = Color32::from_rgb(0xf3, 0x8b, 0xa8);
const YELLOW: Color32 = Color32::from_rgb(0xf9, 0xe2, 0xaf);
const MUTED: Color32 = Color32::from_rgb(0x6c, 0x70, 0x86);
const WHITE: Color32 = Color32::WHITE;

// --- Tailles de police (points logiques) ---
const F_TITLE: f32 = 26.0;
const F_LARGE: f32 = 18.5;
const F_MED: f32 = 16.0;
const F_SMALL: f32 = 13.5;

/// Largeur maximale du contenu (lisibilité sur grand écran).
const LARGEUR_CONTENU: f32 = 840.0;

fn gras(taille: f32) -> FontId {
    FontId::new(taille, FontFamily::Name("Gras".into()))
}

fn lettre(i: usize) -> char {
    char::from(b'A' + i as u8)
}

// ---------------------------------------------------------------------------
// État de l'application
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, PartialEq, Eq)]
enum But {
    Session,
    Reinitialiser,
}

impl But {
    fn titre(self) -> &'static str {
        match self {
            But::Session => "Choisir un set",
            But::Reinitialiser => "Réinitialiser un set",
        }
    }
}

struct Quiz {
    paires: Vec<(Arc<QuestionSet>, usize)>,
    index: usize,
    score: usize,
    selection: Vec<usize>,
    repondu: bool,
    revele: bool,
    correcte: bool,
    saisie: String,
    /// Donner le focus au champ de saisie (TROU) à l'affichage de la question.
    focus_saisie: bool,
}

impl Quiz {
    fn new(paires: Vec<(Arc<QuestionSet>, usize)>) -> Self {
        Self {
            paires,
            index: 0,
            score: 0,
            selection: Vec::new(),
            repondu: false,
            revele: false,
            correcte: false,
            saisie: String::new(),
            focus_saisie: true,
        }
    }

    fn courante(&self) -> (&Arc<QuestionSet>, &Question) {
        let (s, i) = &self.paires[self.index];
        (s, &s.questions[*i])
    }
}

enum Ecran {
    MenuPrincipal {
        sel: usize,
    },
    Categories {
        but: But,
        sel: usize,
    },
    SetsCategorie {
        but: But,
        categorie: String,
        sel: usize,
    },
    ModeSession {
        set: Arc<QuestionSet>,
        sel: usize,
    },
    Quiz(Box<Quiz>),
    Resultat {
        score: usize,
        total: usize,
        /// Session de langue : retour au menu de cette langue.
        langue: Option<String>,
    },
    Statistiques,
    Langues {
        sel: usize,
    },
    LangueMenu {
        langue: String,
        sel: usize,
    },
    QuizLangue(Box<langues::QuizLangue>),
    StatsLangue {
        langue: String,
    },
}

enum ActionNombre {
    Session(Arc<QuestionSet>, Vec<usize>),
    Revision(Vec<(Arc<QuestionSet>, usize)>),
    Langue(String, Mode),
}

enum ActionConfirmee {
    Reinitialiser(Arc<QuestionSet>),
    ReinitialiserLangue(String),
    QuitterQuiz,
    QuitterQuizLangue(String),
}

enum Dialogue {
    Info {
        titre: String,
        message: String,
    },
    Nombre {
        max: usize,
        saisie: String,
        action: ActionNombre,
        init: bool,
    },
    Confirmer {
        titre: String,
        message: String,
        action: ActionConfirmee,
    },
}

/// Ce qu'une liste navigable a produit pendant cette frame.
enum Nav {
    Rien,
    Choix(usize),
    Retour,
}

pub struct RevisionGui {
    prog: Progression,
    sets: Vec<Arc<QuestionSet>>,
    langues: Vec<Arc<SetLangue>>,
    erreurs_affichees: Vec<String>,
    ecran: Ecran,
    dialogue: Option<Dialogue>,
}

impl RevisionGui {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configurer_style(&cc.egui_ctx);
        let mut app = Self {
            prog: Progression::new(paths::fichier_progression()),
            sets: Vec::new(),
            langues: Vec::new(),
            erreurs_affichees: Vec::new(),
            ecran: Ecran::MenuPrincipal { sel: 0 },
            dialogue: None,
        };
        app.charger_sets();
        app
    }

    // -----------------------------------------------------------------------
    // Données
    // -----------------------------------------------------------------------
    /// Recharge les sets depuis questions/ (permet de les modifier à chaud).
    fn charger_sets(&mut self) {
        let bib = bibliotheque::charger(&paths::dossier_questions());
        let erreurs = bib.erreurs;
        self.sets = bib.sets;
        self.langues = bib.langues;
        if !erreurs.is_empty() && erreurs != self.erreurs_affichees && self.dialogue.is_none() {
            self.dialogue = Some(Dialogue::Info {
                titre: "Sets ignorés".into(),
                message: format!(
                    "Certains sets n'ont pas pu être chargés et n'apparaissent pas dans les listes :\n\n{}",
                    erreurs.join("\n")
                ),
            });
        }
        self.erreurs_affichees = erreurs;
    }

    fn sauvegarder(&mut self) {
        if let Err(e) = self.prog.sauvegarder() {
            self.dialogue = Some(Dialogue::Info {
                titre: "Erreur".into(),
                message: format!("Impossible d'enregistrer la progression :\n{e}"),
            });
        }
    }

    fn info(&mut self, titre: &str, message: impl Into<String>) {
        self.dialogue = Some(Dialogue::Info {
            titre: titre.into(),
            message: message.into(),
        });
    }

    // -----------------------------------------------------------------------
    // Navigation
    // -----------------------------------------------------------------------
    fn aller_menu(&mut self) {
        self.charger_sets();
        self.ecran = Ecran::MenuPrincipal { sel: 0 };
    }

    /// Choix d'un set, en passant par un choix de catégorie s'il y en a plusieurs.
    fn aller_choix_set(&mut self, but: But) {
        if self.sets.is_empty() {
            let msg = match but {
                But::Session => format!(
                    "Aucun set trouvé dans :\n{}",
                    paths::dossier_questions().display()
                ),
                But::Reinitialiser => "Aucun set disponible.".into(),
            };
            self.info("Aucun set", msg);
            return;
        }
        let cats = core::categories(&self.sets);
        self.ecran = if cats.len() <= 1 {
            Ecran::SetsCategorie {
                but,
                categorie: cats
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| core::CATEGORIE_DEFAUT.into()),
                sel: 0,
            }
        } else {
            Ecran::Categories { but, sel: 0 }
        };
    }

    fn retour_depuis_sets(&mut self, but: But) {
        if core::categories(&self.sets).len() > 1 {
            self.ecran = Ecran::Categories { but, sel: 0 };
        } else {
            self.aller_menu();
        }
    }

    fn lancer_quiz(&mut self, paires: Vec<(Arc<QuestionSet>, usize)>) {
        if paires.is_empty() {
            self.info("Session vide", "Aucune question dans cette session.");
            self.aller_menu();
            return;
        }
        self.ecran = Ecran::Quiz(Box::new(Quiz::new(paires)));
    }

    fn demarrer_mode(&mut self, set: Arc<QuestionSet>, mode: usize) {
        let mut indices: Vec<usize> = (0..set.len()).collect();
        match mode {
            1 => {
                indices.retain(|&i| self.prog.est_a_reviser(&set.id, &set.questions[i].id));
                indices.sort_by_cached_key(|&i| self.prog.priorite(&set.id, &set.questions[i].id));
                if indices.is_empty() {
                    self.info("Rien à réviser", "Aucune question à réviser pour ce set.");
                    return;
                }
            }
            2 => {
                fastrand::shuffle(&mut indices);
                self.dialogue = Some(Dialogue::Nombre {
                    max: indices.len(),
                    saisie: indices.len().min(10).to_string(),
                    action: ActionNombre::Session(set, indices),
                    init: true,
                });
                return;
            }
            _ => fastrand::shuffle(&mut indices),
        }
        self.lancer_quiz(indices.into_iter().map(|i| (Arc::clone(&set), i)).collect());
    }

    fn revision_intelligente(&mut self) {
        self.charger_sets();
        let a_faire = self.prog.questions_a_reviser(&self.sets);
        if a_faire.is_empty() {
            self.info(
                "Rien à réviser",
                "Rien à réviser pour l'instant — reviens plus tard. ✓",
            );
            return;
        }
        self.dialogue = Some(Dialogue::Nombre {
            max: a_faire.len(),
            saisie: a_faire.len().min(10).to_string(),
            action: ActionNombre::Revision(a_faire),
            init: true,
        });
    }

    // -----------------------------------------------------------------------
    // Éléments communs
    // -----------------------------------------------------------------------
    fn entete(ui: &mut Ui, texte: &str) {
        ui.add_space(18.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(texte).font(gras(F_TITLE)).color(ACCENT));
        });
        ui.add_space(14.0);
        let r = ui.available_rect_before_wrap();
        ui.painter().rect_filled(
            Rect::from_min_size(pos2(r.left() + 20.0, r.top()), vec2(r.width() - 40.0, 2.0)),
            0.0,
            BG3,
        );
        ui.add_space(4.0);
    }

    fn pied(ui: &mut Ui, texte: &str) {
        egui::Panel::bottom("pied")
            .frame(Frame::NONE.fill(BG).inner_margin(Margin::same(8)))
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(texte).size(F_SMALL).color(MUTED))
                });
            });
    }

    /// Liste navigable au clavier (↑↓ + Entrée, Échap = retour) et à la souris.
    fn liste_nav(ui: &mut Ui, items: &[String], sel: &mut usize, actif: bool) -> Nav {
        let mut nav = Nav::Rien;
        let mut defiler = false;
        if actif && !items.is_empty() {
            ui.input_mut(|i| {
                if i.consume_key(Default::default(), Key::ArrowDown) {
                    *sel = (*sel + 1).min(items.len() - 1);
                    defiler = true;
                }
                if i.consume_key(Default::default(), Key::ArrowUp) {
                    *sel = sel.saturating_sub(1);
                    defiler = true;
                }
                if i.consume_key(Default::default(), Key::Home) {
                    *sel = 0;
                    defiler = true;
                }
                if i.consume_key(Default::default(), Key::End) {
                    *sel = items.len() - 1;
                    defiler = true;
                }
                if i.consume_key(Default::default(), Key::Enter) {
                    nav = Nav::Choix(*sel);
                }
            });
        }
        if actif && ui.input_mut(|i| i.consume_key(Default::default(), Key::Escape)) {
            nav = Nav::Retour;
        }

        Frame::NONE
            .inner_margin(Margin::symmetric(40, 16))
            .show(ui, |ui| {
                Frame::NONE.fill(BG2).show(ui, |ui| {
                    ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let largeur = ui.available_width();
                            for (i, item) in items.iter().enumerate() {
                                let (rect, resp) = ui.allocate_exact_size(
                                    vec2(largeur, F_LARGE * 1.75),
                                    Sense::click(),
                                );
                                let choisi = i == *sel;
                                let fond = if choisi {
                                    ACCENT
                                } else if resp.hovered() {
                                    BG3
                                } else {
                                    BG2
                                };
                                ui.painter().rect_filled(rect, 0.0, fond);
                                ui.painter().text(
                                    pos2(rect.left() + 14.0, rect.center().y),
                                    Align2::LEFT_CENTER,
                                    item,
                                    FontId::proportional(F_LARGE),
                                    if choisi { BG } else { FG },
                                );
                                if resp.clicked() && actif {
                                    *sel = i;
                                    nav = Nav::Choix(i);
                                }
                                if choisi && defiler {
                                    resp.scroll_to_me(None);
                                }
                                resp.on_hover_cursor(CursorIcon::PointingHand);
                            }
                        });
                });
            });
        nav
    }

    // -----------------------------------------------------------------------
    // Écrans de navigation
    // -----------------------------------------------------------------------
    fn ecran_menu(&mut self, ui: &mut Ui, actif: bool) {
        let Ecran::MenuPrincipal { sel } = &mut self.ecran else {
            return;
        };
        Self::pied(ui, "↑↓ Naviguer   Entrée Sélectionner   Échap Quitter");
        let mut nav = Nav::Rien;
        centre().show(ui, |ui| {
            Self::entete(ui, "Révision QCM / QCR");
            let items = [
                "Démarrer une session",
                "Révision intelligente",
                "Langues — vocabulaire, conjugaison, grammaire",
                "Mes statistiques",
                "Réinitialiser la progression",
                "Quitter",
            ]
            .map(String::from);
            nav = Self::liste_nav(ui, &items, sel, actif);
        });
        match nav {
            Nav::Choix(0) => self.aller_choix_set(But::Session),
            Nav::Choix(1) => self.revision_intelligente(),
            Nav::Choix(2) => self.aller_langues(),
            Nav::Choix(3) => {
                self.charger_sets();
                self.ecran = Ecran::Statistiques;
            }
            Nav::Choix(4) => self.aller_choix_set(But::Reinitialiser),
            Nav::Choix(_) | Nav::Retour => ui.ctx().send_viewport_cmd(ViewportCommand::Close),
            Nav::Rien => {}
        }
    }

    fn ecran_categories(&mut self, ui: &mut Ui, actif: bool) {
        let Ecran::Categories { but, sel } = &mut self.ecran else {
            return;
        };
        let but = *but;
        let cats = core::categories(&self.sets);
        let items: Vec<String> = cats
            .iter()
            .map(|c| {
                format!(
                    "{c}  ({} sets)",
                    self.sets.iter().filter(|s| &s.categorie == c).count()
                )
            })
            .collect();
        Self::pied(ui, "↑↓ Naviguer   Entrée Sélectionner   Échap Retour");
        let mut nav = Nav::Rien;
        centre().show(ui, |ui| {
            Self::entete(ui, but.titre());
            nav = Self::liste_nav(ui, &items, sel, actif);
        });
        match nav {
            Nav::Choix(i) => {
                self.ecran = Ecran::SetsCategorie {
                    but,
                    categorie: cats[i].clone(),
                    sel: 0,
                };
            }
            Nav::Retour => self.aller_menu(),
            Nav::Rien => {}
        }
    }

    fn ecran_sets(&mut self, ui: &mut Ui, actif: bool) {
        let Ecran::SetsCategorie {
            but,
            categorie,
            sel,
        } = &mut self.ecran
        else {
            return;
        };
        let but = *but;
        let sets_cat: Vec<Arc<QuestionSet>> = self
            .sets
            .iter()
            .filter(|s| &s.categorie == categorie)
            .cloned()
            .collect();
        let items: Vec<String> = sets_cat
            .iter()
            .map(|s| format!("{}  ({} questions)", s.titre, s.len()))
            .collect();
        let titre = format!("{} — {categorie}", but.titre());
        Self::pied(ui, "↑↓ Naviguer   Entrée Sélectionner   Échap Retour");
        let mut nav = Nav::Rien;
        centre().show(ui, |ui| {
            Self::entete(ui, &titre);
            nav = Self::liste_nav(ui, &items, sel, actif);
        });
        match nav {
            Nav::Choix(i) => {
                let set = Arc::clone(&sets_cat[i]);
                match but {
                    But::Session => self.ecran = Ecran::ModeSession { set, sel: 0 },
                    But::Reinitialiser => {
                        self.dialogue = Some(Dialogue::Confirmer {
                            titre: "Confirmer la réinitialisation".into(),
                            message: format!(
                                "Effacer toute la progression de\n« {} » ?\n\nCette action est irréversible.",
                                set.titre
                            ),
                            action: ActionConfirmee::Reinitialiser(set),
                        });
                    }
                }
            }
            Nav::Retour => self.retour_depuis_sets(but),
            Nav::Rien => {}
        }
    }

    fn ecran_mode(&mut self, ui: &mut Ui, actif: bool) {
        let Ecran::ModeSession { set, sel } = &mut self.ecran else {
            return;
        };
        let set = Arc::clone(set);
        Self::pied(ui, "↑↓ Naviguer   Entrée Sélectionner   Échap Retour");
        let mut nav = Nav::Rien;
        centre().show(ui, |ui| {
            Self::entete(ui, &format!("Mode — {}", set.titre));
            let items = [
                "Toutes les questions (ordre aléatoire)",
                "Seulement les questions à réviser",
                "Nombre limité de questions",
            ]
            .map(String::from);
            nav = Self::liste_nav(ui, &items, sel, actif);
        });
        match nav {
            Nav::Choix(mode) => self.demarrer_mode(set, mode),
            Nav::Retour => self.aller_choix_set(But::Session),
            Nav::Rien => {}
        }
    }

    // -----------------------------------------------------------------------
    // Quiz — moteur commun (session unique + révision intelligente)
    // -----------------------------------------------------------------------
    fn ecran_quiz(&mut self, ui: &mut Ui, actif: bool) {
        let Ecran::Quiz(quiz) = &mut self.ecran else {
            return;
        };
        let total = quiz.paires.len();
        let (set, q) = quiz.courante();
        let (set, q) = (Arc::clone(set), q.clone());
        let num = quiz.index + 1;

        // --- Clavier ---
        #[derive(PartialEq)]
        enum Act {
            Rien,
            Basculer(usize),
            Valider,
            Suivante,
            Reveler,
            Repondre(bool),
            Quitter,
        }
        let mut act = Act::Rien;
        if actif {
            ui.input_mut(|i| {
                let entree = i.consume_key(Default::default(), Key::Enter);
                let espace = i.key_pressed(Key::Space);
                let echap = i.consume_key(Default::default(), Key::Escape);
                let lettres: Vec<char> = i
                    .events
                    .iter()
                    .filter_map(|e| match e {
                        egui::Event::Text(t) => t.chars().next(),
                        _ => None,
                    })
                    .filter(|c| c.is_alphabetic())
                    .flat_map(char::to_uppercase)
                    .collect();
                match q.type_q {
                    TypeQuestion::Qcm | TypeQuestion::Qcr => {
                        if entree || espace {
                            act = if quiz.repondu {
                                Act::Suivante
                            } else {
                                Act::Valider
                            };
                        } else if lettres.contains(&'Q') || echap {
                            act = Act::Quitter;
                        } else if let Some(&l) = lettres.first() {
                            let idx = (l as u32).wrapping_sub(65) as usize;
                            if idx < q.choix.len() {
                                act = Act::Basculer(idx);
                            }
                        }
                    }
                    TypeQuestion::Trou => {
                        if quiz.repondu && (entree || espace) {
                            act = Act::Suivante;
                        } else if echap || (quiz.repondu && lettres.contains(&'Q')) {
                            act = Act::Quitter;
                        } else if !quiz.repondu && entree {
                            act = Act::Valider;
                        }
                    }
                    TypeQuestion::Flashcard => {
                        if espace && !quiz.revele {
                            act = Act::Reveler;
                        } else if (espace || entree) && quiz.repondu {
                            act = Act::Suivante;
                        } else if lettres.contains(&'Q') || echap {
                            act = Act::Quitter;
                        } else if quiz.revele && !quiz.repondu && lettres.contains(&'C') {
                            act = Act::Repondre(true);
                        } else if quiz.revele && !quiz.repondu && lettres.contains(&'I') {
                            act = Act::Repondre(false);
                        }
                    }
                }
            });
        }

        // --- Barre du haut : progression + score ---
        egui::Panel::top("progression")
            .frame(
                Frame::NONE
                    .fill(BG2)
                    .inner_margin(Margin::symmetric(20, 10)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let pct = (num - 1) as f32 / total as f32;
                    ui.label(
                        RichText::new(format!("Question {num}/{total}"))
                            .size(F_MED)
                            .color(ACCENT),
                    );
                    ui.add_space(10.0);
                    barre(ui, 200.0, 10.0, pct, ACCENT);
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(format!("{:.0}%", pct * 100.0))
                            .size(F_SMALL)
                            .color(ACCENT),
                    );
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!(
                                "Score : {}/{}     {}",
                                quiz.score,
                                num - 1 + usize::from(quiz.repondu),
                                set.titre
                            ))
                            .size(F_SMALL)
                            .color(MUTED),
                        );
                    });
                });
            });

        // --- Aide clavier en bas ---
        let aide = match q.type_q {
            _ if quiz.repondu => "Entrée / Espace → Question suivante     Q Quitter",
            TypeQuestion::Qcr => "A–D Sélectionner   Entrée Valider   Q Quitter",
            TypeQuestion::Qcm => "A–D Répondre   Entrée Valider   Q Quitter",
            TypeQuestion::Trou => "Tape ta réponse   Entrée Valider   Échap Quitter",
            TypeQuestion::Flashcard if quiz.revele => "C Je savais   I À revoir   Q Quitter",
            TypeQuestion::Flashcard => "Espace Retourner la carte   Q Quitter",
        };
        Self::pied(ui, aide);

        // --- Corps défilant : énoncé + zone de réponse + retour ---
        centre().show(ui, |ui| {
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                Frame::NONE.inner_margin(Margin { left: 40, right: 40, top: 16, bottom: 16 }).show(ui, |ui| {
                    ui.set_max_width(LARGEUR_CONTENU.min(ui.available_width()));
                    let etiquette = match q.type_q {
                        TypeQuestion::Qcm => "(QCM — une seule réponse)",
                        TypeQuestion::Qcr => "(QCR — plusieurs réponses possibles)",
                        TypeQuestion::Trou => "(Texte à trous — tape la réponse)",
                        TypeQuestion::Flashcard => {
                            "(Flashcard — essaie de te souvenir avant de retourner la carte)"
                        }
                    };
                    ui.label(RichText::new(etiquette).size(F_SMALL).color(MUTED));
                    ui.add_space(6.0);
                    math::etiquette(ui, &q.enonce, F_LARGE, WHITE);
                    ui.add_space(14.0);

                    match q.type_q {
                        TypeQuestion::Qcm | TypeQuestion::Qcr => {
                            for (i, choix) in q.choix.iter().enumerate() {
                                let sel = quiz.selection.contains(&i);
                                let (fond, texte) = if quiz.repondu {
                                    if q.reponses.contains(&i) {
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
                                if carte_choix(ui, lettre(i), choix, fond, texte, !quiz.repondu).clicked()
                                    && actif
                                {
                                    act = Act::Basculer(i);
                                }
                                ui.add_space(6.0);
                            }
                        }
                        TypeQuestion::Trou => {
                            let edit = egui::TextEdit::singleline(&mut quiz.saisie)
                                .font(FontId::proportional(F_LARGE))
                                .text_color(FG)
                                .background_color(BG2)
                                .margin(Margin::symmetric(10, 10))
                                .desired_width(f32::INFINITY);
                            let resp = ui.add_enabled(!quiz.repondu, edit);
                            if quiz.focus_saisie && actif {
                                resp.request_focus();
                                quiz.focus_saisie = false;
                            }
                            if !quiz.repondu
                                && resp.lost_focus()
                                && ui.input(|i| i.key_pressed(Key::Enter))
                            {
                                act = Act::Valider;
                            }
                        }
                        TypeQuestion::Flashcard => {
                            if bouton(ui, "Retourner la carte  (Espace)", ACCENT).clicked() && actif {
                                act = Act::Reveler;
                            }
                            if quiz.revele {
                                ui.add_space(12.0);
                                math::etiquette(ui, &q.verso, F_LARGE, ACCENT);
                                ui.add_space(16.0);
                                ui.horizontal(|ui| {
                                    if bouton(ui, "✓ Je savais  (C)", GREEN).clicked() && actif {
                                        act = Act::Repondre(true);
                                    }
                                    ui.add_space(10.0);
                                    if bouton(ui, "✗ À revoir  (I)", RED).clicked() && actif {
                                        act = Act::Repondre(false);
                                    }
                                });
                            }
                        }
                    }

                    // --- Retour : correct/incorrect + explication ---
                    if quiz.repondu {
                        ui.add_space(10.0);
                        let (txt, couleur) =
                            if quiz.correcte { ("✓ Correct !", GREEN) } else { ("✗ Incorrect", RED) };
                        ui.label(RichText::new(txt).font(gras(17.5)).color(couleur));
                        if !quiz.correcte {
                            let extra = match q.type_q {
                                TypeQuestion::Qcm | TypeQuestion::Qcr => Some(format!(
                                    "Bonne(s) réponse(s) : {}",
                                    q.reponses.iter().map(|&r| lettre(r).to_string()).collect::<Vec<_>>().join(", ")
                                )),
                                TypeQuestion::Trou => {
                                    Some(format!("Réponse attendue : {}", q.reponses_texte.join(" / ")))
                                }
                                TypeQuestion::Flashcard => None,
                            };
                            if let Some(extra) = extra {
                                ui.label(RichText::new(extra).size(F_MED).color(GREEN));
                            }
                        }
                        if !q.explication.is_empty() {
                            ui.add_space(4.0);
                            math::etiquette(ui, &format!("ℹ  {}", q.explication), F_SMALL, MUTED);
                        }
                    }
                });
            });
        });

        // --- Application de l'action ---
        let mut enregistrer = None;
        let mut fin = None;
        match act {
            Act::Rien => {}
            Act::Basculer(i) if !quiz.repondu => {
                if q.est_qcr() {
                    if let Some(p) = quiz.selection.iter().position(|&s| s == i) {
                        quiz.selection.remove(p);
                    } else {
                        quiz.selection.push(i);
                    }
                } else {
                    quiz.selection = vec![i];
                }
            }
            Act::Basculer(_) => {}
            Act::Valider => match q.type_q {
                TypeQuestion::Qcm | TypeQuestion::Qcr if !quiz.selection.is_empty() => {
                    enregistrer = Some(q.est_correcte(&quiz.selection));
                }
                TypeQuestion::Trou if !quiz.saisie.trim().is_empty() => {
                    enregistrer = Some(q.est_correcte_texte(quiz.saisie.trim()));
                }
                _ => {}
            },
            Act::Reveler => quiz.revele = true,
            Act::Repondre(c) if !quiz.repondu => enregistrer = Some(c),
            Act::Repondre(_) => {}
            Act::Suivante => {
                if quiz.index + 1 >= total {
                    fin = Some(Ecran::Resultat {
                        score: quiz.score,
                        total,
                        langue: None,
                    });
                } else {
                    let (paires, index, score) =
                        (std::mem::take(&mut quiz.paires), quiz.index + 1, quiz.score);
                    **quiz = Quiz {
                        index,
                        score,
                        ..Quiz::new(paires)
                    };
                }
            }
            Act::Quitter => {
                self.dialogue = Some(Dialogue::Confirmer {
                    titre: "Quitter la session".into(),
                    message: "Quitter la session en cours ?".into(),
                    action: ActionConfirmee::QuitterQuiz,
                });
            }
        }
        if let Some(correcte) = enregistrer {
            quiz.repondu = true;
            quiz.correcte = correcte;
            quiz.score += usize::from(correcte);
        }
        if act != Act::Rien {
            ui.ctx().request_repaint();
        }
        if let Some(correcte) = enregistrer {
            self.prog.enregistrer_reponse(&set.id, &q.id, correcte);
            self.sauvegarder();
        }
        if let Some(ecran) = fin {
            self.ecran = ecran;
        }
    }

    fn ecran_resultat(&mut self, ui: &mut Ui, actif: bool) {
        let Ecran::Resultat {
            score,
            total,
            langue,
        } = &self.ecran
        else {
            return;
        };
        let (score, total, langue) = (*score, (*total).max(1), langue.clone());
        let pct = score as f32 / total as f32 * 100.0;
        let couleur = if pct >= 100.0 {
            GREEN
        } else if pct >= 60.0 {
            YELLOW
        } else {
            RED
        };
        let message = if pct >= 100.0 {
            "Parfait, tout est juste ! ✓"
        } else if pct >= 60.0 {
            "Bon travail — continue de réviser les points faibles."
        } else {
            "À retravailler — relance une révision intelligente bientôt."
        };
        let mut retour = actif
            && ui.input_mut(|i| {
                i.consume_key(Default::default(), Key::Enter)
                    | i.consume_key(Default::default(), Key::Space)
                    | i.consume_key(Default::default(), Key::Escape)
            });
        centre().show(ui, |ui| {
            Self::entete(ui, "Résultat de la session");
            ui.vertical_centered(|ui| {
                ui.add_space(16.0);
                ui.label(
                    RichText::new(format!("{score} / {total}"))
                        .font(gras(68.0))
                        .color(couleur),
                );
                ui.label(
                    RichText::new(format!("{pct:.0} %"))
                        .font(gras(F_TITLE))
                        .color(couleur),
                );
                ui.add_space(10.0);
                barre(ui, 340.0, 14.0, pct / 100.0, couleur);
                ui.add_space(14.0);
                ui.label(RichText::new(message).size(F_MED).color(FG));
                ui.add_space(28.0);
                let libelle = if langue.is_some() {
                    "Continuer  (Entrée)"
                } else {
                    "Menu principal  (Entrée)"
                };
                if bouton(ui, libelle, ACCENT).clicked() {
                    retour = true;
                }
            });
        });
        if retour {
            match langue {
                Some(langue) => self.aller_langue(langue),
                None => self.aller_menu(),
            }
        }
    }

    fn ecran_statistiques(&mut self, ui: &mut Ui, actif: bool) {
        let mut retour = actif && ui.input_mut(|i| i.consume_key(Default::default(), Key::Escape));
        centre().show(ui, |ui| {
            Self::entete(ui, "Mes statistiques");
            Frame::NONE
                .inner_margin(Margin {
                    left: 20,
                    right: 20,
                    top: 8,
                    bottom: 0,
                })
                .show(ui, |ui| {
                    if petit_bouton(ui, "← Retour  (Échap)").clicked() {
                        retour = true;
                    }
                });
            if self.sets.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        RichText::new("Aucun set chargé.")
                            .size(F_LARGE)
                            .color(MUTED),
                    );
                });
                return;
            }
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    Frame::NONE
                        .inner_margin(Margin::symmetric(20, 6))
                        .show(ui, |ui| {
                            for s in &self.sets {
                                let r = self.prog.resume_set(s);
                                let pct = r.pct_maitrise as f32;
                                let couleur = if pct >= 80.0 {
                                    GREEN
                                } else if pct >= 40.0 {
                                    YELLOW
                                } else {
                                    RED
                                };
                                Frame::NONE
                                    .fill(BG2)
                                    .inner_margin(Margin::symmetric(18, 10))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.label(
                                            RichText::new(&s.titre).font(gras(17.0)).color(ACCENT),
                                        );
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new("Maîtrise")
                                                    .size(F_MED)
                                                    .color(couleur),
                                            );
                                            ui.add_space(8.0);
                                            barre(ui, 260.0, 10.0, pct / 100.0, couleur);
                                            ui.add_space(8.0);
                                            ui.label(
                                                RichText::new(format!("{pct:.0}%"))
                                                    .size(F_MED)
                                                    .color(couleur),
                                            );
                                        });
                                        let mut parts = vec![
                                            format!("Vues : {}/{}", r.rencontrees, r.total),
                                            format!("À réviser : {}", r.a_reviser),
                                        ];
                                        if r.rencontrees > 0 {
                                            parts.push(format!("Précision : {:.0}%", r.precision));
                                        }
                                        ui.add_space(2.0);
                                        ui.label(
                                            RichText::new(parts.join("   "))
                                                .size(F_SMALL)
                                                .color(FG),
                                        );
                                        let detail = r
                                            .repartition
                                            .iter()
                                            .enumerate()
                                            .map(|(b, n)| format!("B{b}:{n}"))
                                            .collect::<Vec<_>>()
                                            .join("  ");
                                        ui.label(
                                            RichText::new(format!(
                                                "Leitner : {detail}  (B0=difficile … B5=maîtrisé)"
                                            ))
                                            .size(F_SMALL)
                                            .color(MUTED),
                                        );
                                    });
                                ui.add_space(10.0);
                            }
                        });
                });
        });
        if retour {
            self.aller_menu();
        }
    }

    // -----------------------------------------------------------------------
    // Dialogues modaux
    // -----------------------------------------------------------------------
    fn dialogues(&mut self, ctx: &egui::Context) {
        let Some(dialogue) = &mut self.dialogue else {
            return;
        };
        let mut fermer = false;
        let mut confirme: Option<ActionConfirmee> = None;
        let mut nombre: Option<usize> = None;

        let cadre = Frame::NONE
            .fill(BG)
            .stroke(Stroke::new(1.0, BG3))
            .inner_margin(Margin::same(20))
            .corner_radius(6);
        Modal::new(egui::Id::unique("dialogue"))
            .frame(cadre)
            .show(ctx, |ui| {
                ui.set_max_width(460.0);
                match dialogue {
                    Dialogue::Info { titre, message } => {
                        ui.label(
                            RichText::new(titre.as_str())
                                .font(gras(F_MED))
                                .color(ACCENT),
                        );
                        ui.add_space(10.0);
                        ui.label(RichText::new(message.as_str()).size(F_MED).color(FG));
                        ui.add_space(14.0);
                        let ok = bouton(ui, "OK", ACCENT).clicked();
                        if ok
                            || ui.input_mut(|i| {
                                i.consume_key(Default::default(), Key::Enter)
                                    | i.consume_key(Default::default(), Key::Escape)
                                    | i.consume_key(Default::default(), Key::Space)
                            })
                        {
                            fermer = true;
                        }
                    }
                    Dialogue::Confirmer { titre, message, .. } => {
                        ui.label(
                            RichText::new(titre.as_str())
                                .font(gras(F_MED))
                                .color(ACCENT),
                        );
                        ui.add_space(10.0);
                        ui.label(RichText::new(message.as_str()).size(F_MED).color(FG));
                        ui.add_space(14.0);
                        let (mut oui, mut non) = (false, false);
                        ui.horizontal(|ui| {
                            oui = bouton(ui, "Oui  (O)", ACCENT).clicked();
                            ui.add_space(10.0);
                            non = bouton(ui, "Non  (N)", BG3).clicked();
                        });
                        ui.input_mut(|i| {
                            let texte: String = i
                                .events
                                .iter()
                                .filter_map(|e| {
                                    if let egui::Event::Text(t) = e {
                                        Some(t.to_lowercase())
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            oui |= i.consume_key(Default::default(), Key::Enter)
                                || texte.contains('o')
                                || texte.contains('y');
                            non |= i.consume_key(Default::default(), Key::Escape)
                                || texte.contains('n');
                        });
                        if oui {
                            let Dialogue::Confirmer { action, .. } = std::mem::replace(
                                dialogue,
                                Dialogue::Info {
                                    titre: String::new(),
                                    message: String::new(),
                                },
                            ) else {
                                unreachable!()
                            };
                            confirme = Some(action);
                            fermer = true;
                        } else if non {
                            fermer = true;
                        }
                    }
                    Dialogue::Nombre {
                        max, saisie, init, ..
                    } => {
                        ui.label(
                            RichText::new("Nombre de questions")
                                .font(gras(F_MED))
                                .color(ACCENT),
                        );
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(format!("Combien de questions ? (max {max})"))
                                .size(F_MED)
                                .color(FG),
                        );
                        ui.add_space(8.0);
                        let sortie = egui::TextEdit::singleline(saisie)
                            .font(FontId::proportional(F_LARGE))
                            .align(Align2::CENTER_CENTER)
                            .desired_width(140.0)
                            .background_color(BG2)
                            .text_color(FG)
                            .margin(Margin::symmetric(8, 6))
                            .show(ui);
                        if *init {
                            // Focus + sélection complète : taper un nombre remplace la valeur proposée.
                            sortie.response.request_focus();
                            let mut etat = sortie.state.clone();
                            etat.cursor
                                .set_char_range(Some(egui::text::CCursorRange::two(
                                    egui::text::CCursor::new(0),
                                    egui::text::CCursor::new(saisie.chars().count()),
                                )));
                            etat.store(ui.ctx(), sortie.response.id);
                            *init = false;
                        }
                        ui.add_space(12.0);
                        let mut ok = bouton(ui, "OK", ACCENT).clicked();
                        ok |=
                            sortie.response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        if ok {
                            let n = saisie
                                .trim()
                                .parse::<usize>()
                                .ok()
                                .filter(|&n| n > 0)
                                .unwrap_or(*max);
                            nombre = Some(n.min(*max));
                            fermer = true;
                        } else if ui.input_mut(|i| i.consume_key(Default::default(), Key::Escape)) {
                            fermer = true;
                        }
                    }
                }
            });

        if !fermer {
            return;
        }
        let dialogue = self.dialogue.take();
        if let (Some(n), Some(Dialogue::Nombre { action, .. })) = (nombre, dialogue) {
            match action {
                ActionNombre::Session(set, mut indices) => {
                    indices.truncate(n);
                    self.lancer_quiz(indices.into_iter().map(|i| (Arc::clone(&set), i)).collect());
                }
                ActionNombre::Revision(mut paires) => {
                    paires.truncate(n);
                    self.lancer_quiz(paires);
                }
                ActionNombre::Langue(langue, mode) => self.lancer_session_langue(langue, mode, n),
            }
        }
        match confirme {
            Some(ActionConfirmee::Reinitialiser(set)) => {
                self.prog.reinitialiser_set(&set.id);
                self.sauvegarder();
                self.aller_menu();
                if self.dialogue.is_none() {
                    self.info("Réinitialisé", "Progression effacée.");
                }
            }
            Some(ActionConfirmee::QuitterQuiz) => self.aller_menu(),
            Some(ActionConfirmee::ReinitialiserLangue(langue)) => {
                for s in crate::langues::sets_de(&self.langues, &langue) {
                    self.prog.reinitialiser_set(&s.id);
                }
                self.sauvegarder();
                self.info("Réinitialisé", "Progression effacée.");
            }
            Some(ActionConfirmee::QuitterQuizLangue(langue)) => self.aller_langue(langue),
            None => {}
        }
        ctx.request_repaint();
    }
}

impl eframe::App for RevisionGui {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        // Les touches ne pilotent l'écran que si aucun dialogue n'est ouvert.
        let actif = self.dialogue.is_none();
        match &self.ecran {
            Ecran::MenuPrincipal { .. } => self.ecran_menu(ui, actif),
            Ecran::Categories { .. } => self.ecran_categories(ui, actif),
            Ecran::SetsCategorie { .. } => self.ecran_sets(ui, actif),
            Ecran::ModeSession { .. } => self.ecran_mode(ui, actif),
            Ecran::Quiz(_) => self.ecran_quiz(ui, actif),
            Ecran::Resultat { .. } => self.ecran_resultat(ui, actif),
            Ecran::Statistiques => self.ecran_statistiques(ui, actif),
            Ecran::Langues { .. } => self.ecran_langues(ui, actif),
            Ecran::LangueMenu { .. } => self.ecran_langue_menu(ui, actif),
            Ecran::QuizLangue(_) => self.ecran_quiz_langue(ui, actif),
            Ecran::StatsLangue { .. } => self.ecran_stats_langue(ui, actif),
        }
        let ctx = ui.ctx().clone();
        self.dialogues(&ctx);
    }
}

// ---------------------------------------------------------------------------
// Widgets
// ---------------------------------------------------------------------------
/// Panneau central aux couleurs du thème.
fn centre() -> egui::CentralPanel {
    egui::CentralPanel::default().frame(Frame::NONE.fill(BG))
}

/// Barre de progression arrondie.
fn barre(ui: &mut Ui, largeur: f32, hauteur: f32, fraction: f32, couleur: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(largeur, hauteur), Sense::hover());
    let rayon = CornerRadius::same((hauteur / 2.0) as u8);
    ui.painter().rect_filled(rect, rayon, BG3);
    let f = fraction.clamp(0.0, 1.0);
    if f > 0.0 {
        let plein = Rect::from_min_size(rect.min, vec2((rect.width() * f).max(hauteur), hauteur));
        ui.painter().rect_filled(plein, rayon, couleur);
    }
}

fn bouton(ui: &mut Ui, texte: &str, fond: Color32) -> egui::Response {
    let txt = RichText::new(texte).size(F_MED).color(BG);
    ui.add(
        egui::Button::new(txt)
            .fill(fond)
            .corner_radius(4)
            .min_size(vec2(0.0, 40.0)),
    )
    .on_hover_cursor(CursorIcon::PointingHand)
}

fn petit_bouton(ui: &mut Ui, texte: &str) -> egui::Response {
    let txt = RichText::new(texte).size(F_SMALL).color(FG);
    ui.add(
        egui::Button::new(txt)
            .fill(BG3)
            .corner_radius(3)
            .min_size(vec2(0.0, 28.0)),
    )
    .on_hover_cursor(CursorIcon::PointingHand)
}

/// Carte d'un choix de QCM/QCR : lettre + texte (avec formules), cliquable.
fn carte_choix(
    ui: &mut Ui,
    l: char,
    texte: &str,
    fond: Color32,
    couleur: Color32,
    cliquable: bool,
) -> egui::Response {
    let largeur = ui.available_width();
    let col_lettre = 52.0;
    let marge = 12.0;
    let para = math::disposer(ui, texte, F_MED, largeur - col_lettre - marge);
    let hauteur = (para.taille.y + 2.0 * marge).max(48.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(largeur, hauteur), Sense::click());
    if ui.is_rect_visible(rect) {
        let fond = if cliquable && resp.hovered() && fond == BG2 {
            BG3
        } else {
            fond
        };
        ui.painter().rect_filled(rect, 4, fond);
        ui.painter().text(
            pos2(rect.left() + col_lettre / 2.0, rect.center().y),
            Align2::CENTER_CENTER,
            format!("{l}."),
            FontId::proportional(F_MED),
            couleur,
        );
        let y = rect.top() + (hauteur - para.taille.y) / 2.0;
        para.peindre(
            ui.painter(),
            Pos2::new(rect.left() + col_lettre, y),
            couleur,
        );
    }
    if cliquable {
        resp.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        resp
    }
}

// ---------------------------------------------------------------------------
// Thème et polices
// ---------------------------------------------------------------------------
fn configurer_style(ctx: &egui::Context) {
    // Polices embarquées dans l'exécutable : DejaVu Sans couvre le grec et les
    // symboles mathématiques (∑ ∫ √ ≈ ∞ ℝ ℱ …), comme le moteur mathtext de matplotlib.
    let mut polices = FontDefinitions::default();
    polices.font_data.insert(
        "DejaVuSans".into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/DejaVuSans.ttf"
        ))),
    );
    polices.font_data.insert(
        "DejaVuSans-Bold".into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/DejaVuSans-Bold.ttf"
        ))),
    );
    polices
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "DejaVuSans".into());
    let mut famille_gras = vec!["DejaVuSans-Bold".to_string(), "DejaVuSans".to_string()];
    famille_gras.extend(
        polices.families[&FontFamily::Proportional]
            .iter()
            .skip(1)
            .cloned(),
    );
    polices
        .families
        .insert(FontFamily::Name("Gras".into()), famille_gras);
    ctx.set_fonts(polices);

    ctx.set_visuals(egui::Visuals::dark());
    ctx.global_style_mut(|style| {
        let v = &mut style.visuals;
        v.panel_fill = BG;
        v.window_fill = BG;
        v.extreme_bg_color = BG2;
        v.override_text_color = Some(FG);
        v.selection.bg_fill = ACCENT.linear_multiply(0.5);
        v.selection.stroke = Stroke::new(1.0, ACCENT);
        v.text_cursor.stroke = Stroke::new(2.0, FG);
        v.widgets.inactive.weak_bg_fill = BG3;
        v.widgets.hovered.weak_bg_fill = BG3;
        style.spacing.item_spacing = vec2(8.0, 6.0);
        style.spacing.button_padding = vec2(16.0, 8.0);
        style.spacing.scroll.bar_width = 8.0;
    });
}

pub fn lancer_gui() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Révision QCM")
            .with_inner_size(Vec2::new(920.0, 660.0))
            .with_min_inner_size(Vec2::new(700.0, 500.0)),
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "Révision QCM",
        options,
        Box::new(|cc| Ok(Box::new(RevisionGui::new(cc)))),
    )
}
