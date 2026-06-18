"""
Cœur de l'application : modèles de données, chargement des sets de questions
depuis des fichiers JSON, et suivi de l'apprentissage.

Le suivi repose sur le système de Leitner (révision espacée) : chaque question
appartient à une "boîte" (0 à 5). Une bonne réponse fait monter la question
d'une boîte, une mauvaise la fait redescendre. Plus la boîte est haute, plus
l'intervalle avant la prochaine révision est long.
"""
from __future__ import annotations

import json
import unicodedata
from dataclasses import dataclass, field
from datetime import datetime, timedelta
from pathlib import Path
from typing import Optional


# --- Paramètres de la révision espacée (système de Leitner) ---
# Intervalle (en jours) avant qu'une question d'une boîte donnée soit "à réviser".
INTERVALLES_JOURS = {0: 0, 1: 1, 2: 3, 3: 7, 4: 14, 5: 30}
BOITE_MAX = 5

# QCM/QCR : choix à cocher.  TROU : réponse texte libre (ex. grammaire à trous).
# FLASHCARD : recto/verso, auto-évalué (mémorisation de vocabulaire).
TYPES_VALIDES = {"QCM", "QCR", "TROU", "FLASHCARD"}


def normaliser_texte(s: str) -> str:
    """Normalise un texte pour comparer des réponses libres avec souplesse :
    insensible à la casse, aux accents et aux espaces superflus."""
    s = unicodedata.normalize("NFKD", s.strip().casefold())
    s = "".join(c for c in s if not unicodedata.combining(c))
    return " ".join(s.split())


# ---------------------------------------------------------------------------
# Modèles
# ---------------------------------------------------------------------------
@dataclass
class Question:
    """Une question. Quatre types possibles :

    - QCM : une seule bonne réponse parmi `choix`.
    - QCR : une ou plusieurs bonnes réponses parmi `choix`.
    - TROU : réponse libre (texte), comparée à `reponses_texte` (souple : accents/casse ignorés).
    - FLASHCARD : recto (`enonce`) / verso (`verso`), auto-évaluée par l'utilisateur.
    """
    id: str
    type: str
    enonce: str
    choix: list[str] = field(default_factory=list)
    reponses: list[int] = field(default_factory=list)        # indices (0-based), QCM/QCR
    reponses_texte: list[str] = field(default_factory=list)  # réponses acceptées, TROU
    verso: str = ""                                          # dos de la carte, FLASHCARD
    explication: str = ""

    @property
    def est_qcr(self) -> bool:
        return self.type.upper() == "QCR"

    @property
    def est_trou(self) -> bool:
        return self.type.upper() == "TROU"

    @property
    def est_flashcard(self) -> bool:
        return self.type.upper() == "FLASHCARD"

    def est_correcte(self, selection: list[int]) -> bool:
        """QCM/QCR : vrai si la sélection correspond exactement aux bonnes réponses."""
        return set(selection) == set(self.reponses)

    def est_correcte_texte(self, reponse: str) -> bool:
        """TROU : vrai si la réponse libre correspond à l'une des réponses acceptées."""
        cible = normaliser_texte(reponse)
        return any(cible == normaliser_texte(r) for r in self.reponses_texte)


@dataclass
class QuestionSet:
    """Un ensemble de questions chargé depuis un fichier JSON."""
    id: str                   # identifiant stable = nom du fichier sans extension
    titre: str
    description: str
    questions: list[Question]
    chemin: Path
    categorie: str = "Général"  # nom du sous-dossier de questions/ (ex. "maths")

    def __len__(self) -> int:
        return len(self.questions)


# ---------------------------------------------------------------------------
# Chargement et validation des sets
# ---------------------------------------------------------------------------
def charger_set(chemin: str | Path, racine: str | Path | None = None) -> QuestionSet:
    """Charge et valide un set de questions depuis un fichier JSON.

    `racine` est le dossier `questions/` : si le fichier se trouve dans un
    sous-dossier de `racine` (ex. `questions/maths/algebre.json`), ce sous-dossier
    devient la `categorie` du set (ex. "maths"). Sinon, la catégorie est "Général".

    Lève ValueError avec un message clair si le fichier est malformé.
    """
    chemin = Path(chemin)
    categorie = "Général"
    if racine is not None:
        try:
            relatif = chemin.relative_to(Path(racine))
            if len(relatif.parts) > 1:
                categorie = relatif.parts[0]
        except ValueError:
            pass
    try:
        data = json.loads(chemin.read_text(encoding="utf-8"))
    except FileNotFoundError:
        raise ValueError(f"Fichier introuvable : {chemin}")
    except json.JSONDecodeError as e:
        raise ValueError(f"JSON invalide dans {chemin.name} : {e}")

    if "questions" not in data or not isinstance(data["questions"], list):
        raise ValueError(f"{chemin.name} : clé 'questions' (liste) manquante.")

    questions: list[Question] = []
    for i, q in enumerate(data["questions"], start=1):
        contexte = f"{chemin.name} » question #{i}"
        if "question" not in q:
            raise ValueError(f"{contexte} : clé 'question' manquante.")

        type_q = str(q.get("type", "QCM")).upper()
        if type_q not in TYPES_VALIDES:
            raise ValueError(
                f"{contexte} : type '{type_q}' inconnu (QCM, QCR, TROU ou FLASHCARD)."
            )

        choix: list[str] = []
        reponses: list[int] = []
        reponses_texte: list[str] = []
        verso = ""

        if type_q in ("QCM", "QCR"):
            for clef in ("choix", "reponses"):
                if clef not in q:
                    raise ValueError(f"{contexte} : clé '{clef}' manquante.")
            choix = [str(c) for c in q["choix"]]
            reponses = list(q["reponses"])
            if not choix:
                raise ValueError(f"{contexte} : la liste 'choix' est vide.")
            if not reponses:
                raise ValueError(f"{contexte} : aucune bonne réponse fournie.")
            for r in reponses:
                if not isinstance(r, int) or not (0 <= r < len(choix)):
                    raise ValueError(
                        f"{contexte} : indice de réponse {r} hors limites "
                        f"(0 à {len(choix) - 1})."
                    )
            if type_q == "QCM" and len(reponses) != 1:
                raise ValueError(f"{contexte} : un QCM doit avoir exactement 1 réponse.")

        elif type_q == "TROU":
            if "reponses_texte" not in q:
                raise ValueError(
                    f"{contexte} : clé 'reponses_texte' manquante "
                    "(liste des réponses acceptées)."
                )
            reponses_texte = [str(r) for r in q["reponses_texte"]]
            if not any(r.strip() for r in reponses_texte):
                raise ValueError(
                    f"{contexte} : 'reponses_texte' doit contenir au moins une réponse non vide."
                )

        elif type_q == "FLASHCARD":
            if "verso" not in q:
                raise ValueError(f"{contexte} : clé 'verso' manquante (dos de la carte).")
            verso = str(q["verso"])
            if not verso.strip():
                raise ValueError(f"{contexte} : 'verso' ne doit pas être vide.")

        questions.append(
            Question(
                id=str(q.get("id", f"q{i}")),
                type=type_q,
                enonce=str(q["question"]),
                choix=choix,
                reponses=reponses,
                reponses_texte=reponses_texte,
                verso=verso,
                explication=str(q.get("explication", "")),
            )
        )

    return QuestionSet(
        id=chemin.stem,
        titre=str(data.get("titre", chemin.stem)),
        description=str(data.get("description", "")),
        questions=questions,
        chemin=chemin,
        categorie=categorie,
    )


def lister_sets(dossier: str | Path) -> list[Path]:
    """Retourne la liste triée des fichiers .json présents dans un dossier,
    y compris dans ses sous-dossiers (ex. `questions/maths/algebre.json`)."""
    dossier = Path(dossier)
    if not dossier.exists():
        return []
    return sorted(dossier.rglob("*.json"))


# ---------------------------------------------------------------------------
# Suivi de l'apprentissage
# ---------------------------------------------------------------------------
class Progression:
    """Persiste et exploite l'historique des réponses dans un fichier JSON.

    Structure interne :
        { set_id: { question_id: {
            "boite": int, "vues": int, "correctes": int,
            "derniere_revision": isoformat|None, "derniere_correcte": bool|None
        } } }
    """

    def __init__(self, chemin: str | Path):
        self.chemin = Path(chemin)
        self.data: dict[str, dict[str, dict]] = {}
        self._charger()

    def _charger(self) -> None:
        if self.chemin.exists():
            try:
                self.data = json.loads(self.chemin.read_text(encoding="utf-8"))
            except json.JSONDecodeError:
                self.data = {}  # fichier corrompu : on repart proprement

    def sauvegarder(self) -> None:
        self.chemin.write_text(
            json.dumps(self.data, indent=2, ensure_ascii=False),
            encoding="utf-8",
        )

    def stats_question(self, set_id: str, q_id: str) -> Optional[dict]:
        return self.data.get(set_id, {}).get(q_id)

    def enregistrer_reponse(self, set_id: str, q_id: str, correcte: bool) -> None:
        """Met à jour les statistiques et la boîte de Leitner d'une question."""
        stats_set = self.data.setdefault(set_id, {})
        st = stats_set.setdefault(
            q_id,
            {"boite": 0, "vues": 0, "correctes": 0,
             "derniere_revision": None, "derniere_correcte": None},
        )
        st["vues"] += 1
        if correcte:
            st["correctes"] += 1
            st["boite"] = min(BOITE_MAX, st["boite"] + 1)
        else:
            st["boite"] = max(0, st["boite"] - 1)
        st["derniere_correcte"] = correcte
        st["derniere_revision"] = datetime.now().isoformat(timespec="seconds")

    def est_a_reviser(self, set_id: str, q_id: str) -> bool:
        """Vrai si la question n'a jamais été vue ou si son intervalle est écoulé."""
        st = self.stats_question(set_id, q_id)
        if not st or not st.get("derniere_revision"):
            return True
        intervalle = INTERVALLES_JOURS.get(st.get("boite", 0), 30)
        derniere = datetime.fromisoformat(st["derniere_revision"])
        return datetime.now() >= derniere + timedelta(days=intervalle)

    def priorite(self, set_id: str, q_id: str) -> tuple:
        """Clé de tri pour la révision intelligente.

        Ordre : questions à réviser d'abord, puis boîtes les plus basses
        (moins maîtrisées), puis révision la plus ancienne.
        """
        st = self.stats_question(set_id, q_id)
        a_reviser = 0 if self.est_a_reviser(set_id, q_id) else 1
        boite = st.get("boite", 0) if st else 0
        derniere = st.get("derniere_revision") if st else None
        return (a_reviser, boite, derniere or "")

    def reinitialiser_set(self, set_id: str) -> None:
        self.data.pop(set_id, None)

    def resume_set(self, qset: QuestionSet) -> dict:
        """Renvoie un résumé chiffré de la progression sur un set."""
        vues = correctes = total_reponses = a_reviser = 0
        repartition = {b: 0 for b in range(BOITE_MAX + 1)}
        for q in qset.questions:
            st = self.stats_question(qset.id, q.id)
            if st:
                vues += 1
                correctes += st["correctes"]
                total_reponses += st["vues"]
                repartition[st.get("boite", 0)] += 1
            else:
                repartition[0] += 1
            if self.est_a_reviser(qset.id, q.id):
                a_reviser += 1
        precision = (correctes / total_reponses * 100) if total_reponses else 0.0
        maitrise = sum(b * n for b, n in repartition.items())
        maitrise_max = BOITE_MAX * len(qset)
        pct_maitrise = (maitrise / maitrise_max * 100) if maitrise_max else 0.0
        return {
            "total": len(qset),
            "rencontrees": vues,
            "a_reviser": a_reviser,
            "precision": precision,
            "repartition": repartition,
            "pct_maitrise": pct_maitrise,
        }