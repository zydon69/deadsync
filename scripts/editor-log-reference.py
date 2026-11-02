#!/usr/bin/env python3
"""Execute unchanged original UILog filtering/timestamp/copy loop."""
import argparse,hashlib,json,subprocess,tempfile
from pathlib import Path
PIN='48e18f71655d0f0d86144c63239331833501fda2';p=argparse.ArgumentParser(description=__doc__);p.add_argument('--editor-root',type=Path,required=True);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();raw=subprocess.check_output(['git','-C',str(a.editor_root),'show',PIN+':StepManiaEditor/UI/UILog.cs']);source=raw.decode('utf-8-sig')
start=source.index('var node = logBuffer.First;');brace=source.index('{',source.index('while (node != null)',start));depth=0
for end in range(brace,len(source)):
 if source[end]=='{':depth+=1
 elif source[end]=='}':depth-=1
 if depth==0:break
loop=source[start:end+1]
fumen=a.reference_root/'Fumen'
fumen_pin='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
assert subprocess.check_output(['git','-C',str(fumen),'rev-parse','HEAD'],text=True).strip()==fumen_pin
subprocess.run(['git','-C',str(fumen),'diff','--exit-code',fumen_pin,'--'],check=True,capture_output=True)
subprocess.run([str(a.dotnet),'build',str(fumen/'Fumen/Fumen.csproj'),'--configuration','Release','--nologo','--verbosity','quiet'],check=True)
def field(name):
 start=source.index(name);end=source.index('];',start)+2;return source[start:end]
dates=field('private static readonly string[] LogWindowDateStrings')
colors=field('private static readonly Vector4[] LogWindowLevelColors')
program='''using System.Globalization;using System.Numerics;using System.Text;using System.Text.Json;using Fumen;
enum ImGuiCol{Text}static class ImGui{public static void TextUnformatted(string t){}public static void SameLine(){}public static void PushTextWrapPos(){}public static void PushStyleColor(ImGuiCol c,Vector4 v){}public static void PopStyleColor(){}public static void PopTextWrapPos(){}}
sealed class Preferences{public static readonly Preferences Instance=new();public LogLevel LogWindowLevel;public bool LogWindowLineWrap;}
class Program{DATES\nCOLORS
static string Copy(LinkedList<Logger.LogMessage> logBuffer,int date,int level,bool wrap){Preferences.Instance.LogWindowLevel=(LogLevel)level;Preferences.Instance.LogWindowLineWrap=wrap;var format=LogWindowDateStrings[date];var logDate=date!=0;var first=true;var copyStringBuilder=new StringBuilder();
LOOP
return copyStringBuilder.ToString();}
static void Main(){var rows=new List<object>();var buffer=new LinkedList<Logger.LogMessage>();var time=new DateTime(2026,10,29,12,34,56,123);buffer.AddLast(new Logger.LogMessage("Info français\\nsecond line",LogLevel.Info,time));buffer.AddLast(new Logger.LogMessage("Attention é",LogLevel.Warn,time.AddMilliseconds(1)));buffer.AddLast(new Logger.LogMessage("Erreur 😀",LogLevel.Error,time.AddMilliseconds(2)));foreach(var culture in new[]{"en-US","fr-FR","sv-SE","fi-FI","ar-SA","da-DK"}){CultureInfo.CurrentCulture=CultureInfo.GetCultureInfo(culture);for(int date=0;date<4;date++)foreach(var level in new[]{-1,0,1,2,99})foreach(var wrap in new[]{false,true})rows.Add(new{culture,date,level,wrap,text=Copy(buffer,date,level,wrap),separator=CultureInfo.CurrentCulture.DateTimeFormat.TimeSeparator});}var sync=new LinkedList<Logger.LogMessage>();var gate=new object();Logger.StartUp(new Logger.Config{QueueCapacity=0,WriteToBuffer=true,Buffer=sync,BufferLock=gate,BufferSize=3});Logger.Info("a");Logger.Warn("b");Logger.Error("c");Logger.Info("d");var syncNames=sync.Select(m=>m.Message).ToArray();Logger.ClearBuffer();var cleared=sync.Count;Logger.Shutdown();
var queued=new LinkedList<Logger.LogMessage>();Logger.StartUp(new Logger.Config{WriteToBuffer=true,Buffer=queued,BufferLock=gate,BufferSize=1024});for(int i=0;i<1026;i++)Logger.Info($"i{i:0000}");Logger.Warn("warn");Logger.Error("error");Logger.Shutdown();var queuedNames=queued.Select(m=>m.Message).ToArray();
var filtered=new LinkedList<Logger.LogMessage>();Logger.StartUp(new Logger.Config{QueueCapacity=0,Level=LogLevel.Warn,WriteToBuffer=true,Buffer=filtered,BufferLock=gate,BufferSize=3});Logger.Info("suppressed");Logger.Warn("warning");Logger.Error("failure");var filteredNames=filtered.Select(m=>m.Message).ToArray();Logger.Shutdown();
Console.Write(JsonSerializer.Serialize(new{views=rows,pipeline=new{sync=syncNames,cleared,queued=queuedNames,filtered=filteredNames}}));}}
'''.replace('LOOP',loop).replace('DATES',dates).replace('COLORS',colors)
with tempfile.TemporaryDirectory(prefix='deadsync-log-reference-') as tmp:
 t=Path(tmp);dll=a.reference_root/'Fumen/Fumen/bin/Release/net10.0/Fumen.dll';(t/'Oracle.csproj').write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{dll}</HintPath></Reference></ItemGroup></Project>');(t/'Program.cs').write_text(program);subprocess.run([str(a.dotnet),'build',str(t/'Oracle.csproj'),'--nologo','--verbosity','quiet'],check=True);result=json.loads(subprocess.check_output([str(a.dotnet),str(t/'bin/Debug/net10.0/Oracle.dll')],text=True));rows=result["views"];pipeline=result["pipeline"]
a.output.parent.mkdir(parents=True,exist_ok=True);a.output.with_suffix(".pipeline.expected.json").write_text(json.dumps(pipeline,indent=2)+"\n");a.output.with_suffix(".pipeline.expected.txt").write_text("\n".join(pipeline["queued"])+"\n");a.output.with_suffix('.expected.json').write_text(json.dumps(rows,indent=2,ensure_ascii=False)+'\n');a.output.with_suffix('.provenance.json').write_text(json.dumps(dict(commit=PIN,fumen_commit=fumen_pin,fumen_logger_sha256=hashlib.sha256(subprocess.check_output(['git','-C',str(fumen),'show',fumen_pin+':Fumen/Logger.cs'])).hexdigest(),fumen_assembly_sha256=hashlib.sha256((fumen/'Fumen/bin/Release/net10.0/Fumen.dll').read_bytes()).hexdigest(),source='StepManiaEditor/UI/UILog.cs',source_sha256=hashlib.sha256(raw).hexdigest(),loop_sha256=hashlib.sha256(loop.encode()).hexdigest(),dates_sha256=hashlib.sha256(dates.encode()).hexdigest(),colors_sha256=hashlib.sha256(colors.encode()).hexdigest(),scope='Original filtering/timestamp/copy while loop unchanged with original Fumen.LogMessage/LogLevel and explicit no-op ImGui rendering adapters. Fixed local DateTime inputs; no clipboard or OS launch. 6 cultures ×4 date modes×5 thresholds×2 wrap modes. Actual Fumen.Logger synchronous/queued Info/Warn/Error, eviction1024, ClearBuffer and global Warn emission threshold also executed; no console/file sinks.',sdk=subprocess.check_output([str(a.dotnet),'--version'],text=True).strip()),indent=2)+'\n')
