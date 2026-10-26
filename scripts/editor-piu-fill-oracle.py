#!/usr/bin/env python3
"""Run original Fumen HSV and GrooveAuthor PIU Desaturate/BlendColor functions."""
import argparse,hashlib,json,subprocess,tempfile
from pathlib import Path
PIN='48e18f71655d0f0d86144c63239331833501fda2'
def extract(s,m):
 start=s.index(m);pos=s.index('{',start);end=pos+1;depth=1
 while depth:depth+=(s[end]=='{')-(s[end]=='}');end+=1
 return s[start:end]
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--fumen-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();r=a.reference_root.resolve();f=a.fumen_root.resolve()
 paths={'prefs':r/'StepManiaEditor/Preferences/PreferencesNoteColor.cs','hsv':f/'Fumen/ColorUtils.cs','utils':f/'Fumen/Utils.cs'}
 fc=subprocess.check_output(['git','-C',str(f),'rev-parse','HEAD'],text=True).strip()
 for name,path in paths.items():
  repo,pin=(r,PIN) if name=='prefs' else (f,fc)
  assert path.read_bytes()==subprocess.check_output(['git','-C',str(repo),'show',f'{pin}:{path.relative_to(repo)}'])
 s={k:p.read_text(encoding='utf-8-sig') for k,p in paths.items()}
 m={'rgb':extract(s['hsv'],'public static (float, float, float) RgbToHsv('),'hsv':extract(s['hsv'],'public static (float, float, float) HsvToRgb('),'equal':extract(s['utils'],'public bool FloatEquals(float other)'),'desaturate':extract(s['prefs'],'Vector3 Desaturate(Vector3 color)'),'blend':extract(s['prefs'],'Vector3 BlendColor(Vector3 c)')}
 inputs=[]
 for rgb in [[.7019608,.1058824,.2156863],[1,.9176471,0],[.1058824,.2509804,.7019608],[0,0,0],[.5,.5,.5],[1,0,0],[0,1,0],[0,0,1]]:
  for owner in [None,[.7109375,.09375,.7109375],[.09375,.7109375,.7109375]]:
   inputs.append(dict(rgb=rgb,player=owner,multiplier=.8,alpha=.9))
 program='''using System;using System.Numerics;using System.Text.Json;using System.Collections.Generic;
static class Extensions{const float MinFloatDelta=0.0000001f;
'''+m['equal'].replace('public bool FloatEquals(float other)','public static bool FloatEquals(this float f,float other)')+'''}
static class ColorUtils{'''+m['rgb']+m['hsv']+'''}
class Program{class Prefs{public float PiuHoldSaturationMultiplier;}static Prefs preferences=new();static Vector3 multiplayerColor;static float a;
'''+m['desaturate'].replace('Vector3 Desaturate','static Vector3 Desaturate',1)+m['blend'].replace('Vector3 BlendColor','static Vector3 BlendColor',1)+'''
static void Main(){var req=JsonDocument.Parse(Console.In.ReadToEnd()).RootElement;var output=new List<float[]>();foreach(var q in req.EnumerateArray()){Vector3 V(JsonElement v)=>new(v[0].GetSingle(),v[1].GetSingle(),v[2].GetSingle());var rgb=V(q.GetProperty("rgb"));preferences.PiuHoldSaturationMultiplier=q.GetProperty("multiplier").GetSingle();a=q.GetProperty("alpha").GetSingle();if(q.GetProperty("player").ValueKind!=JsonValueKind.Null){multiplayerColor=V(q.GetProperty("player"));rgb=BlendColor(rgb);}var c=Desaturate(rgb);output.Add(new[]{c.X,c.Y,c.Z});}Console.Write(JsonSerializer.Serialize(output));}}
'''
 with tempfile.TemporaryDirectory(prefix='deadsync-piu-fill-') as d:
  d=Path(d);(d/'oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework></PropertyGroup></Project>');(d/'Program.cs').write_text(program)
  b=subprocess.run([str(a.dotnet),'build',str(d/'oracle.csproj'),'--nologo','-v','q'],capture_output=True,text=True)
  if b.returncode:raise SystemExit(b.stdout+b.stderr)
  out=subprocess.run([str(a.dotnet),str(d/'bin/Debug/net10.0/oracle.dll')],input=json.dumps(inputs),capture_output=True,text=True,check=True)
 doc=dict(reference_commit=PIN,fumen_commit=fc,source_bytes_verified_against_git=True,source_sha256={k:hashlib.sha256(p.read_bytes()).hexdigest() for k,p in paths.items()},method_sha256={k:hashlib.sha256(v.encode()).hexdigest() for k,v in m.items()},scope='Unchanged HSV, FloatEquals, Desaturate and BlendColor bodies. Local functions adapted to static methods with their captured inputs explicit; no preference cache or full graphics manager execution.',cases=[dict(**i,expected=e) for i,e in zip(inputs,json.loads(out.stdout))])
 a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(doc,indent=2)+'\n');print(len(inputs),'original PIU HSV/blend cases')
if __name__=='__main__':main()
