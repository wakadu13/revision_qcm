//! Module Langues : vocabulaire, conjugaison et grammaire.
//!
//! Il applique les techniques dont l'efficacité est la mieux établie par la
//! recherche sur la mémoire et l'acquisition des langues :
//!
//! - **Répétition espacée optimisée (FSRS)** : chaque élément revient juste avant
//!   d'être oublié (voir [`crate::fsrs`]).
//! - **Rappel actif en production** : on tape la réponse au lieu de la
//!   reconnaître ; produire un mot ou une forme verbale ancre bien mieux que le
//!   choisir dans une liste.
//! - **Facettes multiples d'un même mot** : reconnaissance (langue → français),
//!   puis, une fois le mot découvert, production (français → langue) et rappel
//!   en contexte (phrase à compléter). Les facettes d'un même mot ne sont jamais
//!   posées dans la même session.
//! - **Conjugaison personne par personne** : chaque forme est un élément suivi
//!   séparément, le tableau complet est montré à la correction.
//! - **Grammaire par la pratique** : phrases à trous, remise en ordre de mots,
//!   correction d'erreurs, en plus des QCM.
//! - **Entrelacement** : vocabulaire, conjugaison et grammaire sont mélangés au
//!   sein d'une session (meilleure discrimination et rétention qu'en blocs).
//! - **Correction immédiate et réessai** : une erreur est expliquée (différences
//!   surlignées) puis l'exercice revient quelques questions plus tard.
//! - **Tolérance raisonnable** : pour le vocabulaire, un accent manquant ou une
//!   faute de frappe compte comme « presque » (note Difficile) plutôt qu'un échec.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use chrono::{NaiveDateTime, TimeDelta};
use serde_json::{Map, Value};

use crate::core::{self, FichierJson, Progression, StatsQuestion, TypeQuestion, en_texte};
use crate::fsrs::{self, Note};

// ===========================================================================
// Modèle
// ===========================================================================
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Genre {
    Vocabulaire,
    Conjugaison,
    Grammaire,
}

impl Genre {
    pub const TOUS: [Genre; 3] = [Genre::Vocabulaire, Genre::Conjugaison, Genre::Grammaire];

    pub fn nom(self) -> &'static str {
        match self {
            Genre::Vocabulaire => "Vocabulaire",
            Genre::Conjugaison => "Conjugaison",
            Genre::Grammaire => "Grammaire",
        }
    }
}

/// Tableau de conjugaison affiché à la correction.
#[derive(Debug, Clone)]
pub struct Tableau {
    pub titre: String,
    pub lignes: Vec<(String, String)>,
    pub surligne: usize,
}

#[derive(Debug, Clone)]
pub enum Forme {
    /// Carte recto/verso auto-évaluée (4 niveaux).
    Carte { recto: String, verso: String },
    /// Réponse à taper.
    Saisie {
        consigne: String,
        /// Libellé affiché devant le champ (ex. « he / she / it »).
        invite: String,
        reponses: Vec<String>,
        /// Tolérance aux accents et fautes de frappe (vocabulaire).
        tolerant: bool,
        /// Texte initial du champ (correction d'erreur : la phrase fautive).
        prerempli: String,
        tableau: Option<Tableau>,
    },
    /// Choix parmi des propositions (QCM / QCR).
    Choix {
        enonce: String,
        choix: Vec<String>,
        reponses: Vec<usize>,
        multiple: bool,
    },
    /// Remettre des mots dans l'ordre.
    Ordre {
        consigne: String,
        mots: Vec<String>,
        reponses: Vec<String>,
    },
}

/// Un exercice : une facette d'un élément (mot, forme verbale, point de grammaire).
#[derive(Debug, Clone)]
pub struct Exercice {
    /// Clé de suivi dans progression.json (ex. "voc-001#prod").
    pub id: String,
    /// Élément d'origine : ses facettes sont limitées par session.
    pub entree: String,
    pub genre: Genre,
    /// Type d'exercice affiché (ex. « Vocabulaire · production »).
    pub etiquette: String,
    /// Résumé court (statistiques, points faibles).
    pub resume: String,
    pub forme: Forme,
    /// Informations montrées après la réponse (exemple, traduction, règle…).
    pub details: Vec<String>,
    /// Facette à maîtriser avant de proposer celle-ci.
    pub prerequis: Option<String>,
    /// Nombre maximal de facettes de la même entrée dans une session.
    pub max_par_session: usize,
}

/// Un fichier de langue (`"langue": "..."`).
#[derive(Debug, Clone)]
pub struct SetLangue {
    pub id: String,
    pub langue: String,
    pub titre: String,
    pub exercices: Vec<Exercice>,
    /// Mots facultatifs en tête de réponse (ex. « to » pour les verbes anglais).
    pub particules: Vec<String>,
    /// Caractères spéciaux proposés sous le champ de saisie (ex. « ñ », « ß »).
    pub caracteres_speciaux: Vec<String>,
    /// Nombre maximal de nouveaux éléments découverts par jour.
    pub nouveaux_par_jour: Option<usize>,
}

/// Quota par défaut de nouveaux éléments par jour : au-delà, la charge de
/// révisions des jours suivants devient difficile à tenir.
pub const NOUVEAUX_PAR_JOUR: usize = 20;

// ===========================================================================
// Chargement
// ===========================================================================
/// Vrai si le JSON décrit un fichier de langue plutôt qu'un set de questions.
pub fn est_fichier_langue(data: &Value) -> bool {
    data.get("langue").is_some() && data.get("questions").is_none()
}

/// Chaîne ou liste de chaînes → liste (vide si absent).
fn textes(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Array(a)) => a
            .iter()
            .map(en_texte)
            .filter(|s| !s.trim().is_empty())
            .collect(),
        Some(Value::Null) | None => Vec::new(),
        Some(autre) => {
            let s = en_texte(autre);
            if s.trim().is_empty() {
                Vec::new()
            } else {
                vec![s]
            }
        }
    }
}

fn texte_opt(o: &Map<String, Value>, clef: &str) -> String {
    o.get(clef).map(en_texte).unwrap_or_default()
}

fn objet<'a>(v: &'a Value, contexte: &str) -> Result<&'a Map<String, Value>, String> {
    v.as_object()
        .ok_or_else(|| format!("{contexte} : un objet JSON est attendu."))
}

fn requis(o: &Map<String, Value>, clef: &str, contexte: &str) -> Result<String, String> {
    let s = o
        .get(clef)
        .map(en_texte)
        .ok_or_else(|| format!("{contexte} : clé '{clef}' manquante."))?;
    if s.trim().is_empty() {
        return Err(format!("{contexte} : '{clef}' ne doit pas être vide."));
    }
    Ok(s)
}

fn liste_section<'a>(data: &'a Value, clef: &str, nom: &str) -> Result<&'a [Value], String> {
    match data.get(clef) {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(a)) => Ok(a),
        Some(_) => Err(format!("{nom} : '{clef}' doit être une liste.")),
    }
}

/// Remplace la partie entre crochets d'un exemple par un blanc.
/// « She [achieved] her goal. » → (« She ____ her goal. », « achieved »).
fn trouer(exemple: &str) -> Option<(String, String)> {
    let debut = exemple.find('[')?;
    let fin = debut + exemple[debut..].find(']')?;
    let mot = exemple[debut + 1..fin].to_string();
    if mot.trim().is_empty() {
        return None;
    }
    Some((
        format!("{}_____{}", &exemple[..debut], &exemple[fin + 1..]),
        mot,
    ))
}

fn sans_crochets(s: &str) -> String {
    s.replace(['[', ']'], "")
}

pub fn set_depuis_json(f: FichierJson) -> Result<SetLangue, String> {
    let FichierJson {
        nom, stem, data, ..
    } = f;
    let langue = en_texte(&data["langue"]);
    if langue.trim().is_empty() {
        return Err(format!("{nom} : 'langue' ne doit pas être vide."));
    }
    let maternelle = data
        .get("langue_maternelle")
        .map(en_texte)
        .unwrap_or_else(|| "Français".to_string());
    let (abr_l, abr_m) = (abreviation(&langue), abreviation(&maternelle));
    let mut exercices = Vec::new();

    // --- Vocabulaire ---
    for (i, v) in liste_section(&data, "vocabulaire", &nom)?
        .iter()
        .enumerate()
    {
        let contexte = format!("{nom} » vocabulaire #{}", i + 1);
        let o = objet(v, &contexte)?;
        let mot = requis(o, "mot", &contexte)?;
        let trads = textes(o.get("traductions").or_else(|| o.get("traduction")));
        if trads.is_empty() {
            return Err(format!("{contexte} : clé 'traductions' manquante ou vide."));
        }
        let id = o
            .get("id")
            .map(en_texte)
            .unwrap_or_else(|| format!("voc{}", i + 1));
        let trad = trads.join(", ");
        let exemple = texte_opt(o, "exemple");
        let ex_trad = texte_opt(o, "exemple_traduction");
        let prononciation = texte_opt(o, "prononciation");
        let mut reponses = vec![mot.clone()];
        reponses.extend(textes(o.get("variantes")));

        let mut details = Vec::new();
        if !exemple.is_empty() {
            let mut d = format!("Ex. : « {} »", sans_crochets(&exemple));
            if !ex_trad.is_empty() {
                d.push_str(&format!(" = « {ex_trad} »"));
            }
            details.push(d);
        }
        for clef in ["note", "explication"] {
            let n = texte_opt(o, clef);
            if !n.is_empty() {
                details.push(n);
            }
        }

        let id_rec = format!("{id}#rec");
        let recto = if prononciation.is_empty() {
            mot.clone()
        } else {
            format!("{mot}   {prononciation}")
        };
        exercices.push(Exercice {
            id: id_rec.clone(),
            entree: id.clone(),
            genre: Genre::Vocabulaire,
            etiquette: format!("Vocabulaire · reconnaissance ({abr_l} → {abr_m})"),
            resume: format!("{mot} → {trad}"),
            forme: Forme::Carte {
                recto,
                verso: trad.clone(),
            },
            details: details.clone(),
            prerequis: None,
            max_par_session: 1,
        });
        exercices.push(Exercice {
            id: format!("{id}#prod"),
            entree: id.clone(),
            genre: Genre::Vocabulaire,
            etiquette: format!("Vocabulaire · production ({abr_m} → {abr_l})"),
            resume: format!("{trad} → {mot}"),
            forme: Forme::Saisie {
                consigne: format!("Comment dit-on « {trad} » en {} ?", langue.to_lowercase()),
                invite: String::new(),
                reponses: reponses.clone(),
                tolerant: true,
                prerempli: String::new(),
                tableau: None,
            },
            details: details.clone(),
            prerequis: Some(id_rec.clone()),
            max_par_session: 1,
        });
        if let Some((phrase, attendu)) = trouer(&exemple) {
            let mut consigne = format!("Complète : {phrase}");
            consigne.push_str(&format!("\n(indice : {trad})"));
            exercices.push(Exercice {
                id: format!("{id}#ctx"),
                entree: id.clone(),
                genre: Genre::Vocabulaire,
                etiquette: "Vocabulaire · en contexte".to_string(),
                resume: format!("{mot} (en contexte)"),
                forme: Forme::Saisie {
                    consigne,
                    invite: String::new(),
                    reponses: attendu.split('|').map(|s| s.trim().to_string()).collect(),
                    tolerant: true,
                    prerempli: String::new(),
                    tableau: None,
                },
                details,
                prerequis: Some(id_rec),
                max_par_session: 1,
            });
        }
    }

    // --- Conjugaison ---
    for (i, c) in liste_section(&data, "conjugaison", &nom)?
        .iter()
        .enumerate()
    {
        let contexte = format!("{nom} » conjugaison #{}", i + 1);
        let o = objet(c, &contexte)?;
        let verbe = requis(o, "verbe", &contexte)?;
        let temps = requis(o, "temps", &contexte)?;
        let formes = o
            .get("formes")
            .and_then(Value::as_object)
            .filter(|f| !f.is_empty())
            .ok_or_else(|| {
                format!("{contexte} : clé 'formes' (objet personne → forme) manquante.")
            })?;
        let id = o
            .get("id")
            .map(en_texte)
            .unwrap_or_else(|| format!("conj{}", i + 1));
        let traduction = texte_opt(o, "traduction");
        let explication = texte_opt(o, "explication");
        let lignes: Vec<(String, Vec<String>)> = formes
            .iter()
            .map(|(p, f)| (p.clone(), textes(Some(f))))
            .collect();
        if let Some((p, _)) = lignes.iter().find(|(_, f)| f.is_empty()) {
            return Err(format!("{contexte} : forme vide pour « {p} »."));
        }
        let titre_verbe = if traduction.is_empty() {
            format!("« {verbe} »")
        } else {
            format!("« {verbe} » ({traduction})")
        };
        let tableau_lignes: Vec<(String, String)> = lignes
            .iter()
            .map(|(p, f)| (p.clone(), f[0].clone()))
            .collect();
        for (k, (personne, formes)) in lignes.iter().enumerate() {
            exercices.push(Exercice {
                id: format!("{id}#{personne}"),
                entree: id.clone(),
                genre: Genre::Conjugaison,
                etiquette: format!("Conjugaison · {temps}"),
                resume: format!("{verbe} ({temps}) — {personne}"),
                forme: Forme::Saisie {
                    consigne: format!("Conjugue {titre_verbe} — {temps}"),
                    invite: personne.clone(),
                    reponses: formes.clone(),
                    tolerant: false,
                    prerempli: String::new(),
                    tableau: Some(Tableau {
                        titre: format!("{verbe} — {temps}"),
                        lignes: tableau_lignes.clone(),
                        surligne: k,
                    }),
                },
                details: if explication.is_empty() {
                    Vec::new()
                } else {
                    vec![explication.clone()]
                },
                prerequis: None,
                max_par_session: 2,
            });
        }
    }

    // --- Grammaire ---
    for (i, g) in liste_section(&data, "grammaire", &nom)?.iter().enumerate() {
        let contexte = format!("{nom} » grammaire #{}", i + 1);
        let o = objet(g, &contexte)?;
        let type_g = o
            .get("type")
            .map(en_texte)
            .unwrap_or_else(|| "QCM".into())
            .to_uppercase();
        let id = o
            .get("id")
            .map(en_texte)
            .unwrap_or_else(|| format!("gram{}", i + 1));
        let explication = texte_opt(o, "explication");
        let mut details = Vec::new();
        let traduction = texte_opt(o, "traduction");
        let (etiquette, resume, forme) = match type_g.as_str() {
            "ORDRE" => {
                let phrase = requis(o, "phrase", &contexte)?;
                let mut reponses = vec![phrase.clone()];
                reponses.extend(textes(o.get("variantes")));
                let mots = mots_a_ordonner(&phrase);
                if mots.len() < 2 {
                    return Err(format!(
                        "{contexte} : la phrase doit contenir au moins 2 mots."
                    ));
                }
                let consigne = if traduction.is_empty() {
                    "Remets les mots dans l'ordre.".to_string()
                } else {
                    format!("Remets les mots dans l'ordre : « {traduction} »")
                };
                (
                    "Grammaire · remise en ordre",
                    phrase,
                    Forme::Ordre {
                        consigne,
                        mots,
                        reponses,
                    },
                )
            }
            "CORRECTION" => {
                let phrase = requis(o, "phrase", &contexte)?;
                let corrections = textes(o.get("corrections").or_else(|| o.get("correction")));
                if corrections.is_empty() {
                    return Err(format!("{contexte} : clé 'corrections' manquante ou vide."));
                }
                let mut consigne =
                    format!("Cette phrase contient une erreur. Corrige-la :\n« {phrase} »");
                if !traduction.is_empty() {
                    consigne.push_str(&format!("\n(sens : « {traduction} »)"));
                }
                (
                    "Grammaire · correction d'erreur",
                    phrase.clone(),
                    Forme::Saisie {
                        consigne,
                        invite: String::new(),
                        reponses: corrections,
                        tolerant: false,
                        prerempli: phrase,
                        tableau: None,
                    },
                )
            }
            _ => {
                let q = core::question_depuis_json(g, i + 1, &contexte)?;
                if !traduction.is_empty() {
                    details.push(format!("Traduction : « {traduction} »"));
                }
                match q.type_q {
                    TypeQuestion::Trou => (
                        "Grammaire · phrase à trous",
                        q.enonce.clone(),
                        Forme::Saisie {
                            consigne: q.enonce,
                            invite: String::new(),
                            reponses: q.reponses_texte,
                            tolerant: false,
                            prerempli: String::new(),
                            tableau: None,
                        },
                    ),
                    TypeQuestion::Flashcard => (
                        "Grammaire · carte",
                        q.enonce.clone(),
                        Forme::Carte {
                            recto: q.enonce,
                            verso: q.verso,
                        },
                    ),
                    TypeQuestion::Qcm | TypeQuestion::Qcr => (
                        if q.est_qcr() {
                            "Grammaire · QCR (plusieurs réponses)"
                        } else {
                            "Grammaire · QCM"
                        },
                        q.enonce.clone(),
                        Forme::Choix {
                            multiple: q.est_qcr(),
                            enonce: q.enonce,
                            choix: q.choix,
                            reponses: q.reponses,
                        },
                    ),
                }
            }
        };
        if !explication.is_empty() {
            details.push(explication);
        }
        exercices.push(Exercice {
            id: id.clone(),
            entree: id,
            genre: Genre::Grammaire,
            etiquette: etiquette.to_string(),
            resume: resume.chars().take(70).collect(),
            forme,
            details,
            prerequis: None,
            max_par_session: 1,
        });
    }

    if exercices.is_empty() {
        return Err(format!(
            "{nom} : aucun élément ('vocabulaire', 'conjugaison' ou 'grammaire')."
        ));
    }
    let mut ids = std::collections::HashSet::new();
    if let Some(doublon) = exercices.iter().find(|e| !ids.insert(e.id.as_str())) {
        return Err(format!("{nom} : identifiant en double « {} ».", doublon.id));
    }

    Ok(SetLangue {
        id: stem.clone(),
        titre: data.get("titre").map(en_texte).unwrap_or(stem),
        langue,
        exercices,
        particules: textes(data.get("particules_optionnelles"))
            .iter()
            .map(|p| p.to_lowercase())
            .collect(),
        caracteres_speciaux: textes(data.get("caracteres_speciaux")),
        nouveaux_par_jour: data
            .get("nouveaux_par_jour")
            .and_then(Value::as_u64)
            .map(|n| n as usize),
    })
}

/// « Anglais » → « EN », « Français » → « FR » (deux premières lettres).
fn abreviation(langue: &str) -> String {
    match langue.to_lowercase().as_str() {
        "anglais" | "english" => "EN".into(),
        "français" | "francais" | "french" => "FR".into(),
        "espagnol" | "español" | "spanish" => "ES".into(),
        "allemand" | "deutsch" | "german" => "DE".into(),
        "italien" | "italiano" | "italian" => "IT".into(),
        "portugais" | "português" => "PT".into(),
        autre => autre
            .chars()
            .filter(|c| c.is_alphabetic())
            .take(2)
            .collect::<String>()
            .to_uppercase(),
    }
}

/// Mots d'une phrase à remettre en ordre (ponctuation finale retirée, première
/// lettre en minuscule pour ne pas donner d'indice, sauf « I »).
fn mots_a_ordonner(phrase: &str) -> Vec<String> {
    let nette = phrase.trim().trim_end_matches(['.', '!', '?', '…']).trim();
    let mut mots: Vec<String> = nette.split_whitespace().map(String::from).collect();
    if let Some(premier) = mots.first_mut()
        && premier != "I"
        && !premier.starts_with("I'")
    {
        // Première lettre (après une éventuelle ponctuation ouvrante : « ¿Dónde »).
        *premier = match premier.char_indices().find(|(_, c)| c.is_alphabetic()) {
            Some((i, c)) => format!(
                "{}{}{}",
                &premier[..i],
                c.to_lowercase(),
                &premier[i + c.len_utf8()..]
            ),
            None => premier.clone(),
        };
    }
    mots
}

/// Langues distinctes, triées.
pub fn langues(sets: &[Arc<SetLangue>]) -> Vec<String> {
    let mut l: Vec<String> = sets.iter().map(|s| s.langue.clone()).collect();
    l.sort();
    l.dedup();
    l
}

pub fn sets_de(sets: &[Arc<SetLangue>], langue: &str) -> Vec<Arc<SetLangue>> {
    sets.iter()
        .filter(|s| s.langue == langue)
        .cloned()
        .collect()
}

// ===========================================================================
// Correction des réponses
// ===========================================================================
/// Résultat d'une réponse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Juste,
    /// Juste à un accent près (vocabulaire).
    Accents,
    /// Juste à une faute de frappe près (vocabulaire).
    Faute,
    Faux,
}

impl Verdict {
    pub fn note(self) -> Note {
        match self {
            Verdict::Juste => Note::Bien,
            Verdict::Accents | Verdict::Faute => Note::Difficile,
            Verdict::Faux => Note::ARevoir,
        }
    }

    pub fn message(self) -> &'static str {
        match self {
            Verdict::Juste => "✓ Correct !",
            Verdict::Accents => "≈ Presque : attention aux accents",
            Verdict::Faute => "≈ Presque : faute de frappe",
            Verdict::Faux => "✗ Incorrect",
        }
    }
}

/// Normalisation stricte : casse, espaces, apostrophes typographiques et
/// ponctuation finale ignorés ; accents conservés.
pub fn normaliser_strict(s: &str) -> String {
    let s = s
        .trim()
        .to_lowercase()
        .replace(['’', '‘', '`', 'ʼ'], "'")
        .replace(['“', '”', '«', '»'], "\"");
    let s = s.trim_end_matches(|c: char| ".!?…;:".contains(c) || c.is_whitespace());
    let mut res = String::with_capacity(s.len());
    for mot in s.split_whitespace() {
        // Espace typographique avant « ? ! : ; » (usage français) ignoré.
        if !res.is_empty() && !(mot.len() == 1 && "?!:;,".contains(mot)) {
            res.push(' ');
        }
        res.push_str(mot);
    }
    res
}

fn sans_particule(s: &str, particules: &[String]) -> String {
    match s.split_once(' ') {
        Some((tete, reste)) if particules.iter().any(|p| p == tete) => reste.to_string(),
        _ => s.to_string(),
    }
}

/// Distance d'édition (Levenshtein avec transpositions adjacentes).
pub fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, ligne) in d.iter_mut().enumerate() {
        ligne[0] = i;
    }
    for (j, case) in d[0].iter_mut().enumerate() {
        *case = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cout = usize::from(a[i - 1] != b[j - 1]);
            d[i][j] = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cout);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d[i][j] = d[i][j].min(d[i - 2][j - 2] + 1);
            }
        }
    }
    d[a.len()][b.len()]
}

/// Note une réponse tapée. Renvoie le verdict et la réponse attendue la plus
/// proche de la saisie (pour l'afficher à la correction).
pub fn noter_saisie(
    saisie: &str,
    reponses: &[String],
    tolerant: bool,
    particules: &[String],
) -> (Verdict, String) {
    let s = sans_particule(&normaliser_strict(saisie), particules);
    let s_plat = core::normaliser_texte(&s);
    let mut meilleur = (
        Verdict::Faux,
        usize::MAX,
        reponses.first().cloned().unwrap_or_default(),
    );
    for r in reponses {
        let a = sans_particule(&normaliser_strict(r), particules);
        if a == s {
            return (Verdict::Juste, r.clone());
        }
        let a_plat = core::normaliser_texte(&a);
        let dist = distance(&s_plat, &a_plat);
        let verdict = if !tolerant {
            Verdict::Faux
        } else if dist == 0 {
            Verdict::Accents
        } else {
            let n = a_plat.chars().count();
            let seuil = if n >= 10 {
                2
            } else if n >= 5 {
                1
            } else {
                0
            };
            if dist <= seuil {
                Verdict::Faute
            } else {
                Verdict::Faux
            }
        };
        let rang = |v: Verdict| match v {
            Verdict::Juste => 0,
            Verdict::Accents => 1,
            Verdict::Faute => 2,
            Verdict::Faux => 3,
        };
        if (rang(verdict), dist) < (rang(meilleur.0), meilleur.1) {
            meilleur = (verdict, dist, r.clone());
        }
    }
    (meilleur.0, meilleur.2)
}

/// Caractères de `attendu`, chacun marqué `true` s'il correspond à la saisie
/// (plus longue sous-séquence commune, casse ignorée) : sert à surligner les
/// différences à la correction.
pub fn diff(saisie: &str, attendu: &str) -> Vec<(char, bool)> {
    let a: Vec<char> = attendu.chars().collect();
    let s: Vec<char> = saisie.chars().collect();
    let eq = |x: char, y: char| x.to_lowercase().eq(y.to_lowercase());
    let mut l = vec![vec![0u16; s.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..s.len()).rev() {
            l[i][j] = if eq(a[i], s[j]) {
                l[i + 1][j + 1] + 1
            } else {
                l[i + 1][j].max(l[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut res = Vec::with_capacity(a.len());
    while i < a.len() {
        if j < s.len() && eq(a[i], s[j]) {
            res.push((a[i], true));
            i += 1;
            j += 1;
        } else if j < s.len() && l[i][j + 1] >= l[i + 1][j] {
            j += 1;
        } else {
            res.push((a[i], a[i].is_whitespace()));
            i += 1;
        }
    }
    res
}

// ===========================================================================
// Suivi FSRS
// ===========================================================================
fn jours_depuis(st: &StatsQuestion, now: NaiveDateTime) -> f64 {
    st.derniere_revision
        .map_or(0.0, |d| (now - d).num_seconds().max(0) as f64 / 86_400.0)
}

fn est_nouveau(st: Option<&StatsQuestion>) -> bool {
    st.and_then(|s| s.stabilite).is_none()
}

fn est_du(st: &StatsQuestion, now: NaiveDateTime) -> bool {
    st.echeance.is_none_or(|e| e <= now)
}

fn retention(st: &StatsQuestion, now: NaiveDateTime) -> f64 {
    st.stabilite
        .map_or(0.0, |s| fsrs::retrievabilite(jours_depuis(st, now), s))
}

fn etat_suivant(st: Option<&StatsQuestion>, note: Note, now: NaiveDateTime) -> fsrs::Etat {
    match st.and_then(|s| Some((s.stabilite?, s.difficulte?, s))) {
        Some((s, d, st)) => fsrs::suivante(
            fsrs::Etat {
                stabilite: s,
                difficulte: d,
            },
            jours_depuis(st, now),
            note,
        ),
        None => fsrs::premiere(note),
    }
}

/// Délai avant la prochaine révision pour un état et une note.
fn delai(etat: fsrs::Etat, note: Note) -> TimeDelta {
    if note == Note::ARevoir {
        // Oublié : revient dès la prochaine session (et plus loin dans celle-ci).
        TimeDelta::minutes(10)
    } else {
        let jours = fsrs::intervalle_jours(etat.stabilite)
            .round()
            .clamp(1.0, 36_500.0);
        TimeDelta::days(jours as i64)
    }
}

/// Texte court d'un délai : « 10 min », « 3 j », « 2 mois », « 1,5 an ».
pub fn texte_delai(d: TimeDelta) -> String {
    let jours = d.num_days();
    if jours < 1 {
        format!("{} min", d.num_minutes())
    } else if jours < 31 {
        format!("{jours} j")
    } else if jours < 365 {
        format!("{} mois", (jours as f64 / 30.4).round())
    } else {
        format!("{:.1} an", jours as f64 / 365.0).replace('.', ",")
    }
}

/// Délai prévu si l'on donne `note` maintenant (affiché sur les boutons).
pub fn prevision(prog: &Progression, set_id: &str, exo_id: &str, note: Note) -> TimeDelta {
    let st = prog.stats_question(set_id, exo_id);
    delai(etat_suivant(st, note, core::maintenant()), note)
}

/// Enregistre une révision : statistiques, boîte de Leitner et état FSRS.
pub fn enregistrer(prog: &mut Progression, set_id: &str, exo_id: &str, note: Note) {
    let now = core::maintenant();
    let avant = prog.stats_question(set_id, exo_id);
    let deja_appris = !est_nouveau(avant);
    let decouverte = avant.and_then(|s| s.decouverte);
    let etat = etat_suivant(avant, note, now);
    prog.enregistrer_reponse(set_id, exo_id, note.reussie());
    if let Some(st) = prog.data.get_mut(set_id).and_then(|m| m.get_mut(exo_id)) {
        st.stabilite = Some((etat.stabilite * 1000.0).round() / 1000.0);
        st.difficulte = Some((etat.difficulte * 1000.0).round() / 1000.0);
        st.echeance = Some(now + delai(etat, note));
        st.decouverte = decouverte.or(if deja_appris { None } else { Some(now) });
        if deja_appris && note == Note::ARevoir {
            st.oublis += 1;
        }
    }
}

// ===========================================================================
// Sessions
// ===========================================================================
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Révisions dues + nouveaux éléments, tous genres entrelacés.
    Quotidien,
    /// Idem, limité à un genre.
    Genre(Genre),
    /// Éléments souvent oubliés ou ratés à la dernière révision.
    PointsFaibles,
}

impl Mode {
    pub fn nom(self) -> &'static str {
        match self {
            Mode::Quotidien => "Session du jour",
            Mode::Genre(g) => g.nom(),
            Mode::PointsFaibles => "Points faibles",
        }
    }
}

/// Un exercice dans une session.
#[derive(Debug, Clone)]
pub struct ExoSession {
    pub set: Arc<SetLangue>,
    pub exo: usize,
    /// 0 = premier essai ; > 0 = nouvelle tentative après une erreur.
    pub tentative: u8,
    pub nouveau: bool,
    /// Ordre de présentation des mots (exercice de remise en ordre).
    pub melange: Vec<usize>,
}

impl ExoSession {
    pub fn exercice(&self) -> &Exercice {
        &self.set.exercices[self.exo]
    }
}

fn debloque(prog: &Progression, set: &SetLangue, exo: &Exercice) -> bool {
    match &exo.prerequis {
        None => true,
        Some(p) => prog
            .stats_question(&set.id, p)
            .is_some_and(|st| st.stabilite.is_some() && st.derniere_correcte == Some(true)),
    }
}

/// Nouveaux éléments qu'il reste possible de découvrir aujourd'hui.
pub fn quota_nouveaux(sets: &[Arc<SetLangue>], prog: &Progression) -> usize {
    let aujourdhui = core::maintenant().date();
    let quota = sets
        .iter()
        .filter_map(|s| s.nouveaux_par_jour)
        .max()
        .unwrap_or(NOUVEAUX_PAR_JOUR);
    let decouverts = sets
        .iter()
        .flat_map(|s| s.exercices.iter().map(move |e| (s, e)))
        .filter(|(s, e)| {
            prog.stats_question(&s.id, &e.id)
                .and_then(|st| st.decouverte)
                .is_some_and(|d| d.date() == aujourdhui)
        })
        .count();
    quota.saturating_sub(decouverts)
}

/// Construit une session d'au plus `n` exercices.
///
/// Les révisions dues passent en premier (les plus menacées d'oubli d'abord),
/// complétées par de nouveaux éléments ; puis les genres sont entrelacés.
pub fn construire_session(
    sets: &[Arc<SetLangue>],
    prog: &Progression,
    mode: Mode,
    n: usize,
) -> Vec<ExoSession> {
    let now = core::maintenant();
    let mut revues: Vec<(f64, ExoSession)> = Vec::new();
    let mut nouveaux: Vec<ExoSession> = Vec::new();
    let mut faibles: Vec<((u32, f64), ExoSession)> = Vec::new();

    for set in sets {
        for (i, exo) in set.exercices.iter().enumerate() {
            if let Mode::Genre(g) = mode
                && exo.genre != g
            {
                continue;
            }
            let st = prog.stats_question(&set.id, &exo.id);
            let item = ExoSession {
                set: Arc::clone(set),
                exo: i,
                tentative: 0,
                nouveau: est_nouveau(st),
                melange: Vec::new(),
            };
            match st.filter(|_| !item.nouveau) {
                None => {
                    if debloque(prog, set, exo) {
                        nouveaux.push(item);
                    }
                }
                Some(st) => {
                    let r = retention(st, now);
                    if mode == Mode::PointsFaibles {
                        if st.oublis > 0 || st.derniere_correcte == Some(false) {
                            faibles.push(((st.oublis, r), item));
                        }
                    } else if est_du(st, now) {
                        revues.push((r, item));
                    }
                }
            }
        }
    }

    let candidats: Vec<ExoSession> = if mode == Mode::PointsFaibles {
        faibles.sort_by(|a, b| b.0.0.cmp(&a.0.0).then(a.0.1.total_cmp(&b.0.1)));
        faibles.into_iter().map(|(_, e)| e).collect()
    } else {
        revues.sort_by(|a, b| a.0.total_cmp(&b.0));
        // Les révisions passent avant la nouveauté, mais une session garde une part
        // de nouveaux éléments (un tiers) quand il y en a.
        let quota = quota_nouveaux(sets, prog);
        let reserve = nouveaux.len().min((n / 3).max(3)).min(quota);
        let nb_revues = revues.len().min(n.saturating_sub(reserve));
        let mut c: Vec<ExoSession> = revues.into_iter().take(nb_revues).map(|(_, e)| e).collect();
        // Nouveautés réparties entre les genres dès la sélection, et limitées au
        // quota du jour (les facettes d'une même entrée comptent séparément).
        c.extend(repartir_equitablement(nouveaux));
        c
    };

    // Limite de facettes par entrée (pas « réussir » à la production juste
    // après avoir vu la carte du même mot).
    let quota = if mode == Mode::PointsFaibles {
        0
    } else {
        quota_nouveaux(sets, prog)
    };
    let mut nb_nouveaux = 0;
    let mut par_entree: HashMap<(String, String), usize> = HashMap::new();
    let mut choisis = Vec::with_capacity(n.min(256));
    for e in candidats {
        if choisis.len() >= n {
            break;
        }
        if e.nouveau && nb_nouveaux >= quota {
            continue;
        }
        let exo = e.exercice();
        let compte = par_entree
            .entry((e.set.id.clone(), exo.entree.clone()))
            .or_insert(0);
        if *compte < exo.max_par_session {
            *compte += 1;
            nb_nouveaux += usize::from(e.nouveau);
            choisis.push(e);
        }
    }

    let mut session = entrelacer(choisis);
    for e in &mut session {
        if let Forme::Ordre { mots, .. } = &e.exercice().forme {
            e.melange = melanger_mots(mots.len());
        }
    }
    session
}

/// Permutation aléatoire, différente de l'ordre d'origine.
fn melanger_mots(n: usize) -> Vec<usize> {
    let mut m: Vec<usize> = (0..n).collect();
    for _ in 0..10 {
        fastrand::shuffle(&mut m);
        if m.iter().enumerate().any(|(i, &x)| i != x) {
            break;
        }
    }
    m
}

fn files_par_genre(items: Vec<ExoSession>) -> Vec<(Genre, VecDeque<ExoSession>)> {
    let mut files: Vec<(Genre, VecDeque<ExoSession>)> =
        Genre::TOUS.iter().map(|&g| (g, VecDeque::new())).collect();
    for e in items {
        let g = e.exercice().genre;
        files
            .iter_mut()
            .find(|(x, _)| *x == g)
            .unwrap()
            .1
            .push_back(e);
    }
    files
}

/// Nouveautés prises à tour de rôle dans chaque genre (parts égales).
fn repartir_equitablement(items: Vec<ExoSession>) -> Vec<ExoSession> {
    let mut files = files_par_genre(items);
    let mut res = Vec::new();
    while files.iter().any(|(_, f)| !f.is_empty()) {
        for (_, f) in &mut files {
            if let Some(e) = f.pop_front() {
                res.push(e);
            }
        }
    }
    res
}

/// Entrelace les genres : chaque genre est réparti uniformément sur la session
/// (on puise dans la file la moins avancée proportionnellement), et jamais deux
/// exercices du même genre à la suite tant qu'un autre genre est disponible.
fn entrelacer(items: Vec<ExoSession>) -> Vec<ExoSession> {
    let mut files = files_par_genre(items);
    let tailles: Vec<f64> = files.iter().map(|(_, f)| f.len().max(1) as f64).collect();
    let mut res: Vec<ExoSession> = Vec::new();
    let mut dernier: Option<Genre> = None;
    loop {
        let mut ordre: Vec<usize> = (0..files.len())
            .filter(|&i| !files[i].1.is_empty())
            .collect();
        if ordre.is_empty() {
            break;
        }
        // Proportion restante la plus élevée d'abord.
        ordre.sort_by(|&a, &b| {
            (files[b].1.len() as f64 / tailles[b])
                .total_cmp(&(files[a].1.len() as f64 / tailles[a]))
        });
        let i = ordre
            .iter()
            .copied()
            .find(|&i| Some(files[i].0) != dernier)
            .unwrap_or(ordre[0]);
        let file = &mut files[i].1;
        // Évite deux formes du même verbe à la suite.
        if let Some(prec) = res.last()
            && file.len() > 1
            && file[0].exercice().entree == prec.exercice().entree
        {
            file.swap(0, 1);
        }
        let e = file.pop_front().unwrap();
        dernier = Some(files[i].0);
        res.push(e);
    }
    res
}

/// Après un échec au premier essai, l'exercice revient quelques questions plus
/// tard (deux nouvelles tentatives au plus).
pub fn reprogrammer(file: &mut Vec<ExoSession>, index: usize) {
    let e = &file[index];
    if e.tentative >= 2 {
        return;
    }
    let mut copie = e.clone();
    copie.tentative += 1;
    copie.nouveau = false;
    if !copie.melange.is_empty() {
        copie.melange = melanger_mots(copie.melange.len());
    }
    let pos = (index + 4).min(file.len());
    file.insert(pos, copie);
}

/// Note d'un exercice de remise en ordre.
pub fn noter_ordre(phrase: &str, reponses: &[String]) -> (Verdict, String) {
    noter_saisie(phrase, reponses, false, &[])
}

// ===========================================================================
// Statistiques
// ===========================================================================
#[derive(Debug, Clone, Default)]
pub struct ResumeGenre {
    pub total: usize,
    pub nouveaux: usize,
    pub a_revoir: usize,
    /// Stabilité ≥ 21 jours.
    pub maitrises: usize,
    pub retention: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct ResumeLangue {
    pub genres: Vec<(Genre, ResumeGenre)>,
    pub a_revoir: usize,
    /// Nouveaux éléments découvrables aujourd'hui (débloqués, dans le quota).
    pub nouveaux_disponibles: usize,
    pub sept_jours: usize,
    /// (résumé de l'élément, nombre d'oublis), les plus difficiles d'abord.
    pub difficiles: Vec<(String, u32)>,
}

pub fn resume(sets: &[Arc<SetLangue>], prog: &Progression) -> ResumeLangue {
    let now = core::maintenant();
    let dans_7j = now + TimeDelta::days(7);
    let mut r = ResumeLangue {
        genres: Genre::TOUS
            .iter()
            .map(|&g| (g, ResumeGenre::default()))
            .collect(),
        ..Default::default()
    };
    let mut sommes = [(0.0f64, 0usize); 3];
    for set in sets {
        for exo in &set.exercices {
            let gi = Genre::TOUS.iter().position(|&g| g == exo.genre).unwrap();
            let rg = &mut r.genres[gi].1;
            rg.total += 1;
            let st = prog.stats_question(&set.id, &exo.id);
            match st.filter(|s| s.stabilite.is_some()) {
                None => {
                    rg.nouveaux += 1;
                    if debloque(prog, set, exo) {
                        r.nouveaux_disponibles += 1;
                    }
                }
                Some(st) => {
                    if est_du(st, now) {
                        rg.a_revoir += 1;
                        r.a_revoir += 1;
                    } else if st.echeance.is_some_and(|e| e <= dans_7j) {
                        r.sept_jours += 1;
                    }
                    if st.stabilite.unwrap_or(0.0) >= 21.0 {
                        rg.maitrises += 1;
                    }
                    sommes[gi].0 += retention(st, now);
                    sommes[gi].1 += 1;
                    if st.oublis >= 2 {
                        r.difficiles.push((exo.resume.clone(), st.oublis));
                    }
                }
            }
        }
    }
    for (i, (s, n)) in sommes.iter().enumerate() {
        if *n > 0 {
            r.genres[i].1.retention = Some(s / *n as f64);
        }
    }
    r.nouveaux_disponibles = r.nouveaux_disponibles.min(quota_nouveaux(sets, prog));
    r.difficiles.sort_by_key(|d| std::cmp::Reverse(d.1));
    r.difficiles.truncate(8);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_test() -> SetLangue {
        let data: Value = serde_json::from_str(
            r#"{"langue": "Anglais", "particules_optionnelles": ["to"],
            "vocabulaire": [{"id": "v1", "mot": "to achieve", "traductions": ["réussir", "accomplir"],
                             "exemple": "She [achieved] her goal."}],
            "conjugaison": [{"id": "c1", "verbe": "to be", "temps": "present simple",
                             "formes": {"I": "am", "you": "are", "he / she / it": "is"}}],
            "grammaire": [{"id": "g1", "type": "ORDRE", "phrase": "She has never been to London."},
                          {"id": "g2", "type": "CORRECTION", "phrase": "He don't like it.",
                           "corrections": ["He doesn't like it.", "He does not like it."]},
                          {"id": "g3", "type": "TROU", "question": "I ___ (go)", "reponses_texte": ["went"]}]}"#,
        )
        .unwrap();
        set_depuis_json(FichierJson {
            nom: "t.json".into(),
            stem: "t".into(),
            categorie: "langues".into(),
            chemin: "t.json".into(),
            data,
        })
        .unwrap()
    }

    #[test]
    fn facettes() {
        let s = set_test();
        let ids: Vec<&str> = s.exercices.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "v1#rec",
                "v1#prod",
                "v1#ctx",
                "c1#I",
                "c1#you",
                "c1#he / she / it",
                "g1",
                "g2",
                "g3"
            ]
        );
        let Forme::Ordre { mots, .. } = &s.exercices[6].forme else {
            panic!()
        };
        assert_eq!(mots, &["she", "has", "never", "been", "to", "London"]);
    }

    #[test]
    fn correction_tolerante() {
        let p = vec!["to".to_string()];
        let r = vec!["to achieve".to_string()];
        assert_eq!(noter_saisie("achieve", &r, true, &p).0, Verdict::Juste);
        assert_eq!(noter_saisie("To Achieve", &r, true, &p).0, Verdict::Juste);
        assert_eq!(noter_saisie("acheive", &r, true, &p).0, Verdict::Faute);
        assert_eq!(noter_saisie("receive", &r, true, &p).0, Verdict::Faux);
        let r = vec!["café".to_string()];
        assert_eq!(noter_saisie("cafe", &r, true, &[]).0, Verdict::Accents);
        // Conjugaison / grammaire : aucune tolérance (« goed » n'est pas « goes »).
        let r = vec!["goes".to_string()];
        assert_eq!(noter_saisie("goed", &r, false, &[]).0, Verdict::Faux);
        let r = vec!["He doesn't like it.".to_string()];
        assert_eq!(
            noter_saisie("he doesn’t like it", &r, false, &[]).0,
            Verdict::Juste
        );
    }

    #[test]
    fn surlignage() {
        let d = diff("recieve", "receive");
        let rate: String = d.iter().filter(|(_, ok)| !ok).map(|(c, _)| *c).collect();
        assert_eq!(rate.len(), 1);
    }

    #[test]
    fn session_et_deblocage() {
        let set = Arc::new(set_test());
        let tmp = std::env::temp_dir().join(format!("prog_langue_{}.json", std::process::id()));
        let mut prog = Progression::new(&tmp);
        let s = construire_session(&[Arc::clone(&set)], &prog, Mode::Quotidien, 50);
        let ids: Vec<&str> = s.iter().map(|e| e.exercice().id.as_str()).collect();
        // Production verrouillée tant que la carte n'est pas apprise ; 2 formes max par verbe.
        assert!(ids.contains(&"v1#rec") && !ids.contains(&"v1#prod"));
        assert_eq!(ids.iter().filter(|i| i.starts_with("c1#")).count(), 2);
        // Entrelacement : pas deux genres identiques à la suite au début.
        assert_ne!(s[0].exercice().genre, s[1].exercice().genre);

        enregistrer(&mut prog, "t", "v1#rec", Note::Bien);
        let st = prog.stats_question("t", "v1#rec").unwrap();
        assert!(st.stabilite.is_some() && st.echeance.unwrap() > core::maintenant());
        let s = construire_session(
            &[Arc::clone(&set)],
            &prog,
            Mode::Genre(Genre::Vocabulaire),
            50,
        );
        let ids: Vec<&str> = s.iter().map(|e| e.exercice().id.as_str()).collect();
        assert_eq!(
            ids.len(),
            1,
            "une seule facette du mot par session : {ids:?}"
        );
        assert!(ids[0] == "v1#prod" || ids[0] == "v1#ctx");

        enregistrer(&mut prog, "t", "v1#rec", Note::ARevoir);
        assert_eq!(prog.stats_question("t", "v1#rec").unwrap().oublis, 1);
        let faibles = construire_session(&[set], &prog, Mode::PointsFaibles, 10);
        assert_eq!(faibles[0].exercice().id, "v1#rec");
        let _ = std::fs::remove_file(&tmp);
    }
}
