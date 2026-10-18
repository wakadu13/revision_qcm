//! Chargement de tout le contenu de `questions/` : sets de questions classiques
//! et fichiers de langue, en parallèle.

use std::path::Path;
use std::sync::Arc;

use crate::core::{self, QuestionSet};
use crate::langues::{self, SetLangue};

#[derive(Default)]
pub struct Bibliotheque {
    pub sets: Vec<Arc<QuestionSet>>,
    pub langues: Vec<Arc<SetLangue>>,
    /// Messages des fichiers ignorés (malformés).
    pub erreurs: Vec<String>,
}

enum Charge {
    Set(QuestionSet),
    Langue(SetLangue),
}

pub fn charger(dossier: &Path) -> Bibliotheque {
    let resultats = core::traiter_en_parallele(dossier, |chemin| {
        let f = core::lire_fichier(chemin, Some(dossier))?;
        if langues::est_fichier_langue(&f.data) {
            langues::set_depuis_json(f).map(Charge::Langue)
        } else {
            core::set_depuis_json(f).map(Charge::Set)
        }
    });
    let mut b = Bibliotheque::default();
    for r in resultats {
        match r {
            Ok(Charge::Set(s)) => b.sets.push(Arc::new(s)),
            Ok(Charge::Langue(l)) => b.langues.push(Arc::new(l)),
            Err(e) => b.erreurs.push(e),
        }
    }
    b
}

#[cfg(test)]
mod tests {
    #[test]
    fn tout_le_depot_se_charge() {
        let dossier = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("questions");
        let b = super::charger(&dossier);
        assert!(b.erreurs.is_empty(), "{:?}", b.erreurs);
        assert!(b.sets.iter().any(|s| s.categorie == "maths"));
        assert!(!b.langues.is_empty());
    }

    #[test]
    fn premiere_session_entrelacee_et_limitee() {
        use crate::langues::{self, Genre, Mode};
        let dossier = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("questions");
        let b = super::charger(&dossier);
        let sets = langues::sets_de(&b.langues, "Anglais");
        let prog = crate::core::Progression::new(std::env::temp_dir().join("inexistant_xyz.json"));
        let s = langues::construire_session(&sets, &prog, Mode::Quotidien, 6);
        let genres: Vec<Genre> = s.iter().map(|e| e.exercice().genre).collect();
        assert!(Genre::TOUS.iter().all(|g| genres.contains(g)), "{genres:?}");
        // Quota de nouveautés : 20 par jour au plus.
        let tout = langues::construire_session(&sets, &prog, Mode::Quotidien, usize::MAX);
        assert_eq!(tout.len(), langues::NOUVEAUX_PAR_JOUR);
    }
}
