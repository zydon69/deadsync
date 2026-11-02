#!/usr/bin/env python3
"""Run pinned original UIFTUE Draw/navigation with recording UI adapters.

Adapters implement ImGui drawing, preferences storage and documentation buttons;
UIOptions value editors are stubs, so this proves progression, not option values.
"""
import argparse
import hashlib
import json
import subprocess
import tempfile
from pathlib import Path
PIN = "48e18f71655d0f0d86144c63239331833501fda2"
HOST = 'using System.Numerics;using System.Text.Json;\nnamespace ImGuiNET{[Flags]enum ImGuiWindowFlags{NoResize=1,Modal=2,NoMove=4,NoDecoration=8}enum ImGuiCond{Always}static class ImGui{public static string Click="";public static List<string> Buttons=new();public static bool Button(string label,Vector2 size){Buttons.Add(label);if(Click==label){Click="";return true;}return false;}public static void TextUnformatted(string s){}public static void TextWrapped(string s){}public static void Separator(){}public static void EndPopup(){}public static void OpenPopup(string s){}public static void SetNextWindowSize(Vector2 v){}public static void SetNextWindowPos(Vector2 v,ImGuiCond c){}public static bool BeginPopupModal(string s,ref bool o,ImGuiWindowFlags f)=>true;public static void SameLine(){}public static void Dummy(Vector2 v){}public static Vector2 GetContentRegionAvail()=>new(700,500);public static Style GetStyle()=>new();public static void CloseCurrentPopup(){} }class Style{public Vector2 ItemSpacing=new(1,1);}}\nnamespace StepManiaEditor{static class ImGuiUtils{public static int UiScaled(int i)=>i;}class Editor{public int GetViewportWidth()=>1000;public int GetViewportHeight()=>800;}class Preferences{public static Preferences Instance=new();public Version LastCompletedFtueVersion;public int FtueIndex;}static class Utils{public static Version AppVersion=new(0,1,0);public static Version GetAppVersion()=>AppVersion;public static string GetAppName()=>"GrooveAuthor";}static class ImGuiLayoutUtils{public static bool BeginTable(string s,int i)=>true;public static void EndTable(){}public static bool DrawRowButton(string a,string b,string c)=>false;}static class UIOptions{public static void DrawDefaultType(bool b){}public static void DrawNewSongSync(bool b){}public static void DrawDefaultSongSync(bool b){}public static void DrawStartupStepGraphs(bool b){}}static class Documentation{public static string GitHubUrl="";public static void OpenDocumentation(){}public static void OpenGitHub(){}}class Program{static void Main(){var rows=new List<object>();foreach(var version in new[]{"0.0.9","0.1.0","1.0.0"})foreach(var completed in new[]{"null","0.0.9","0.1.0","1.0.0"})foreach(var index in new[]{-1,0,1,2,3,4,5,6})foreach(var click in new[]{"","Back","Next","Done"}){Utils.AppVersion=Version.Parse(version);Preferences.Instance=new(){LastCompletedFtueVersion=completed=="null"?null:Version.Parse(completed),FtueIndex=index};ImGuiNET.ImGui.Buttons.Clear();ImGuiNET.ImGui.Click=click;var ui=new UIFTUE(new());ui.Draw();rows.Add(new{version,completed,index,click,resultIndex=Preferences.Instance.FtueIndex,resultCompleted=Preferences.Instance.LastCompletedFtueVersion?.ToString(),hasCompleted=ui.HasCompletedFTUE(),buttons=ImGuiNET.ImGui.Buttons.ToArray()});}Console.Write(JsonSerializer.Serialize(rows));}}}\n'

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--dotnet', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    source = subprocess.check_output(['git', '-C', str(args.source), 'show', PIN + ':StepManiaEditor/UI/UIFTUE.cs'])
    with tempfile.TemporaryDirectory(prefix='deadsync-ftue-reference-') as directory:
        work = Path(directory)
        (work / 'UIFTUE.cs').write_bytes(source)
        (work / 'Program.cs').write_text(HOST)
        (work / 'Oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup></Project>')
        subprocess.run([args.dotnet, 'build', str(work / 'Oracle.csproj'), '--nologo', '--verbosity', 'quiet'], check=True)
        result = subprocess.check_output([args.dotnet, str(work / 'bin/Debug/net10.0/Oracle.dll')])
    cases = json.loads(result)
    if len(cases) != 384:
        raise SystemExit('Unexpected FTUE oracle coverage')
    args.output.write_bytes(result)
    args.output.with_suffix('.provenance.json').write_text(json.dumps({'revision':PIN,'source':'StepManiaEditor/UI/UIFTUE.cs','sha256':hashlib.sha256(source).hexdigest(),'cases':len(cases),'scope':'unchanged original Draw/navigation; UIOptions editors stubbed'},indent=2)+'\n')
    print(f'Executed original FTUE: {len(cases)} cases')
if __name__ == '__main__':
    main()
