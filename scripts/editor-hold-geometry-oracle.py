#!/usr/bin/env python3
"""Execute original Draw/DrawBody/DrawStart/DrawEnd geometry with a recording graphics adapter."""
import argparse, hashlib, json, subprocess, tempfile
from pathlib import Path
PIN='48e18f71655d0f0d86144c63239331833501fda2'
def extract(s, marker):
    start=s.index(marker); pos=s.index('{',start); end=pos+1; depth=1
    while depth:
        depth+=(s[end]=='{')-(s[end]=='}');end+=1
    return s[start:end]
def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--reference-root',type=Path,required=True)
    p.add_argument('--dotnet',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    a=p.parse_args();r=a.reference_root.resolve()
    path='StepManiaEditor/EditorEvents/EditorHoldNoteEvent.cs'; raw=(r/path).read_bytes()
    assert raw==subprocess.check_output(['git','-C',str(r),'show',f'{PIN}:{path}'])
    s=raw.decode('utf-8-sig')
    methods={k:extract(s,m) for k,m in {
        'draw':'public override void Draw(TextureAtlas textureAtlas',
        'body':'public void DrawBody(', 'start':'public void DrawStart(',
        'end':'public void DrawEnd(', 'reflect':'private double ReflectTopIfNeeded(',
        'flip':'private SpriteEffects ApplyFlip(', 'cap_flip':'private SpriteEffects ApplyEndCapFlip('
    }.items()}
    requests=[]
    for reverse in [False,True]:
      for centered in [False,True]:
       for start in [0,28]:
        for cutoff in [None,200.0]:
         requests.append(dict(head_top=100.0,height=-300.0 if reverse else 300.0,scale=.375,
          arrow_height=128,body_height=128,cap_height=65,start_height=start,
          cap_centered=centered,reverse=reverse,screen_height=480.0,cutoff=cutoff))
    requests += [dict(requests[0],head_top=-1e9,height=2e9,scale=1.0),dict(requests[0],height=-20.0,scale=1.0)]
    program='''using System;using System.Collections.Generic;using System.Text.Json;
record Rect(double X,double Y,double Width,double Height);
record Rectangle(int X,int Y,int Width,int Height);
record RectangleF(float X,float Y,float Width,float Height);
record Color(byte R,byte G,byte B,byte A=255);
[Flags]enum SpriteEffects{None=0,FlipVertically=1,FlipHorizontally=2}
class SpriteBatch{}
class TextureAtlas{
 public int ArrowH,BodyH,CapH,StartH;public List<object> Body=new();public object Start=null,Cap=null;public double Head;
 public (int,int) GetDimensions(string id)=>(128,id switch{"arrow"=>ArrowH,"cap-rim" or "cap-fill"=>CapH,"start-rim" or "start-fill"=>StartH,_=>BodyH});
 void Emit(string id,RectangleF d,int sy,int sh){var o=new{top=(double)d.Y,height=(double)d.Height,source_top=sy,source_height=sh};if(id=="body-fill")Body.Add(o);if(id=="start-fill")Start=o;if(id=="cap-fill")Cap=o;}
 public void Draw(string id,SpriteBatch s,RectangleF d,float rot,Color c,SpriteEffects fx){Emit(id,d,0,GetDimensions(id).Item2);}
 public void Draw(string id,SpriteBatch s,RectangleF d,float rot,float a,SpriteEffects fx){Emit(id,d,0,GetDimensions(id).Item2);}
 public void Draw(string id,SpriteBatch s,Rectangle src,RectangleF d,float rot,Color c,SpriteEffects fx){Emit(id,d,src.Y,src.Height);}
 public void Draw(string id,SpriteBatch s,Rectangle src,RectangleF d,float rot,float a,SpriteEffects fx){Emit(id,d,src.Y,src.Height);}
}
class ArrowGraphicManager {public bool Centered;public bool AreHoldCapsCentered()=>Centered;}
class Preferences {public static Preferences Instance=new();public Scroll PreferencesScroll=new();public Receptors PreferencesReceptors=new();public class Scroll{public bool Reverse;}public class Receptors{public bool AutoPlayLightHolds=true,AutoPlayHideArrows=true;}}
class EditorHoldNoteEvent{
 public static double ScreenHeight;public double X,Y,W=128,H,Scale;public bool NextDrawActive;public double NextDrawActiveYCutoffPoint;public TextureAtlas Atlas;
 float GetRenderAlpha()=>1f;bool IsFake()=>false;
 void DrawTap(TextureAtlas t,SpriteBatch s,ArrowGraphicManager g,double x,double y){t.Head=y;}
 void DrawFakeMarker(TextureAtlas t,SpriteBatch s,string id,double x,double y){}
'''+methods['draw'].replace('public override void','public void',1)+'''
 class HoldRenderState{
 readonly TextureAtlas TextureAtlas;readonly SpriteBatch SpriteBatch;readonly float Alpha;readonly double Scale;readonly bool Reverse,FlipEndCapsInReverse=true;
 public readonly string StartArrowRimTextureId="arrow",EndRimTextureId="cap-rim";
 public readonly double StartHeight;
 readonly string BodyFillTextureId="body-fill",BodyRimTextureId="body-rim",StartFillTextureId="start-fill",StartRimTextureId="start-rim",EndFillTextureId="cap-fill";
 readonly bool BodyFillMirrored=false,BodyRimMirrored=false,StartFillMirrored=false,StartRimMirrored=false;
 readonly Color BodyColor=new(255,255,255),StartColor=new(255,255,255),EndColor=new(255,255,255);
 readonly float EndFillRotation=0,EndRimRotation=0;readonly int BodyTextureWidth=128,BodyTextureHeight;
 public HoldRenderState(TextureAtlas t,SpriteBatch b,ArrowGraphicManager g,EditorHoldNoteEvent e,bool active,bool startActive,float alpha,double scale){TextureAtlas=t;SpriteBatch=b;Alpha=alpha;Scale=scale;Reverse=Preferences.Instance.PreferencesScroll.Reverse;BodyTextureHeight=t.BodyH;StartHeight=t.StartH*scale;if(t.StartH==0){StartFillTextureId=null;StartRimTextureId=null;}}
'''+''.join(methods[k] for k in ['reflect','flip','cap_flip','start','body','end'])+'''
 }
 static void Main(){var req=JsonDocument.Parse(Console.In.ReadToEnd()).RootElement;var result=new List<object>();foreach(var q in req.EnumerateArray()){
 double D(string key)=>q.GetProperty(key).GetDouble();int I(string key)=>q.GetProperty(key).GetInt32();bool B(string key)=>q.GetProperty(key).GetBoolean();
 var t=new TextureAtlas{ArrowH=I("arrow_height"),BodyH=I("body_height"),CapH=I("cap_height"),StartH=I("start_height")};
 var g=new ArrowGraphicManager{Centered=B("cap_centered")};Preferences.Instance.PreferencesScroll.Reverse=B("reverse");ScreenHeight=D("screen_height");
 var e=new EditorHoldNoteEvent{Y=D("head_top"),H=D("height"),Scale=D("scale"),Atlas=t,NextDrawActive=q.GetProperty("cutoff").ValueKind!=JsonValueKind.Null};
 if(e.NextDrawActive)e.NextDrawActiveYCutoffPoint=D("cutoff");e.Draw(t,new(),g);result.Add(new{head_top=t.Head,body=t.Body,start=t.Start,cap=t.Cap});}
 Console.Write(JsonSerializer.Serialize(result));}
}
'''
    with tempfile.TemporaryDirectory(prefix='deadsync-hold-geometry-') as d:
      d=Path(d);(d/'oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework></PropertyGroup></Project>')
      (d/'Program.cs').write_text(program)
      b=subprocess.run([str(a.dotnet),'build',str(d/'oracle.csproj'),'--nologo','-v','q'],capture_output=True,text=True)
      if b.returncode:raise SystemExit(b.stdout+b.stderr)
      out=subprocess.run([str(a.dotnet),str(d/'bin/Debug/net10.0/oracle.dll')],input=json.dumps(requests),capture_output=True,text=True,check=True)
    doc=dict(reference_commit=PIN,source_bytes_verified_against_git=True,source_path=path,source_sha256=hashlib.sha256(raw).hexdigest(),method_sha256={k:hashlib.sha256(v.encode()).hexdigest() for k,v in methods.items()},scope='Original Draw and HoldRenderState draw/reflection/flip methods. Adapted constructor supplies explicit dimensions; TextureAtlas records destination rectangles. No original texture/color/layout resolution, GPU or timing execution.',cases=[dict(input=i,expected=e) for i,e in zip(requests,json.loads(out.stdout))])
    a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(doc,indent=2)+'\n');print(f'{len(requests)} original hold geometry cases')
if __name__=='__main__':main()
