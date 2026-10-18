//! Rendu natif des expressions mathématiques LaTeX (`$...$`) pour la GUI.
//!
//! Mini-moteur de mise en page inspiré de TeX : l'expression est analysée en
//! arbre (fractions, racines, indices/exposants, grands opérateurs, délimiteurs
//! extensibles, flèches annotées, accents…), puis disposée en « boîtes » dessinées
//! directement avec le moteur de texte d'egui. Aucune dépendance externe (ni LaTeX,
//! ni matplotlib) : le rendu est vectoriel, net à toutes les échelles, et le texte
//! autour des formules est renvoyé à la ligne proprement (les formules longues
//! peuvent être coupées après un signe = ou un opérateur, comme en TeX).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use eframe::egui::{
    Color32, FontFamily, FontId, Galley, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui,
    Vec2, pos2, vec2,
};
use eframe::epaint::FontsView;
use eframe::epaint::text::{LayoutJob, TextFormat};

// ===========================================================================
// Arbre syntaxique
// ===========================================================================
/// Classe d'atome TeX : détermine l'espacement entre symboles voisins.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Classe {
    Ord,
    Op,
    Bin,
    Rel,
    Ouv,
    Ferm,
    Ponct,
    Inner,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Fleche {
    Droite,
    Gauche,
    Double,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Accent {
    Vecteur,
    Chapeau,
    Barre,
    Tilde,
    Point,
    DeuxPoints,
    Souligne,
}

#[derive(Clone, Debug)]
enum Noeud {
    /// Symbole, nombre ou lettre. `repli` : texte de secours si la police n'a pas le glyphe.
    Sym {
        txt: String,
        classe: Classe,
        italique: bool,
        repli: Option<String>,
    },
    /// Texte droit, espaces conservés (\text{…}).
    Texte(String),
    Groupe(Vec<Noeud>),
    Frac {
        num: Vec<Noeud>,
        den: Vec<Noeud>,
        display: bool,
        barre: bool,
    },
    Racine {
        indice: Option<Vec<Noeud>>,
        corps: Vec<Noeud>,
    },
    Scripts {
        base: Box<Noeud>,
        sub: Option<Vec<Noeud>>,
        sup: Option<Vec<Noeud>>,
    },
    /// Grand opérateur (∑, ∫) ou nom de fonction (lim, log).
    GrandOp {
        txt: String,
        limites: bool,
        texte: bool,
    },
    Delims {
        gauche: String,
        corps: Vec<Noeud>,
        droite: String,
    },
    /// Espace explicite, en em.
    Espace(f32),
    XFleche {
        fleche: Fleche,
        label: Vec<Noeud>,
    },
    Accent {
        accent: Accent,
        corps: Vec<Noeud>,
    },
    Pile {
        dessus: Option<Vec<Noeud>>,
        base: Vec<Noeud>,
        dessous: Option<Vec<Noeud>>,
    },
}

impl Noeud {
    fn sym(txt: &str, classe: Classe) -> Self {
        Noeud::Sym {
            txt: txt.to_string(),
            classe,
            italique: false,
            repli: None,
        }
    }

    fn classe(&self) -> Option<Classe> {
        Some(match self {
            Noeud::Sym { classe, .. } => *classe,
            Noeud::Scripts { base, .. } => base.classe().unwrap_or(Classe::Ord),
            Noeud::GrandOp { .. } => Classe::Op,
            Noeud::Delims { .. } => Classe::Inner,
            Noeud::XFleche { .. } => Classe::Rel,
            Noeud::Pile { base, .. } => {
                if base.len() == 1 && base[0].classe() == Some(Classe::Rel) {
                    Classe::Rel
                } else {
                    Classe::Ord
                }
            }
            Noeud::Espace(_) => return None,
            _ => Classe::Ord,
        })
    }
}

/// Symboles nommés : commande → (caractère, classe).
#[rustfmt::skip]
fn symbole(nom: &str) -> Option<(&'static str, Classe)> {
    use Classe::*;
    Some(match nom {
        // Grec minuscule
        "alpha" => ("α", Ord), "beta" => ("β", Ord), "gamma" => ("γ", Ord),
        "delta" => ("δ", Ord), "epsilon" => ("ϵ", Ord), "varepsilon" => ("ε", Ord),
        "zeta" => ("ζ", Ord), "eta" => ("η", Ord), "theta" => ("θ", Ord),
        "vartheta" => ("ϑ", Ord), "iota" => ("ι", Ord), "kappa" => ("κ", Ord),
        "lambda" => ("λ", Ord), "mu" => ("μ", Ord), "nu" => ("ν", Ord), "xi" => ("ξ", Ord),
        "omicron" => ("ο", Ord), "pi" => ("π", Ord), "varpi" => ("ϖ", Ord),
        "rho" => ("ρ", Ord), "varrho" => ("ϱ", Ord), "sigma" => ("σ", Ord),
        "varsigma" => ("ς", Ord), "tau" => ("τ", Ord), "upsilon" => ("υ", Ord),
        "phi" => ("ϕ", Ord), "varphi" => ("φ", Ord), "chi" => ("χ", Ord),
        "psi" => ("ψ", Ord), "omega" => ("ω", Ord),
        // Grec majuscule
        "Gamma" => ("Γ", Ord), "Delta" => ("Δ", Ord), "Theta" => ("Θ", Ord),
        "Lambda" => ("Λ", Ord), "Xi" => ("Ξ", Ord), "Pi" => ("Π", Ord),
        "Sigma" => ("Σ", Ord), "Upsilon" => ("Υ", Ord), "Phi" => ("Φ", Ord),
        "Psi" => ("Ψ", Ord), "Omega" => ("Ω", Ord),
        // Divers
        "ell" => ("ℓ", Ord), "infty" => ("∞", Ord), "partial" => ("∂", Ord),
        "nabla" => ("∇", Ord), "hbar" => ("ℏ", Ord), "forall" => ("∀", Ord),
        "exists" => ("∃", Ord), "emptyset" | "varnothing" => ("∅", Ord),
        "neg" | "lnot" => ("¬", Ord), "angle" => ("∠", Ord), "degree" | "circ" => ("°", Ord),
        "prime" => ("′", Ord), "dots" | "ldots" => ("…", Inner), "cdots" => ("⋯", Inner),
        "vdots" => ("⋮", Ord), "ddots" => ("⋱", Ord), "Re" => ("ℜ", Ord), "Im" => ("ℑ", Ord),
        "aleph" => ("ℵ", Ord), "triangle" => ("△", Ord), "square" | "Box" => ("□", Ord),
        "checkmark" => ("✓", Ord), "star" => ("⋆", Bin), "dagger" => ("†", Ord),
        "%" => ("%", Ord), "$" => ("$", Ord), "#" => ("#", Ord), "&" => ("&", Ord),
        "_" => ("_", Ord), "backslash" => ("\\", Ord), "|" | "Vert" => ("‖", Ord),
        "vert" => ("|", Ord),
        "{" | "lbrace" => ("{", Ouv), "}" | "rbrace" => ("}", Ferm),
        "langle" => ("⟨", Ouv), "rangle" => ("⟩", Ferm),
        "lfloor" => ("⌊", Ouv), "rfloor" => ("⌋", Ferm),
        "lceil" => ("⌈", Ouv), "rceil" => ("⌉", Ferm),
        // Opérateurs binaires
        "cdot" => ("⋅", Bin), "times" => ("×", Bin), "pm" => ("±", Bin), "mp" => ("∓", Bin),
        "div" => ("÷", Bin), "ast" => ("∗", Bin), "bullet" => ("•", Bin),
        "cup" => ("∪", Bin), "cap" => ("∩", Bin), "wedge" | "land" => ("∧", Bin),
        "vee" | "lor" => ("∨", Bin), "oplus" => ("⊕", Bin), "otimes" => ("⊗", Bin),
        "ominus" => ("⊖", Bin), "odot" => ("⊙", Bin), "setminus" => ("∖", Bin),
        // Relations
        "approx" => ("≈", Rel), "neq" | "ne" => ("≠", Rel), "leq" | "le" => ("≤", Rel),
        "geq" | "ge" => ("≥", Rel), "leqslant" => ("⩽", Rel), "geqslant" => ("⩾", Rel),
        "ll" => ("≪", Rel), "gg" => ("≫", Rel), "sim" => ("∼", Rel), "simeq" => ("≃", Rel),
        "equiv" => ("≡", Rel), "cong" => ("≅", Rel), "propto" => ("∝", Rel),
        "doteq" => ("≐", Rel), "triangleq" => ("≜", Rel), "coloneqq" => ("≔", Rel),
        "to" | "rightarrow" => ("→", Rel), "leftarrow" | "gets" => ("←", Rel),
        "leftrightarrow" => ("↔", Rel), "Rightarrow" => ("⇒", Rel), "Leftarrow" => ("⇐", Rel),
        "Leftrightarrow" | "iff" => ("⇔", Rel), "implies" => ("⟹", Rel),
        "impliedby" => ("⟸", Rel), "longrightarrow" => ("⟶", Rel),
        "longleftarrow" => ("⟵", Rel), "longleftrightarrow" => ("⟷", Rel),
        "Longrightarrow" => ("⟹", Rel), "Longleftrightarrow" => ("⟺", Rel),
        "mapsto" => ("↦", Rel), "uparrow" => ("↑", Rel), "downarrow" => ("↓", Rel),
        "in" => ("∈", Rel), "notin" => ("∉", Rel), "ni" => ("∋", Rel),
        "subset" => ("⊂", Rel), "subseteq" => ("⊆", Rel), "supset" => ("⊃", Rel),
        "supseteq" => ("⊇", Rel), "perp" => ("⊥", Rel), "parallel" => ("∥", Rel),
        "mid" => ("∣", Rel), "models" => ("⊨", Rel), "vdash" => ("⊢", Rel),
        _ => return None,
    })
}

/// Grands opérateurs : commande → (symbole, limites dessus/dessous ?).
fn grand_operateur(nom: &str) -> Option<(&'static str, bool)> {
    Some(match nom {
        "sum" => ("∑", true),
        "prod" => ("∏", true),
        "coprod" => ("∐", true),
        "bigcup" => ("⋃", true),
        "bigcap" => ("⋂", true),
        "bigoplus" => ("⨁", true),
        "bigotimes" => ("⨂", true),
        "int" => ("∫", false),
        "iint" => ("∬", false),
        "iiint" => ("∭", false),
        "oint" => ("∮", false),
        _ => return None,
    })
}

/// Noms de fonctions écrits en romain : commande → limites dessus/dessous ?
fn fonction(nom: &str) -> Option<bool> {
    Some(match nom {
        "lim" | "liminf" | "limsup" | "max" | "min" | "sup" | "inf" | "det" | "gcd" | "Pr"
        | "argmax" | "argmin" => true,
        "sin" | "cos" | "tan" | "cot" | "sec" | "csc" | "arcsin" | "arccos" | "arctan" | "sinh"
        | "cosh" | "tanh" | "coth" | "log" | "ln" | "lg" | "exp" | "arg" | "deg" | "dim"
        | "hom" | "ker" | "sgn" | "sinc" | "rect" | "tr" | "Re" | "Im" => false,
        _ => return None,
    })
}

#[rustfmt::skip]
fn lettre_speciale(c: char, style: &str) -> Option<char> {
    if !c.is_ascii_alphabetic() {
        return None;
    }
    let maj = c.is_ascii_uppercase();
    let idx = (c.to_ascii_uppercase() as u32) - ('A' as u32);
    let special = match (style, c) {
        ("mathbb", 'C') => 'ℂ', ("mathbb", 'H') => 'ℍ', ("mathbb", 'N') => 'ℕ',
        ("mathbb", 'P') => 'ℙ', ("mathbb", 'Q') => 'ℚ', ("mathbb", 'R') => 'ℝ',
        ("mathbb", 'Z') => 'ℤ',
        ("mathcal" | "mathscr", 'B') => 'ℬ', ("mathcal" | "mathscr", 'E') => 'ℰ',
        ("mathcal" | "mathscr", 'F') => 'ℱ', ("mathcal" | "mathscr", 'H') => 'ℋ',
        ("mathcal" | "mathscr", 'I') => 'ℐ', ("mathcal" | "mathscr", 'L') => 'ℒ',
        ("mathcal" | "mathscr", 'M') => 'ℳ', ("mathcal" | "mathscr", 'R') => 'ℛ',
        ("mathfrak", 'C') => 'ℭ', ("mathfrak", 'H') => 'ℌ', ("mathfrak", 'I') => 'ℑ',
        ("mathfrak", 'R') => 'ℜ', ("mathfrak", 'Z') => 'ℨ',
        _ => {
            let base = match (style, maj) {
                ("mathbb", true) => 0x1D538,
                ("mathbb", false) => 0x1D552,
                ("mathcal" | "mathscr", true) => 0x1D49C,
                ("mathcal" | "mathscr", false) => 0x1D4B6,
                ("mathfrak", true) => 0x1D504,
                ("mathfrak", false) => 0x1D51E,
                _ => return None,
            };
            return char::from_u32(base + idx);
        }
    };
    Some(special)
}

// ===========================================================================
// Analyse syntaxique
// ===========================================================================
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fin {
    Eof,
    Accolade,
    Crochet,
    Right,
}

struct Analyseur {
    s: Vec<char>,
    i: usize,
}

impl Analyseur {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    fn sauter_espaces(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.i += 1;
        }
    }

    /// Lit un nom de commande (après le `\`). Tolère `\\cmd` (backslash doublé par erreur).
    fn commande(&mut self) -> String {
        if self.peek() == Some('\\')
            && self
                .s
                .get(self.i + 1)
                .is_some_and(|c| c.is_ascii_alphabetic())
        {
            self.i += 1;
        }
        let debut = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
            self.i += 1;
        }
        if self.i == debut {
            match self.peek() {
                Some(c) => {
                    self.i += 1;
                    return c.to_string();
                }
                None => return String::new(),
            }
        }
        let mut nom: String = self.s[debut..self.i].iter().collect();
        if self.peek() == Some('*') && nom == "operatorname" {
            self.i += 1;
            nom.push('*');
        }
        nom
    }

    /// Prochaine commande sans la consommer.
    fn regarder_commande(&self) -> Option<String> {
        if self.peek() != Some('\\') {
            return None;
        }
        let mut j = self.i + 1;
        if self.s.get(j) == Some(&'\\') && self.s.get(j + 1).is_some_and(char::is_ascii_alphabetic)
        {
            j += 1;
        }
        let debut = j;
        while self.s.get(j).is_some_and(char::is_ascii_alphabetic) {
            j += 1;
        }
        Some(self.s[debut..j].iter().collect())
    }

    /// Contenu brut d'un argument `{…}` (pour \text), accolades imbriquées incluses.
    fn texte_brut(&mut self) -> String {
        self.sauter_espaces();
        if self.peek() != Some('{') {
            return self
                .peek()
                .map(|c| {
                    self.i += 1;
                    c.to_string()
                })
                .unwrap_or_default();
        }
        self.i += 1;
        let mut profondeur = 1;
        let mut res = String::new();
        while let Some(c) = self.peek() {
            self.i += 1;
            match c {
                '{' => profondeur += 1,
                '}' => {
                    profondeur -= 1;
                    if profondeur == 0 {
                        break;
                    }
                }
                '\\' => {
                    // Échappements usuels dans \text : \%, \&, \_, \{, \}, \$
                    if let Some(n) = self.peek().filter(|n| "%&_{}$# ".contains(*n)) {
                        self.i += 1;
                        res.push(n);
                        continue;
                    }
                }
                _ => {}
            }
            res.push(c);
        }
        res
    }

    /// Argument : `{…}` ou un seul atome.
    fn argument(&mut self) -> Vec<Noeud> {
        self.sauter_espaces();
        match self.peek() {
            Some('{') => {
                self.i += 1;
                self.liste(Fin::Accolade)
            }
            Some(_) => self.atome().into_iter().collect(),
            None => Vec::new(),
        }
    }

    fn argument_optionnel(&mut self) -> Option<Vec<Noeud>> {
        self.sauter_espaces();
        if self.peek() == Some('[') {
            self.i += 1;
            Some(self.liste(Fin::Crochet))
        } else {
            None
        }
    }

    fn delimiteur(&mut self) -> String {
        self.sauter_espaces();
        match self.peek() {
            Some('\\') => {
                self.i += 1;
                let nom = self.commande();
                match nom.as_str() {
                    "{" | "lbrace" => "{",
                    "}" | "rbrace" => "}",
                    "|" | "Vert" | "lVert" | "rVert" => "‖",
                    "vert" | "lvert" | "rvert" => "|",
                    "langle" => "⟨",
                    "rangle" => "⟩",
                    "lfloor" => "⌊",
                    "rfloor" => "⌋",
                    "lceil" => "⌈",
                    "rceil" => "⌉",
                    _ => "",
                }
                .to_string()
            }
            Some('.') => {
                self.i += 1;
                String::new()
            }
            Some(c) => {
                self.i += 1;
                c.to_string()
            }
            None => String::new(),
        }
    }

    fn liste(&mut self, fin: Fin) -> Vec<Noeud> {
        let mut noeuds: Vec<Noeud> = Vec::new();
        loop {
            self.sauter_espaces();
            let Some(c) = self.peek() else { break };
            match c {
                '}' => {
                    self.i += 1;
                    if fin == Fin::Accolade {
                        break;
                    }
                }
                ']' if fin == Fin::Crochet => {
                    self.i += 1;
                    break;
                }
                '^' | '_' => {
                    self.i += 1;
                    let arg = self.argument();
                    attacher_script(&mut noeuds, c == '^', arg);
                }
                '\'' => {
                    self.i += 1;
                    attacher_script(&mut noeuds, true, vec![Noeud::sym("′", Classe::Ord)]);
                }
                '\\' if self.regarder_commande().as_deref() == Some("right") => {
                    if fin == Fin::Right {
                        break;
                    }
                    // \right isolé : on l'ignore.
                    self.i += 1;
                    self.commande();
                    self.delimiteur();
                }
                '\\' if matches!(
                    self.regarder_commande().as_deref(),
                    Some("limits" | "nolimits")
                ) =>
                {
                    self.i += 1;
                    let avec = self.commande() == "limits";
                    if let Some(Noeud::GrandOp { limites, .. }) = noeuds.last_mut() {
                        *limites = avec;
                    }
                }
                _ => {
                    if let Some(n) = self.atome() {
                        noeuds.push(n);
                    }
                }
            }
        }
        noeuds
    }

    fn atome(&mut self) -> Option<Noeud> {
        let c = self.peek()?;
        self.i += 1;
        use Classe::*;
        Some(match c {
            '\\' => return self.commande_atome(),
            '{' => Noeud::Groupe(self.liste(Fin::Accolade)),
            '0'..='9' | '.' => {
                let debut = self.i - 1;
                while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '.') {
                    self.i += 1;
                }
                Noeud::sym(&self.s[debut..self.i].iter().collect::<String>(), Ord)
            }
            c if c.is_ascii_alphabetic() => Noeud::Sym {
                txt: c.to_string(),
                classe: Ord,
                italique: true,
                repli: None,
            },
            '+' => Noeud::sym("+", Bin),
            '-' => Noeud::sym("−", Bin),
            '*' => Noeud::sym("∗", Bin),
            '=' | '<' | '>' | ':' => Noeud::sym(&c.to_string(), Rel),
            ',' | ';' => Noeud::sym(&c.to_string(), Ponct),
            '(' | '[' => Noeud::sym(&c.to_string(), Ouv),
            ')' | ']' | '!' | '?' => Noeud::sym(&c.to_string(), Ferm),
            '~' => Noeud::Espace(0.33),
            '&' => return None,
            '·' => Noeud::sym("⋅", Bin),
            c => Noeud::sym(&c.to_string(), Ord),
        })
    }

    fn commande_atome(&mut self) -> Option<Noeud> {
        let nom = self.commande();
        use Classe::*;
        Some(match nom.as_str() {
            "" => return None,
            "frac" | "dfrac" | "tfrac" | "cfrac" => {
                let num = self.argument();
                let den = self.argument();
                Noeud::Frac {
                    num,
                    den,
                    display: matches!(nom.as_str(), "dfrac" | "cfrac"),
                    barre: true,
                }
            }
            "binom" | "dbinom" | "tbinom" => {
                let num = self.argument();
                let den = self.argument();
                Noeud::Delims {
                    gauche: "(".into(),
                    corps: vec![Noeud::Frac {
                        num,
                        den,
                        display: nom == "dbinom",
                        barre: false,
                    }],
                    droite: ")".into(),
                }
            }
            "sqrt" => {
                let indice = self.argument_optionnel();
                Noeud::Racine {
                    indice,
                    corps: self.argument(),
                }
            }
            "left" => {
                let gauche = self.delimiteur();
                let corps = self.liste(Fin::Right);
                if self.regarder_commande().as_deref() == Some("right") {
                    self.i += 1;
                    self.commande();
                }
                let droite = self.delimiteur();
                Noeud::Delims {
                    gauche,
                    corps,
                    droite,
                }
            }
            "big" | "Big" | "bigg" | "Bigg" | "bigl" | "bigr" | "Bigl" | "Bigr" | "biggl"
            | "biggr" | "Biggl" | "Biggr" => {
                let d = self.delimiteur();
                let classe = if nom.ends_with('l') {
                    Ouv
                } else if nom.ends_with('r') {
                    Ferm
                } else {
                    Ord
                };
                Noeud::sym(&d, classe)
            }
            "text" | "textrm" | "textup" | "textnormal" | "mbox" | "textsf" | "texttt"
            | "textbf" | "textit" | "hbox" => Noeud::Texte(self.texte_brut()),
            "operatorname" | "operatorname*" => Noeud::GrandOp {
                txt: self.texte_brut(),
                limites: nom.ends_with('*'),
                texte: true,
            },
            "mathrm" | "mathup" | "mathsf" | "mathtt" | "mathbf" | "boldsymbol" | "bm" => {
                Noeud::Groupe(styliser(self.argument(), Some(false), None))
            }
            "mathit" => Noeud::Groupe(styliser(self.argument(), Some(true), None)),
            "mathbb" | "mathcal" | "mathscr" | "mathfrak" => {
                Noeud::Groupe(styliser(self.argument(), Some(false), Some(&nom)))
            }
            "vec" | "overrightarrow" => Noeud::Accent {
                accent: Accent::Vecteur,
                corps: self.argument(),
            },
            "hat" | "widehat" => Noeud::Accent {
                accent: Accent::Chapeau,
                corps: self.argument(),
            },
            "bar" | "overline" => Noeud::Accent {
                accent: Accent::Barre,
                corps: self.argument(),
            },
            "tilde" | "widetilde" => Noeud::Accent {
                accent: Accent::Tilde,
                corps: self.argument(),
            },
            "dot" => Noeud::Accent {
                accent: Accent::Point,
                corps: self.argument(),
            },
            "ddot" => Noeud::Accent {
                accent: Accent::DeuxPoints,
                corps: self.argument(),
            },
            "underline" => Noeud::Accent {
                accent: Accent::Souligne,
                corps: self.argument(),
            },
            "xrightarrow" | "xRightarrow" | "xleftarrow" | "xLeftarrow" | "xleftrightarrow"
            | "xLeftrightarrow" => {
                let _dessous = self.argument_optionnel();
                let fleche = match nom.as_str() {
                    "xleftarrow" | "xLeftarrow" => Fleche::Gauche,
                    "xleftrightarrow" | "xLeftrightarrow" => Fleche::Double,
                    _ => Fleche::Droite,
                };
                Noeud::XFleche {
                    fleche,
                    label: self.argument(),
                }
            }
            "overset" | "stackrel" => {
                let dessus = self.argument();
                Noeud::Pile {
                    dessus: Some(dessus),
                    base: self.argument(),
                    dessous: None,
                }
            }
            "underset" => {
                let dessous = self.argument();
                Noeud::Pile {
                    dessus: None,
                    base: self.argument(),
                    dessous: Some(dessous),
                }
            }
            "overbrace" | "underbrace" | "boxed" | "phantom" | "mathstrut" => {
                Noeud::Groupe(self.argument())
            }
            "displaystyle" | "textstyle" | "scriptstyle" | "scriptscriptstyle" | "limits"
            | "nolimits" | "nonumber" | "notag" => return None,
            "," | "thinspace" => Noeud::Espace(3.0 / 18.0),
            ":" | ">" | "medspace" => Noeud::Espace(4.0 / 18.0),
            ";" | "thickspace" => Noeud::Espace(5.0 / 18.0),
            "!" | "negthinspace" => Noeud::Espace(-3.0 / 18.0),
            " " => Noeud::Espace(0.25),
            "enspace" => Noeud::Espace(0.5),
            "quad" => Noeud::Espace(1.0),
            "qquad" => Noeud::Espace(2.0),
            // Saut de ligne : sans objet dans une formule en ligne.
            "\\" | "newline" => Noeud::Espace(0.5),
            autre => {
                if let Some((s, limites)) = grand_operateur(autre) {
                    Noeud::GrandOp {
                        txt: s.to_string(),
                        limites,
                        texte: false,
                    }
                } else if let Some(limites) = fonction(autre) {
                    Noeud::GrandOp {
                        txt: autre.to_string(),
                        limites,
                        texte: true,
                    }
                } else if let Some((s, classe)) = symbole(autre) {
                    Noeud::sym(s, classe)
                } else {
                    // Commande inconnue : affichée telle quelle.
                    Noeud::Texte(format!("\\{autre}"))
                }
            }
        })
    }
}

fn attacher_script(noeuds: &mut Vec<Noeud>, exposant: bool, arg: Vec<Noeud>) {
    let base = noeuds.pop().unwrap_or(Noeud::Groupe(Vec::new()));
    let nouveau = match base {
        Noeud::Scripts {
            base,
            mut sub,
            mut sup,
        } if (exposant && sup.is_none()) || (!exposant && sub.is_none()) => {
            if exposant {
                sup = Some(arg)
            } else {
                sub = Some(arg)
            }
            Noeud::Scripts { base, sub, sup }
        }
        Noeud::Scripts {
            base,
            sub,
            sup: Some(mut s),
        } if exposant => {
            // Primes successifs : x'' → x^{′′}
            s.extend(arg);
            Noeud::Scripts {
                base,
                sub,
                sup: Some(s),
            }
        }
        autre => {
            let (sub, sup) = if exposant {
                (None, Some(arg))
            } else {
                (Some(arg), None)
            };
            Noeud::Scripts {
                base: Box::new(autre),
                sub,
                sup,
            }
        }
    };
    noeuds.push(nouveau);
}

/// Applique un style de police (\mathrm, \mathbb…) aux symboles d'une liste.
fn styliser(noeuds: Vec<Noeud>, italique: Option<bool>, alphabet: Option<&str>) -> Vec<Noeud> {
    noeuds
        .into_iter()
        .map(|n| match n {
            Noeud::Sym {
                txt,
                classe,
                italique: it,
                repli,
            } => {
                let (txt, repli) = match alphabet {
                    Some(a) => {
                        let conv: String = txt
                            .chars()
                            .map(|c| lettre_speciale(c, a).unwrap_or(c))
                            .collect();
                        if conv != txt {
                            (conv, Some(txt))
                        } else {
                            (txt, repli)
                        }
                    }
                    None => (txt, repli),
                };
                Noeud::Sym {
                    txt,
                    classe,
                    italique: italique.unwrap_or(it),
                    repli,
                }
            }
            Noeud::Groupe(g) => Noeud::Groupe(styliser(g, italique, alphabet)),
            autre => autre,
        })
        .collect()
}

fn analyser_formule(src: &str) -> Vec<Noeud> {
    Analyseur {
        s: src.chars().collect(),
        i: 0,
    }
    .liste(Fin::Eof)
}

// ===========================================================================
// Texte riche : alternance de texte et de formules $…$
// ===========================================================================
enum Segment {
    Texte(String),
    Formule(Vec<Noeud>),
}

fn analyser_texte(texte: &str) -> Vec<Segment> {
    // Découpe sur les `$` non échappés ; un nombre impair de `$` = texte brut.
    let mut morceaux: Vec<String> = vec![String::new()];
    let mut chars = texte.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'$') => {
                chars.next();
                morceaux.last_mut().unwrap().push_str("\\$");
            }
            '$' => morceaux.push(String::new()),
            c => morceaux.last_mut().unwrap().push(c),
        }
    }
    if morceaux.len().is_multiple_of(2) {
        return vec![Segment::Texte(texte.replace("\\$", "$"))];
    }
    morceaux
        .into_iter()
        .enumerate()
        .filter(|(_, m)| !m.is_empty())
        .map(|(i, m)| {
            if i % 2 == 1 {
                Segment::Formule(analyser_formule(&m))
            } else {
                Segment::Texte(m.replace("\\$", "$"))
            }
        })
        .collect()
}

thread_local! {
    static CACHE: RefCell<HashMap<String, Rc<Vec<Segment>>>> = RefCell::new(HashMap::new());
}

/// Analyse (avec cache) d'un texte pouvant contenir des formules.
fn segments(texte: &str) -> Rc<Vec<Segment>> {
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if let Some(s) = c.get(texte) {
            return Rc::clone(s);
        }
        if c.len() > 4096 {
            c.clear();
        }
        let s = Rc::new(analyser_texte(texte));
        c.insert(texte.to_string(), Rc::clone(&s));
        s
    })
}

// ===========================================================================
// Boîtes et mise en page
// ===========================================================================
#[derive(Clone)]
enum Elem {
    /// Galerie de texte, positionnée par son coin haut-gauche.
    Texte(Pos2, Arc<Galley>),
    Rect(Rect),
    Trait(Vec<Pos2>, f32),
    Disque(Pos2, f32),
}

/// Boîte TeX : origine sur la ligne de base, à gauche. `h` au-dessus, `p` en dessous.
#[derive(Clone, Default)]
struct Boite {
    l: f32,
    h: f32,
    p: f32,
    elems: Vec<Elem>,
}

impl Boite {
    /// Place `b` avec sa ligne de base en (x, y) (y vers le bas).
    fn placer(&mut self, b: &Boite, x: f32, y: f32) {
        let d = vec2(x, y);
        self.elems.extend(b.elems.iter().map(|e| match e {
            Elem::Texte(p, g) => Elem::Texte(*p + d, Arc::clone(g)),
            Elem::Rect(r) => Elem::Rect(r.translate(d)),
            Elem::Trait(pts, ep) => Elem::Trait(pts.iter().map(|p| *p + d).collect(), *ep),
            Elem::Disque(c, r) => Elem::Disque(*c + d, *r),
        }));
        self.h = self.h.max(b.h - y);
        self.p = self.p.max(b.p + y);
        self.l = self.l.max(x + b.l);
    }

    fn englober(&mut self, haut: f32, bas: f32) {
        self.h = self.h.max(-haut);
        self.p = self.p.max(bas);
    }
}

const POLICE_MATH: FontFamily = FontFamily::Proportional;

#[derive(Clone, Copy)]
struct Style {
    taille: f32,
    /// Taille du texte courant (référence pour les tailles minimales).
    base: f32,
    /// Style « script » (indices, exposants, fractions en ligne) : espacement réduit.
    script: bool,
}

impl Style {
    fn reduit(self, facteur: f32) -> Style {
        Style {
            taille: (self.taille * facteur).max(self.base * 0.5),
            base: self.base,
            script: true,
        }
    }
    fn epaisseur(self) -> f32 {
        (self.taille * 0.055).max(1.0)
    }
}

struct Moteur<'a, 'b> {
    f: &'a mut FontsView<'b>,
}

impl Moteur<'_, '_> {
    fn galley(&mut self, txt: &str, taille: f32, italique: bool) -> Arc<Galley> {
        let job = LayoutJob::single_section(
            txt.to_string(),
            TextFormat {
                font_id: FontId::new(taille, POLICE_MATH),
                color: Color32::WHITE,
                italics: italique,
                ..Default::default()
            },
        );
        self.f.layout_job(job)
    }

    fn ligne_de_base(g: &Galley, taille: f32) -> f32 {
        g.rows
            .first()
            .and_then(|r| r.row.glyphs.first().map(|gl| r.pos.y + gl.pos.y))
            .unwrap_or(taille * 0.8)
    }

    /// Texte dont la hauteur suit l'encre réelle des glyphes (mise en page serrée).
    fn texte_encre(&mut self, txt: &str, taille: f32, italique: bool) -> Boite {
        let g = self.galley(txt, taille, italique);
        let base = Self::ligne_de_base(&g, taille);
        let (h, p) = if g.mesh_bounds.is_positive() {
            (base - g.mesh_bounds.min.y, g.mesh_bounds.max.y - base)
        } else {
            (0.0, 0.0)
        };
        Boite {
            l: g.size().x,
            h,
            p,
            elems: vec![Elem::Texte(pos2(0.0, -base), g)],
        }
    }

    /// Texte avec les métriques de la police (hauteur de ligne régulière).
    fn texte_police(&mut self, txt: &str, taille: f32, gras: bool) -> Boite {
        let family = if gras {
            FontFamily::Name("Gras".into())
        } else {
            POLICE_MATH
        };
        let g = self.f.layout_job(LayoutJob::single_section(
            txt.to_string(),
            TextFormat {
                font_id: FontId::new(taille, family),
                color: Color32::WHITE,
                ..Default::default()
            },
        ));
        let base = Self::ligne_de_base(&g, taille);
        let (asc, haut) = g
            .rows
            .first()
            .and_then(|r| {
                r.row
                    .glyphs
                    .first()
                    .map(|gl| (gl.font_ascent, gl.font_height))
            })
            .unwrap_or((taille * 0.93, taille * 1.17));
        Boite {
            l: g.size().x,
            h: asc,
            p: haut - asc,
            elems: vec![Elem::Texte(pos2(0.0, -base), g)],
        }
    }

    /// Hauteur de l'axe mathématique (centre du signe −) au-dessus de la ligne de base.
    fn axe(&mut self, taille: f32) -> f32 {
        let b = self.texte_encre("−", taille, false);
        (b.h - b.p) / 2.0
    }

    fn espace(&mut self, taille: f32) -> f32 {
        self.f.glyph_width(&FontId::new(taille, POLICE_MATH), ' ')
    }

    // --- Atomes ------------------------------------------------------------
    fn noeud(&mut self, n: &Noeud, st: Style) -> Boite {
        match n {
            Noeud::Sym {
                txt,
                italique,
                repli,
                ..
            } => {
                let fid = FontId::new(st.taille, POLICE_MATH);
                let txt = match repli {
                    Some(r) if !self.f.has_glyphs(&fid, txt) => r,
                    _ => txt,
                };
                let mut b = self.texte_encre(txt, st.taille, *italique);
                if *italique {
                    // Correction d'italique : l'inclinaison fait déborder le haut des
                    // lettres sur le symbole suivant, surtout pour les lettres hautes.
                    let derniere = txt.chars().last().unwrap_or(' ');
                    let corr = match derniere {
                        'f' => 0.17,
                        'b' | 'd' | 'h' | 'k' | 'l' | 't' | 'A'..='Z' => 0.11,
                        _ => 0.05,
                    };
                    b.l += st.taille * corr;
                }
                // Hauteur minimale d'un symbole : celle d'un chiffre (alignement régulier).
                b.h = b.h.max(st.taille * 0.5);
                b
            }
            Noeud::Texte(t) => self.texte_encre(t, st.taille, false),
            Noeud::Groupe(g) => self.hliste(g, st),
            Noeud::Espace(em) => Boite {
                l: em * st.taille,
                ..Default::default()
            },
            Noeud::Frac {
                num,
                den,
                display,
                barre,
            } => self.fraction(num, den, *display, *barre, st),
            Noeud::Racine { indice, corps } => self.racine(indice.as_deref(), corps, st),
            Noeud::Scripts { base, sub, sup } => {
                self.scripts(base, sub.as_deref(), sup.as_deref(), st)
            }
            Noeud::GrandOp { .. } => self.grand_op(n, st),
            Noeud::Delims {
                gauche,
                corps,
                droite,
            } => self.delims(gauche, corps, droite, st),
            Noeud::XFleche { fleche, label } => self.xfleche(*fleche, label, st),
            Noeud::Accent { accent, corps } => self.accent(*accent, corps, st),
            Noeud::Pile {
                dessus,
                base,
                dessous,
            } => {
                let b = self.hliste(base, st);
                let dessus = dessus.as_ref().map(|d| self.hliste(d, st.reduit(0.7)));
                let dessous = dessous.as_ref().map(|d| self.hliste(d, st.reduit(0.7)));
                self.empiler(b, dessus, dessous, st)
            }
        }
    }

    fn fraction(
        &mut self,
        num: &[Noeud],
        den: &[Noeud],
        display: bool,
        barre: bool,
        st: Style,
    ) -> Boite {
        let st_enf = if display {
            st
        } else {
            st.reduit(if st.script { 0.8 } else { 0.85 })
        };
        let n = self.hliste(num, st_enf);
        let d = self.hliste(den, st_enf);
        let a = self.axe(st.taille);
        let t = if barre { st.epaisseur() } else { 0.0 };
        let ecart = st.taille * if display { 0.2 } else { 0.14 };
        let marge = st.taille * 0.12;
        let l = n.l.max(d.l) + 2.0 * marge;
        let mut b = Boite::default();
        b.placer(&n, (l - n.l) / 2.0, -a - t / 2.0 - ecart - n.p);
        b.placer(&d, (l - d.l) / 2.0, -a + t / 2.0 + ecart + d.h);
        if barre {
            b.elems.push(Elem::Rect(Rect::from_min_max(
                pos2(marge * 0.5, -a - t / 2.0),
                pos2(l - marge * 0.5, -a + t / 2.0),
            )));
        }
        b.l = l;
        b
    }

    fn racine(&mut self, indice: Option<&[Noeud]>, corps: &[Noeud], st: Style) -> Boite {
        let c = self.hliste(corps, st);
        let t = st.epaisseur();
        let ecart = st.taille * 0.12;
        let haut = -(c.h.max(st.taille * 0.6) + ecart + t / 2.0);
        let bas = c.p.max(st.taille * 0.05) + st.taille * 0.04;
        let largeur_signe = st.taille * 0.55;
        let milieu = haut + (bas - haut) * 0.62;

        let ind = indice.map(|i| self.hliste(i, st.reduit(0.6)));
        let decal = ind
            .as_ref()
            .map_or(0.0, |i| (i.l - largeur_signe * 0.45).max(0.0));
        let x0 = decal;
        let x_fin = x0 + largeur_signe + c.l + st.taille * 0.08;

        let mut b = Boite::default();
        if let Some(i) = &ind {
            b.placer(
                i,
                x0 + largeur_signe * 0.45 - i.l,
                milieu - st.taille * 0.06 - i.p,
            );
        }
        b.elems.push(Elem::Trait(
            vec![
                pos2(x0, milieu + st.taille * 0.04),
                pos2(x0 + largeur_signe * 0.18, milieu - st.taille * 0.03),
                pos2(x0 + largeur_signe * 0.45, bas),
                pos2(x0 + largeur_signe, haut),
                pos2(x_fin, haut),
            ],
            t,
        ));
        b.placer(&c, x0 + largeur_signe, 0.0);
        b.englober(haut - t, bas);
        b.l = x_fin + st.taille * 0.04;
        b
    }

    fn scripts(
        &mut self,
        base: &Noeud,
        sub: Option<&[Noeud]>,
        sup: Option<&[Noeud]>,
        st: Style,
    ) -> Boite {
        if let Noeud::GrandOp { limites: true, .. } = base {
            let b = self.noeud(base, st);
            let dessus = sup.map(|s| self.hliste(s, st.reduit(0.7)));
            let dessous = sub.map(|s| self.hliste(s, st.reduit(0.7)));
            return self.empiler(b, dessus, dessous, st);
        }
        let b = self.noeud(base, st);
        let st_s = st.reduit(0.7);
        let s = sup.map(|s| self.hliste(s, st_s));
        let d = sub.map(|d| self.hliste(d, st_s));
        let ta = st.taille;

        let mut u = (b.h - ta * 0.3).max(ta * 0.36);
        let mut v = (b.p + ta * 0.05).max(ta * 0.16);
        if let Some(s) = &s {
            u = u.max(s.p + ta * 0.2);
        }
        if let Some(d) = &d {
            v = v.max(d.h - ta * 0.42);
        }
        if let (Some(s), Some(d)) = (&s, &d) {
            let jeu = (u - s.p) - (d.h - v);
            let min = ta * 0.12;
            if jeu < min {
                v += min - jeu;
            }
        }
        let mut r = Boite::default();
        r.placer(&b, 0.0, 0.0);
        let x = b.l;
        if let Some(s) = &s {
            r.placer(s, x, -u);
        }
        if let Some(d) = &d {
            let correction = if matches!(base, Noeud::GrandOp { texte: false, .. }) {
                -ta * 0.2
            } else {
                0.0
            };
            r.placer(d, x + correction, v);
        }
        r.l += ta * 0.04;
        r
    }

    /// Empile des limites au-dessus et en dessous d'une boîte (∑ limites, \overset).
    fn empiler(
        &mut self,
        b: Boite,
        dessus: Option<Boite>,
        dessous: Option<Boite>,
        st: Style,
    ) -> Boite {
        let ecart = st.taille * 0.12;
        let l =
            b.l.max(dessus.as_ref().map_or(0.0, |d| d.l))
                .max(dessous.as_ref().map_or(0.0, |d| d.l));
        let mut r = Boite::default();
        r.placer(&b, (l - b.l) / 2.0, 0.0);
        if let Some(d) = &dessus {
            r.placer(d, (l - d.l) / 2.0, -b.h - ecart - d.p);
        }
        if let Some(d) = &dessous {
            r.placer(d, (l - d.l) / 2.0, b.p + ecart + d.h);
        }
        r.l = l;
        r
    }

    fn grand_op(&mut self, n: &Noeud, st: Style) -> Boite {
        let Noeud::GrandOp { txt, texte, .. } = n else {
            unreachable!()
        };
        if *texte {
            let mut b = self.texte_encre(txt, st.taille, false);
            b.h = b.h.max(st.taille * 0.5);
            return b;
        }
        let facteur = if st.script {
            1.0
        } else if txt == "∫" || txt == "∮" {
            1.5
        } else {
            1.65
        };
        let g = self.texte_encre(txt, st.taille * facteur, false);
        // Centré verticalement sur l'axe mathématique.
        let a = self.axe(st.taille);
        let dy = -a - (g.p - g.h) / 2.0;
        let mut b = Boite::default();
        b.placer(&g, 0.0, dy);
        b
    }

    fn delims(&mut self, gauche: &str, corps: &[Noeud], droite: &str, st: Style) -> Boite {
        let c = self.hliste(corps, st);
        let a = self.axe(st.taille);
        let demi = (c.h - a).max(c.p + a).max(st.taille * 0.45) + st.taille * 0.08;
        let g = self.delimiteur(gauche, demi, a, st);
        let d = self.delimiteur(droite, demi, a, st);
        let mut b = Boite::default();
        b.placer(&g, 0.0, 0.0);
        b.placer(&c, g.l, 0.0);
        b.placer(&d, g.l + c.l, 0.0);
        b
    }

    /// Délimiteur extensible couvrant ±`demi` autour de l'axe.
    fn delimiteur(&mut self, d: &str, demi: f32, a: f32, st: Style) -> Boite {
        if d.is_empty() {
            return Boite {
                l: st.taille * 0.1,
                ..Default::default()
            };
        }
        let normal = self.texte_encre(d, st.taille, false);
        if normal.h + normal.p >= 2.0 * demi * 0.9 {
            return normal;
        }
        let (haut, bas) = (-a - demi, -a + demi);
        let t = st.epaisseur() * 1.1;
        let l = st.taille * 0.4;
        let w = l * 0.6;
        let (x0, x1) = ((l - w) / 2.0, (l + w) / 2.0);
        let mut b = Boite {
            l,
            h: -haut,
            p: bas,
            elems: Vec::new(),
        };
        let mut trait_ = |pts: Vec<Pos2>| b.elems.push(Elem::Trait(pts, t));
        match d {
            "(" | ")" => {
                let n = 24;
                let pts = (0..=n)
                    .map(|i| {
                        let s = i as f32 / n as f32;
                        let bombe = (std::f32::consts::PI * s).sin() * w;
                        let x = if d == "(" { x1 - bombe } else { x0 + bombe };
                        pos2(x, haut + (bas - haut) * s)
                    })
                    .collect();
                trait_(pts);
            }
            "[" => trait_(vec![
                pos2(x1, haut),
                pos2(x0, haut),
                pos2(x0, bas),
                pos2(x1, bas),
            ]),
            "]" => trait_(vec![
                pos2(x0, haut),
                pos2(x1, haut),
                pos2(x1, bas),
                pos2(x0, bas),
            ]),
            "⌊" => trait_(vec![pos2(x0, haut), pos2(x0, bas), pos2(x1, bas)]),
            "⌋" => trait_(vec![pos2(x1, haut), pos2(x1, bas), pos2(x0, bas)]),
            "⌈" => trait_(vec![pos2(x1, haut), pos2(x0, haut), pos2(x0, bas)]),
            "⌉" => trait_(vec![pos2(x0, haut), pos2(x1, haut), pos2(x1, bas)]),
            "⟨" => trait_(vec![pos2(x1, haut), pos2(x0, -a), pos2(x1, bas)]),
            "⟩" => trait_(vec![pos2(x0, haut), pos2(x1, -a), pos2(x0, bas)]),
            "|" => trait_(vec![pos2(l / 2.0, haut), pos2(l / 2.0, bas)]),
            "‖" => {
                trait_(vec![pos2(x0 + w * 0.2, haut), pos2(x0 + w * 0.2, bas)]);
                trait_(vec![pos2(x1 - w * 0.2, haut), pos2(x1 - w * 0.2, bas)]);
            }
            _ => {
                // Accolades et autres : glyphe agrandi.
                let taille = st.taille * (2.0 * demi) / (normal.h + normal.p).max(1.0);
                let g = self.texte_encre(d, taille, false);
                let mut r = Boite::default();
                r.placer(&g, 0.0, -a - (g.p - g.h) / 2.0);
                return r;
            }
        }
        b
    }

    fn xfleche(&mut self, fleche: Fleche, label: &[Noeud], st: Style) -> Boite {
        let lab = self.hliste(label, st.reduit(0.7));
        let a = self.axe(st.taille);
        let t = st.epaisseur();
        let tete = st.taille * 0.28;
        let l = (lab.l + st.taille * 0.8).max(st.taille * 1.6);
        let marge = st.taille * 0.15;
        let mut b = Boite::default();
        b.placer(
            &lab,
            (l - lab.l) / 2.0 + marge,
            -a - st.taille * 0.18 - lab.p,
        );
        let (x0, x1, y) = (marge, l + marge, -a);
        b.elems.push(Elem::Trait(vec![pos2(x0, y), pos2(x1, y)], t));
        if matches!(fleche, Fleche::Droite | Fleche::Double) {
            b.elems.push(Elem::Trait(
                vec![
                    pos2(x1 - tete, y - tete * 0.6),
                    pos2(x1, y),
                    pos2(x1 - tete, y + tete * 0.6),
                ],
                t,
            ));
        }
        if matches!(fleche, Fleche::Gauche | Fleche::Double) {
            b.elems.push(Elem::Trait(
                vec![
                    pos2(x0 + tete, y - tete * 0.6),
                    pos2(x0, y),
                    pos2(x0 + tete, y + tete * 0.6),
                ],
                t,
            ));
        }
        b.englober(y - tete, y + tete);
        b.l = l + 2.0 * marge;
        b
    }

    fn accent(&mut self, accent: Accent, corps: &[Noeud], st: Style) -> Boite {
        let c = self.hliste(corps, st);
        let t = st.epaisseur();
        let ta = st.taille;
        let mut b = Boite::default();
        b.placer(&c, 0.0, 0.0);
        let haut = -c.h.max(ta * 0.5) - ta * 0.12;
        let (x0, x1) = (ta * 0.05, c.l.max(ta * 0.45) - ta * 0.02);
        let xm = (x0 + x1) / 2.0;
        match accent {
            Accent::Vecteur => {
                let y = haut - ta * 0.06;
                let tete = ta * 0.16;
                b.elems.push(Elem::Trait(vec![pos2(x0, y), pos2(x1, y)], t));
                b.elems.push(Elem::Trait(
                    vec![
                        pos2(x1 - tete, y - tete * 0.7),
                        pos2(x1, y),
                        pos2(x1 - tete, y + tete * 0.7),
                    ],
                    t,
                ));
                b.englober(y - tete, 0.0);
            }
            Accent::Chapeau => {
                let y = haut;
                let dx = ((x1 - x0) / 2.0).min(ta * 0.25);
                b.elems.push(Elem::Trait(
                    vec![pos2(xm - dx, y), pos2(xm, y - ta * 0.16), pos2(xm + dx, y)],
                    t,
                ));
                b.englober(y - ta * 0.18, 0.0);
            }
            Accent::Barre => {
                b.elems
                    .push(Elem::Trait(vec![pos2(x0, haut), pos2(x1, haut)], t));
                b.englober(haut - t, 0.0);
            }
            Accent::Tilde => {
                let n = 12;
                let pts = (0..=n)
                    .map(|i| {
                        let s = i as f32 / n as f32;
                        pos2(
                            x0 + (x1 - x0) * s,
                            haut - ta * 0.06 - (s * std::f32::consts::TAU).sin() * ta * 0.05,
                        )
                    })
                    .collect();
                b.elems.push(Elem::Trait(pts, t));
                b.englober(haut - ta * 0.15, 0.0);
            }
            Accent::Point => {
                b.elems
                    .push(Elem::Disque(pos2(xm, haut - ta * 0.04), t * 1.1));
                b.englober(haut - ta * 0.1, 0.0);
            }
            Accent::DeuxPoints => {
                b.elems
                    .push(Elem::Disque(pos2(xm - ta * 0.1, haut - ta * 0.04), t * 1.1));
                b.elems
                    .push(Elem::Disque(pos2(xm + ta * 0.1, haut - ta * 0.04), t * 1.1));
                b.englober(haut - ta * 0.1, 0.0);
            }
            Accent::Souligne => {
                let y = c.p + ta * 0.1;
                b.elems
                    .push(Elem::Trait(vec![pos2(0.0, y), pos2(c.l, y)], t));
                b.englober(0.0, y + t);
            }
        }
        b
    }

    // --- Listes d'atomes ---------------------------------------------------
    /// Dispose une liste d'atomes. Renvoie les atomes avec leur classe effective
    /// et l'espacement TeX à insérer avant chacun.
    fn atomes(&mut self, noeuds: &[Noeud], st: Style) -> Vec<(Boite, Option<Classe>, f32)> {
        // Classes effectives : un « binaire » en tête ou après un opérateur devient ordinaire
        // (signe moins unaire, par exemple).
        let mut classes: Vec<Option<Classe>> = noeuds.iter().map(Noeud::classe).collect();
        let mut prec: Option<Classe> = None;
        for i in 0..classes.len() {
            let Some(c) = classes[i] else { continue };
            if c == Classe::Bin {
                let suivant = classes[i + 1..].iter().flatten().next().copied();
                let avant_ok = matches!(prec, Some(Classe::Ord | Classe::Ferm | Classe::Inner));
                let apres_ok = !matches!(
                    suivant,
                    None | Some(Classe::Rel | Classe::Ferm | Classe::Ponct)
                );
                if !avant_ok || !apres_ok {
                    classes[i] = Some(Classe::Ord);
                }
            }
            prec = classes[i];
        }

        let mut res = Vec::with_capacity(noeuds.len());
        let mut prec: Option<Classe> = None;
        for (n, classe) in noeuds.iter().zip(classes) {
            let b = self.noeud(n, st);
            let esp = match (prec, classe) {
                (Some(p), Some(c)) => espacement(p, c, st.script) * st.taille / 18.0,
                _ => 0.0,
            };
            if classe.is_some() {
                prec = classe;
            }
            res.push((b, classe, esp));
        }
        res
    }

    fn hliste(&mut self, noeuds: &[Noeud], st: Style) -> Boite {
        let mut b = Boite::default();
        let mut x = 0.0;
        for (a, _, esp) in self.atomes(noeuds, st) {
            x += esp;
            b.placer(&a, x, 0.0);
            x += a.l;
        }
        b.l = b.l.max(x);
        b
    }

    /// Formule de premier niveau, découpée en morceaux sécables
    /// (coupure autorisée après une relation ou un opérateur binaire).
    fn formule(&mut self, noeuds: &[Noeud], taille: f32) -> Vec<Boite> {
        let st = Style {
            taille,
            base: taille,
            script: false,
        };
        let mut morceaux = Vec::new();
        let mut cour = Boite::default();
        let mut x = 0.0;
        for (a, classe, esp) in self.atomes(noeuds, st) {
            x += esp;
            cour.placer(&a, x, 0.0);
            x += a.l;
            cour.l = cour.l.max(x);
            if matches!(classe, Some(Classe::Rel | Classe::Bin)) {
                morceaux.push(std::mem::take(&mut cour));
                x = 0.0;
            }
        }
        if !cour.elems.is_empty() || morceaux.is_empty() {
            morceaux.push(cour);
        }
        morceaux
    }
}

/// Espacement TeX entre deux atomes, en mu (1/18 em).
fn espacement(gauche: Classe, droite: Classe, script: bool) -> f32 {
    use Classe::*;
    // (valeur, seulement hors style script ?)
    let (v, hors_script) = match (gauche, droite) {
        (Ord, Op) | (Op, Ord) | (Op, Op) | (Ferm, Op) | (Inner, Op) => (3.0, false),
        (Ord, Inner)
        | (Op, Inner)
        | (Ferm, Inner)
        | (Inner, Ord)
        | (Inner, Ouv)
        | (Inner, Ponct)
        | (Inner, Inner) => (3.0, true),
        (Ponct, _) => (3.0, true),
        (Ord | Ferm | Inner, Bin) | (Bin, Ord | Op | Ouv | Inner) => (4.0, true),
        (Ord | Op | Ferm | Inner, Rel) | (Rel, Ord | Op | Ouv | Inner) => (5.0, true),
        _ => (0.0, false),
    };
    if hors_script && script { 0.0 } else { v }
}

// ===========================================================================
// Paragraphe : texte + formules, avec retour à la ligne
// ===========================================================================
/// Texte mis en page, prêt à être dessiné.
pub struct Paragraphe {
    lignes: Vec<(f32, Vec<(f32, Boite)>)>,
    pub taille: Vec2,
}

enum Atome {
    Boite(Boite),
    /// Espace sécable de la largeur donnée.
    Espace(f32),
    /// Point de coupure sans espace.
    Coupure,
    Saut,
}

fn disposer_paragraphe(
    f: &mut FontsView<'_>,
    texte: &str,
    taille: f32,
    gras: bool,
    largeur_max: f32,
) -> Paragraphe {
    let mut m = Moteur { f };
    let segs = segments(texte);
    let largeur_espace = m.espace(taille);

    let mut atomes = Vec::new();
    for seg in segs.iter() {
        match seg {
            Segment::Texte(t) => {
                for (i, ligne) in t.split('\n').enumerate() {
                    if i > 0 {
                        atomes.push(Atome::Saut);
                    }
                    for (j, mot) in ligne.split(' ').enumerate() {
                        if j > 0 {
                            atomes.push(Atome::Espace(largeur_espace));
                        }
                        if !mot.is_empty() {
                            atomes.push(Atome::Boite(m.texte_police(mot, taille, gras)));
                        }
                    }
                }
            }
            Segment::Formule(noeuds) => {
                for (i, b) in m.formule(noeuds, taille).into_iter().enumerate() {
                    if i > 0 {
                        atomes.push(Atome::Coupure);
                    }
                    atomes.push(Atome::Boite(b));
                }
            }
        }
    }

    let ref_ligne = m.texte_police("Ag", taille, gras);
    let interligne = taille * 0.28;
    let mut lignes: Vec<(f32, Vec<(f32, Boite)>)> = Vec::new();
    let mut ligne: Vec<(f32, Boite)> = Vec::new();
    let (mut x, mut attente, mut secable) = (0.0f32, 0.0f32, false);
    let mut largeur = 0.0f32;
    let mut y = 0.0f32;

    let mut finir = |ligne: &mut Vec<(f32, Boite)>, x: f32| {
        let h = ligne.iter().map(|(_, b)| b.h).fold(ref_ligne.h, f32::max);
        let p = ligne.iter().map(|(_, b)| b.p).fold(ref_ligne.p, f32::max);
        if !lignes.is_empty() {
            y += interligne;
        }
        lignes.push((y + h, std::mem::take(ligne)));
        y += h + p;
        largeur = largeur.max(x);
    };

    for a in atomes {
        match a {
            Atome::Saut => {
                finir(&mut ligne, x);
                (x, attente, secable) = (0.0, 0.0, false);
            }
            Atome::Espace(l) => {
                attente += l;
                secable = true;
            }
            Atome::Coupure => secable = true,
            Atome::Boite(b) => {
                if secable && !ligne.is_empty() && x + attente + b.l > largeur_max {
                    finir(&mut ligne, x);
                    (x, attente) = (0.0, 0.0);
                }
                if !ligne.is_empty() {
                    x += attente;
                }
                let l = b.l;
                ligne.push((x, b));
                x += l;
                (attente, secable) = (0.0, false);
            }
        }
    }
    finir(&mut ligne, x);
    Paragraphe {
        lignes,
        taille: vec2(largeur.ceil(), y.ceil()),
    }
}

impl Paragraphe {
    /// Dessine le paragraphe avec son coin haut-gauche en `origine`.
    pub fn peindre(&self, painter: &Painter, origine: Pos2, couleur: Color32) {
        for (base, boites) in &self.lignes {
            for (x, b) in boites {
                let d = origine.to_vec2() + vec2(*x, *base);
                for e in &b.elems {
                    match e {
                        Elem::Texte(p, g) => {
                            painter.galley_with_override_text_color(*p + d, Arc::clone(g), couleur);
                        }
                        Elem::Rect(r) => {
                            painter.rect_filled(r.translate(d), 0.0, couleur);
                        }
                        Elem::Trait(pts, ep) => {
                            painter.add(Shape::line(
                                pts.iter().map(|p| *p + d).collect(),
                                Stroke::new(*ep, couleur),
                            ));
                        }
                        Elem::Disque(c, r) => {
                            painter.circle_filled(*c + d, *r, couleur);
                        }
                    }
                }
            }
        }
    }
}

/// Met en page un texte (avec formules `$…$`) sur une largeur maximale.
pub fn disposer(ui: &Ui, texte: &str, taille: f32, largeur_max: f32) -> Paragraphe {
    ui.fonts_mut(|f| disposer_paragraphe(f, texte, taille, false, largeur_max))
}

/// Étiquette de texte riche (texte + formules LaTeX), renvoyée à la ligne.
pub fn etiquette(ui: &mut Ui, texte: &str, taille: f32, couleur: Color32) -> Response {
    let para = disposer(ui, texte, taille, ui.available_width());
    afficher(ui, &para, couleur)
}

pub fn afficher(ui: &mut Ui, para: &Paragraphe, couleur: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(para.taille, Sense::hover());
    if ui.is_rect_visible(rect) {
        para.peindre(ui.painter(), rect.min, couleur);
    }
    resp
}

/// Problèmes de rendu d'un texte (utilisé par `--verifier`) : formule `$`
/// non fermée, commandes LaTeX inconnues (affichées telles quelles).
pub fn diagnostiquer(texte: &str) -> Vec<String> {
    let dollars = texte.matches('$').count() - texte.matches("\\$").count();
    if dollars % 2 == 1 {
        return vec!["nombre impair de $ : formule non fermée (le texte s'affichera brut)".into()];
    }
    fn visiter(noeuds: &[Noeud], acc: &mut Vec<String>) {
        for n in noeuds {
            match n {
                Noeud::Texte(t) if t.starts_with('\\') => {
                    acc.push(format!("commande LaTeX inconnue : {t}"));
                }
                Noeud::Groupe(g) => visiter(g, acc),
                Noeud::Frac { num, den, .. } => {
                    visiter(num, acc);
                    visiter(den, acc);
                }
                Noeud::Racine { indice, corps } => {
                    visiter(indice.as_deref().unwrap_or_default(), acc);
                    visiter(corps, acc);
                }
                Noeud::Scripts { base, sub, sup } => {
                    visiter(std::slice::from_ref(base), acc);
                    visiter(sub.as_deref().unwrap_or_default(), acc);
                    visiter(sup.as_deref().unwrap_or_default(), acc);
                }
                Noeud::Delims { corps, .. } | Noeud::Accent { corps, .. } => visiter(corps, acc),
                Noeud::XFleche { label, .. } => visiter(label, acc),
                Noeud::Pile {
                    dessus,
                    base,
                    dessous,
                } => {
                    visiter(dessus.as_deref().unwrap_or_default(), acc);
                    visiter(base, acc);
                    visiter(dessous.as_deref().unwrap_or_default(), acc);
                }
                _ => {}
            }
        }
    }
    let mut pb = Vec::new();
    for seg in analyser_texte(texte) {
        if let Segment::Formule(f) = seg {
            visiter(&f, &mut pb);
        }
    }
    pb.dedup();
    pb
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resume(n: &[Noeud]) -> String {
        n.iter()
            .map(|n| match n {
                Noeud::Sym { txt, .. } => txt.clone(),
                Noeud::Texte(t) => format!("T({t})"),
                Noeud::Groupe(g) => format!("{{{}}}", resume(g)),
                Noeud::Frac { num, den, .. } => format!("F({}/{})", resume(num), resume(den)),
                Noeud::Racine { corps, .. } => format!("R({})", resume(corps)),
                Noeud::Scripts { base, sub, sup } => format!(
                    "S({}|{}|{})",
                    resume(std::slice::from_ref(base)),
                    sub.as_deref().map(resume).unwrap_or_default(),
                    sup.as_deref().map(resume).unwrap_or_default()
                ),
                Noeud::GrandOp { txt, .. } => format!("OP({txt})"),
                Noeud::Delims {
                    gauche,
                    corps,
                    droite,
                } => format!("D{gauche}{}{droite}", resume(corps)),
                Noeud::Espace(_) => "_".into(),
                Noeud::XFleche { label, .. } => format!("X({})", resume(label)),
                Noeud::Accent { corps, .. } => format!("A({})", resume(corps)),
                Noeud::Pile { base, .. } => format!("P({})", resume(base)),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn analyse() {
        assert_eq!(resume(&analyser_formule(r"\frac{1}{2}")), "F(1/2)");
        assert_eq!(resume(&analyser_formule(r"x^2_i")), "S(x|i|2)");
        assert_eq!(
            resume(&analyser_formule(r"\sum_{i=1}^{n} i")),
            "S(OP(∑)|i = 1|n) i"
        );
        assert_eq!(
            resume(&analyser_formule(r"\left(\frac{a}{b}\right)^2")),
            "S(D(F(a/b))||2)"
        );
        assert_eq!(resume(&analyser_formule(r"\mathbb{R}")), "{ℝ}");
        assert_eq!(
            resume(&analyser_formule(r"\xrightarrow{\mathcal{F}}")),
            "X({ℱ})"
        );
        assert_eq!(resume(&analyser_formule(r"33 \text{ dB}")), "33 T( dB)");
        // Backslash doublé par erreur dans le JSON : toléré.
        assert_eq!(resume(&analyser_formule(r"t - \\tau")), "t − τ");
        assert_eq!(resume(&analyser_formule(r"f'(x)")), "S(f||′) ( x )");
    }

    #[test]
    fn diagnostic() {
        assert!(diagnostiquer(r"Aire $\pi r^2$").is_empty());
        assert_eq!(diagnostiquer(r"coût 5$").len(), 1);
        assert!(diagnostiquer(r"$\foo{x}$")[0].contains(r"\foo"));
    }

    #[test]
    fn segments_texte() {
        let s = analyser_texte("Aire $\\pi r^2$ et 5\\$");
        assert_eq!(s.len(), 3);
        assert!(matches!(&s[2], Segment::Texte(t) if t == " et 5$"));
        // Nombre impair de $ : texte brut.
        assert!(matches!(
            &analyser_texte("coût 5$")[..],
            [Segment::Texte(_)]
        ));
    }

    #[test]
    fn toutes_les_formules_du_depot_s_analysent() {
        let dossier = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("questions");
        let sets = crate::bibliotheque::charger(&dossier).sets;
        let mut n = 0;
        for s in &sets {
            for q in &s.questions {
                for t in std::iter::once(&q.enonce)
                    .chain(&q.choix)
                    .chain([&q.explication, &q.verso])
                {
                    for seg in analyser_texte(t) {
                        if let Segment::Formule(f) = seg {
                            n += 1;
                            assert!(!f.is_empty() || t.contains("$$"), "{t}");
                            let r = resume(&f);
                            assert!(!r.contains("T(\\"), "commande inconnue dans {t} : {r}");
                        }
                    }
                }
            }
        }
        assert!(n > 1000);
    }
}
