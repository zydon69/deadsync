#!/usr/bin/env python3
"""Run unchanged ArrowColor and pinned MonoGame Color constructors; UI helpers are inert."""
import argparse,hashlib,json,subprocess,tempfile
from pathlib import Path
PIN='48e18f71655d0f0d86144c63239331833501fda2'
MONOGAME='8e61b2effa9f8775bd63f24dc33b2f7924f9eba2'
MONOGAME_COLOR_SHA256='bb40ac7bfc5b275965a9c7bdf14acf134df9756fbee1ba17f88181239b0e70ac'
def extract(s,marker):
 start=s.index(marker); pos=s.index('{',start);end=pos+1;depth=1
 while depth:depth+=(s[end]=='{')-(s[end]=='}');end+=1
 return s[start:end]
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--monogame-color',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
 r=a.reference_root.resolve();path='StepManiaEditor/Preferences/PreferencesNoteColor.cs';raw=(r/path).read_bytes()
 assert raw==subprocess.check_output(['git','-C',str(r),'show',f'{PIN}:{path}'])
 assert MONOGAME in subprocess.check_output(['git','-C',str(r),'ls-tree',PIN,'StepManiaEditor/MonoGame'],text=True)
 color=a.monogame_color.read_bytes();assert hashlib.sha256(color).hexdigest()==MONOGAME_COLOR_SHA256;s=raw.decode('utf-8-sig');c=color.decode('utf-8-sig')
 methods={'arrow_color':extract(s,'public class ArrowColor'),'float_color':extract(c,'public Color(float r, float g, float b)'),'int_color':extract(c,'public Color(int r, int g, int b)')}
 inputs=[]
 for rgb in [[0.2666667]*3,[0.7803921,0.4666667,0.1607843],[0.7109375,0.09375,0.7109375],[0.094,0.999,0.5],[0,1,.1]]:
  for held in [False,True]:
   for selected in [False,True]:inputs.append(dict(rgb=rgb,held=held,selected=selected))
 program='''using System;using System.Numerics;using System.Collections.Generic;using System.Text.Json;
static class MathHelper{public static int Clamp(int x,int lo,int hi)=>Math.Clamp(x,lo,hi);}
public struct Color{uint _packedValue;public byte R=>(byte)_packedValue;public byte G=>(byte)(_packedValue>>8);public byte B=>(byte)(_packedValue>>16);public byte A=>(byte)(_packedValue>>24);
'''+methods['float_color']+methods['int_color']+'''}
static class Utils{public static uint ToRGBA(float r,float g,float b,float a)=>0;public static uint ColorRGBAMultiply(uint c,float m)=>0;}
public class PreferencesNoteColor{public float HeldColorMultiplier=1.5f,SelectionColorMultiplier=1.7f,ArrowUIColorMultiplier=1.5f,ArrowUISelectedColorMultiplier=8f;}
'''+methods['arrow_color']+'''
class Program{static void Main(){var values=JsonDocument.Parse(Console.In.ReadToEnd()).RootElement;var output=new List<byte[]>();foreach(var i in values.EnumerateArray()){var a=i.GetProperty("rgb");var rgb=new Vector3(a[0].GetSingle(),a[1].GetSingle(),a[2].GetSingle());var color=new ArrowColor(rgb,new());bool held=i.GetProperty("held").GetBoolean(),selected=i.GetProperty("selected").GetBoolean();var c=selected?(held?color.XnaHeldAndSelectedColor:color.XnaSelectedColor):(held?color.XnaHeldColor:color.XnaColor);output.Add(new[]{c.R,c.G,c.B,c.A});}Console.Write(JsonSerializer.Serialize(output));}}
'''
 with tempfile.TemporaryDirectory(prefix='deadsync-hold-colors-') as folder:
  d=Path(folder);(d/'oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework></PropertyGroup></Project>');(d/'Program.cs').write_text(program)
  b=subprocess.run([str(a.dotnet),'build',str(d/'oracle.csproj'),'--nologo','-v','q'],capture_output=True,text=True)
  if b.returncode:raise SystemExit(b.stdout+b.stderr)
  o=subprocess.run([str(a.dotnet),str(d/'bin/Debug/net10.0/oracle.dll')],input=json.dumps(inputs),capture_output=True,text=True,check=True)
 # System.Text.Json encodes byte[] as base64; retain numeric channel fixtures.
 import base64
 outputs=[list(base64.b64decode(x)) for x in json.loads(o.stdout)]
 doc=dict(reference_commit=PIN,source_bytes_verified_against_git=True,source_path=path,source_sha256=hashlib.sha256(raw).hexdigest(),monogame_commit=MONOGAME,monogame_path='MonoGame.Framework/Color.cs',monogame_sha256=hashlib.sha256(color).hexdigest(),monogame_provenance='Raw pinned submodule URL, supplied as --monogame-color; no local submodule checkout claimed.',method_sha256={k:hashlib.sha256(v.encode()).hexdigest() for k,v in methods.items()},scope='Original ArrowColor plus original MonoGame float/int constructors. Inert UI helper adapters; only XNA fills compared, not UI packing or full preference cache/layout resolution.',cases=[dict(**i,expected=e) for i,e in zip(inputs,outputs)])
 a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(doc,indent=2)+'\n');print(len(inputs),'original XNA hold color variants')
if __name__=='__main__':main()
