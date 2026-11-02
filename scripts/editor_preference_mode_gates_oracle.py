#!/usr/bin/env python3
"""Original pinned UI conditional sections with deterministic widget capture.
Captures only source enabled/visible gates. Widget APIs and their values are host
adapters; this does not execute ImGui, native rendering or persistence.
"""
import argparse,subprocess,tempfile,json,hashlib
from pathlib import Path
PIN="48e18f71655d0f0d86144c63239331833501fda2"
def show(root,path):return subprocess.check_output(['git','-C',str(root),'show',PIN+':'+path]).decode('utf-8-sig')
def block(source,start):
 opening=source.index('{',start);i=opening+1;depth=1
 while depth:depth+=(source[i]=='{')-(source[i]=='}');i+=1
 return source[start:i]
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--source',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
 scroll=show(a.source,'StepManiaEditor/UI/UIScrollPreferences.cs');wave=show(a.source,'StepManiaEditor/UI/UIWaveFormPreferences.cs');model=show(a.source,'StepManiaEditor/Preferences/PreferencesWaveForm.cs')
 start=scroll.index('if (p.SpacingMode != Editor.SpacingMode.ConstantTime)');last=scroll.index('if (p.SpacingMode != Editor.SpacingMode.Variable)',start);last=scroll.index('PopDisabled();',last)+len('PopDisabled();');scrollbody=scroll[start:last]
 wavebody=block(wave,wave.index('switch (p.WaveFormSparseColorOption)'));enum=block(model,model.index('public enum SparseColorOption'))
 host=r'''using System;using System.Linq;using System.Collections.Generic;using System.Text.Json;
class Editor{public enum SpacingMode{ConstantTime,ConstantRow,Variable}}
class PreferencesScroll{public Editor.SpacingMode SpacingMode;public float TimeBasedPixelsPerSecondFloat,RowBasedPixelsPerRowFloat,VariablePixelsPerSecondAtDefaultBPMFloat;public const double DefaultTimeBasedPixelsPerSecond=300,DefaultRowBasedPixelsPerRow=2,DefaultVariablePixelsPerSecondAtDefaultBPM=300;public const int DefaultVariableSpeedBPM=300;}
class PreferencesWaveForm{ENUM public SparseColorOption WaveFormSparseColorOption;public float WaveFormSparseColorScale;public object WaveFormSparseColor;}
static class ZoomManager{public const double MinConstantTimeSpeed=10,MaxConstantTimeSpeed=100000,MinConstantRowSpacing=.1,MaxConstantRowSpacing=10000,MinVariableSpeed=10,MaxVariableSpeed=100000;}
static class SMCommon{public const int MaxValidDenominator=192,NumBeatsPerMeasure=4;}
enum ImGuiSliderFlags{Logarithmic}enum ImGuiColorEditFlags{AlphaPreviewHalf=1,AlphaBar=2}
static class ImGuiLayoutUtils{public static Dictionary<string,bool> Draws=new();public static int Disabled;public static void DrawRowSliderFloatWithReset(params object[] args){Draws[(string)args[1]]=Disabled==0;}public static void DrawRowSliderFloat(params object[] args){Draws[(string)args[1]]=Disabled==0;}public static void DrawRowColorEdit4(params object[] args){Draws[(string)args[1]]=Disabled==0;}}
class Probe{static void PushDisabled(){ImGuiLayoutUtils.Disabled++;}static void PopDisabled(){ImGuiLayoutUtils.Disabled--;}public static void Scroll(PreferencesScroll p){SCROLL}public static void Wave(PreferencesWaveForm p){WAVE}}
class Program{static void Main(){var cases=new List<object>();foreach(var mode in new[]{0,1,2,-1,99}){ImGuiLayoutUtils.Draws.Clear();Probe.Scroll(new(){SpacingMode=(Editor.SpacingMode)mode});cases.Add(new{kind="scroll",raw=mode,enabled=new[]{ImGuiLayoutUtils.Draws["Constant Time Speed"],ImGuiLayoutUtils.Draws["Constant Row Spacing"],ImGuiLayoutUtils.Draws["Variable Speed"]}});ImGuiLayoutUtils.Draws.Clear();Probe.Wave(new(){WaveFormSparseColorOption=(PreferencesWaveForm.SparseColorOption)mode});cases.Add(new{kind="sparse",raw=mode,scale=ImGuiLayoutUtils.Draws.ContainsKey("Sparse Color Scale"),color=ImGuiLayoutUtils.Draws.ContainsKey("Sparse Color")});}Console.Write(JsonSerializer.Serialize(cases));}}
'''.replace('ENUM',enum).replace('SCROLL',scrollbody).replace('WAVE',wavebody)
 with tempfile.TemporaryDirectory(prefix='deadsync-mode-gates-')as t:
  w=Path(t);(w/'Program.cs').write_text(host);(w/'Oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup></Project>');b=subprocess.run([str(a.dotnet),'build',str(w/'Oracle.csproj'),'-o',str(w/'out'),'--nologo'],capture_output=True,text=True)
  if b.returncode:raise SystemExit(b.stdout+b.stderr)
  result=json.loads(subprocess.check_output([str(a.dotnet),str(w/'out/Oracle.dll')]))
 a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(result,indent=2)+'\n');a.output.with_suffix('.reference.json').write_text(json.dumps(dict(editor_commit=PIN,scroll_source_sha256=hashlib.sha256(scroll.encode()).hexdigest(),waveform_source_sha256=hashlib.sha256(wave.encode()).hexdigest(),waveform_model_sha256=hashlib.sha256(model.encode()).hexdigest(),host_sha256=hashlib.sha256(host.encode()).hexdigest(),scope=__doc__),indent=2)+'\n');print('Executed',len(result),'original UI gate cases')
if __name__=='__main__':main()
