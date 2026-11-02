#!/usr/bin/env python3
"""Execute unchanged source ImGui numeric wrappers after deterministic widget output.

The widget adapter supplies values that a committed input returns; actual ImGui
mouse/keyboard parsing is outside this fixture. Post-widget bounds and casts are
original source bodies, not a reimplementation. No renderer or native UI claim.
"""
import argparse,json,subprocess,tempfile,hashlib
from pathlib import Path
PIN='48e18f71655d0f0d86144c63239331833501fda2'
def method(text,key):
 a=text.index(key);i=text.index('{',a)+1;n=1
 while n:n+=(text[i]=='{')-(text[i]=='}');i+=1
 return text[a:i]
def main():
 p=argparse.ArgumentParser();p.add_argument('--source',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
 raw=subprocess.check_output(['git','-C',str(a.source),'show',PIN+':StepManiaEditor/ImGui/ImGuiUtils.cs']);text=raw.decode('utf-8-sig')
 bodies='\n'.join(method(text,key)for key in ['public static bool SliderUInt(','public static bool DragInt(','public static unsafe bool DragDouble('])
 host='''using System.Runtime.InteropServices;enum ImGuiDataType{Double}static class ImGui{public static int IntValue;public static double DoubleValue;public static bool SliderInt(string text,ref int value,int min,int max,string format,int flags){value=IntValue;return true;}public static bool DragInt(string text,ref int value,float speed,int min,int max,string format){value=IntValue;return true;}public static bool DragScalar(string text,ImGuiDataType type,IntPtr p,float speed,IntPtr min,IntPtr max,string format){Marshal.Copy(new[]{DoubleValue},0,p,1);return true;}}static class Probe{'''+bodies.replace('ImGuiSliderFlags','int')+'}'
 program='''using System.Text.Json;var rows=new List<object>();foreach(var control in new[]{("receptor-x",0,853),("receptor-y",0,479),("stream-min",0,8),("stream-cutoff",0,64),("history",1,32768),("frames",2,1024)})foreach(var raw in new[]{int.MinValue,-1,0,7,137,32769,int.MaxValue}){ImGui.IntValue=raw;int i=0;Probe.DragInt(ref i,"",1,"",control.Item2,control.Item3);rows.Add(new{kind="integer",control=control.Item1,raw,result=i});}foreach(var control in new[]{("sync",0.0,double.MaxValue),("dpi",0.25,8.0),("explicit",0.0,1.0)})foreach(var raw in new[]{-100.0,-0.0,0.0,0.123456789012345,1.23456789,12.0,1e100}){ImGui.DoubleValue=raw;double d=0;Probe.DragDouble(ref d,"",.001f,"",control.Item2,control.Item3);rows.Add(new{kind="double",control=control.Item1,raw,raw_text=raw.ToString("R",System.Globalization.CultureInfo.InvariantCulture),result=d,result_bits=BitConverter.DoubleToInt64Bits(d).ToString("X16")});}Console.Write(JsonSerializer.Serialize(rows));'''
 with tempfile.TemporaryDirectory()as d:
  w=Path(d);(w/'Host.cs').write_text(host);(w/'Program.cs').write_text(program);(w/'Probe.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><AllowUnsafeBlocks>true</AllowUnsafeBlocks></PropertyGroup></Project>')
  build=subprocess.run([str(a.dotnet),'build',str(w/'Probe.csproj'),'-c','Release','--nologo','-v','q'],capture_output=True,text=True)
  if build.returncode:raise SystemExit(build.stdout+build.stderr)
  result=json.loads(subprocess.check_output([str(a.dotnet),str(w/'bin/Release/net10.0/Probe.dll')]))
 a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(result,indent=2)+'\n');a.output.with_suffix('.reference.json').write_text(json.dumps(dict(editor_commit=PIN,source_sha256=hashlib.sha256(raw).hexdigest(),callsite_sha256={path:hashlib.sha256(subprocess.check_output(['git','-C',str(a.source),'show',PIN+':'+path])).hexdigest()for path in ['StepManiaEditor/UI/UIOptions.cs','StepManiaEditor/UI/UIStreamPreferences.cs','StepManiaEditor/UI/UIReceptorPreferences.cs','StepManiaEditor/UI/UIPerformance.cs','StepManiaEditor/PerformanceMonitor.cs']},host_sha256=hashlib.sha256(host.encode()).hexdigest(),scope=__doc__),indent=2)+'\n');print('Executed',len(result),'original post-widget bounds/cast cases')
if __name__=='__main__':main()
