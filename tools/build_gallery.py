#!/usr/bin/env python3
"""Assemble the static gallery; no framework or server dependency."""
import shutil
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'dist/gallery';OUT.mkdir(parents=True,exist_ok=True)
for name in ('catalog','config'):shutil.copytree(ROOT/name,OUT/name,dirs_exist_ok=True)
for p in (ROOT/'web').iterdir():
 if p.is_file():shutil.copy2(p,OUT/p.name)
print(OUT)
