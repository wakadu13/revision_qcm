//! Cœur de l'application : modèles de données, chargement des sets de questions
//! depuis des fichiers JSON, et suivi de l'apprentissage.
//!
//! Le suivi repose sur le système de Leitner (révision espacée) : chaque question
//! appartient à une "boîte" (0 à 5). Une bonne réponse fait monter la question
//! d'une boîte, une mauvaise la fait redescendre. Plus la boîte est haute, plus
//! l'intervalle avant la prochaine révision est long.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{Local, NaiveDateTime, TimeDelta, Timelike};
use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::canonical_combining_class;

// --- Paramètres de la révision espacée (système de Leitner) ---
/// Intervalle (en jours) avant qu'une question d'une boîte donnée soit "à réviser".
pub const INTERVALLES_JOURS: [i64; 6] = [0, 1, 3, 7, 14, 30];
pub const BOITE_MAX: u8 = 5;

pub const CATEGORIE_DEFAUT: &str = "Général";

/// Normalise un texte pour comparer des réponses libres avec souplesse :
/// insensible à la casse, aux accents et aux espaces superflus.
pub fn normaliser_texte(s: &str) -> String {
    // Équivalent de str.casefold() pour les cas courants (ß → ss, ſ → s).
    let plie: String = s.trim().to_lowercase().replace('ß', "ss").replace('ſ', "s");
    let sans_accents: String = plie
        .nfkd()
        .filter(|&c| canonical_combining_class(c) == 0)
        .collect();
    sans_accents
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// Modèles
// ---------------------------------------------------------------------------
/// QCM/QCR : choix à cocher.  TROU : réponse texte libre (ex. grammaire à trous).
/// FLASHCARD : recto/verso, auto-évalué (mémorisation de vocabulaire).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeQuestion {
    Qcm,
    Qcr,
    Trou,
    Flashcard,
}

impl TypeQuestion {
    fn depuis_str(s: &str) -> Option<Self> {
        match s {
            "QCM" => Some(Self::Qcm),
            "QCR" => Some(Self::Qcr),
            "TROU" => Some(Self::Trou),
            "FLASHCARD" => Some(Self::Flashcard),
            _ => None,
        }
    }
}

/// Une question. Quatre types possibles :
///
/// - QCM : une seule bonne réponse parmi `choix`.
/// - QCR : une ou plusieurs bonnes réponses parmi `choix`.
/// - TROU : réponse libre (texte), comparée à `reponses_texte` (souple : accents/casse ignorés).
/// - FLASHCARD : recto (`enonce`) / verso (`verso`), auto-évaluée par l'utilisateur.
#[derive(Debug, Clone)]
pub struct Question {
    pub id: String,
    pub type_q: TypeQuestion,
    pub enonce: String,
    pub choix: Vec<String>,
    /// Indices (0-based) des bonnes réponses, QCM/QCR (triés, sans doublon).
    pub reponses: Vec<usize>,
    /// Réponses acceptées, TROU.
    pub reponses_texte: Vec<String>,
    /// Réponses acceptées déjà normalisées (calculées une seule fois au chargement).
    reponses_normalisees: Vec<String>,
    /// Dos de la carte, FLASHCARD.
    pub verso: String,
    pub explication: String,
}

impl Question {
    pub fn est_qcr(&self) -> bool {
        self.type_q == TypeQuestion::Qcr
    }

    pub fn est_trou(&self) -> bool {
        self.type_q == TypeQuestion::Trou
    }

    pub fn est_flashcard(&self) -> bool {
        self.type_q == TypeQuestion::Flashcard
    }

    /// QCM/QCR : vrai si la sélection correspond exactement aux bonnes réponses.
    pub fn est_correcte(&self, selection: &[usize]) -> bool {
        let mut sel = selection.to_vec();
        sel.sort_unstable();
        sel.dedup();
        sel == self.reponses
    }

    /// TROU : vrai si la réponse libre correspond à l'une des réponses acceptées.
    pub fn est_correcte_texte(&self, reponse: &str) -> bool {
        let cible = normaliser_texte(reponse);
        self.reponses_normalisees.contains(&cible)
    }
}

/// Un ensemble de questions chargé depuis un fichier JSON.
#[derive(Debug, Clone)]
pub struct QuestionSet {
    /// Identifiant stable = nom du fichier sans extension.
    pub id: String,
    pub titre: String,
    #[allow(dead_code)] // lu depuis le JSON, non affiché pour l'instant
    pub description: String,
    pub questions: Vec<Question>,
    #[allow(dead_code)]
    pub chemin: PathBuf,
    /// Nom du sous-dossier de questions/ (ex. "maths").
    pub categorie: String,
}

impl QuestionSet {
    pub fn len(&self) -> usize {
        self.questions.len()
    }
}

// ---------------------------------------------------------------------------
// Chargement et validation des sets
// ---------------------------------------------------------------------------
/// Équivalent de `str(x)` en Python pour les valeurs JSON scalaires.
fn en_texte(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "None".to_string(),
        Value::Bool(true) => "True".to_string(),
        Value::Bool(false) => "False".to_string(),
        autre => autre.to_string(),
    }
}

fn liste<'a>(v: &'a Value, contexte: &str, clef: &str) -> Result<&'a Vec<Value>, String> {
    v.as_array()
        .ok_or_else(|| format!("{contexte} : '{clef}' doit être une liste."))
}

/// Charge et valide un set de questions depuis un fichier JSON.
///
/// `racine` est le dossier `questions/` : si le fichier se trouve dans un
/// sous-dossier de `racine` (ex. `questions/maths/algebre.json`), ce sous-dossier
/// devient la `categorie` du set (ex. "maths"). Sinon, la catégorie est "Général".
///
/// Renvoie une erreur avec un message clair si le fichier est malformé.
pub fn charger_set(chemin: &Path, racine: Option<&Path>) -> Result<QuestionSet, String> {
    let nom = chemin
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = chemin
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let mut categorie = CATEGORIE_DEFAUT.to_string();
    if let Some(relatif) = racine.and_then(|r| chemin.strip_prefix(r).ok()) {
        let parts: Vec<_> = relatif.components().collect();
        if parts.len() > 1 {
            categorie = parts[0].as_os_str().to_string_lossy().into_owned();
        }
    }

    let octets = fs::read(chemin).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => format!("Fichier introuvable : {}", chemin.display()),
        _ => format!("Lecture impossible de {nom} : {e}"),
    })?;
    let data: Value =
        serde_json::from_slice(&octets).map_err(|e| format!("JSON invalide dans {nom} : {e}"))?;

    let brutes = data
        .get("questions")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{nom} : clé 'questions' (liste) manquante."))?;

    let mut questions = Vec::with_capacity(brutes.len());
    for (i, q) in brutes.iter().enumerate() {
        let i = i + 1;
        let contexte = format!("{nom} » question #{i}");
        let q = q
            .as_object()
            .ok_or_else(|| format!("{contexte} : la question doit être un objet JSON."))?;
        let enonce = q
            .get("question")
            .ok_or_else(|| format!("{contexte} : clé 'question' manquante."))?;

        let type_txt = q
            .get("type")
            .map(en_texte)
            .unwrap_or_else(|| "QCM".to_string())
            .to_uppercase();
        let type_q = TypeQuestion::depuis_str(&type_txt).ok_or_else(|| {
            format!("{contexte} : type '{type_txt}' inconnu (QCM, QCR, TROU ou FLASHCARD).")
        })?;

        let mut choix = Vec::new();
        let mut reponses = Vec::new();
        let mut reponses_texte = Vec::new();
        let mut verso = String::new();

        match type_q {
            TypeQuestion::Qcm | TypeQuestion::Qcr => {
                for clef in ["choix", "reponses"] {
                    if !q.contains_key(clef) {
                        return Err(format!("{contexte} : clé '{clef}' manquante."));
                    }
                }
                choix = liste(&q["choix"], &contexte, "choix")?
                    .iter()
                    .map(en_texte)
                    .collect();
                let brutes_rep = liste(&q["reponses"], &contexte, "reponses")?;
                if choix.is_empty() {
                    return Err(format!("{contexte} : la liste 'choix' est vide."));
                }
                if brutes_rep.is_empty() {
                    return Err(format!("{contexte} : aucune bonne réponse fournie."));
                }
                for r in brutes_rep {
                    match r.as_u64().filter(|&n| (n as usize) < choix.len()) {
                        Some(n) => reponses.push(n as usize),
                        None => {
                            return Err(format!(
                                "{contexte} : indice de réponse {r} hors limites (0 à {}).",
                                choix.len() - 1
                            ));
                        }
                    }
                }
                if type_q == TypeQuestion::Qcm && reponses.len() != 1 {
                    return Err(format!(
                        "{contexte} : un QCM doit avoir exactement 1 réponse."
                    ));
                }
                reponses.sort_unstable();
                reponses.dedup();
            }
            TypeQuestion::Trou => {
                let brutes_txt = q.get("reponses_texte").ok_or_else(|| {
                    format!(
                        "{contexte} : clé 'reponses_texte' manquante (liste des réponses acceptées)."
                    )
                })?;
                reponses_texte = liste(brutes_txt, &contexte, "reponses_texte")?
                    .iter()
                    .map(en_texte)
                    .collect();
                if !reponses_texte.iter().any(|r| !r.trim().is_empty()) {
                    return Err(format!(
                        "{contexte} : 'reponses_texte' doit contenir au moins une réponse non vide."
                    ));
                }
            }
            TypeQuestion::Flashcard => {
                verso = en_texte(q.get("verso").ok_or_else(|| {
                    format!("{contexte} : clé 'verso' manquante (dos de la carte).")
                })?);
                if verso.trim().is_empty() {
                    return Err(format!("{contexte} : 'verso' ne doit pas être vide."));
                }
            }
        }

        let reponses_normalisees = reponses_texte.iter().map(|r| normaliser_texte(r)).collect();
        questions.push(Question {
            id: q.get("id").map(en_texte).unwrap_or_else(|| format!("q{i}")),
            type_q,
            enonce: en_texte(enonce),
            choix,
            reponses,
            reponses_texte,
            reponses_normalisees,
            verso,
            explication: q.get("explication").map(en_texte).unwrap_or_default(),
        });
    }

    Ok(QuestionSet {
        id: stem.clone(),
        titre: data.get("titre").map(en_texte).unwrap_or(stem),
        description: data.get("description").map(en_texte).unwrap_or_default(),
        questions,
        chemin: chemin.to_path_buf(),
        categorie,
    })
}

/// Retourne la liste triée des fichiers .json présents dans un dossier,
/// y compris dans ses sous-dossiers (ex. `questions/maths/algebre.json`).
pub fn lister_sets(dossier: &Path) -> Vec<PathBuf> {
    fn parcourir(dossier: &Path, acc: &mut Vec<PathBuf>) {
        let Ok(entrees) = fs::read_dir(dossier) else {
            return;
        };
        for entree in entrees.flatten() {
            let chemin = entree.path();
            let Ok(ft) = entree.file_type() else { continue };
            if ft.is_dir() {
                parcourir(&chemin, acc);
            } else if chemin.extension().is_some_and(|e| e == "json") {
                acc.push(chemin);
            }
        }
    }
    let mut fichiers = Vec::new();
    parcourir(dossier, &mut fichiers);
    fichiers.sort();
    fichiers
}

/// Charge tous les sets de `dossier` en parallèle (un thread par lot de fichiers).
///
/// Renvoie les sets valides (dans l'ordre de `lister_sets`) et les messages
/// d'erreur des fichiers ignorés.
pub fn charger_tous_les_sets(dossier: &Path) -> (Vec<Arc<QuestionSet>>, Vec<String>) {
    let chemins = lister_sets(dossier);
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(chemins.len().max(1));
    let taille_lot = chemins.len().div_ceil(threads).max(1);

    let resultats: Vec<Result<QuestionSet, String>> = std::thread::scope(|s| {
        let handles: Vec<_> = chemins
            .chunks(taille_lot)
            .map(|lot| {
                s.spawn(move || {
                    lot.iter()
                        .map(|c| charger_set(c, Some(dossier)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("thread de chargement"))
            .collect()
    });

    let mut sets = Vec::with_capacity(resultats.len());
    let mut erreurs = Vec::new();
    for r in resultats {
        match r {
            Ok(s) => sets.push(Arc::new(s)),
            Err(e) => erreurs.push(e),
        }
    }
    (sets, erreurs)
}

/// Catégories distinctes, triées.
pub fn categories(sets: &[Arc<QuestionSet>]) -> Vec<String> {
    let mut cats: Vec<String> = sets.iter().map(|s| s.categorie.clone()).collect();
    cats.sort();
    cats.dedup();
    cats
}

// ---------------------------------------------------------------------------
// Suivi de l'apprentissage
// ---------------------------------------------------------------------------
const FORMAT_DATE: &str = "%Y-%m-%dT%H:%M:%S";

fn ser_date<S: Serializer>(d: &Option<NaiveDateTime>, s: S) -> Result<S::Ok, S::Error> {
    match d {
        Some(d) => s.serialize_str(&d.format(FORMAT_DATE).to_string()),
        None => s.serialize_none(),
    }
}

fn de_date<'de, D: Deserializer<'de>>(d: D) -> Result<Option<NaiveDateTime>, D::Error> {
    let brut: Option<String> = Option::deserialize(d)?;
    Ok(brut.and_then(|s| {
        // Accepte aussi les fractions de seconde (datetime.isoformat() par défaut).
        NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S%.f").ok()
    }))
}

/// Statistiques d'une question dans `progression.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatsQuestion {
    #[serde(default)]
    pub boite: u8,
    #[serde(default)]
    pub vues: u32,
    #[serde(default)]
    pub correctes: u32,
    #[serde(default, serialize_with = "ser_date", deserialize_with = "de_date")]
    pub derniere_revision: Option<NaiveDateTime>,
    #[serde(default)]
    pub derniere_correcte: Option<bool>,
}

/// Clé de tri pour la révision intelligente (voir [`Progression::priorite`]).
pub type Priorite = (bool, u8, Option<NaiveDateTime>);

/// Résumé chiffré de la progression sur un set.
#[derive(Debug, Clone)]
pub struct Resume {
    pub total: usize,
    pub rencontrees: usize,
    pub a_reviser: usize,
    pub precision: f64,
    pub repartition: [usize; BOITE_MAX as usize + 1],
    pub pct_maitrise: f64,
}

/// Persiste et exploite l'historique des réponses dans un fichier JSON.
///
/// Structure :
/// `{ set_id: { question_id: { "boite", "vues", "correctes", "derniere_revision", "derniere_correcte" } } }`
pub struct Progression {
    pub chemin: PathBuf,
    pub data: IndexMap<String, IndexMap<String, StatsQuestion>>,
}

impl Progression {
    pub fn new(chemin: impl Into<PathBuf>) -> Self {
        let chemin = chemin.into();
        // Fichier absent ou corrompu : on repart proprement.
        let data = fs::read(&chemin)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self { chemin, data }
    }

    /// Écrit la progression de façon atomique (fichier temporaire puis renommage) :
    /// une coupure pendant l'écriture ne peut pas corrompre `progression.json`.
    pub fn sauvegarder(&self) -> std::io::Result<()> {
        let json = serde_json::to_vec_pretty(&self.data)?;
        let tmp = self.chemin.with_extension("json.tmp");
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(&json)?;
            f.sync_all()?;
        }
        fs::rename(&tmp, &self.chemin)
    }

    pub fn stats_question(&self, set_id: &str, q_id: &str) -> Option<&StatsQuestion> {
        self.data.get(set_id)?.get(q_id)
    }

    /// Met à jour les statistiques et la boîte de Leitner d'une question.
    pub fn enregistrer_reponse(&mut self, set_id: &str, q_id: &str, correcte: bool) {
        let st = self
            .data
            .entry(set_id.to_string())
            .or_default()
            .entry(q_id.to_string())
            .or_default();
        st.vues += 1;
        if correcte {
            st.correctes += 1;
            st.boite = (st.boite + 1).min(BOITE_MAX);
        } else {
            st.boite = st.boite.saturating_sub(1);
        }
        st.derniere_correcte = Some(correcte);
        st.derniere_revision = Some(maintenant());
    }

    fn st_a_reviser(st: Option<&StatsQuestion>, now: NaiveDateTime) -> bool {
        let Some(derniere) = st.and_then(|s| s.derniere_revision) else {
            return true;
        };
        let boite = st.map_or(0, |s| s.boite) as usize;
        let jours = INTERVALLES_JOURS.get(boite).copied().unwrap_or(30);
        now >= derniere + TimeDelta::days(jours)
    }

    /// Vrai si la question n'a jamais été vue ou si son intervalle est écoulé.
    pub fn est_a_reviser(&self, set_id: &str, q_id: &str) -> bool {
        Self::st_a_reviser(self.stats_question(set_id, q_id), maintenant())
    }

    /// Clé de tri pour la révision intelligente.
    ///
    /// Ordre : questions à réviser d'abord, puis boîtes les plus basses
    /// (moins maîtrisées), puis révision la plus ancienne.
    pub fn priorite(&self, set_id: &str, q_id: &str) -> Priorite {
        let st = self.stats_question(set_id, q_id);
        (
            !Self::st_a_reviser(st, maintenant()),
            st.map_or(0, |s| s.boite),
            st.and_then(|s| s.derniere_revision),
        )
    }

    pub fn reinitialiser_set(&mut self, set_id: &str) {
        self.data.shift_remove(set_id);
    }

    /// Renvoie un résumé chiffré de la progression sur un set.
    pub fn resume_set(&self, qset: &QuestionSet) -> Resume {
        let now = maintenant();
        let stats_set = self.data.get(&qset.id);
        let (mut vues, mut correctes, mut total_reponses, mut a_reviser) = (0, 0u64, 0u64, 0);
        let mut repartition = [0usize; BOITE_MAX as usize + 1];
        for q in &qset.questions {
            let st = stats_set.and_then(|s| s.get(&q.id));
            if let Some(st) = st {
                vues += 1;
                correctes += u64::from(st.correctes);
                total_reponses += u64::from(st.vues);
                repartition[st.boite.min(BOITE_MAX) as usize] += 1;
            } else {
                repartition[0] += 1;
            }
            if Self::st_a_reviser(st, now) {
                a_reviser += 1;
            }
        }
        let precision = if total_reponses > 0 {
            correctes as f64 / total_reponses as f64 * 100.0
        } else {
            0.0
        };
        let maitrise: usize = repartition.iter().enumerate().map(|(b, n)| b * n).sum();
        let maitrise_max = BOITE_MAX as usize * qset.len();
        let pct_maitrise = if maitrise_max > 0 {
            maitrise as f64 / maitrise_max as f64 * 100.0
        } else {
            0.0
        };
        Resume {
            total: qset.len(),
            rencontrees: vues,
            a_reviser,
            precision,
            repartition,
            pct_maitrise,
        }
    }

    /// Toutes les questions à réviser (tous sets confondus), triées par priorité.
    pub fn questions_a_reviser(&self, sets: &[Arc<QuestionSet>]) -> Vec<(Arc<QuestionSet>, usize)> {
        let now = maintenant();
        let mut a_faire: Vec<(Priorite, Arc<QuestionSet>, usize)> = Vec::new();
        for s in sets {
            let stats_set = self.data.get(&s.id);
            for (i, q) in s.questions.iter().enumerate() {
                let st = stats_set.and_then(|m| m.get(&q.id));
                if Self::st_a_reviser(st, now) {
                    let prio = (
                        false,
                        st.map_or(0, |x| x.boite),
                        st.and_then(|x| x.derniere_revision),
                    );
                    a_faire.push((prio, Arc::clone(s), i));
                }
            }
        }
        // Tri stable : à priorité égale, l'ordre des sets et des questions est conservé.
        a_faire.sort_by_key(|a| a.0);
        a_faire.into_iter().map(|(_, s, i)| (s, i)).collect()
    }
}

fn maintenant() -> NaiveDateTime {
    let now = Local::now().naive_local();
    // Précision à la seconde, comme isoformat(timespec="seconds").
    now.with_nanosecond(0).unwrap_or(now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalisation() {
        assert_eq!(normaliser_texte("  Élève   À  l'École "), "eleve a l'ecole");
        assert_eq!(normaliser_texte("GOES"), "goes");
        assert_eq!(normaliser_texte("Straße"), "strasse");
    }

    #[test]
    fn leitner() {
        let tmp = std::env::temp_dir().join(format!("prog_test_{}.json", std::process::id()));
        let mut p = Progression::new(&tmp);
        assert!(p.est_a_reviser("s", "q"));
        p.enregistrer_reponse("s", "q", true);
        assert_eq!(p.stats_question("s", "q").unwrap().boite, 1);
        assert!(!p.est_a_reviser("s", "q"));
        p.enregistrer_reponse("s", "q", false);
        p.enregistrer_reponse("s", "q", false);
        assert_eq!(p.stats_question("s", "q").unwrap().boite, 0);
        assert!(p.est_a_reviser("s", "q"));
        p.sauvegarder().unwrap();
        let relu = Progression::new(&tmp);
        let st = relu.stats_question("s", "q").unwrap();
        assert_eq!(
            (st.vues, st.correctes, st.derniere_correcte),
            (3, 1, Some(false))
        );
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn compat_format_python() {
        let json = r#"{"set": {"q1": {"boite": 3, "vues": 4, "correctes": 3,
            "derniere_revision": "2026-06-17T10:00:00", "derniere_correcte": true},
            "q2": {"boite": 0, "vues": 0, "correctes": 0,
            "derniere_revision": null, "derniere_correcte": null}}}"#;
        let data: IndexMap<String, IndexMap<String, StatsQuestion>> =
            serde_json::from_str(json).unwrap();
        let q1 = &data["set"]["q1"];
        assert_eq!(q1.boite, 3);
        assert_eq!(
            q1.derniere_revision
                .unwrap()
                .format(FORMAT_DATE)
                .to_string(),
            "2026-06-17T10:00:00"
        );
        let ecrit = serde_json::to_string(&data).unwrap();
        assert!(ecrit.contains(r#""derniere_revision":"2026-06-17T10:00:00""#));
        assert!(ecrit.contains(r#""derniere_revision":null"#));
    }

    #[test]
    fn tous_les_sets_du_depot_se_chargent() {
        let dossier = Path::new(env!("CARGO_MANIFEST_DIR")).join("questions");
        let (sets, erreurs) = charger_tous_les_sets(&dossier);
        assert!(erreurs.is_empty(), "{erreurs:?}");
        assert!(!sets.is_empty());
        assert!(sets.iter().any(|s| s.categorie == "maths"));
    }
}
