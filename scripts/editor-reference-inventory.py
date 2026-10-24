#!/usr/bin/env python3
"""Record the exact reference surface; file presence does not imply parity.

Run with --grooveauthor and --step-library pointing at the pinned checkouts.
The inventory deliberately makes no automatic implementation/validation claim.
"""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path


def inventory(root: Path, kinds: set[str]) -> dict:
    paths = subprocess.check_output(["git", "ls-files", "-z"], cwd=root).decode().split("\0")
    files = []
    for name in sorted(paths):
        path = root / name
        if path.suffix not in kinds or not path.is_file():
            continue
        data = path.read_bytes()
        files.append({"path": name, "sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)})
    return {
        "commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root).decode().strip(),
        "files": files,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--grooveauthor", type=Path, required=True)
    parser.add_argument("--step-library", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=Path(__file__).resolve().parents[1] / "docs/editor/reference-inventory.json")
    args = parser.parse_args()
    result = {
        "notice": "Inventory of reference files, not a claim of review, implementation, or test coverage.",
        "grooveauthor": inventory(args.grooveauthor, {".cs", ".json", ".md", ".csproj", ".fsg"}),
        "stepmania_library": inventory(args.step_library, {".cs", ".json", ".md", ".csproj", ".fsg"}),
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    for name in ("grooveauthor", "stepmania_library"):
        print(f"{name}: {result[name]['commit']}, {len(result[name]['files'])} files")


if __name__ == "__main__":
    main()
