#!/usr/bin/env python3
"""Execute the pinned original EditorItgManiaPack.Delete against real files."""
import argparse,hashlib,json,subprocess,tempfile
from pathlib import Path
PIN='48e18f71655d0f0d86144c63239331833501fda2'
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--editor-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
raw=subprocess.check_output(['git','-C',str(a.editor_root),'show',PIN+':StepManiaEditor/EditorItgManiaPack.cs']);text=raw.decode('utf-8-sig');start=text.index('public void Delete()');brace=text.index('{',start);depth=0
for end in range(brace,len(text)):
 if text[end]=='{':depth+=1
 elif text[end]=='}':depth-=1
 if depth==0:break
body=text[start:end+1]
program='''using System.Text.Json;
static class Logger {public static int Errors;public static void Error(string s){Errors++;}}
sealed class Original(string path){const string FileName="Pack.ini";readonly string FilePath=path;public bool Stopped;void StopObservingFile(){Stopped=true;}
BODY
}
class Program{static void Main(){var rows=new List<object>();var root=Path.Combine(Path.GetTempPath(),Guid.NewGuid().ToString());Directory.CreateDirectory(root);try{foreach(var name in new[]{"regular","dirty","cancel","missing","directory","symlink"}){var dir=Path.Combine(root,name);Directory.CreateDirectory(dir);var path=Path.Combine(dir,"Pack.ini");var target=Path.Combine(dir,"target.ini");File.WriteAllText(target,"original target");File.WriteAllText(path,"original pack");if(name=="missing")File.Delete(path);if(name=="directory"){File.Delete(path);Directory.CreateDirectory(path);}if(name=="symlink"){File.Delete(path);File.CreateSymbolicLink(path,target);}Logger.Errors=0;var original=new Original(path);if(name!="cancel")original.Delete();rows.Add(new{name,stopped=original.Stopped,errors=Logger.Errors,entryExists=File.Exists(path)||Directory.Exists(path),targetBytes=File.ReadAllText(target)});}Console.Write(JsonSerializer.Serialize(rows));}finally{Directory.Delete(root,true);}}}
'''.replace('BODY',body)
with tempfile.TemporaryDirectory(prefix='deadsync-pack-delete-reference-') as tmp:
 t=Path(tmp);(t/'Oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup></Project>');(t/'Program.cs').write_text(program)
 subprocess.run([str(a.dotnet),'build',str(t/'Oracle.csproj'),'--nologo','--verbosity','quiet'],check=True)
 result=subprocess.check_output([str(a.dotnet),str(t/'bin/Debug/net10.0/Oracle.dll')],text=True);rows=json.loads(result)
a.output.parent.mkdir(parents=True,exist_ok=True);a.output.with_suffix('.expected.json').write_text(json.dumps(rows,indent=2)+'\n');a.output.with_suffix('.provenance.json').write_text(json.dumps(dict(commit=PIN,source='StepManiaEditor/EditorItgManiaPack.cs',source_sha256=hashlib.sha256(raw).hexdigest(),method_sha256=hashlib.sha256(body.encode()).hexdigest(),scope='Unchanged original Delete method; real filesystem. StopObservingFile and Logger are explicit recording adapters. No original watcher or editor-owner queue execution; dirty label uses same disk operation, no GUI simulation.',sdk=subprocess.check_output([str(a.dotnet),'--version'],text=True).strip()),indent=2)+'\n')
