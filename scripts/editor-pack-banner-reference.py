#!/usr/bin/env python3
"""Execute pinned original pack banner scan and FileInfoNameComparer."""
import argparse,hashlib,json,subprocess,tempfile
from pathlib import Path
PIN='48e18f71655d0f0d86144c63239331833501fda2';p=argparse.ArgumentParser(description=__doc__);p.add_argument('--editor-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
def blob(path):return subprocess.check_output(['git','-C',str(a.editor_root),'show',PIN+':'+path]).decode('utf-8-sig')
def member(text,signature):
 start=text.index(signature);brace=text.index('{',start);depth=0
 for end in range(brace,len(text)):
  if text[end]=='{':depth+=1
  elif text[end]=='}':depth-=1
  if depth==0:return text[start:end+1]
scan=member(blob('StepManiaEditor/PackLoadTask.cs'),'public static (string, string) GetBannerFromDirectory(DirectoryInfo packDirectoryInfo)');compare=member(blob('StepManiaEditor/Utils.cs'),'internal sealed class FileInfoNameComparer')
program='''using System.Globalization;using System.Text.Json;
class Program{
COMPARE
SCAN
static void Main(){var rows=new List<object>();var root=Path.Combine(Path.GetTempPath(),Guid.NewGuid().ToString());Directory.CreateDirectory(root);try{var cases=new[]{new[]{"z.jpg","z.png","A.png","a.PNG"},new[]{"é.png","e.png","Z.png","a.PNG"},new[]{"\\uE000.png","\\U00010000.png"},new[]{"a.PNG","b.JPG"}};foreach(var culture in new[]{"en-US","fr-FR","sv-SE"}){CultureInfo.CurrentCulture=CultureInfo.GetCultureInfo(culture);for(int i=0;i<cases.Length;i++){var dir=Path.Combine(root,culture+i);Directory.CreateDirectory(dir);foreach(var fileName in cases[i])File.WriteAllText(Path.Combine(dir,fileName),"x");Directory.CreateDirectory(Path.Combine(dir,"0.png"));var(_,name)=GetBannerFromDirectory(new DirectoryInfo(dir));rows.Add(new{culture,index=i,files=cases[i],selected=name});}}
var links=Path.Combine(root,"links");Directory.CreateDirectory(links);var targetDir=Path.Combine(root,"targetdir");Directory.CreateDirectory(targetDir);Directory.CreateSymbolicLink(Path.Combine(links,"0.png"),targetDir);File.CreateSymbolicLink(Path.Combine(links,"1.png"),Path.Combine(root,"absent"));var(_,picked)=GetBannerFromDirectory(new DirectoryInfo(links));rows.Add(new{culture="links",index=4,files=new[]{"0.png(directory symlink)","1.png(dangling file symlink)"},selected=picked});Console.Write(JsonSerializer.Serialize(rows));}finally{Directory.Delete(root,true);}}
}'''.replace('COMPARE',compare).replace('SCAN',scan)
with tempfile.TemporaryDirectory(prefix='deadsync-pack-banner-reference-') as tmp:
 t=Path(tmp);(t/'Oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup></Project>');(t/'Program.cs').write_text(program);subprocess.run([str(a.dotnet),'build',str(t/'Oracle.csproj'),'--nologo','--verbosity','quiet'],check=True);rows=json.loads(subprocess.check_output([str(a.dotnet),str(t/'bin/Debug/net10.0/Oracle.dll')],text=True))
a.output.parent.mkdir(parents=True,exist_ok=True);a.output.with_suffix('.expected.json').write_text(json.dumps(rows,indent=2,ensure_ascii=False)+'\n');a.output.with_suffix('.provenance.json').write_text(json.dumps(dict(commit=PIN,method_sha256=hashlib.sha256(scan.encode()).hexdigest(),comparer_sha256=hashlib.sha256(compare.encode()).hexdigest(),scope='Original GetBannerFromDirectory(DirectoryInfo) and FileInfoNameComparer unchanged. Real files/directories/symlinks, en-US/fr-FR/sv-SE. No GUI/texture decoding.',sdk=subprocess.check_output([str(a.dotnet),'--version'],text=True).strip()),indent=2)+'\n')
