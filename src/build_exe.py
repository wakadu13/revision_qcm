"""
Génère un .exe autonome avec PyInstaller.

Usage :
    python src/build_exe.py            # console (CLI + GUI, fenêtre noire visible)
    python src/build_exe.py --windowed # GUI uniquement, pas de fenêtre console

Prérequis :
    pip install pyinstaller

dist/ est entièrement régénéré par ce script (exécutable, questions/, manifest.json,
revision_qcm.html, documentation) : il ne doit rien contenir qui ne soit pas reproductible.
"""
import json
import shutil
import subprocess
import sys
from pathlib import Path

import core

BASE = Path(__file__).resolve().parent          # src/
RACINE = BASE.parent                            # racine du projet
DIST = RACINE / "dist"
BUILD = RACINE / "build"


def generer_manifest_questions(dossier_questions: Path, dist: Path) -> None:
    """Génère dist/manifest.json : liste des sets (chemins relatifs à questions/,
    ex. "maths/algebre.json"), pour que revision_qcm.html puisse les charger
    automatiquement via fetch() — nécessite un serveur HTTP local (pas file://,
    bloqué par les navigateurs pour des raisons de sécurité)."""
    fichiers = core.lister_sets(dossier_questions)
    manifest = [f.relative_to(dossier_questions).as_posix() for f in fichiers]
    (dist / "manifest.json").write_text(
        json.dumps(manifest, indent=2, ensure_ascii=False), encoding="utf-8"
    )
    print(f"manifest.json généré ({len(manifest)} sets)")


def build(windowed: bool = False):
    mode_label = "windowed (GUI uniquement)" if windowed else "console (CLI + GUI)"
    print(f"\n{'='*52}")
    print(f"  Build : {mode_label}")
    print(f"{'='*52}\n")

    cmd = [
        "uv", "run", "pyinstaller",
        "--onefile",
        "--name", "revision_qcm",
        "--clean",
        "--noconfirm",
        "--distpath", str(DIST),
        "--workpath", str(BUILD),
        "--specpath", str(RACINE),
    ]

    if windowed:
        cmd.append("--windowed")

    cmd.append(str(BASE / "app.py"))

    try:
        subprocess.run(cmd, check=True, cwd=str(RACINE))
    except subprocess.CalledProcessError as e:
        print(f"\nErreur lors du build : {e}")
        sys.exit(1)
    except FileNotFoundError:
        print("\nuv introuvable. Assurez-vous que uv est installé et dans le PATH.")
        sys.exit(1)

    exe = DIST / "revision_qcm.exe"
    if not exe.exists():
        print("\nBuild terminé mais .exe introuvable dans dist/.")
        sys.exit(1)

    # Copier le dossier questions/ à côté du .exe
    q_src = RACINE / "questions"
    q_dst = DIST / "questions"
    if q_src.exists():
        if q_dst.exists():
            shutil.rmtree(q_dst)
        shutil.copytree(q_src, q_dst)
        print(f"Dossier questions/ copié dans dist/")
        generer_manifest_questions(q_dst, DIST)

    # Copier le README et la documentation à côté du .exe
    for filename in ("README.md", "architecture.txt"):
        src = RACINE / filename
        if src.exists():
            shutil.copy2(src, DIST / filename)
            print(f"{filename} copié dans dist/")

    # Copier la version web autonome (revision_qcm.html vit dans web/, pas dans dist/ :
    # c'est du code source à part entière, pas un artefact généré)
    html_src = RACINE / "web" / "revision_qcm.html"
    if html_src.exists():
        shutil.copy2(html_src, DIST / "revision_qcm.html")
        print(f"revision_qcm.html copié dans dist/")

    print(f"\n{'='*52}")
    print(f"  Exécutable généré : {exe}")
    print(f"  Lancez-le depuis le dossier dist/")
    print(f"{'='*52}\n")


if __name__ == "__main__":
    build(windowed="--windowed" in sys.argv)
