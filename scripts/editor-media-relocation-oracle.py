#!/usr/bin/env python3
"""Execute pinned original media-path rebasing, without performing file saves."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

GA_PIN = "48e18f71655d0f0d86144c63239331833501fda2"
FUMEN_PIN = "0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89"


def revision(root):
    return subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-root", type=Path, required=True)
    parser.add_argument("--fumen-root", type=Path, required=True)
    parser.add_argument("--dotnet", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if revision(args.reference_root) != GA_PIN or revision(args.fumen_root) != FUMEN_PIN:
        raise SystemExit("Reference revision does not match the pinned source")
    song = args.reference_root / "StepManiaEditor/EditorSong.cs"
    path_source = args.fumen_root / "Fumen/Path.cs"
    for root, pin, relative, file in [
        (args.reference_root, GA_PIN, "StepManiaEditor/EditorSong.cs", song),
        (args.fumen_root, FUMEN_PIN, "Fumen/Path.cs", path_source),
    ]:
        pinned = subprocess.check_output(["git", "-C", str(root), "show", f"{pin}:{relative}"])
        if file.read_bytes() != pinned:
            raise SystemExit(f"Reference source differs from pinned Git blob: {relative}")
    source = song.read_text(encoding="utf-8-sig")
    start = source.index("\tprivate static string UpdateRelativePath(")
    brace = source.index("{", start)
    depth, end = 1, brace + 1
    while depth:
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    method = source[start:end]
    cases = []
    for old, new in [
        ("/songs/Pack/Song/chart.ssc", "/exports/New/chart.ssc"),
        ("/songs/Pack/Song/chart.ssc", "/songs/Pack/Song/copy.ssc"),
        ("", "/exports/New/chart.ssc"),
    ]:
        for media in ["", "music.ogg", "../Other/music.ogg", "./assets/../cover.png",
                      "/assets/a b #100%.png", "images/été 日本語.png", "missing/not-created.ogg"]:
            cases.append(dict(Old=old, New=new, Media=media))
    program = '''using System; using System.Text.Json; using Path = Fumen.Path;
class Input { public string Old {get;set;} public string New {get;set;} public string Media {get;set;} }
class Oracle {
''' + method + '''
static void Main() {
 var inputs = JsonSerializer.Deserialize<Input[]>(Console.In.ReadToEnd());
 var results = Array.ConvertAll(inputs, i => UpdateRelativePath(i.Old, i.New, i.Media));
 Console.Write(JsonSerializer.Serialize(new {platform=System.Runtime.InteropServices.RuntimeInformation.OSDescription, values=results}));
}
}'''
    with tempfile.TemporaryDirectory(prefix="deadsync-media-relocation-") as directory:
        root = Path(directory)
        (root / "oracle.csproj").write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework></PropertyGroup></Project>')
        (root / "Program.cs").write_text(program)
        (root / "OriginalPath.cs").write_bytes(path_source.read_bytes())
        build = subprocess.run([str(args.dotnet), "build", str(root / "oracle.csproj"), "--nologo", "-v", "q"], text=True, capture_output=True)
        if build.returncode:
            raise SystemExit(build.stdout + build.stderr)
        run = subprocess.run([str(args.dotnet), str(root / "bin/Debug/net10.0/oracle.dll")], input=json.dumps(cases), text=True, capture_output=True, check=True)
    result = json.loads(run.stdout)
    output = dict(reference_commit=GA_PIN, fumen_commit=FUMEN_PIN,
                  source_sha256=hashlib.sha256(song.read_bytes()).hexdigest(),
                  method_sha256=hashlib.sha256(method.encode()).hexdigest(),
                  fumen_path_sha256=hashlib.sha256(path_source.read_bytes()).hexdigest(),
                  platform=result["platform"],
                  scope="Unchanged UpdateRelativePath and complete original Fumen.Path; fixture paths only. No filesystem save, history, media decoding or Windows-runtime equivalence claim.",
                  cases=[dict(input=i, expected=e) for i, e in zip(cases, result["values"])])
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n")
    print(f"Generated {len(cases)} original media relocation cases")


if __name__ == "__main__":
    main()
