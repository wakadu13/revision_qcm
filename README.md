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
- **Module Langues** : vocabulaire, conjugaison et grammaire avec répétition espacée
  FSRS, rappel actif et exercices entrelacés (voir [Module Langues](#module-langues)).
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

### Langues

Un module dédié aux langues (vocabulaire, conjugaison, grammaire) est accessible depuis
le menu principal. Voir [Module Langues](#module-langues) ci-dessous.

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

## Module Langues

Menu principal → **Langues**. Le module est construit sur les techniques dont l'efficacité
est la mieux établie par la recherche sur la mémoire et l'apprentissage des langues :

| Technique | Mise en œuvre |
|-----------|---------------|
| **Répétition espacée optimisée** | Algorithme **FSRS** (celui d'Anki depuis 2023) : chaque élément a sa propre stabilité et sa difficulté, et revient au moment où la probabilité de s'en souvenir tombe à 90 %. À rétention égale, il demande moins de révisions que les boîtes de Leitner ou SM-2. |
| **Rappel actif en production** | On **tape** le mot ou la forme verbale (au lieu de le reconnaître dans une liste) : produire ancre bien mieux que reconnaître. |
| **Plusieurs facettes par mot** | Un mot est d'abord découvert en **reconnaissance** (anglais → français, carte auto-évaluée). Une fois connu, il revient en **production** (français → anglais) et **en contexte** (phrase à compléter). Deux facettes d'un même mot ne tombent jamais dans la même session. |
| **Conjugaison personne par personne** | Chaque forme (« he / she / it → is ») est suivie séparément (2 formes d'un même tableau au plus par session), et le **tableau complet** s'affiche à la correction, avec la ligne demandée surlignée. Les verbes irréguliers sont travaillés de la même façon (prétérit, participe passé). |
| **Grammaire par la pratique** | Phrases à trous, **remise en ordre de mots**, **correction d'erreurs** (la phrase fautive est pré-remplie, il suffit de la corriger), et QCM/QCR pour les règles. |
| **Entrelacement** | Vocabulaire, conjugaison et grammaire sont mélangés dans la session du jour (jamais deux exercices du même genre d'affilée quand c'est possible) : apprendre en mélangeant les types d'exercices retient mieux que de les enchaîner par blocs. |
| **Correction immédiate + réessai** | Les lettres fausses ou manquantes sont surlignées dans la bonne réponse, et un exercice raté revient **quelques questions plus tard** dans la même session. |
| **Quota de nouveautés** | 20 nouveaux éléments par jour au plus (réglable), pour garder une charge de révisions tenable les jours suivants. Les révisions dues passent toujours en premier. |
| **Tolérance raisonnable** | En vocabulaire, un accent oublié ou une faute de frappe compte comme « presque » (révision un peu plus rapprochée), pas comme un échec. En conjugaison et en grammaire, la réponse doit être exacte (« goed » n'est pas « goes », « hablo » n'est pas « habló »). Casse, espaces et ponctuation finale sont toujours ignorés. |

Modes proposés : **Session du jour** (recommandé : tout mélangé), **Vocabulaire**,
**Conjugaison**, **Grammaire**, **Points faibles** (éléments souvent oubliés),
**Statistiques** (éléments appris, maîtrisés, rétention estimée, révisions à venir,
éléments les plus difficiles).

### Touches (GUI)

| Exercice | Touches |
|----------|---------|
| Carte (reconnaissance) | `Espace` retourne la carte, puis `1` À revoir · `2` Difficile · `3` Bien (ou `Espace`) · `4` Facile. Chaque bouton indique quand la carte reviendra. |
| Réponse à taper | `Entrée` valide ; des boutons insèrent les caractères spéciaux de la langue (á, ñ, ß…) à la position du curseur |
| Remise en ordre | clic sur les mots, ou leur numéro au clavier ; `⌫` retire le dernier ; `Entrée` valide |
| Après la correction | `Entrée`/`Espace` suivant · `F` « c'était trop facile » (espace davantage la prochaine révision) · `Q` quitter |

En CLI, les mêmes exercices sont proposés (remise en ordre : taper la phrase ou les
numéros des mots).

### Format d'un fichier de langue

Un fichier de langue se reconnaît à sa clé `"langue"` (et l'absence de `"questions"`).
Il se place dans `questions/` comme les autres sets (par exemple `questions/langues/`).
Plusieurs fichiers d'une même langue sont regroupés automatiquement.

```json
{
  "langue": "Anglais",
  "titre": "Anglais — vocabulaire, conjugaison & grammaire",
  "particules_optionnelles": ["to", "a", "an", "the"],
  "caracteres_speciaux": [],
  "nouveaux_par_jour": 20,

  "vocabulaire": [
    {
      "id": "voc-001",
      "mot": "to achieve",
      "traductions": ["réussir", "accomplir"],
      "exemple": "She finally [achieved] her goal.",
      "exemple_traduction": "Elle a enfin atteint son objectif.",
      "note": "achievement (n.) = une réussite"
    }
  ],

  "conjugaison": [
    {
      "id": "conj-be-present",
      "verbe": "to be", "traduction": "être", "temps": "present simple",
      "formes": { "I": "am", "you": "are", "he / she / it": "is", "we": "are", "they": "are" }
    },
    {
      "id": "irr-go", "verbe": "to go", "traduction": "aller", "temps": "verbe irrégulier",
      "formes": { "prétérit": "went", "participe passé": "gone" }
    }
  ],

  "grammaire": [
    { "id": "g1", "type": "TROU", "question": "She ___ (go) to school every day.", "reponses_texte": ["goes"] },
    { "id": "g2", "type": "ORDRE", "phrase": "She has never been to London.", "traduction": "Elle n'est jamais allée à Londres." },
    { "id": "g3", "type": "CORRECTION", "phrase": "He don't like coffee.",
      "corrections": ["He doesn't like coffee.", "He does not like coffee."] },
    { "id": "g4", "type": "QCM", "question": "...", "choix": ["...", "..."], "reponses": [0] }
  ]
}
```

| Champ | Détail |
|-------|--------|
| `langue` | **obligatoire** — nom affiché ; regroupe les fichiers d'une même langue |
| `langue_maternelle` | défaut « Français » (affiché dans le sens des exercices : EN → FR) |
| `particules_optionnelles` | mots facultatifs en tête de réponse : avec `"to"`, « achieve » est accepté pour « to achieve » |
| `caracteres_speciaux` | boutons d'insertion sous le champ de saisie (ex. `["á", "é", "ñ", "¿"]`) |
| `nouveaux_par_jour` | quota de nouveaux éléments par jour (défaut 20) |
| `vocabulaire[]` | `mot` et `traductions` (texte ou liste) obligatoires ; `exemple` avec le mot à trouver **entre crochets** (crée l'exercice en contexte), `exemple_traduction`, `note`, `prononciation`, `variantes` (autres réponses acceptées) facultatifs |
| `conjugaison[]` | `verbe`, `temps` et `formes` (personne → forme, ou liste de formes acceptées) obligatoires ; `traduction`, `explication` facultatifs |
| `grammaire[]` | `TROU`, `QCM`, `QCR`, `FLASHCARD` (même format que les sets classiques), `ORDRE` (`phrase`, `variantes`, `traduction`) ou `CORRECTION` (`phrase` fautive, `corrections`) ; `explication` et `traduction` facultatifs partout |

Les `id` servent au suivi : garde-les **stables**. Exemples complets :
`questions/langues/langues_anglais.json` (40 mots dont les faux amis courants, temps
principaux, 27 verbes irréguliers, 32 exercices de grammaire) et
`questions/langues/espagnol_debutant.json` (caractères spéciaux, ser/estar).

La version web convertit ces fichiers en questions classiques (cartes et textes à trous)
pour rester utilisable, sans les fonctions avancées du module (FSRS, facettes, remise
en ordre interactive).

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
│   ├── langues.rs        # module Langues : exercices, correction, sessions
│   ├── fsrs.rs           # algorithme de répétition espacée FSRS
│   ├── bibliotheque.rs   # chargement de questions/ (sets + fichiers de langue)
│   ├── cli/langues.rs    # module Langues en ligne de commande
│   ├── gui/langues.rs    # module Langues en interface graphique
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
