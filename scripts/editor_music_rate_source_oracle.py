#!/usr/bin/env python3
"""Run pinned rate shortcuts, complete PreferencesAudio and real Fumen actions."""
import argparse
import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path

EDITOR_PIN = "48e18f71655d0f0d86144c63239331833501fda2"
FUMEN_PIN = "0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89"


def original(root, path):
    return subprocess.check_output(
        ["git", "-C", str(root), "show", f"{EDITOR_PIN}:{path}"]
    ).decode("utf-8-sig")


def method(source, signature):
    start = source.index(signature)
    opening = source.index("{", start)
    depth = 1
    end = opening + 1
    while depth:
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    return source[start:end]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["editor-root", "reference-root", "dotnet", "output"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    fumen = args.reference_root / "Fumen"
    for root, pin in [(args.editor_root, EDITOR_PIN), (fumen, FUMEN_PIN)]:
        assert subprocess.check_output(
            ["git", "-C", str(root), "rev-parse", "HEAD"], text=True
        ).strip() == pin
    editor = original(args.editor_root, "StepManiaEditor/Editor.cs")
    audio = original(args.editor_root, "StepManiaEditor/Preferences/PreferencesAudio.cs")
    manager = original(args.editor_root, "StepManiaEditor/Sound/MusicManager.cs")
    callbacks = "\n".join(method(editor, "private void " + name + "()") for name in [
        "OnDecreaseMusicRate", "OnIncreaseMusicRate"
    ])
    constants = "\n".join(line.strip() for line in manager.splitlines() if
                          "public const double MinMusicRate =" in line or
                          "public const double MaxMusicRate =" in line)
    assert constants.count("public const double") == 2
    harness = '''using System.Text.Json;using System.Globalization;using Fumen;
namespace StepManiaEditor {
class Preferences {public static Preferences Instance=new();public PreferencesAudio PreferencesAudio=new();}
class MusicManager { CONSTANTS }
class Editor { CALLBACKS public void Run(bool increase){if(increase)OnIncreaseMusicRate();else OnDecreaseMusicRate();}}
class Program {static void Main(){
 CultureInfo.CurrentCulture=CultureInfo.InvariantCulture;
 var cases=JsonDocument.Parse(Console.In.ReadToEnd()).RootElement;var output=new List<object>();
 foreach(var raw in cases.EnumerateArray())foreach(var increase in new[]{false,true}){
  ActionQueue.Instance.Clear();
  var p=JsonSerializer.Deserialize<PreferencesAudio>("{\\"MusicRate\\":"+raw.GetRawText()+"}");
  Preferences.Instance.PreferencesAudio=p;var before=p.MusicRate;
  new Editor().Run(increase);var after=p.MusicRate;
  var changed=BitConverter.DoubleToInt64Bits(before)!=BitConverter.DoubleToInt64Bits(after);
  var undone=after;var redone=after;
  if(changed){ActionQueue.Instance.Undo();undone=p.MusicRate;ActionQueue.Instance.Redo();redone=p.MusicRate;}
  output.Add(new{raw=raw.GetDouble(),increase,before,after,changed,undone,redone});
 }
 Console.Write(JsonSerializer.Serialize(output));}}
}'''.replace("CONSTANTS", constants).replace("CALLBACKS", callbacks)
    values = [sys.float_info.max, 1e308, -sys.float_info.max, -1e308,
              214748364.7, -214748364.8, 214748364.6, -214748364.7,
              0.0, -1.0, .25, .3, 1.0, 1.0000000000000002, 1.35, 3.99, 4.0, 8.0]
    dll = fumen / "Fumen/bin/Release/net10.0/Fumen.dll"
    with tempfile.TemporaryDirectory(prefix="deadsync-rate-shortcuts-") as folder:
        work = Path(folder)
        (work / "Audio.cs").write_text(audio)
        (work / "Program.cs").write_text(harness)
        (work / "Oracle.csproj").write_text(
            '<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType>'
            '<TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings>'
            '</PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>'
            + str(dll) + '</HintPath></Reference></ItemGroup></Project>'
        )
        build = subprocess.run([str(args.dotnet), "build", str(work / "Oracle.csproj"),
                                "-c", "Release", "--nologo", "-v", "quiet"],
                               capture_output=True, text=True)
        if build.returncode:
            raise SystemExit(build.stdout + build.stderr)
        assembly = work / "bin/Release/net10.0/Oracle.dll"
        result = json.loads(subprocess.run([str(args.dotnet), str(assembly)],
                            input=json.dumps(values), capture_output=True, text=True,
                            check=True).stdout)
        provenance = dict(editor=EDITOR_PIN, fumen=FUMEN_PIN,
                          callbacks_sha256=hashlib.sha256(callbacks.encode()).hexdigest(),
                          audio_sha256=hashlib.sha256(audio.encode()).hexdigest(),
                          harness_sha256=hashlib.sha256(harness.encode()).hexdigest(),
                          assembly_sha256=hashlib.sha256(assembly.read_bytes()).hexdigest(),
                          fumen_assembly_sha256=hashlib.sha256(dll.read_bytes()).hexdigest(),
                          scope=__doc__)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    args.output.with_suffix(".reference.json").write_text(json.dumps(provenance, indent=2) + "\n")
    print("Executed", len(result), "original shortcut/preferences/action/Undo/Redo cases")


if __name__ == "__main__":
    main()
