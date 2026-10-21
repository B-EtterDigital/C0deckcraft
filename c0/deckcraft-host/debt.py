#!/usr/bin/env python3
"""Verify the committed added-line ratchet (or the working tree during implementation)."""
import argparse
from pathlib import Path
import re
import subprocess

root = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser()
parser.add_argument("--working", action="store_true")
args = parser.parse_args()
command = ["git", "diff", "--numstat", "--diff-filter=M", "main"]
if not args.working:
    command.append("c0/main")
command.extend(["--", ".", ":!c0"])
rows = subprocess.check_output(command, cwd=root, text=True).splitlines()
actual = {path: int(added) for added, _removed, path in (line.split("\t", 2) for line in rows)}
document = (root / "c0/deckcraft-host/PATCH-DEBT.md").read_text()
listed = {path: int(added) for path, added in re.findall(r"^\| `([^`]+)` \| (\d+) \|", document, re.M)}
assert listed == actual, {"listed": listed, "actual": actual}
ratchet = int(re.search(r"^Ratchet: (\d+)$", document, re.M).group(1))
assert ratchet == sum(actual.values()), (ratchet, actual)
assert ratchet < 70, f"Patch debt exceeds the lane target: {ratchet}"
print("\n".join(f"{path}: {added}" for path, added in actual.items()))
print(f"Total: {ratchet}; Ratchet: {ratchet}; under 70")
