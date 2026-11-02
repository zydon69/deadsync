"""Execute full pinned RenderMusicAndTicks+helpers versus actual core RenderState.
The only IO stub is EditorSound; tick PCM/list are empty, so this does not prove ticks.
"""
from pathlib import Path
import subprocess,tempfile
from source_reference_paths import arguments
opts=arguments()
ROOT=opts.native_root.resolve()
repo=str(opts.grooveauthor_root);pin='48e18f71655d0f0d86144c63239331833501fda2'
def ga(n):return subprocess.check_output(['git','-C',repo,'show',pin+':StepManiaEditor/Sound/'+n],text=True)
s=ga('MusicManager.cs')
def method(sig):
 a=s.index(sig);b=s.index('{',a)+1;depth=1
 while depth:depth+=(s[b]=='{')-(s[b]=='}');b+=1
 return s[a:b]
methods='\n'.join(method(sig)for sig in ['public void SetMusicRate(','private unsafe bool RenderMusicAndTicks(','private (float[], int, long, long, bool) UpdateTicks(','private List<long> GetNextTickTimes(','private static int GetFirstTickInRange(','private static void CheckForStartingTick(','private static void GetTickValuesForSample(','private static void Mix('])
from xml.sax.saxutils import escape
dll=escape(str(opts.fumen_dll))
with tempfile.TemporaryDirectory(prefix='tick-original-tail-')as tmp:
 r=Path(tmp);(r/'O.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><TargetFramework>net10.0</TargetFramework><OutputType>Exe</OutputType><AllowUnsafeBlocks>true</AllowUnsafeBlocks><WarningLevel>0</WarningLevel></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>'+dll+'</HintPath></Reference></ItemGroup></Project>')
 (r/'SoundPlaybackState.cs').write_text(ga('SoundPlaybackState.cs'));(r/'TimeStretcher.cs').write_text(ga('TimeStretcher.cs'))
 cs='''using System;using System.Collections.Generic;using System.Threading;using Fumen;namespace StepManiaEditor{
class EditorSound:IDisposable{public const string NotificationSampleDataChanged="changed";float[]data;public EditorSound(float[]pcm){data=pcm;}public(int,float[])GetSampleData()=>(2,data);public bool IsLoaded()=>data!=null;public void Dispose(){}}
class O{Lock Lock=new();SoundPlaybackState MusicData,AssistTickData,BeatTickData=new(new(null));List<long>NextAssistTickStartMusicSamples=new(){0},NextBeatTickStartMusicSamples;long SampleIndex;double MusicRate;const double MinMusicRate=.25,MaxMusicRate=4;float[]TimeStretchBuffer;uint SampleRate=48000;bool UseAssistTick=true,UseBeatTick;float MainVolume=1,MusicVolume=0,AssistTickVolume=1,BeatTickVolume;
'''+methods+'''
unsafe float[] Callback(){var output=new float[4098];int channels=2;fixed(float*b=output)RenderMusicAndTicks(b,2049,ref channels);return output;}
unsafe static void Main(){foreach(var rate in new[]{1.1,1.35}){var o=new O();o.SetMusicRate(rate);var main=new float[80000];var tick=new float[60000];for(int frame=0;frame<30000;frame++)tick[frame*2]=tick[frame*2+1]=(frame+100)/65536f;o.MusicData=new(new(main));o.MusicData.StartPlaying(0);o.AssistTickData=new(new(tick));var first=o.Callback();var next=o.Callback();int last=(int)(first[^1]*65536)-100;int resumed=(int)(next[0]*65536)-100;if(last!=2048||resumed!=2048)throw new Exception($"Unexpected source witness {rate}: {last}->{resumed}");Console.WriteLine($"Original unchanged callback rate={rate}: tail finalframe={last}, nextfirst={resumed} repeatsoneframe; nativephysical1xexpects2049");}}}
}''';(r/'Program.cs').write_text(cs)
 subprocess.run([opts.dotnet,'run','--project',str(r/'O.csproj'),'--nologo'],check=True)
