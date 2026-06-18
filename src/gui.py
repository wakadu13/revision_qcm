"""Interface graphique tkinter pour Révision QCM / QCR."""
from __future__ import annotations

import random
import sys
import tkinter as tk
from pathlib import Path
from tkinter import messagebox

import core

# Racine du projet (un niveau au-dessus de src/), ou dossier du .exe PyInstaller une fois figé.
BASE = (
    Path(sys.executable).parent
    if getattr(sys, "frozen", False)
    else Path(__file__).resolve().parent.parent
)
DOSSIER_QUESTIONS = BASE / "questions"
FICHIER_PROGRESSION = BASE / "progression.json"

# --- Palette sombre (Catppuccin Mocha) ---
BG     = "#1e1e2e"
BG2    = "#313244"
BG3    = "#45475a"
FG     = "#cdd6f4"
ACCENT = "#89b4fa"
GREEN  = "#a6e3a1"
RED    = "#f38ba8"
YELLOW = "#f9e2af"
MUTED  = "#6c7086"
WHITE  = "#ffffff"

F_TITLE  = ("Segoe UI", 20, "bold")
F_LARGE  = ("Segoe UI", 14)
F_MED    = ("Segoe UI", 12)
F_SMALL  = ("Segoe UI", 10)
F_MONO   = ("Consolas", 11)

# ---------------------------------------------------------------------------
# Rendu LaTeX (optionnel — nécessite matplotlib + pillow)
# Utilise le moteur mathtext de matplotlib : pas d'installation LaTeX requise.
# Syntaxe : délimiteurs $...$ pour les expressions mathématiques.
# ---------------------------------------------------------------------------
_LATEX_OK = False
try:
    import matplotlib
    matplotlib.use("Agg")                          # backend sans fenêtre
    from matplotlib.figure import Figure
    from matplotlib.backends.backend_agg import FigureCanvasAgg as _FigCanvas
    import io as _io
    from PIL import Image as _PILImg, ImageTk as _ImgTk
    _LATEX_OK = True
except ImportError:
    pass


def _has_math(text: str) -> bool:
    """Vrai si le texte contient au moins une expression $...$."""
    return _LATEX_OK and "$" in text


def _render_math(text: str, fontsize: float, max_w_px: int = 820) -> "tk.PhotoImage | None":
    """
    Rend un texte contenant $...$ via matplotlib mathtext.
    Fond transparent — le bg du widget parent transparaît.
    Retourne None si le rendu échoue (expression invalide, etc.).
    """
    try:
        dpi = 96
        fig = Figure(figsize=(max_w_px / dpi, 1.4), dpi=dpi)
        _FigCanvas(fig)
        ax = fig.add_axes([0, 0, 1, 1])
        ax.axis("off")
        ax.text(0.0, 0.5, text, color=FG, fontsize=fontsize,
                ha="left", va="center", transform=ax.transAxes)
        buf = _io.BytesIO()
        fig.savefig(buf, format="png", bbox_inches="tight",
                    pad_inches=0.06, transparent=True, dpi=dpi)
        buf.seek(0)
        return _ImgTk.PhotoImage(_PILImg.open(buf).copy())
    except Exception:
        return None


def _label_or_math(parent, text: str, font, fg: str, bg: str,
                   max_w: int = 840, **kwargs) -> tk.Label:
    """
    Crée un tk.Label classique, ou un Label image si le texte contient $...$.
    Le fond transparent de l'image s'adapte automatiquement au `bg` du Label.
    """
    if _has_math(text):
        fontsize = font[1] if isinstance(font, tuple) else 12
        photo = _render_math(text, fontsize, max_w)
        if photo is not None:
            lbl = tk.Label(parent, image=photo, bg=bg, anchor="w", **kwargs)
            lbl._photo = photo          # empêche le garbage collector
            return lbl
    return tk.Label(parent, text=text, font=font, fg=fg, bg=bg,
                    anchor="w", wraplength=max_w, justify="left", **kwargs)


# ---------------------------------------------------------------------------

class RevisionGUI:
    def __init__(self):
        self.root = tk.Tk()
        self.root.title("Révision QCM")
        self.root.configure(bg=BG)
        self.root.minsize(700, 500)
        self.root.update_idletasks()

        # --- Taille adaptée à l'écran (évite qu'une fenêtre fixe dépasse un petit écran) ---
        screen_w = self.root.winfo_screenwidth()
        screen_h = self.root.winfo_screenheight()
        marge_x, marge_y = 60, 100  # espace réservé pour barre des tâches / décorations
        largeur = max(700, min(920, screen_w - marge_x))
        hauteur = max(500, min(660, screen_h - marge_y))
        x = max(0, (screen_w - largeur) // 2)
        y = max(0, (screen_h - hauteur) // 2)
        self.root.geometry(f"{largeur}x{hauteur}+{x}+{y}")

        # Écran trop petit pour la taille confortable -> on maximise pour ne rien perdre
        if screen_w - marge_x < 920 or screen_h - marge_y < 660:
            try:
                self.root.state("zoomed")
            except tk.TclError:
                pass

        self.prog = core.Progression(FICHIER_PROGRESSION)
        self.sets: list[core.QuestionSet] = []
        self._frame: tk.Frame | None = None

        self._show_main_menu()

    # -----------------------------------------------------------------------
    # Helpers de mise en page
    # -----------------------------------------------------------------------
    def _charger_sets(self):
        self.sets = []
        erreurs = []
        for chemin in core.lister_sets(DOSSIER_QUESTIONS):
            try:
                self.sets.append(core.charger_set(chemin, racine=DOSSIER_QUESTIONS))
            except ValueError as e:
                erreurs.append(str(e))
        if erreurs:
            messagebox.showwarning(
                "Sets ignorés",
                "Certains sets n'ont pas pu être chargés et n'apparaissent pas "
                "dans les listes :\n\n" + "\n".join(erreurs),
            )

    def _clear(self) -> tk.Frame:
        if self._frame:
            self._frame.destroy()
        self._frame = tk.Frame(self.root, bg=BG)
        self._frame.pack(fill=tk.BOTH, expand=True)
        return self._frame

    def _header(self, parent: tk.Frame, text: str):
        tk.Label(parent, text=text, font=F_TITLE, bg=BG, fg=ACCENT,
                 pady=18).pack(fill=tk.X)
        tk.Frame(parent, height=2, bg=BG3).pack(fill=tk.X, padx=20)

    def _footer(self, parent: tk.Frame, text: str):
        tk.Label(parent, text=text, font=F_SMALL, bg=BG, fg=MUTED,
                 pady=6).pack(side=tk.BOTTOM, fill=tk.X)

    def _nav_list(self, parent: tk.Frame, items: list[str], on_select, on_back=None) -> tk.Listbox:
        """Listbox navigable au clavier (↑↓ + Entrée)."""
        wrap = tk.Frame(parent, bg=BG)
        wrap.pack(fill=tk.BOTH, expand=True, padx=40, pady=16)

        sb = tk.Scrollbar(wrap, bg=BG2)
        sb.pack(side=tk.RIGHT, fill=tk.Y)

        lb = tk.Listbox(
            wrap,
            font=F_LARGE, bg=BG2, fg=FG,
            selectbackground=ACCENT, selectforeground=BG,
            activestyle="none", relief=tk.FLAT, bd=0,
            highlightthickness=0, yscrollcommand=sb.set,
        )
        lb.pack(fill=tk.BOTH, expand=True)
        sb.config(command=lb.yview)

        for item in items:
            lb.insert(tk.END, f"  {item}")
        if items:
            lb.selection_set(0)
            lb.focus_set()

        lb.bind("<Return>",   lambda e: on_select(lb.curselection()[0]) if lb.curselection() else None)
        lb.bind("<KP_Enter>", lambda e: on_select(lb.curselection()[0]) if lb.curselection() else None)
        if on_back:
            lb.bind("<Escape>", lambda e: on_back())
        return lb

    def _scrollable_body(self, parent: tk.Frame) -> tk.Frame:
        """Zone de contenu défilante (molette + barre à droite) à l'intérieur de `parent`.

        Retourne le frame interne dans lequel placer les widgets : si leur hauteur
        dépasse l'espace disponible, une barre de défilement apparaît automatiquement
        au lieu de couper le contenu en bas de la fenêtre.
        """
        wrap = tk.Frame(parent, bg=BG)
        wrap.pack(fill=tk.BOTH, expand=True)

        canvas = tk.Canvas(wrap, bg=BG, highlightthickness=0)
        sb = tk.Scrollbar(wrap, orient="vertical", command=canvas.yview, bg=BG2)
        inner = tk.Frame(canvas, bg=BG)

        canvas_window = canvas.create_window((0, 0), window=inner, anchor="nw")
        inner.bind("<Configure>", lambda e: canvas.configure(scrollregion=canvas.bbox("all")))
        canvas.bind("<Configure>", lambda e: canvas.itemconfig(canvas_window, width=e.width))
        canvas.configure(yscrollcommand=sb.set)

        sb.pack(side=tk.RIGHT, fill=tk.Y)
        canvas.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)

        def _wheel(e):
            if canvas.winfo_exists():
                canvas.yview_scroll(-1 * (e.delta // 120), "units")
        # Une seule fenêtre/écran défilant est visible à la fois : on retire l'ancien
        # gestionnaire avant d'attacher le nouveau pour éviter qu'ils s'accumulent
        # (le quiz reconstruit cet écran à chaque question).
        self.root.unbind_all("<MouseWheel>")
        self.root.bind_all("<MouseWheel>", _wheel)

        return inner

    # -----------------------------------------------------------------------
    # Catégories (sous-dossiers de questions/, ex. "maths", "langues")
    # -----------------------------------------------------------------------
    def _categories(self) -> list[str]:
        return sorted({s.categorie for s in self.sets})

    def _show_categorized_sets(self, titre: str, on_select_set, on_back) -> None:
        """Choix d'un set, en passant par un choix de catégorie s'il y en a plusieurs.

        S'il n'y a qu'une seule catégorie (ou aucune), on saute directement à la
        liste des sets pour ne pas ajouter une étape inutile.
        """
        categories = self._categories()
        if len(categories) <= 1:
            self._show_set_selection_in_category(
                categories[0] if categories else "Général", titre, on_select_set, on_back,
            )
            return

        frame = self._clear()
        self._header(frame, titre)
        self._footer(frame, "↑↓ Naviguer   Entrée Sélectionner   Échap Retour")
        labels = [
            f"{c}  ({sum(1 for s in self.sets if s.categorie == c)} sets)"
            for c in categories
        ]
        self._nav_list(
            frame, labels,
            on_select=lambda i: self._show_set_selection_in_category(
                categories[i], titre, on_select_set,
                on_back=lambda: self._show_categorized_sets(titre, on_select_set, on_back),
            ),
            on_back=on_back,
        )

    def _show_set_selection_in_category(self, categorie: str, titre: str,
                                         on_select_set, on_back) -> None:
        frame = self._clear()
        self._header(frame, f"{titre} — {categorie}")
        self._footer(frame, "↑↓ Naviguer   Entrée Sélectionner   Échap Retour")
        sets_cat = [s for s in self.sets if s.categorie == categorie]
        labels = [f"{s.titre}  ({len(s)} questions)" for s in sets_cat]
        self._nav_list(
            frame, labels,
            on_select=lambda i: on_select_set(sets_cat[i]),
            on_back=on_back,
        )

    # -----------------------------------------------------------------------
    # Menu principal
    # -----------------------------------------------------------------------
    def _show_main_menu(self):
        self._charger_sets()
        frame = self._clear()
        self._header(frame, "Révision QCM / QCR")
        self._footer(frame, "↑↓ Naviguer   Entrée Sélectionner   Échap Quitter")

        options = [
            ("Démarrer une session",        self._show_set_selection_session),
            ("Révision intelligente",        self._start_revision_intelligente),
            ("Mes statistiques",             self._show_statistiques),
            ("Réinitialiser la progression", self._show_set_selection_reset),
            ("Quitter",                      self.root.quit),
        ]
        lb = self._nav_list(frame, [o[0] for o in options],
                             on_select=lambda i: options[i][1]())
        lb.bind("<Escape>", lambda e: self.root.quit())

    # -----------------------------------------------------------------------
    # Session normale
    # -----------------------------------------------------------------------
    def _show_set_selection_session(self):
        if not self.sets:
            messagebox.showwarning("Aucun set", f"Aucun set trouvé dans :\n{DOSSIER_QUESTIONS}")
            return
        self._show_categorized_sets(
            "Choisir un set",
            on_select_set=self._show_session_mode,
            on_back=self._show_main_menu,
        )

    def _show_session_mode(self, qset: core.QuestionSet):
        frame = self._clear()
        self._header(frame, f"Mode — {qset.titre}")
        self._footer(frame, "↑↓ Naviguer   Entrée Sélectionner   Échap Retour")

        def start(mode: str):
            questions = list(qset.questions)
            if mode == "reviser":
                questions = [q for q in questions if self.prog.est_a_reviser(qset.id, q.id)]
                questions.sort(key=lambda q: self.prog.priorite(qset.id, q.id))
                if not questions:
                    messagebox.showinfo("Rien à réviser", "Aucune question à réviser pour ce set.")
                    self._show_session_mode(qset)
                    return
            elif mode == "limite":
                random.shuffle(questions)
                self._dialog_nombre(len(questions), lambda n: self._show_quiz(qset, questions[:n]))
                return
            else:
                random.shuffle(questions)
            self._show_quiz(qset, questions)

        modes  = ["toutes", "reviser", "limite"]
        labels = [
            "Toutes les questions (ordre aléatoire)",
            "Seulement les questions à réviser",
            "Nombre limité de questions",
        ]
        self._nav_list(frame, labels,
                       on_select=lambda i: start(modes[i]),
                       on_back=self._show_set_selection_session)

    def _dialog_nombre(self, max_n: int, callback):
        """Fenêtre modale pour saisir un nombre de questions."""
        dlg = tk.Toplevel(self.root)
        dlg.title("Nombre de questions")
        dlg.configure(bg=BG)
        dlg.geometry("360x180")
        dlg.transient(self.root)
        dlg.grab_set()
        dlg.resizable(False, False)
        tk.Label(dlg, text=f"Combien de questions ? (max {max_n})",
                 font=F_MED, bg=BG, fg=FG, pady=20).pack()
        var = tk.StringVar(value=str(min(10, max_n)))
        entry = tk.Entry(dlg, textvariable=var, font=F_LARGE, bg=BG2, fg=FG,
                         insertbackground=FG, relief=tk.FLAT, width=10, justify="center")
        entry.pack(pady=8)
        entry.select_range(0, tk.END)
        entry.focus_set()

        def ok(event=None):
            val = var.get().strip()
            dlg.destroy()
            n = max(1, min(max_n, int(val))) if val.isdigit() and int(val) > 0 else max_n
            callback(n)

        entry.bind("<Return>", ok)
        tk.Button(dlg, text="OK", command=ok, font=F_MED,
                  bg=ACCENT, fg=BG, relief=tk.FLAT, padx=20, pady=6).pack(pady=10)

    # -----------------------------------------------------------------------
    # Quiz — moteur commun (session unique + révision intelligente)
    # -----------------------------------------------------------------------
    def _show_quiz(self, qset: core.QuestionSet, questions: list[core.Question]):
        """Lance un quiz sur un seul set."""
        self._run_quiz([(qset, q) for q in questions])

    def _run_quiz(self, pairs: list[tuple[core.QuestionSet, core.Question]]):
        """
        Moteur de quiz commun.
        pairs : liste de (QuestionSet, Question) — peut mélanger plusieurs sets.
        """
        if not pairs:
            messagebox.showinfo("Session vide", "Aucune question dans cette session.")
            self._show_main_menu()
            return

        total = len(pairs)
        state = {"index": 0, "score": 0, "selection": [], "answered": False, "revealed": False}

        def build():
            frame = self._clear()
            s, q = pairs[state["index"]]
            num = state["index"] + 1

            # --- Barre de progression ---
            top = tk.Frame(frame, bg=BG2, pady=8, padx=20)
            top.pack(fill=tk.X)
            pct_top = (num - 1) / total * 100
            plein   = int(pct_top / 100 * 24)
            tk.Label(top,
                     text=f"Question {num}/{total}   {'█' * plein}{'░' * (24 - plein)}   {pct_top:.0f}%",
                     font=F_MONO, bg=BG2, fg=ACCENT).pack(side=tk.LEFT)
            tk.Label(top, text=f"Score : {state['score']}/{num - 1}     {s.titre}",
                     font=F_SMALL, bg=BG2, fg=MUTED).pack(side=tk.RIGHT)

            # --- Corps défilant (énoncé + zone de réponse + retour) : une barre
            #     apparaît à droite si le contenu dépasse la hauteur de la fenêtre. ---
            body = self._scrollable_body(frame)

            # --- Énoncé (supporte LaTeX) ---
            q_frame = tk.Frame(body, bg=BG, pady=16, padx=40)
            q_frame.pack(fill=tk.X)
            etiquettes = {
                "QCM": "(QCM — une seule réponse)",
                "QCR": "(QCR — plusieurs réponses possibles)",
                "TROU": "(Texte à trous — tape la réponse)",
                "FLASHCARD": "(Flashcard — essaie de te souvenir avant de retourner la carte)",
            }
            tk.Label(q_frame, text=etiquettes.get(q.type, ""), font=F_SMALL, bg=BG, fg=MUTED).pack(anchor="w")
            _label_or_math(q_frame, q.enonce, F_LARGE, WHITE, BG,
                           max_w=840).pack(anchor="w", pady=(6, 0))

            # --- Zone de réponse, dépend du type de question ---
            c_frame = tk.Frame(body, bg=BG, padx=40)
            c_frame.pack(fill=tk.BOTH, expand=True, pady=6)

            def feedback(correcte: bool, lignes_extra: list[str] = ()):
                """Bandeau de retour commun à tous les types : correct/incorrect + explication."""
                color = GREEN if correcte else RED
                fb = tk.Frame(body, bg=BG, padx=40)
                fb.pack(fill=tk.X, pady=4)
                tk.Label(fb, text="✓ Correct !" if correcte else "✗ Incorrect",
                         font=("Segoe UI", 13, "bold"), bg=BG, fg=color).pack(anchor="w")
                for ligne in lignes_extra:
                    tk.Label(fb, text=ligne, font=F_MED, bg=BG, fg=GREEN).pack(anchor="w")
                if q.explication:
                    _label_or_math(fb, f"ⓘ  {q.explication}", F_SMALL, MUTED, BG,
                                   max_w=840).pack(anchor="w")
                hint.configure(text="Entrée / Espace → Question suivante     Q Quitter")
                frame.focus_set()

            def enregistrer(correcte: bool):
                if correcte:
                    state["score"] += 1
                self.prog.enregistrer_reponse(s.id, q.id, correcte)
                self.prog.sauvegarder()

            def next_q():
                state["index"] += 1
                state["selection"] = []
                state["answered"] = False
                state["revealed"] = False
                if state["index"] >= total:
                    show_results()
                else:
                    build()

            def quit_quiz(event=None):
                if messagebox.askyesno("Quitter la session", "Quitter la session en cours ?"):
                    self._show_main_menu()

            # =====================================================================
            # QCM / QCR — sélection parmi des choix lettrés
            # =====================================================================
            if q.type in ("QCM", "QCR"):
                # Chaque élément : {'frame', 'letter', 'content', 'is_img'}
                choice_items: list[dict] = []

                def refresh():
                    for i, item in enumerate(choice_items):
                        sel = i in state["selection"]
                        if state["answered"]:
                            if i in q.reponses:
                                bg_c, fg_c = GREEN, BG
                            elif sel:
                                bg_c, fg_c = RED, BG
                            else:
                                bg_c, fg_c = BG2, FG
                        else:
                            bg_c, fg_c = (ACCENT, BG) if sel else (BG2, FG)

                        item["frame"].configure(bg=bg_c)
                        item["letter"].configure(bg=bg_c, fg=fg_c)
                        item["content"].configure(bg=bg_c)
                        if not item["is_img"]:
                            item["content"].configure(fg=fg_c)

                def toggle(i: int):
                    if state["answered"]:
                        return
                    if q.est_qcr:
                        if i in state["selection"]:
                            state["selection"].remove(i)
                        else:
                            state["selection"].append(i)
                    else:
                        state["selection"] = [i]
                    refresh()

                def validate(event=None):
                    if state["answered"]:
                        next_q()
                        return
                    if not state["selection"]:
                        return
                    state["answered"] = True
                    correcte = q.est_correcte(state["selection"])
                    enregistrer(correcte)
                    refresh()
                    extra = []
                    if not correcte:
                        bonnes = ", ".join(chr(65 + r) for r in sorted(q.reponses))
                        extra.append(f"Bonne(s) réponse(s) : {bonnes}")
                    feedback(correcte, extra)

                def on_key(event):
                    key = event.keysym.upper()
                    if key in ("RETURN", "KP_ENTER", "SPACE"):
                        validate()
                    elif key == "Q":
                        quit_quiz()
                    elif len(key) == 1 and key.isalpha():
                        idx = ord(key) - 65
                        if 0 <= idx < len(q.choix):
                            toggle(idx)

                # Créer les widgets de choix
                for i, choix_txt in enumerate(q.choix):
                    letter = chr(65 + i)

                    outer = tk.Frame(c_frame, bg=BG2, cursor="hand2")
                    outer.pack(fill=tk.X, pady=3)

                    letter_lbl = tk.Label(outer, text=f"  {letter}.  ",
                                          font=F_MED, bg=BG2, fg=FG, pady=8)
                    letter_lbl.pack(side=tk.LEFT, pady=8)

                    if _has_math(choix_txt):
                        photo = _render_math(choix_txt, fontsize=12, max_w_px=680)
                        if photo is not None:
                            content = tk.Label(outer, image=photo, bg=BG2,
                                               anchor="w", pady=4)
                            content._photo = photo      # empêche le garbage collector
                            is_img = True
                        else:
                            content = tk.Label(outer, text=choix_txt, font=F_MED,
                                               bg=BG2, fg=FG, anchor="w", padx=4, pady=8)
                            is_img = False
                    else:
                        content = tk.Label(outer, text=choix_txt, font=F_MED,
                                           bg=BG2, fg=FG, anchor="w", padx=4, pady=8)
                        is_img = False

                    content.pack(side=tk.LEFT, fill=tk.X, expand=True, pady=4)

                    for w in (outer, letter_lbl, content):
                        w.bind("<Button-1>", lambda e, idx=i: toggle(idx))
                        w.bind("<FocusIn>",  lambda e: frame.focus_set())

                    choice_items.append({"frame": outer, "letter": letter_lbl,
                                         "content": content, "is_img": is_img})

                hint_text = ("A–D Sélectionner   Entrée Valider   Q Quitter" if q.est_qcr
                             else "A–D Répondre   Entrée Valider   Q Quitter")
                hint = tk.Label(frame, text=hint_text, font=F_SMALL, bg=BG, fg=MUTED, pady=6)
                hint.pack(side=tk.BOTTOM, fill=tk.X)

                frame.bind("<Key>", on_key)
                frame.focus_set()

            # =====================================================================
            # TROU — réponse libre (grammaire), comparée avec souplesse
            # =====================================================================
            elif q.type == "TROU":
                entry = tk.Entry(c_frame, font=F_LARGE, bg=BG2, fg=FG,
                                 insertbackground=FG, relief=tk.FLAT)
                entry.pack(fill=tk.X, pady=10, ipady=8)

                def on_enter(event=None):
                    if state["answered"]:
                        next_q()
                        return
                    reponse = entry.get().strip()
                    if not reponse:
                        return
                    state["answered"] = True
                    correcte = q.est_correcte_texte(reponse)
                    enregistrer(correcte)
                    entry.configure(state="disabled")
                    extra = []
                    if not correcte:
                        attendu = " / ".join(q.reponses_texte)
                        extra.append(f"Réponse attendue : {attendu}")
                    feedback(correcte, extra)

                entry.bind("<Return>", on_enter)
                entry.bind("<KP_Enter>", on_enter)
                entry.bind("<Escape>", lambda e: quit_quiz())
                entry.focus_set()

                def on_key(event):
                    # Une fois répondu, le focus repasse au frame (entrée désactivée) :
                    # on y capte Entrée/Espace pour avancer, et Q pour quitter.
                    key = event.keysym.upper()
                    if state["answered"] and key in ("RETURN", "KP_ENTER", "SPACE"):
                        next_q()
                    elif key == "Q":
                        quit_quiz()

                frame.bind("<Key>", on_key)

                hint = tk.Label(frame, text="Tape ta réponse   Entrée Valider   Échap Quitter",
                                font=F_SMALL, bg=BG, fg=MUTED, pady=6)
                hint.pack(side=tk.BOTTOM, fill=tk.X)

            # =====================================================================
            # FLASHCARD — recto/verso, auto-évaluée (vocabulaire)
            # =====================================================================
            else:
                def reveal():
                    if state["revealed"]:
                        return
                    state["revealed"] = True
                    _label_or_math(c_frame, q.verso, F_LARGE, ACCENT, BG,
                                   max_w=840).pack(anchor="w", pady=(10, 16))
                    btns = tk.Frame(c_frame, bg=BG)
                    btns.pack(anchor="w")
                    tk.Button(btns, text="✓ Je savais  (C)", font=F_MED, bg=GREEN, fg=BG,
                              relief=tk.FLAT, padx=16, pady=8,
                              command=lambda: repondre(True)).pack(side=tk.LEFT, padx=(0, 10))
                    tk.Button(btns, text="✗ À revoir  (I)", font=F_MED, bg=RED, fg=BG,
                              relief=tk.FLAT, padx=16, pady=8,
                              command=lambda: repondre(False)).pack(side=tk.LEFT)
                    hint.configure(text="C Je savais   I À revoir   Q Quitter")

                def repondre(correcte: bool):
                    if state["answered"]:
                        return
                    state["answered"] = True
                    enregistrer(correcte)
                    feedback(correcte)

                tk.Button(c_frame, text="Retourner la carte  (Espace)", font=F_MED,
                          bg=ACCENT, fg=BG, relief=tk.FLAT, padx=16, pady=10,
                          command=reveal).pack(anchor="w", pady=10)

                def on_key(event):
                    key = event.keysym.upper()
                    if key == "SPACE":
                        if not state["revealed"]:
                            reveal()
                        elif state["answered"]:
                            next_q()
                    elif key in ("RETURN", "KP_ENTER") and state["answered"]:
                        next_q()
                    elif key == "C" and state["revealed"]:
                        repondre(True)
                    elif key == "I" and state["revealed"]:
                        repondre(False)
                    elif key == "Q":
                        quit_quiz()

                hint = tk.Label(frame, text="Espace Retourner la carte   Q Quitter",
                                font=F_SMALL, bg=BG, fg=MUTED, pady=6)
                hint.pack(side=tk.BOTTOM, fill=tk.X)

                frame.bind("<Key>", on_key)
                frame.focus_set()

        def show_results():
            frame = self._clear()
            self._header(frame, "Résultat de la session")
            score = state["score"]
            pct   = score / total * 100
            color = GREEN if pct == 100 else (YELLOW if pct >= 60 else RED)

            tk.Label(frame, text=f"{score} / {total}",
                     font=("Segoe UI", 52, "bold"), bg=BG, fg=color).pack(pady=16)
            tk.Label(frame, text=f"{pct:.0f} %", font=F_TITLE, bg=BG, fg=color).pack()
            plein = int(pct / 100 * 32)
            tk.Label(frame, text="█" * plein + "░" * (32 - plein),
                     font=F_MONO, bg=BG, fg=color).pack(pady=8)

            if pct == 100:
                msg = "Parfait, tout est juste ! ✓"
            elif pct >= 60:
                msg = "Bon travail — continue de réviser les points faibles."
            else:
                msg = "À retravailler — relance une révision intelligente bientôt."
            tk.Label(frame, text=msg, font=F_MED, bg=BG, fg=FG).pack(pady=10)

            tk.Button(frame, text="Menu principal  (Entrée)", font=F_MED,
                      bg=ACCENT, fg=BG, relief=tk.FLAT, padx=20, pady=10,
                      command=self._show_main_menu).pack(pady=28)
            frame.bind("<Return>", lambda e: self._show_main_menu())
            frame.bind("<KP_Enter>", lambda e: self._show_main_menu())
            frame.bind("<space>", lambda e: self._show_main_menu())
            frame.focus_set()

        build()

    # -----------------------------------------------------------------------
    # Révision intelligente
    # -----------------------------------------------------------------------
    def _start_revision_intelligente(self):
        self._charger_sets()
        a_faire: list[tuple[core.QuestionSet, core.Question]] = [
            (s, q)
            for s in self.sets
            for q in s.questions
            if self.prog.est_a_reviser(s.id, q.id)
        ]
        if not a_faire:
            messagebox.showinfo("Rien à réviser",
                                "Rien à réviser pour l'instant — reviens plus tard. ✓")
            self._show_main_menu()
            return
        a_faire.sort(key=lambda sq: self.prog.priorite(sq[0].id, sq[1].id))
        self._dialog_nombre(len(a_faire), lambda n: self._run_quiz(a_faire[:n]))

    # -----------------------------------------------------------------------
    # Statistiques
    # -----------------------------------------------------------------------
    def _show_statistiques(self):
        self._charger_sets()
        frame = self._clear()
        self._header(frame, "Mes statistiques")

        tk.Button(frame, text="← Retour  (Échap)", font=F_SMALL,
                  bg=BG3, fg=FG, relief=tk.FLAT, padx=12, pady=4,
                  command=self._show_main_menu).pack(anchor="w", padx=20, pady=(8, 0))

        if not self.sets:
            tk.Label(frame, text="Aucun set chargé.", font=F_LARGE, bg=BG, fg=MUTED).pack(pady=40)
        else:
            inner = self._scrollable_body(frame)
            for s in self.sets:
                r     = self.prog.resume_set(s)
                pct   = r["pct_maitrise"]
                plein = int(pct / 100 * 28)
                color = GREEN if pct >= 80 else (YELLOW if pct >= 40 else RED)

                sf = tk.Frame(inner, bg=BG2, pady=10, padx=18)
                sf.pack(fill=tk.X, padx=20, pady=6)

                tk.Label(sf, text=s.titre, font=("Segoe UI", 13, "bold"),
                         bg=BG2, fg=ACCENT).pack(anchor="w")
                tk.Label(sf, text=f"Maîtrise  {'█' * plein}{'░' * (28 - plein)}  {pct:.0f}%",
                         font=F_MONO, bg=BG2, fg=color).pack(anchor="w")

                parts = [f"Vues : {r['rencontrees']}/{r['total']}",
                         f"À réviser : {r['a_reviser']}"]
                if r["rencontrees"]:
                    parts.append(f"Précision : {r['precision']:.0f}%")
                tk.Label(sf, text="   ".join(parts), font=F_SMALL,
                         bg=BG2, fg=FG).pack(anchor="w", pady=(4, 0))

                rep    = r["repartition"]
                detail = "  ".join(f"B{b}:{rep[b]}" for b in range(core.BOITE_MAX + 1))
                tk.Label(sf, text=f"Leitner : {detail}  (B0=difficile … B5=maîtrisé)",
                         font=F_SMALL, bg=BG2, fg=MUTED).pack(anchor="w")

        frame.bind("<Escape>", lambda e: self._show_main_menu())
        frame.focus_set()

    # -----------------------------------------------------------------------
    # Réinitialisation
    # -----------------------------------------------------------------------
    def _show_set_selection_reset(self):
        if not self.sets:
            messagebox.showwarning("Aucun set", "Aucun set disponible.")
            return
        self._show_categorized_sets(
            "Réinitialiser un set",
            on_select_set=self._confirm_reset,
            on_back=self._show_main_menu,
        )

    def _confirm_reset(self, qset: core.QuestionSet):
        if messagebox.askyesno(
            "Confirmer la réinitialisation",
            f"Effacer toute la progression de\n« {qset.titre} » ?\n\nCette action est irréversible.",
        ):
            self.prog.reinitialiser_set(qset.id)
            self.prog.sauvegarder()
            messagebox.showinfo("Réinitialisé", "Progression effacée.")
        self._show_main_menu()

    # -----------------------------------------------------------------------
    def run(self):
        self.root.mainloop()


def lancer_gui():
    app = RevisionGUI()
    app.run()


if __name__ == "__main__":
    lancer_gui()

