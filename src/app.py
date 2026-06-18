#!/usr/bin/env python3
"""
Application de révision QCM / QCR pour partiels.

Lancement :
    python src/app.py

Les sets de questions sont des fichiers JSON dans le dossier `questions/`.
La progression est sauvegardée dans `progression.json`.
"""
from __future__ import annotations

import random
import sys
from pathlib import Path

import core

# --- Chemins : racine du projet (un niveau au-dessus de src/), ou dossier du
# .exe PyInstaller une fois figé. ---
RACINE = (
    Path(sys.executable).parent
    if getattr(sys, "frozen", False)
    else Path(__file__).resolve().parent.parent
)
DOSSIER_QUESTIONS = RACINE / "questions"
FICHIER_PROGRESSION = RACINE / "progression.json"


# ---------------------------------------------------------------------------
# Affichage
# ---------------------------------------------------------------------------
class C:
    """Codes couleur ANSI, désactivés automatiquement hors terminal."""
    actif = sys.stdout.isatty()

    @classmethod
    def _c(cls, code: str, txt: str) -> str:
        return f"\033[{code}m{txt}\033[0m" if cls.actif else txt

    @classmethod
    def gras(cls, t):  return cls._c("1", t)
    @classmethod
    def faible(cls, t): return cls._c("2", t)
    @classmethod
    def vert(cls, t):  return cls._c("92", t)
    @classmethod
    def rouge(cls, t): return cls._c("91", t)
    @classmethod
    def jaune(cls, t): return cls._c("93", t)
    @classmethod
    def bleu(cls, t):  return cls._c("96", t)


def titre(texte: str) -> None:
    barre = "=" * max(40, len(texte) + 4)
    print("\n" + C.bleu(barre))
    print(C.bleu(C.gras("  " + texte)))
    print(C.bleu(barre))


def barre_progression(pct: float, largeur: int = 24) -> str:
    plein = int(round(pct / 100 * largeur))
    return "[" + "#" * plein + "-" * (largeur - plein) + f"] {pct:5.1f}%"


def demander(invite: str) -> str:
    try:
        return input(invite).strip()
    except (EOFError, KeyboardInterrupt):
        print()
        return "q"


# ---------------------------------------------------------------------------
# Logique de quiz
# ---------------------------------------------------------------------------
def parser_selection(saisie: str, n: int) -> list[int]:
    """Transforme 'a c', 'AC' ou 'a,c' en [0, 2]. Ignore les lettres invalides."""
    indices = set()
    for ch in saisie.upper():
        if ch.isalpha():
            idx = ord(ch) - 65
            if 0 <= idx < n:
                indices.add(idx)
    return sorted(indices)


def poser_question(q: core.Question, numero: int, total: int) -> bool | None:
    """Affiche une question, recueille la réponse, donne le retour.

    Retourne True/False selon la justesse, ou None si l'utilisateur quitte.
    """
    print()
    if q.est_trou:
        return _poser_trou(q, numero, total)
    if q.est_flashcard:
        return _poser_flashcard(q, numero, total)
    return _poser_choix(q, numero, total)


def _poser_choix(q: core.Question, numero: int, total: int) -> bool | None:
    """QCM / QCR : sélection parmi des choix lettrés."""
    etiquette = "QCR (plusieurs réponses possibles)" if q.est_qcr else "QCM (une seule réponse)"
    print(C.faible(f"Question {numero}/{total}  ·  {etiquette}"))
    print(C.gras(q.enonce))
    for i, choix in enumerate(q.choix):
        print(f"   {C.bleu(chr(65 + i))}. {choix}")

    invite = "\nVotre réponse (lettres, 'q' pour quitter) : "
    while True:
        saisie = demander(invite)
        if saisie.lower() == "q":
            return None
        selection = parser_selection(saisie, len(q.choix))
        if selection:
            break
        print(C.jaune("  Saisie non reconnue, réessayez (ex. : A, ou 'AC' pour un QCR)."))

    correcte = q.est_correcte(selection)
    bonnes = ", ".join(chr(65 + i) for i in sorted(q.reponses))
    if correcte:
        print(C.vert("  ✓ Correct !"))
    else:
        choisies = ", ".join(chr(65 + i) for i in selection)
        print(C.rouge(f"  ✗ Incorrect. Vous avez répondu {choisies}."))
        print(C.vert(f"  Bonne(s) réponse(s) : {bonnes}"))
    if q.explication:
        print(C.faible(f"  ⓘ  {q.explication}"))
    return correcte


def _poser_trou(q: core.Question, numero: int, total: int) -> bool | None:
    """Texte à trous (grammaire) : réponse libre, comparée avec souplesse (casse/accents)."""
    print(C.faible(f"Question {numero}/{total}  ·  Texte à trous"))
    print(C.gras(q.enonce))

    saisie = demander("\nVotre réponse ('q' pour quitter) : ")
    if saisie.lower() == "q":
        return None

    correcte = q.est_correcte_texte(saisie)
    if correcte:
        print(C.vert("  ✓ Correct !"))
    else:
        attendu = " / ".join(q.reponses_texte)
        print(C.rouge(f"  ✗ Incorrect. Vous avez répondu « {saisie} »."))
        print(C.vert(f"  Réponse attendue : {attendu}"))
    if q.explication:
        print(C.faible(f"  ⓘ  {q.explication}"))
    return correcte


def _poser_flashcard(q: core.Question, numero: int, total: int) -> bool | None:
    """Flashcard (vocabulaire) : recto, verso révélé à la demande, auto-évaluation."""
    print(C.faible(f"Question {numero}/{total}  ·  Flashcard"))
    print(C.gras(q.enonce))

    saisie = demander("\nAppuyez sur Entrée pour révéler la réponse ('q' pour quitter) : ")
    if saisie.lower() == "q":
        return None
    print(C.bleu(f"  → {q.verso}"))
    if q.explication:
        print(C.faible(f"  ⓘ  {q.explication}"))

    while True:
        saisie = demander("\nAviez-vous trouvé ? (o/n, 'q' pour quitter) : ").lower()
        if saisie == "q":
            return None
        if saisie in ("o", "n"):
            return saisie == "o"
        print(C.jaune("  Répondez par 'o' ou 'n'."))


def lancer_session(qset: core.QuestionSet,
                   questions: list[core.Question],
                   prog: core.Progression) -> None:
    """Déroule une session de quiz sur une liste de questions."""
    if not questions:
        print(C.jaune("Aucune question à réviser pour le moment. Bravo !"))
        return

    titre(f"{qset.titre}  ({len(questions)} questions)")
    score = 0
    repondues = 0
    for i, q in enumerate(questions, start=1):
        resultat = poser_question(q, i, len(questions))
        if resultat is None:
            print(C.jaune("\nSession interrompue. Progression enregistrée."))
            break
        prog.enregistrer_reponse(qset.id, q.id, resultat)
        repondues += 1
        if resultat:
            score += 1
        prog.sauvegarder()  # sauvegarde après chaque réponse (robustesse)

    if repondues:
        pct = score / repondues * 100
        titre("Résultat de la session")
        print(f"  Score : {C.gras(f'{score}/{repondues}')}  ({pct:.0f} %)")
        print("  " + barre_progression(pct))
        if pct == 100:
            print(C.vert("  Parfait, tout est juste ! ✓"))
        elif pct >= 60:
            print(C.jaune("  Bon travail, continue de réviser les points faibles."))
        else:
            print(C.rouge("  À retravailler — relance une révision intelligente bientôt."))


# ---------------------------------------------------------------------------
# Écrans / menus
# ---------------------------------------------------------------------------
def charger_tous_les_sets() -> list[core.QuestionSet]:
    sets = []
    for chemin in core.lister_sets(DOSSIER_QUESTIONS):
        try:
            sets.append(core.charger_set(chemin, racine=DOSSIER_QUESTIONS))
        except ValueError as e:
            print(C.rouge(f"  ⚠ {e}"))
    return sets


def choisir_categorie(sets: list[core.QuestionSet]) -> str | None:
    """Demande une catégorie (sous-dossier de questions/) si plusieurs existent."""
    categories = sorted({s.categorie for s in sets})
    if len(categories) <= 1:
        return categories[0] if categories else None

    print("\nCatégories disponibles :")
    for i, c in enumerate(categories, start=1):
        n = sum(1 for s in sets if s.categorie == c)
        print(f"  {C.bleu(str(i))}. {c}  {C.faible(f'({n} sets)')}")
    print(f"  {C.bleu('0')}. Retour")
    choix = demander("Choix : ")
    if choix in ("0", "q", ""):
        return None
    if choix.isdigit() and 1 <= int(choix) <= len(categories):
        return categories[int(choix) - 1]
    print(C.jaune("Choix invalide."))
    return None


def choisir_set(sets: list[core.QuestionSet]) -> core.QuestionSet | None:
    categorie = choisir_categorie(sets)
    if categorie is None:
        return None
    sets_cat = [s for s in sets if s.categorie == categorie]

    print(f"\nSets disponibles — {categorie} :")
    for i, s in enumerate(sets_cat, start=1):
        print(f"  {C.bleu(str(i))}. {s.titre}  {C.faible(f'({len(s)} questions)')}")
    print(f"  {C.bleu('0')}. Retour")
    choix = demander("Choix : ")
    if choix in ("0", "q", ""):
        return None
    if choix.isdigit() and 1 <= int(choix) <= len(sets_cat):
        return sets_cat[int(choix) - 1]
    print(C.jaune("Choix invalide."))
    return None


def menu_session(sets, prog) -> None:
    qset = choisir_set(sets)
    if not qset:
        return
    print("\nMode :")
    print(f"  {C.bleu('1')}. Toutes les questions (ordre aléatoire)")
    print(f"  {C.bleu('2')}. Seulement les questions à réviser")
    print(f"  {C.bleu('3')}. Un nombre limité de questions")
    mode = demander("Choix : ")

    questions = list(qset.questions)
    if mode == "2":
        questions = [q for q in questions if prog.est_a_reviser(qset.id, q.id)]
        questions.sort(key=lambda q: prog.priorite(qset.id, q.id))
    elif mode == "3":
        random.shuffle(questions)
        n = demander(f"Combien de questions (max {len(questions)}) ? ")
        if n.isdigit():
            questions = questions[: max(1, int(n))]
    else:
        random.shuffle(questions)

    lancer_session(qset, questions, prog)


def menu_revision_intelligente(sets, prog) -> None:
    """Rassemble toutes les questions à réviser, tous sets confondus."""
    titre("Révision intelligente")
    a_faire: list[tuple[core.QuestionSet, core.Question]] = []
    for s in sets:
        for q in s.questions:
            if prog.est_a_reviser(s.id, q.id):
                a_faire.append((s, q))
    if not a_faire:
        print(C.vert("Rien à réviser pour l'instant — reviens plus tard. ✓"))
        return

    a_faire.sort(key=lambda sq: prog.priorite(sq[0].id, sq[1].id))
    print(f"{len(a_faire)} question(s) à réviser au total.")
    n = demander("Combien veux-tu en faire maintenant ? (Entrée = toutes) : ")
    if n.isdigit():
        a_faire = a_faire[: max(1, int(n))]

    # On déroule en regroupant la sauvegarde par question
    score = repondues = 0
    for i, (s, q) in enumerate(a_faire, start=1):
        resultat = poser_question(q, i, len(a_faire))
        if resultat is None:
            print(C.jaune("\nSession interrompue. Progression enregistrée."))
            break
        prog.enregistrer_reponse(s.id, q.id, resultat)
        prog.sauvegarder()
        repondues += 1
        score += int(resultat)
    if repondues:
        pct = score / repondues * 100
        titre("Résultat")
        print(f"  Score : {C.gras(f'{score}/{repondues}')}  ({pct:.0f} %)")
        print("  " + barre_progression(pct))


def menu_statistiques(sets, prog) -> None:
    titre("Mes statistiques")
    if not sets:
        print("Aucun set chargé.")
        return
    for s in sets:
        r = prog.resume_set(s)
        print(f"\n{C.gras(s.titre)}")
        print(f"  Maîtrise globale  {barre_progression(r['pct_maitrise'])}")
        print(f"  Questions vues    {r['rencontrees']}/{r['total']}")
        print(f"  À réviser         {r['a_reviser']}")
        if r["rencontrees"]:
            print(f"  Précision moyenne {r['precision']:.0f} %")
        rep = r["repartition"]
        detail = "  ".join(f"B{b}:{rep[b]}" for b in range(core.BOITE_MAX + 1))
        print(C.faible(f"  Répartition par boîte de Leitner : {detail}"))
    print(C.faible("\n  (Boîte 0 = à revoir souvent … Boîte 5 = bien maîtrisé)"))


def menu_reinitialiser(sets, prog) -> None:
    qset = choisir_set(sets)
    if not qset:
        return
    conf = demander(
        f"Effacer toute la progression de « {qset.titre} » ? (o/N) : "
    )
    if conf.lower() == "o":
        prog.reinitialiser_set(qset.id)
        prog.sauvegarder()
        print(C.vert("Progression réinitialisée."))
    else:
        print("Annulé.")


# ---------------------------------------------------------------------------
# Boucle principale
# ---------------------------------------------------------------------------
def main() -> None:
    DOSSIER_QUESTIONS.mkdir(exist_ok=True)
    prog = core.Progression(FICHIER_PROGRESSION)

    titre("Révision QCM / QCR — Préparation aux partiels")
    while True:
        sets = charger_tous_les_sets()
        if not sets:
            print(C.jaune(
                f"\nAucun set trouvé dans {DOSSIER_QUESTIONS}.\n"
                "Ajoute un fichier .json (voir le README pour le format)."
            ))

        print("\nMenu principal :")
        print(f"  {C.bleu('1')}. Démarrer une session sur un set")
        print(f"  {C.bleu('2')}. Révision intelligente (questions à réviser)")
        print(f"  {C.bleu('3')}. Voir mes statistiques")
        print(f"  {C.bleu('4')}. Réinitialiser la progression d'un set")
        print(f"  {C.bleu('5')}. Quitter")
        choix = demander("Choix : ")

        if choix == "1" and sets:
            menu_session(sets, prog)
        elif choix == "2" and sets:
            menu_revision_intelligente(sets, prog)
        elif choix == "3":
            menu_statistiques(sets, prog)
        elif choix == "4" and sets:
            menu_reinitialiser(sets, prog)
        elif choix in ("5", "q"):
            print("À bientôt, et bon courage pour tes partiels ! 👋")
            break
        else:
            print(C.jaune("Choix invalide."))


def choisir_interface() -> str:
    """Retourne 'gui' ou 'cli' selon l'argument passé ou le choix de l'utilisateur."""
    if "--gui" in sys.argv:
        return "gui"
    if "--cli" in sys.argv:
        return "cli"

    # Si pas de terminal interactif (ex. : .exe sans console), on lance la GUI directement.
    try:
        tty = sys.stdout.isatty()
    except Exception:
        tty = False
    if not tty:
        return "gui"

    barre = "=" * 42
    print(f"\n{barre}")
    print("  Révision QCM — Choisir l'interface")
    print(barre)
    print("  1. Ligne de commande  (CLI)")
    print("  2. Interface graphique (GUI)")
    print(barre)
    while True:
        choix = demander("Votre choix (1/2) : ")
        if choix == "1":
            return "cli"
        if choix == "2":
            return "gui"
        print(C.jaune("Entrez 1 ou 2."))


if __name__ == "__main__":
    mode = choisir_interface()
    if mode == "gui":
        from gui import lancer_gui
        lancer_gui()
    else:
        main()
