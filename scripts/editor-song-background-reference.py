#!/usr/bin/env python3
"""Execute pinned original CPU Box geometry + HSV clear-color methods; no GPU."""
import argparse,hashlib,json,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--editor-root',type=Path,required=True);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
pin='48e18f71655d0f0d86144c63239331833501fda2';fpin='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
for root,commit in [(a.editor_root,pin),(a.reference_root/'Fumen',fpin)]:
 assert subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip()==commit
source=subprocess.check_output(['git','-C',str(a.editor_root),'show',f'{pin}:MonoGameExtensions/TextureUtils.cs'],text=True)
def block(signature):
 start=source.index(signature);brace=source.index('{',start);depth=1;end=brace+1
 while depth:
  if source[end]=='{':depth+=1
  elif source[end]=='}':depth-=1
  end+=1
 return source[start:end]
parts=[block('public enum TextureLayoutMode'),block('public static (float xOffset'),block('public static uint GetTextureColor(uint[] colorData)')]
adapter='''using System;using System.Text.Json;
class Texture2D{public int Width;public int Height;public Texture2D(int w,int h){Width=w;Height=h;}}
class SourceTexture {
'''+ '\n'.join(parts)+'''
}
class Program{static void Main(){var geometry=new object[3];int[][] inputs={new[]{400,200,800,600},new[]{400,200,800,410},new[]{200,400,800,600}};for(int i=0;i<inputs.Length;i++){var d=inputs[i];var r=SourceTexture.GetTextureUVs(new Texture2D(d[0],d[1]),(uint)d[2],(uint)d[3],SourceTexture.TextureLayoutMode.Box);geometry[i]=new{Input=d,Offset=new[]{r.xOffset,r.yOffset},Size=new[]{r.size.X,r.size.Y}};}uint[][] pixels={new uint[]{0x000000FF,0xFF000000},new uint[]{0x0000FF00},new uint[]{0xFFFF0000,0xFF0000FF}};var colors=new object[pixels.Length];for(int i=0;i<pixels.Length;i++){var color=SourceTexture.GetTextureColor(pixels[i]);var c=Fumen.ColorUtils.ToFloats(color);colors[i]=new{Pixels=pixels[i],Color=new[]{c.Item1,c.Item2,c.Item3,c.Item4}};}Console.Write(JsonSerializer.Serialize(new{Geometry=geometry,Colors=colors}));}}
'''
with tempfile.TemporaryDirectory(prefix='deadsync-background-oracle-') as t:
 root=Path(t);(root/'Program.cs').write_text(adapter)
 (root/'Oracle.csproj').write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{a.reference_root}/Fumen/Fumen/bin/Release/net10.0/Fumen.dll</HintPath></Reference></ItemGroup></Project>')
 subprocess.run([str(a.dotnet),'build',str(root/'Oracle.csproj'),'-c','Release','--nologo','-v','quiet'],check=True)
 dll=root/'bin/Release/net10.0/Oracle.dll';result=json.loads(subprocess.check_output([str(a.dotnet),str(dll)],text=True));assembly=hashlib.sha256(dll.read_bytes()).hexdigest()
a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(result,indent=2)+'\n')
a.output.with_suffix('.provenance.json').write_text(json.dumps({'EditorCommit':pin,'FumenCommit':fpin,'SourceSha256':hashlib.sha256(source.encode()).hexdigest(),'AdapterSha256':hashlib.sha256(adapter.encode()).hexdigest(),'AssemblySha256':assembly,'Scope':'Original GetTextureUVs and GetTextureColor(uint[]) with real Fumen color conversion. Stub Texture2D only exposes dimensions. No GPU, file decoder, viewport docking or background preference UI execution.'},indent=2)+'\n')
print('Executed 3 original Box geometry and 3 original HSV color cases; no GPU.')
