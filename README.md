# Révision QCM / QCR

Application de révision pour partiels à partir de sets de questions JSON, avec
**suivi d'apprentissage** par révision espacée (système de Leitner).
Disponible en interface graphique (GUI) et en ligne de commande (CLI).

---

## Lancement

```bash
uv run python src/app.py
```

Au démarrage, le programme demande le mode d'interface :

```
==========================================
  Révision QCM — Choisir l'interface
==========================================
  1. Ligne de commande  (CLI)
  2. Interface graphique (GUI)
```

On peut aussi forcer le mode via un argument :

```bash
uv run python src/app.py --cli
uv run python src/app.py --gui
```

Une version web autonome (sans installation Python) est aussi disponible dans
[`web/revision_qcm.html`](web/revision_qcm.html) — ouvrable directement dans un
navigateur, ou servie avec les vrais sets de `questions/` via un petit serveur local
(voir [Version web](#version-web) plus bas).

---

## Interface graphique (GUI)

Fenêtre sombre avec navigation entièrement au clavier.

### Navigation dans les menus

| Touche | Action |
|--------|--------|
| `↑` / `↓` | Se déplacer dans la liste |
| `Entrée` | Sélectionner |
| `Échap` | Retour au menu précédent |

### Navigation pendant le quiz

La touche utile dépend du **type de question** :

| Type | Touches |
|------|---------|
| **QCM** (une seule réponse) | Une lettre sélectionne le choix · `Entrée`/`Espace` valide, puis passe à la suivante · `Q` quitte |
| **QCR** (plusieurs réponses) | Chaque lettre bascule la sélection · `Entrée`/`Espace` valide l'ensemble, puis passe à la suivante · `Q` quitte |
| **TROU** (texte à trous) | Taper la réponse au clavier · `Entrée` valide, puis repasser `Entrée`/`Espace` pour la suivante · `Échap` quitte |
| **FLASHCARD** (recto/verso) | `Espace` retourne la carte · `C` = « je savais », `I` = « à revoir » · puis `Entrée`/`Espace` pour la suivante · `Q` quitte |

Pour les questions à choix (QCM/QCR), un clic sur un choix fonctionne aussi.
Pour les flashcards, le bouton « Retourner la carte » fait la même chose que `Espace`.

### Équations mathématiques (LaTeX)

La GUI affiche nativement les expressions mathématiques écrites en LaTeX avec la
syntaxe `$...$` dans les questions et les choix de réponse.

Exemples de syntaxe utilisable dans les fichiers JSON :

| Écriture dans le JSON | Rendu affiché |
|-----------------------|---------------|
| `$\alpha + \beta$` | α + β |
| `$\frac{a}{b}$` | fraction a/b |
| `$x^2 + y^2 = r^2$` | x² + y² = r² |
| `$\int_0^1 f(x)\,dx$` | intégrale de 0 à 1 |
| `$\sum_{i=1}^{n} i$` | somme de 1 à n |
| `$\vec{F} = m\vec{a}$` | vecteurs |

Le rendu utilise le moteur **mathtext de matplotlib** — aucune installation LaTeX
système n'est requise. La CLI affiche le texte brut tel qu'écrit dans le JSON.

---

## Interface ligne de commande (CLI)

### Navigation dans les menus

Saisir le numéro de l'option affichée, puis `Entrée`.

### Saisie pendant le quiz

| Type | Saisie | Exemple | Description |
|------|--------|---------|-------------|
| QCM | Lettre seule | `C` | réponse C |
| QCR | Plusieurs lettres | `AC`, `A C`, `a,c` | réponses A et C |
| TROU | Texte libre | `goes` | la réponse à trou (casse/accents ignorés) |
| FLASHCARD | `Entrée` puis `o`/`n` | — | révèle la réponse, puis indique si tu l'avais trouvée |
| (tous types) | `q` | `q` | Quitter la session |

---

## Fonctionnalités

- **4 types de questions**, pour varier les modes de mémorisation :
  - **QCM** — une seule bonne réponse parmi plusieurs choix.
  - **QCR** — plusieurs bonnes réponses possibles.
  - **TROU** — texte à trous (réponse libre, idéal pour la grammaire d'une langue) ;
    la comparaison ignore la casse, les accents et les espaces superflus.
  - **FLASHCARD** — recto/verso auto-évalué (idéal pour le vocabulaire) :
    on essaie de se rappeler la réponse avant de retourner la carte.
- Questions chargées depuis des fichiers `.json` dans le dossier `questions/`
  (et ses sous-dossiers, voir [Catégories](#catégories) ci-dessous).
- **Feedback immédiat** après chaque réponse : résultat, bonne réponse, explication.
- **Révision intelligente** : ne propose que les questions « à réviser »
  (jamais vues ou dont l'intervalle de Leitner est écoulé), triées par priorité.
- **Statistiques** : maîtrise globale, précision, questions à réviser, répartition par boîte.
- Progression sauvegardée automatiquement après chaque réponse dans `progression.json`.

### Catégories

Les sets peuvent être rangés dans des **sous-dossiers** de `questions/` pour les
regrouper par matière (ex. tous les sets de maths au même endroit) :

```
questions/
├── maths/
│   ├── s_ch1_signaux_deterministes.json
│   └── ...
├── langues/
│   └── langues_anglais.json
├── reseaux/
│   └── ...
└── mon_cours.json          # un fichier à la racine reste dans la catégorie "Général"
```

Le nom du sous-dossier devient la **catégorie** affichée dans l'app. Dès qu'il existe
plus d'une catégorie, un écran de choix de catégorie s'intercale avant la liste des
sets (en GUI comme en CLI) — utile dès que tu accumules beaucoup de sets, pour ne pas
avoir à faire défiler une liste géante. S'il n'y a qu'une seule catégorie (ou aucun
sous-dossier), cette étape est sautée automatiquement.

Un fichier directement à la racine de `questions/` (sans sous-dossier) appartient à
la catégorie par défaut **« Général »**. Déplacer un fichier dans un sous-dossier ne
casse pas sa progression : l'identifiant de suivi reste le nom du fichier, pas son
chemin.

### Révision de langues

Les types **FLASHCARD** (vocabulaire) et **TROU** (grammaire) ont été pensés pour
réviser une langue en combinant plusieurs formes d'apprentissage dans un même set :
flashcards pour mémoriser du vocabulaire, phrases à trous pour la conjugaison/les
prépositions, QCM/QCR pour les règles de grammaire. Voir l'exemple complet dans
`questions/langues/langues_anglais.json`, qui mélange les 4 types.

### Système de Leitner

Chaque question appartient à une boîte (0 à 5) :

- Bonne réponse → monte d'une boîte
- Mauvaise réponse → redescend d'une boîte

Plus la boîte est haute, plus l'intervalle avant la prochaine révision est long :

| Boîte | Intervalle |
|:-----:|:----------:|
| 0 | immédiatement |
| 1 | 1 jour |
| 2 | 3 jours |
| 3 | 7 jours |
| 4 | 14 jours |
| 5 | 30 jours |

---

## Format d'un set de questions

Crée un fichier dans `questions/`, par exemple `questions/mon_cours.json` :

```json
{
  "titre": "Titre affiché du set",
  "description": "Courte description (optionnel)",
  "questions": [
    {
      "id": "identifiant-unique",
      "type": "QCM",
      "question": "Quelle est l'aire d'un cercle de rayon $r$ ?",
      "choix": ["$2\\pi r$", "$\\pi r^2$", "$\\pi r$", "$2r$"],
      "reponses": [1],
      "explication": "L'aire vaut $\\pi r^2$, le périmètre vaut $2\\pi r$."
    },
    {
      "id": "autre-question",
      "type": "QCR",
      "question": "Question à réponses multiples ?",
      "choix": ["A", "B", "C", "D"],
      "reponses": [0, 2],
      "explication": "On peut cocher A et C."
    },
    {
      "id": "grammaire-trou",
      "type": "TROU",
      "question": "She ___ (go) to school every day.",
      "reponses_texte": ["goes"],
      "explication": "Présent simple, 3e personne du singulier → -es."
    },
    {
      "id": "vocabulaire-flashcard",
      "type": "FLASHCARD",
      "question": "to achieve",
      "verso": "réussir, accomplir",
      "explication": "Ex. : « She achieved her goal. »"
    }
  ]
}
```

### Règles du format

| Champ | Obligatoire | Détail |
|-------|:-----------:|--------|
| `titre` | non | défaut : nom du fichier |
| `description` | non | — |
| `questions` | **oui** | liste des questions |
| `id` | non | défaut : `q1`, `q2`… ; sert au suivi, garde-le **stable** |
| `type` | non | `"QCM"` (défaut), `"QCR"`, `"TROU"` ou `"FLASHCARD"` |
| `question` | **oui** | l'énoncé / recto (peut contenir `$...$`) |
| `choix` | QCM/QCR | liste de propositions (peut contenir `$...$`) |
| `reponses` | QCM/QCR | indices (à partir de **0**) des bonnes réponses |
| `reponses_texte` | TROU | liste des réponses texte acceptées (synonymes/variantes) |
| `verso` | FLASHCARD | le dos de la carte (la réponse à retrouver) |
| `explication` | non | affichée après la réponse (peut contenir `$...$`) |

Selon le `type`, seuls certains champs sont requis :

- `QCM` / `QCR` → `choix` + `reponses` obligatoires.
- `TROU` → `reponses_texte` obligatoire (une liste, même avec une seule réponse, pour
  pouvoir accepter plusieurs formulations correctes).
- `FLASHCARD` → `verso` obligatoire ; pas de `choix`/`reponses`.

- Les `reponses` (QCM/QCR) sont des **indices** : `0` = premier choix, `1` = deuxième, etc.
- Un `QCM` doit avoir exactement **une** réponse ; un `QCR` peut en avoir plusieurs.
- Pour `TROU`, la comparaison de la réponse saisie est **souple** : casse, accents et
  espaces superflus sont ignorés (`normaliser_texte` dans `core.py`). « Goes », « GOES »
  et « goes » sont donc équivalents.
- Dans le JSON, le backslash LaTeX doit être doublé : `\\frac`, `\\alpha`, etc.
- Un fichier mal formé est signalé au lancement et simplement ignoré.

---

## Générer un exécutable (.exe)

```bash
# Build avec console (CLI + GUI)
uv run python src/build_exe.py

# Build GUI uniquement (pas de fenêtre noire)
uv run python src/build_exe.py --windowed
```

`dist/` est entièrement régénéré à chaque build (rien à y modifier à la main) :

```
dist/
├── revision_qcm.exe
├── revision_qcm.html   # copie de web/revision_qcm.html
├── manifest.json       # liste des sets, pour le chargement auto. de la version web
├── README.md
├── architecture.txt
└── questions/
    └── *.json
```

`progression.json` est créé automatiquement dans le même dossier au premier lancement.

---

## Version web

[`web/revision_qcm.html`](web/revision_qcm.html) est une version autonome (HTML/JS,
sans dépendance Python) de l'application, avec les mêmes fonctionnalités (4 types de
questions, catégories, suivi de Leitner). La progression est sauvegardée dans le
`localStorage` du navigateur.

- **Ouverte directement** (double-clic, `file://`) : les sets se chargent via import
  manuel ou glisser-déposer d'un dossier (écran « Gérer les sets »).
- **Servie par un serveur HTTP local** (ex. `python -m http.server` depuis `dist/`) :
  les sets de `questions/` se chargent **automatiquement** au démarrage via
  `manifest.json`, comme la CLI/GUI. Nécessaire car les navigateurs bloquent les
  requêtes `fetch()` vers des fichiers locaux en `file://`.

---

## Structure du projet

```
revision_qcm/
├── src/
│   ├── app.py           # point d'entrée, choix CLI/GUI, interface CLI
│   ├── core.py          # modèles, chargement JSON, algorithme de Leitner
│   ├── gui.py            # interface graphique tkinter avec rendu LaTeX
│   └── build_exe.py      # génération du .exe + dist/ via PyInstaller
├── web/
│   └── revision_qcm.html # version web autonome (HTML/JS, sans Python)
├── questions/             # sets de questions (.json), organisés en sous-dossiers/catégories
├── pyproject.toml         # dépendances Python (uv)
├── dist/                  # généré par build_exe.py (gitignored)
├── build/                 # fichiers intermédiaires PyInstaller (gitignored)
└── progression.json       # progression utilisateur (créé automatiquement, gitignored)
```

---

## Dépendances

Gérées automatiquement par `uv` :

| Paquet | Rôle |
|--------|------|
| `matplotlib` | Rendu des équations LaTeX ($...$) dans la GUI |
| `pillow` | Conversion des images matplotlib → tkinter |
| `pyinstaller` | Génération du .exe (dépendance de développement) |

`tkinter` est inclus dans la bibliothèque standard Python.
