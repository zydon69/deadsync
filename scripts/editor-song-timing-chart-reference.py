#!/usr/bin/env python3
"""Run pinned song timing-chart selection, mutation and serialization methods."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tempfile

EDITOR = "48e18f71655d0f0d86144c63239331833501fda2"
LIBRARY = "bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8"
FUMEN = "0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89"


def member(text, signature):
    start = text.index(signature)
    opening = text.index("{", start)
    depth = 0
    for end in range(opening, len(text)):
        depth += (text[end] == "{") - (text[end] == "}")
        if depth == 0:
            return text[start:end + 1]
    raise ValueError(signature)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["editor-root", "reference-root", "dotnet", "output"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    sources = {}
    provenance = {}
    for name, root, pin, path in [
        ("song", args.editor_root, EDITOR, "StepManiaEditor/EditorSong.cs"),
        ("chart", args.editor_root, EDITOR, "StepManiaEditor/EditorChart.cs"),
        ("editor", args.editor_root, EDITOR, "StepManiaEditor/Editor.cs"),
        ("options", args.editor_root, EDITOR, "StepManiaEditor/Preferences/PreferencesOptions.cs"),
    ]:
        raw = subprocess.check_output(["git", "-C", str(root), "show", f"{pin}:{path}"])
        sources[name] = raw.decode("utf-8-sig")
        provenance[name] = {"path": path, "sha256": hashlib.sha256(raw).hexdigest()}
    for root, pin in [(args.reference_root, LIBRARY), (args.reference_root / "Fumen", FUMEN)]:
        assert subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip() == pin
        subprocess.run(["git", "-C", str(root), "diff", "--exit-code", pin, "--"], check=True, capture_output=True)

    signatures = ["private class CustomSaveDataV1", "public EditorChart TimingChart", "private void ChooseTimingChart()", "public IReadOnlyList<EditorChart> GetCharts(ChartType chartType)", "public EditorChart SelectBestChart(", "private void UpdateChartSortInternal()", "public IReadOnlyList<EditorChart> GetSortedCharts()", "public void DeleteChart(EditorChart chart)", "private void SerializeCustomSongData(", "private bool DeserializeV1CustomData("]
    extracted = "\n".join(member(sources["song"], signature) for signature in signatures)
    options = member(sources["song"], "private static readonly JsonSerializerOptions CustomSaveDataSerializationOptions") + ";"
    start = sources["editor"].index("public static readonly ChartType[] SupportedChartTypes =")
    supported = sources["editor"][start:sources["editor"].index(";", start) + 1]
    comparer = sources["chart"][sources["chart"].index("internal sealed class ChartComparer :"):]
    constants = "\n".join(re.findall(r"public const (?:ChartType|ChartDifficultyType) DefaultDefault(?:Steps|Difficulty)Type = .*?;", sources["options"]))
    charts = [
        {"Type": "dance_single", "Difficulty": 4, "Rating": 15, "Name": "zebra"},
        {"Type": "dance_single", "Difficulty": 4, "Rating": 1, "Name": "éclair"},
        {"Type": "dance_single", "Difficulty": 3, "Rating": 20, "Name": "Ångström"},
        {"Type": "pump_double", "Difficulty": 4, "Rating": 8, "Name": "pump"},
    ]
    custom = [None, {}, {"DefaultChartType": "dance_single", "DefaultChartIndex": 1}, {"DefaultChartType": 0, "DefaultChartIndex": 0}, {"DefaultChartType": "DANCE_SINGLE", "DefaultChartIndex": 2}, {"DefaultChartType": None}, {"DefaultChartType": "dance_single", "DefaultChartIndex": -1}, {"DefaultChartType": "dance_single", "DefaultChartIndex": 999}, {"DefaultChartType": "smx_team"}, {"DefaultChartType": "invalid"}, {"DefaultChartIndex": "1"}, {"SyncOffset": 0.125, "DefaultChartType": "pump_double"}]
    cases = []
    for index, data in enumerate(custom):
        for deletion in [None, 0, 1, 3]:
            cases.append({"Name": f"custom-{index}-delete-{deletion}", "Charts": charts, "Custom": data, "Delete": deletion})
    for selected in [-1, 0, 1, 3, 999]:
        cases.append({"Name": f"setter-{selected}", "Charts": charts, "Custom": None, "Set": selected})
    for deletion in [0, 3]:
        cases.append({"Name": f"null-setter-delete-{deletion}", "Charts": charts, "Custom": None, "Set": -1, "Delete": deletion})
    cases.append({"Name": "empty", "Charts": [], "Custom": None})
    request = {"Cultures": ["en-US", "fr-FR"], "Cases": cases}
    program = r'''
using System;using System.Linq;using System.Collections.Generic;using System.Globalization;using System.Text.Json;using System.Text.Json.Serialization;using static System.Diagnostics.Debug;using static Fumen.Converters.SMCommon;using StepManiaEditor;
namespace StepManiaEditor {
sealed class Editor { SUPPORTED }
sealed class Options { CONSTANTS public ChartType DefaultStepsType=DefaultDefaultStepsType;public ChartDifficultyType DefaultDifficultyType=DefaultDefaultDifficultyType; }
sealed class Preferences { public static Preferences Instance=new();public Options PreferencesOptions=new(); }
static class Logger { public static void Warn(string s){} }
sealed class EditorChart {public int Source;public ChartType ChartType;public ChartDifficultyType ChartDifficultyType;public int Rating;public string Name;public string Description="";public int Steps=1;public Guid Id;public EditorChart GetStepTotals()=>this;public int GetStepCount()=>Steps;public Guid GetGuid()=>Id;public void SetIndexInSong(int i){} }
sealed class EditorSong {
readonly Dictionary<ChartType,List<EditorChart>> Charts=new();EditorChart TimingChartInternal;public double SyncOffset;const string TagCustomSongDataVersion="GASongDataVersion",TagCustomSongData="GASongData";const int CustomSaveDataVersion=1;bool CanBeEdited()=>true;string GetCustomPropertyName(string name)=>name;
OPTIONS
METHODS
public static object Run(JsonElement input){var song=new EditorSong{SyncOffset=0.009};int index=0;foreach(var e in input.GetProperty("Charts").EnumerateArray()){var c=new EditorChart{Source=index++,ChartType=Enum.Parse<ChartType>(e.GetProperty("Type").GetString()),ChartDifficultyType=(ChartDifficultyType)e.GetProperty("Difficulty").GetInt32(),Rating=e.GetProperty("Rating").GetInt32(),Name=e.GetProperty("Name").GetString(),Id=new Guid(index,0,0,new byte[8])};if(!ChartsSupported(c))continue;if(!song.Charts.ContainsKey(c.ChartType))song.Charts[c.ChartType]=new();song.Charts[c.ChartType].Add(c);}
bool? decoded=null;if(input.GetProperty("Custom").ValueKind!=JsonValueKind.Null)decoded=song.DeserializeV1CustomData(input.GetProperty("Custom").GetRawText());if(song.TimingChart==null)song.ChooseTimingChart();var loaded=song.TimingChart?.Source;song.UpdateChartSortInternal();var sorted=song.GetSortedCharts().Select(c=>c.Source).ToArray();if(input.TryGetProperty("Set",out var set)){int id=set.GetInt32();song.TimingChart=id==-1?null:song.GetSortedCharts().FirstOrDefault(c=>c.Source==id)??new EditorChart{Source=id};}if(input.TryGetProperty("Delete",out var del)&&del.ValueKind!=JsonValueKind.Null){var chart=song.GetSortedCharts().First(c=>c.Source==del.GetInt32());song.DeleteChart(chart);}var output=new Dictionary<string,string>();song.SerializeCustomSongData(output);return new{Name=input.GetProperty("Name").GetString(),Decoded=decoded,Loaded=loaded,Order=sorted,Selected=song.TimingChart?.Source,Metadata=JsonDocument.Parse(output[TagCustomSongData]).RootElement.Clone()};}
static bool ChartsSupported(EditorChart chart)=>Editor.SupportedChartTypes.Contains(chart.ChartType);
}
COMPARER
}
class Program {static void Main(){var input=JsonDocument.Parse(Console.In.ReadToEnd()).RootElement;var results=new List<object>();foreach(var culture in input.GetProperty("Cultures").EnumerateArray()){CultureInfo.CurrentCulture=CultureInfo.GetCultureInfo(culture.GetString());results.Add(new{Culture=culture.GetString(),Cases=input.GetProperty("Cases").EnumerateArray().Select(EditorSong.Run).ToArray()});}Console.Write(JsonSerializer.Serialize(results));}}
'''.replace("SUPPORTED", supported).replace("CONSTANTS", constants).replace("OPTIONS", options).replace("METHODS", extracted).replace("COMPARER", comparer)
    subprocess.run([str(args.dotnet), "build", str(args.reference_root / "Fumen/Fumen/Fumen.csproj"), "-c", "Release", "--nologo", "-v", "quiet"], check=True)
    with tempfile.TemporaryDirectory(prefix="deadsync-song-timing-") as directory:
        root = Path(directory)
        (root / "Program.cs").write_text(program)
        (root / "Oracle.csproj").write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{args.reference_root}/Fumen/Fumen/bin/Release/net10.0/Fumen.dll</HintPath></Reference></ItemGroup></Project>')
        subprocess.run([str(args.dotnet), "build", str(root / "Oracle.csproj"), "-c", "Release", "--nologo", "-v", "quiet"], check=True)
        assembly = root / "bin/Release/net10.0/Oracle.dll"
        result = subprocess.run([str(args.dotnet), str(assembly)], input=json.dumps(request), capture_output=True, text=True, check=True)
        expected = json.loads(result.stdout)
        assembly_hash = hashlib.sha256(assembly.read_bytes()).hexdigest()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.with_suffix(".request.json").write_text(json.dumps(request, indent=2, ensure_ascii=False) + "\n")
    args.output.with_suffix(".expected.json").write_text(json.dumps(expected, indent=2, ensure_ascii=False) + "\n")
    args.output.with_suffix(".reference.json").write_text(json.dumps({"editor_commit": EDITOR, "library_commit": LIBRARY, "fumen_commit": FUMEN, "sources": provenance, "methods_sha256": hashlib.sha256(extracted.encode()).hexdigest(), "program_sha256": hashlib.sha256(program.encode()).hexdigest(), "assembly_sha256": assembly_hash, "scope": "Original timing setter, Choose/SelectBest, sorting, DeleteChart, V1 deserialize/serialize. Property-only valid chart adapter, editable host, fixed open sync preference0.009, and identity custom tag names. Constructor sequence copied explicitly: deserialize/fallback before sort. No full EditorSong load, clocks, writer or native lifecycle proof."}, indent=2) + "\n")
    print(f"Wrote {len(cases)} timing-chart scenarios in two cultures")


if __name__ == "__main__":
    main()
