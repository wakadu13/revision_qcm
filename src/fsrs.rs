//! Planificateur de révisions FSRS (Free Spaced Repetition Scheduler, v4.5).
//!
//! FSRS modélise la mémoire de chaque élément par deux variables :
//! - la **stabilité** S (en jours) : délai au bout duquel la probabilité de
//!   rappel tombe à 90 % ;
//! - la **difficulté** D (de 1 à 10) : vitesse à laquelle S augmente.
//!
//! La probabilité de rappel (« retrievabilité ») après t jours vaut
//! R(t, S) = (1 + 19/81 · t/S)^(-0,5). Chaque révision met à jour S et D selon
//! la note donnée et le moment où elle a eu lieu : une révision réussie alors que
//! le souvenir était presque oublié (R faible) fait davantage progresser S.
//! L'élément est reprogrammé quand R atteint la rétention visée (90 %).
//!
//! Pour une même rétention, FSRS demande nettement moins de révisions que les
//! boîtes de Leitner ou SM-2 (algorithme historique d'Anki) ; les paramètres
//! ci-dessous sont les valeurs par défaut publiées, optimisées sur des centaines
//! de millions de révisions réelles.

/// Paramètres par défaut de FSRS-4.5.
const W: [f64; 17] = [
    0.4072, 1.1829, 3.1262, 15.4722, 7.2102, 0.5316, 1.0651, 0.0234, 1.616, 0.1544, 1.0824, 1.9813,
    0.0953, 0.2975, 2.2042, 0.2407, 2.9466,
];
const DECAY: f64 = -0.5;
const FACTOR: f64 = 19.0 / 81.0;
/// Probabilité de rappel visée au moment de la révision.
pub const RETENTION: f64 = 0.9;
/// Stabilité minimale (évite les divisions par zéro).
const S_MIN: f64 = 0.01;

/// Note d'une révision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Note {
    /// Oublié / faux.
    ARevoir = 1,
    /// Trouvé avec effort (ou petite erreur : accent, faute de frappe).
    Difficile = 2,
    /// Trouvé.
    Bien = 3,
    /// Trouvé sans aucun effort.
    Facile = 4,
}

impl Note {
    fn g(self) -> f64 {
        self as u8 as f64
    }

    pub fn reussie(self) -> bool {
        self != Note::ARevoir
    }
}

/// État mémoriel d'un élément.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Etat {
    pub stabilite: f64,
    pub difficulte: f64,
}

/// Probabilité de se souvenir de l'élément `jours` après la dernière révision.
pub fn retrievabilite(jours: f64, stabilite: f64) -> f64 {
    (1.0 + FACTOR * jours.max(0.0) / stabilite.max(S_MIN)).powf(DECAY)
}

fn difficulte_initiale(note: Note) -> f64 {
    (W[4] - (note.g() - 3.0) * W[5]).clamp(1.0, 10.0)
}

/// État après la toute première révision.
pub fn premiere(note: Note) -> Etat {
    Etat {
        stabilite: W[note as usize - 1],
        difficulte: difficulte_initiale(note),
    }
}

/// État après une révision faite `jours` après la précédente.
pub fn suivante(etat: Etat, jours: f64, note: Note) -> Etat {
    let Etat {
        stabilite: s,
        difficulte: d,
    } = etat;
    let s = s.max(S_MIN);
    let r = retrievabilite(jours, s);

    // Difficulté : ajustée selon la note, avec retour progressif vers la moyenne.
    let d_note = d - W[6] * (note.g() - 3.0);
    let d2 = (W[7] * difficulte_initiale(Note::Facile) + (1.0 - W[7]) * d_note).clamp(1.0, 10.0);

    let s2 = if note == Note::ARevoir {
        // Oubli : la stabilité repart d'une valeur basse (jamais plus haute qu'avant).
        let s_oubli =
            W[11] * d.powf(-W[12]) * ((s + 1.0).powf(W[13]) - 1.0) * (W[14] * (1.0 - r)).exp();
        s_oubli.min(s)
    } else {
        let penalite = if note == Note::Difficile { W[15] } else { 1.0 };
        let bonus = if note == Note::Facile { W[16] } else { 1.0 };
        s * (1.0
            + W[8].exp()
                * (11.0 - d)
                * s.powf(-W[9])
                * ((W[10] * (1.0 - r)).exp() - 1.0)
                * penalite
                * bonus)
    };
    Etat {
        stabilite: s2.max(S_MIN),
        difficulte: d2,
    }
}

/// Intervalle (en jours) avant que la probabilité de rappel retombe à `RETENTION`.
pub fn intervalle_jours(stabilite: f64) -> f64 {
    stabilite / FACTOR * (RETENTION.powf(1.0 / DECAY) - 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retention_a_l_echeance() {
        // Par construction, R vaut 90 % au bout de S jours.
        assert!((retrievabilite(10.0, 10.0) - 0.9).abs() < 1e-9);
        assert!((intervalle_jours(10.0) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn notes_ordonnees() {
        let e = premiere(Note::Bien);
        let s: Vec<f64> = [Note::ARevoir, Note::Difficile, Note::Bien, Note::Facile]
            .iter()
            .map(|&n| suivante(e, e.stabilite, n).stabilite)
            .collect();
        assert!(s[0] < s[1] && s[1] < s[2] && s[2] < s[3], "{s:?}");
        assert!(s[0] <= e.stabilite);
        // Une révision réussie au bon moment augmente fortement la stabilité.
        assert!(s[2] > 2.0 * e.stabilite);
    }

    #[test]
    fn reviser_tard_rapporte_plus() {
        let e = Etat {
            stabilite: 10.0,
            difficulte: 5.0,
        };
        assert!(suivante(e, 20.0, Note::Bien).stabilite > suivante(e, 5.0, Note::Bien).stabilite);
    }
}
