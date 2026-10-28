#!/usr/bin/env python3
"""Execute pinned original binding default declarations under an explicit OS host stub."""
import argparse, hashlib, json, re, subprocess, tempfile
from pathlib import Path
PIN='48e18f71655d0f0d86144c63239331833501fda2'
SOURCE='StepManiaEditor/Preferences/PreferencesKeyBinds.cs'
def main():
 p=argparse.ArgumentParser(__doc__);p.add_argument('--source',required=True,type=Path);p.add_argument('--dotnet',required=True);p.add_argument('--output',required=True,type=Path);a=p.parse_args()
 data=(a.source/SOURCE).read_bytes();pinned=subprocess.check_output(['git','show',f'{PIN}:{SOURCE}'],cwd=a.source)
 if data!=pinned:raise SystemExit('Source differs from pinned Git blob')
 text=data.decode('utf-8-sig');lines=text.splitlines()
 fields=[line.strip() for line in lines if re.match(r'\s*private static readonly (?:Keys Ctrl|List<Keys\[\]> Default\w+)\s*=',line)]
 if len(fields)!=171:raise SystemExit(f'Expected Ctrl plus170 defaults, got {len(fields)}')
 names=sorted(set(re.findall(r'Keys\.(\w+)',text)))
 program='''using System.Reflection;\nusing System.Runtime.InteropServices;\nusing System.Text.Json;\nHost.IsMac=args[0]=="mac";\nConsole.WriteLine(JsonSerializer.Serialize(typeof(Defaults).GetFields(BindingFlags.NonPublic|BindingFlags.Static).Where(f=>f.FieldType==typeof(List<Keys[]>)).ToDictionary(f=>f.Name.Substring(7),f=>((List<Keys[]>)f.GetValue(null)!).Select(keys=>keys.Select(k=>k.ToString()).ToArray()).ToArray())));\nstatic class Host {public static bool IsMac;}\nstatic class RuntimeInformation {public static bool IsOSPlatform(OSPlatform platform)=>platform==OSPlatform.OSX&&Host.IsMac;}\nenum Keys {None,'''+','.join(names)+'''}\nclass Defaults {\n'''+ '\n'.join(fields)+'\n}\n'
 with tempfile.TemporaryDirectory(prefix='deadsync-key-profile-') as temp:
  t=Path(temp);(t/'Program.cs').write_text(program);(t/'Oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup></Project>')
  subprocess.run([a.dotnet,'build',str(t/'Oracle.csproj'),'--verbosity','quiet'],check=True,capture_output=True,text=True)
  results={}
  for platform in ['mac','other']:
   run=subprocess.run([a.dotnet,str(t/'bin/Debug/net10.0/Oracle.dll'),platform],check=True,capture_output=True,text=True)
   results[platform]=json.loads(run.stdout)
  if any(len(v)!=170 for v in results.values()):raise SystemExit('Unexpected default count')
 out={'commit':PIN,'path':SOURCE,'sha256':hashlib.sha256(data).hexdigest(),'method':'170 original static List<Keys[]> default declarations and original Ctrl declaration, unchanged; host-only RuntimeInformation stub selects mac/other; original class methods and PostLoad are not executed','platforms':results}
 a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(out,indent=2,ensure_ascii=False)+'\n')
 print('Executed 340 default fields across 2 host platforms')
if __name__=='__main__':main()
