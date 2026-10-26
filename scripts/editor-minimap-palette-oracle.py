#!/usr/bin/env python3
"""Original cached UI color constructors with no-op XNA drawing-color adapter.
Palette cache, HSV and byte helpers are unchanged; no native preference UI claim.
"""
import argparse,hashlib,json,subprocess,tempfile
from pathlib import Path
PIN='48e18f71655d0f0d86144c63239331833501fda2'
FUMEN='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
def block(source,marker):
 start=source.index(marker);brace=source.index('{',start);depth=1;end=brace+1
 while depth:depth+=(source[end]=='{')-(source[end]=='}');end+=1
 return source[start:end]
def method(source,name):
 line=next(line for line in source.splitlines() if line.startswith('\tpublic static ') and f' {name}(' in line)
 return block(source,line)
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--fumen-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
 sources={}
 def read(root,pin,path):
  raw=(root/path).read_bytes()
  if raw!=subprocess.check_output(['git','-C',str(root),'show',f'{pin}:{path}']):raise SystemExit('Working source differs from pin: '+path)
  sources[path]=hashlib.sha256(raw).hexdigest();return raw.decode('utf-8-sig')
 prefs=read(a.reference_root,PIN,'StepManiaEditor/Preferences/PreferencesNoteColor.cs');utils=read(a.reference_root,PIN,'StepManiaEditor/Utils.cs');hsv=read(a.fumen_root,FUMEN,'Fumen/ColorUtils.cs')
 equality=read(a.fumen_root,FUMEN,'Fumen/Utils.cs');common=read(a.fumen_root,FUMEN,'Fumen/Converters/SMCommon.cs')
 if 'private const float MinFloatDelta = 0.0000001f;' not in equality or 'return f - MinFloatDelta <= other && f + MinFloatDelta >= other;' not in equality:raise SystemExit('FloatEquals adapter differs from source')
 if 'public const int MaxValidDenominator = 48;' not in common:raise SystemExit('Source denominator changed')
 selected={name:method(utils,name) for name in ['ToRGBA','ColorRGBAMultiply']};selected.update({name:method(hsv,name) for name in ['RgbToHsv','HsvToRgb']})
 nested=block(prefs,'\tprivate class PerPlayerNoteColors');color_set=block(prefs,'\tpublic class NoteColorSet')
 start=prefs.index('\tpublic static readonly NoteColorSet[] DefaultNoteColors =');end=prefs.index('\n\t];',start)+len('\n\t];');defaults=prefs[start:end]
 constants='\n'.join(line for line in prefs.splitlines() if line.startswith('\tpublic static readonly Vector3 DefaultPlayer') or line.startswith('\tpublic static readonly Vector3 DefaultPiu'))
 program='''using System;using System.Collections.Generic;using System.Text.Json;using Vector3=System.Numerics.Vector3;
struct Color {public Color(float r,float g,float b){}}
struct Fraction {public int Numerator,Denominator;public Fraction(int n,int d){Numerator=n;Denominator=d;} public Fraction Reduce(){int a=Numerator,b=Denominator;while(b!=0){int t=a%b;a=b;b=t;}return new Fraction(Numerator/a,Denominator/a);}}
static class SMCommon {public const int MaxValidDenominator=48;}
static class Extensions { public static bool FloatEquals(this float f,float other)=>f-0.0000001f<=other&&f+0.0000001f>=other;}
static class Utils {'''+selected['ToRGBA']+selected['ColorRGBAMultiply']+'''}
static class ColorUtils {'''+selected['RgbToHsv']+selected['HsvToRgb']+'''}
class PreferencesNoteColor {
'''+color_set+defaults+constants+nested+'''
 public Vector3 QuarterColor,EighthColor,TwelfthColor,SixteenthColor,TwentyForthColor,ThirtySecondColor,FortyEighthColor,SixtyForthColor,OneHundredNinetySecondColor,HoldColor,RollColor,MineColor;
 public Vector3 PiuTopColor=DefaultPiuTopColor,PiuMiddleColor=DefaultPiuMiddleColor,PiuBottomColor=DefaultPiuBottomColor;
 public float SelectionColorMultiplier=1.7f,HeldColorMultiplier=1.5f,ArrowUIColorMultiplier=1.5f,ArrowUISelectedColorMultiplier=8.0f,RoutineNoteColorAlpha=0.9f,PiuHoldSaturationMultiplier=0.8f;
 static float[] Rgb(Vector3 c)=>new[]{c.X,c.Y,c.Z};
 void Apply(int preset) {var c=DefaultNoteColors[preset];QuarterColor=c.Quarter;EighthColor=c.Eighth;TwelfthColor=c.Twelfth;SixteenthColor=c.Sixteenth;TwentyForthColor=c.TwentyForth;ThirtySecondColor=c.ThirtySecond;FortyEighthColor=c.FortyEighth;SixtyForthColor=c.SixtyForth;OneHundredNinetySecondColor=c.OneHundredNinetySecond;HoldColor=c.Hold;RollColor=c.Roll;MineColor=c.Mine;}
 static uint U(PerPlayerNoteColors.ArrowColor c,bool selected)=>selected?c.UISelectedColor:c.UIColor;
 static void Main(){var output=new List<object>();var playerColors=new[]{DefaultPlayer0Color,DefaultPlayer1Color,DefaultPlayer2Color,DefaultPlayer3Color,DefaultPlayer4Color,DefaultPlayer5Color,DefaultPlayer6Color,DefaultPlayer7Color,DefaultPlayer8Color,DefaultPlayer9Color};
 for(int config=0;config<4;config++) {
  var p=new PreferencesNoteColor();p.Apply(config%3);if(config==3){p.ArrowUIColorMultiplier=0.8f;p.ArrowUISelectedColorMultiplier=2.25f;p.RoutineNoteColorAlpha=0.25f;p.PiuHoldSaturationMultiplier=0.5f;}
  var ordinary=new PerPlayerNoteColors(p);var multi=Array.ConvertAll(playerColors,c=>new PerPlayerNoteColors(p,c));var cases=new List<object>();
  foreach(var family in new[]{"row","routine","pump","pumproutine"}) foreach(int player in new[]{0,1,11}) foreach(bool selected in new[]{false,true}) foreach(int row in new[]{0,1,2,3,4,6,8,12,16,24,47}) foreach(int lane in new[]{0,1,2,4}) {
   var palette=family=="routine"||family=="pumproutine"?multi[player%10]:ordinary;bool pump=family=="pump"||family=="pumproutine";
   var head=pump?palette.PiuColorsByLane[lane]:palette.ColorsByRow[row];var hold=pump?palette.PiuHoldColorsByLane[lane]:palette.HoldBodyColor;var roll=pump?palette.PiuHoldColorsByLane[lane]:palette.RollBodyColor;
   cases.Add(new{family,player,selected,row,lane,expected=new{head=U(head,selected),hold=U(hold,selected),roll=U(roll,selected),mine=U(palette.MineColor,selected)}});
  }
  output.Add(new{input=new{subdivisions=Array.ConvertAll(new[]{p.QuarterColor,p.EighthColor,p.TwelfthColor,p.SixteenthColor,p.TwentyForthColor,p.ThirtySecondColor,p.FortyEighthColor,p.SixtyForthColor,p.OneHundredNinetySecondColor},Rgb),hold=Rgb(p.HoldColor),roll=Rgb(p.RollColor),mine=Rgb(p.MineColor),piuTop=Rgb(p.PiuTopColor),piuMiddle=Rgb(p.PiuMiddleColor),piuBottom=Rgb(p.PiuBottomColor),players=Array.ConvertAll(playerColors,Rgb),normalMultiplier=p.ArrowUIColorMultiplier,selectedMultiplier=p.ArrowUISelectedColorMultiplier,routineAlpha=p.RoutineNoteColorAlpha,piuSaturation=p.PiuHoldSaturationMultiplier},cases});
 }Console.Write(JsonSerializer.Serialize(output));}
}'''
 with tempfile.TemporaryDirectory(prefix='deadsync-minimap-palette-') as folder:
  root=Path(folder);(root/'oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework></PropertyGroup></Project>');(root/'Program.cs').write_text(program)
  result=subprocess.run([str(a.dotnet),'build',str(root/'oracle.csproj'),'--nologo','-v','q'],capture_output=True,text=True)
  if result.returncode:raise SystemExit(result.stdout+result.stderr)
  run=subprocess.run([str(a.dotnet),str(root/'bin/Debug/net10.0/oracle.dll')],capture_output=True,text=True,check=True)
 configs=json.loads(run.stdout);a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(dict(reference_commit=PIN,fumen_commit=FUMEN,source_sha256=sources,method_sha256={name:hashlib.sha256(body.encode()).hexdigest() for name,body in selected.items()},constructors_sha256=hashlib.sha256(nested.encode()).hexdigest(),scope='Unchanged cached palette constructors and numeric helpers; XNA Color no-op, exact integer fraction adapter, copied FloatEquals expression; no preference loader or texture rendering.',configurations=configs),indent=2)+'\n');print(f'Generated {sum(len(c["cases"]) for c in configs)} color lookups from4 source configurations')
if __name__=='__main__':main()
