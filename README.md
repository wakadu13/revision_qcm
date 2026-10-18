# Révision QCM / QCR

Application de révision pour partiels à partir de sets de questions JSON, avec
**suivi d'apprentissage** par révision espacée (système de Leitner).
Disponible en interface graphique (GUI) et en ligne de commande (CLI).

Écrite en **Rust** : un seul exécutable natif de ~8 Mo, sans interpréteur ni
dépendance à installer, qui démarre instantanément (voir [Performances](#performances)).

---

## Lancement

Prérequis pour compiler : la chaîne d'outils Rust ([rustup.rs](https://rustup.rs)).

```bash
cargo run --release
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
cargo run --release -- --cli
cargo run --release -- --gui
```

(ou directement `revision_qcm --cli` / `revision_qcm --gui` avec l'exécutable compilé).

Une version web autonome (sans installation) est aussi disponible dans
[`web/revision_qcm.html`](web/revision_qcm.html) — ouvrable directement dans un
navigateur, ou servie avec les vrais sets de `questions/` via un petit serveur local
(voir [Version web](#version-web) plus bas).

---

## Interface graphique (GUI)

Fenêtre sombre avec navigation entièrement au clavier (ou à la souris), rendue par le
GPU via [egui](https://github.com/emilk/egui) : l'écran n'est redessiné que lorsqu'un
événement survient, l'application ne consomme donc rien au repos.

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
Les lettres suivent la disposition du clavier (AZERTY comme QWERTY).
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
| `$\left(\frac{\pi D}{\lambda}\right)^2$` | parenthèses extensibles |
| `$x(t) \xleftrightarrow{\mathcal{F}} X(f)$` | flèche annotée |

Le rendu est assuré par un **moteur de mise en page mathématique intégré** (`src/math.rs`,
inspiré de TeX) : aucune installation LaTeX ni bibliothèque externe n'est requise, le
rendu est vectoriel (net à tous les zooms) et s'adapte aux couleurs de l'interface.
Le texte autour des formules passe à la ligne automatiquement, et une formule trop
longue est coupée après un signe `=` ou un opérateur, comme en TeX.

Commandes prises en charge : fractions (`\frac`, `\dfrac`, `\binom`), racines
(`\sqrt`, `\sqrt[n]`), indices/exposants, grands opérateurs (`\sum`, `\prod`, `\int`,
`\oint`…) et fonctions (`\lim`, `\log`, `\sin`, `\max`, `\operatorname`…),
délimiteurs extensibles (`\left` / `\right`), lettres grecques et symboles usuels
(`\cdot`, `\approx`, `\leq`, `\to`, `\infty`…), `\text`, `\mathrm`, `\mathbb`,
`\mathcal`, accents (`\vec`, `\hat`, `\bar`, `\overline`, `\tilde`, `\dot`),
flèches annotées (`\xrightarrow`, `\xleftrightarrow`), `\overset` / `\underset` et
espacements (`\,`, `\;`, `\quad`…). Une commande inconnue est affichée telle quelle.

La CLI affiche le texte brut tel qu'écrit dans le JSON.

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
  espaces superflus sont ignorés (`normaliser_texte` dans `core.rs`). « Goes », « GOES »
  et « goes » sont donc équivalents.
- Dans le JSON, le backslash LaTeX doit être doublé : `\\frac`, `\\alpha`, etc.
- Un fichier mal formé est signalé au lancement et simplement ignoré.

`progression.json` garde exactement le même format que la version Python précédente :
une progression existante est reprise telle quelle. Il est écrit de façon atomique
(fichier temporaire puis renommage) : une coupure pendant la sauvegarde ne peut pas le
corrompre.

---

## Générer un exécutable (.exe)

```bash
# Build avec console (CLI + GUI)
cargo run --release --bin build_dist

# Build GUI uniquement (pas de fenêtre noire sous Windows)
cargo run --release --bin build_dist -- --windowed
```

L'exécutable est compilé en mode release avec optimisations maximales (LTO complète,
une seule unité de compilation, symboles retirés). Les éléments de `dist/` sont
régénérés à chaque build (rien à y modifier à la main) ; `dist/progression.json` est
conservé :

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

Sous Windows l'exécutable s'appelle `revision_qcm.exe`, sous Linux/macOS `revision_qcm`.
`progression.json` est créé automatiquement dans le même dossier au premier lancement.

---

## Version web

[`web/revision_qcm.html`](web/revision_qcm.html) est une version autonome (HTML/JS,
le seul langage exécutable nativement par un navigateur) de l'application, avec les mêmes fonctionnalités (4 types de
questions, catégories, suivi de Leitner). La progression est sauvegardée dans le
`localStorage` du navigateur.

- **Ouverte directement** (double-clic, `file://`) : les sets se chargent via import
  manuel ou glisser-déposer d'un dossier (écran « Gérer les sets »).
- **Servie par un serveur HTTP local** (ex. `python -m http.server` ou `npx serve` depuis `dist/`) :
  les sets de `questions/` se chargent **automatiquement** au démarrage via
  `manifest.json`, comme la CLI/GUI. Nécessaire car les navigateurs bloquent les
  requêtes `fetch()` vers des fichiers locaux en `file://`.

---

## Performances

Mesures sur la même machine, avec les 34 sets du dépôt (moyenne de 20 lancements) :

| Opération | Python (avant) | Rust (maintenant) |
|-----------|---------------:|------------------:|
| Démarrage CLI + chargement des sets + sortie | 72 ms | **7 ms** |
| Écran de statistiques (34 sets) | 82 ms | **13 ms** |
| Ouverture de la fenêtre GUI | > 1 s (import de matplotlib) | **≈ 85 ms** |
| Taille de l'exécutable distribué | dizaines de Mo (PyInstaller + matplotlib) | **≈ 8 Mo** |

Ce qui fait la différence :

- code natif compilé (aucun interpréteur, aucune extraction temporaire au lancement
  comme avec un `.exe` PyInstaller « onefile ») ;
- sets chargés **en parallèle** sur tous les cœurs ;
- réponses texte (TROU) normalisées une seule fois au chargement, dates de révision
  converties une seule fois à la lecture de `progression.json` ;
- formules LaTeX analysées une seule fois puis mises en cache, et dessinées directement
  par le GPU (plus de passage par une image PNG générée par matplotlib) ;
- interface redessinée uniquement sur événement : 0 % de CPU au repos.

---

## Structure du projet

```
revision_qcm/
├── src/
│   ├── main.rs           # point d'entrée, choix CLI/GUI
│   ├── cli.rs            # interface ligne de commande
│   ├── core.rs           # modèles, chargement JSON, algorithme de Leitner
│   ├── gui.rs            # interface graphique (egui)
│   ├── math.rs           # moteur de rendu des formules LaTeX
│   ├── paths.rs          # emplacement de questions/ et progression.json
│   └── bin/
│       └── build_dist.rs # génération de dist/ (exécutable + questions + web)
├── assets/fonts/         # police DejaVu Sans embarquée (texte + symboles mathématiques)
├── web/
│   └── revision_qcm.html # version web autonome (HTML/JS)
├── questions/            # sets de questions (.json), organisés en sous-dossiers/catégories
├── Cargo.toml            # dépendances Rust et profil d'optimisation
├── dist/                 # généré par build_dist (gitignored)
├── target/               # compilation Rust (gitignored)
└── progression.json      # progression utilisateur (créé automatiquement, gitignored)
```

---

## Dépendances

Gérées automatiquement par `cargo` et compilées dans l'exécutable :

| Crate | Rôle |
|-------|------|
| `eframe` / `egui` | Interface graphique accélérée par le GPU |
| `serde`, `serde_json`, `indexmap` | Lecture des sets et de `progression.json` |
| `chrono` | Dates de révision (intervalles de Leitner) |
| `unicode-normalization` | Comparaison souple des réponses (accents) |
| `fastrand` | Mélange aléatoire des questions |

Tests : `cargo test` (chargement de tous les sets du dépôt, analyse de toutes leurs
formules LaTeX, algorithme de Leitner, compatibilité du format de progression).

Sous Linux, l'interface graphique nécessite les bibliothèques système habituelles
(X11 ou Wayland, `libxkbcommon-x11`, OpenGL) ; rien à installer sous Windows et macOS.
