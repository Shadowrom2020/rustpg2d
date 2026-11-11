"""Paths shared by the pipeline. WORK (raw generations, references, job files) is
$ART_WORK or ./work next to these scripts (git-ignored)."""
import os

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "../../.."))
WORK = os.environ.get("ART_WORK", os.path.join(HERE, "work"))
STARTER = os.path.join(REPO, "crates/editor/assets/starter_library")
os.makedirs(WORK, exist_ok=True)
