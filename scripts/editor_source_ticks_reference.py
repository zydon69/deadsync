"""Execute full pinned RenderMusicAndTicks+helpers versus actual core RenderState.
Real nonempty assist/beat PCM and authored source onsets; physical tails are diagnosed separately.
"""
from pathlib import Path
import subprocess,tempfile
import argparse
args=argparse.ArgumentParser();args.add_argument('--native-root',type=Path,default=Path(__file__).resolve().parent.parent);opts=args.parse_args()
ROOT=Path('/Users/zydon/Sites/feetness/deadsync')
repo='/Users/zydon/Sites/feetness/grooveauthor';pin='48e18f71655d0f0d86144c63239331833501fda2'
def ga(n):return subprocess.check_output(['git','-C',repo,'show',pin+':StepManiaEditor/Sound/'+n],text=True)
s=ga('MusicManager.cs')
def method(sig):
 a=s.index(sig);b=s.index('{',a)+1;depth=1
 while depth:depth+=(s[b]=='{')-(s[b]=='}');b+=1
 return s[a:b]
methods='\n'.join(method(sig)for sig in ['public void SetMusicRate(','private unsafe bool RenderMusicAndTicks(','private (float[], int, long, long, bool) UpdateTicks(','private List<long> GetNextTickTimes(','private static int GetFirstTickInRange(','private static void CheckForStartingTick(','private static void GetTickValuesForSample(','private static void Mix('])
dll='/tmp/deadsync-grooveauthor-step-library/Fumen/Fumen/bin/Release/net10.0/Fumen.dll'
assist=[0,17,300,1000,2253,2254,2765,3001,5000,6000,10000,14000]
beat=[0,31,301,700,2253,2766,4000,8000,12000,16000]
render=method('private unsafe bool RenderMusicAndTicks(')
correction='''var lastScaled=musicStartSampleIndex+(long)((length-1)*MusicRate);
long? lastA=null,lastB=null;foreach(var t in nextAssistTickTimes)if(t<sampleIndexEndExclusive)lastA=t;foreach(var t in nextBeatTickTimes)if(t<sampleIndexEndExclusive)lastB=t;
if(lastA==null||lastA<=lastScaled)AssistTickData.SetSampleIndex(assistTickSampleIndex);
if(lastB==null||lastB<=lastScaled)BeatTickData.SetSampleIndex(beatTickSampleIndex);
'''
fixed_methods=methods.replace(render,render.replace('return true;',correction+'return true;'))
with tempfile.TemporaryDirectory(prefix='source-main-ticks-')as tmp:
 r=Path(tmp);lib=r/'libdeadlib_audio_core.rlib';dependency=ROOT/'target/debug/deps';rtrb=max(dependency.glob('librtrb-*.rlib'),key=lambda p:p.stat().st_mtime)
 subprocess.run(['rustc','--edition=2024','--crate-name','deadlib_audio_core','--crate-type','rlib',str(opts.native_root/'crates/deadlib-audio-core/src/lib.rs'),'--extern','rtrb='+str(rtrb),'-L','dependency='+str(dependency),'-o',str(lib)],check=True)
 (r/'O.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><TargetFramework>net10.0</TargetFramework><OutputType>Exe</OutputType><AllowUnsafeBlocks>true</AllowUnsafeBlocks><WarningLevel>0</WarningLevel></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>'+dll+'</HintPath></Reference></ItemGroup></Project>')
 (r/'SoundPlaybackState.cs').write_text(ga('SoundPlaybackState.cs'));(r/'TimeStretcher.cs').write_text(ga('TimeStretcher.cs'))
 def cls(name,body):return '''class '''+name+'''{Lock Lock=new();SoundPlaybackState MusicData,AssistTickData,BeatTickData;List<long>NextAssistTickStartMusicSamples=new(){'''+','.join(map(str,assist))+'''},NextBeatTickStartMusicSamples=new(){'''+','.join(map(str,beat))+'''};long SampleIndex;double MusicRate;const double MinMusicRate=.25,MaxMusicRate=4;float[]TimeStretchBuffer;uint SampleRate=48000;bool UseAssistTick=true,UseBeatTick=true;float MainVolume=1,MusicVolume=0,AssistTickVolume=.7f,BeatTickVolume=.75f;
'''+body+'''
static ulong Hash(float[] d){ulong h=1469598103934665603;foreach(var v in d)h=unchecked((h^(uint)BitConverter.SingleToInt32Bits(v))*1099511628211);return h;}
unsafe public static void Run(){foreach(var frames in new[]{257,2049,4097})foreach(var rate in new[]{.3,1.1,1.35})foreach(var start in new long[]{-3,0,42}){var o=new '''+name+'''();o.SetMusicRate(rate);o.SampleIndex=start;o.MusicData=new(new(new float[80000]));o.MusicData.StartPlaying(start);var a=new float[60000];var b=new float[60000];for(int i=0;i<30000;i++)for(int ch=0;ch<2;ch++){a[i*2+ch]=((i*37+ch*5)%113-56)/32768f;b[i*2+ch]=((i*7+ch*3+11)%113-56)/32768f;}o.AssistTickData=new(new(a));o.BeatTickData=new(new(b));for(int cb=0;cb<3;cb++){var output=new float[frames*2];int channels=2;fixed(float* ptr=output)o.RenderMusicAndTicks(ptr,(uint)frames,ref channels);Console.WriteLine($"{o.SampleIndex};{Hash(output)}");}}}}
'''
 master=cls('SourceMaster',methods).replace('MainVolume=1,MusicVolume=0,AssistTickVolume=.7f,BeatTickVolume=.75f','MainVolume=.61f,MusicVolume=.43f,AssistTickVolume=.71f,BeatTickVolume=.79f').replace('o.MusicData=new(new(new float[80000]));','var music=new float[80000];for(int m=0;m<music.Length;m++)music[m]=((m*19)%89-44)/32768f;o.MusicData=new(new(music));')
 premult=master.replace('SourceMaster','PremultipliedMaster').replace('MainVolume=.61f,MusicVolume=.43f,AssistTickVolume=.71f,BeatTickVolume=.79f','MainVolume=1,MusicVolume=.43f*.61f,AssistTickVolume=.71f*.61f,BeatTickVolume=.79f*.61f')
 cs='''using System;using System.Collections.Generic;using System.Threading;using Fumen;namespace StepManiaEditor{
class EditorSound:IDisposable{public const string NotificationSampleDataChanged="changed";float[]data;public EditorSound(float[]pcm){data=pcm;}public(int,float[])GetSampleData()=>(2,data);public bool IsLoaded()=>data!=null;public void Dispose(){}}
'''+cls('Original',methods)+cls('CorrectedTail',fixed_methods)+master+premult+'''class P{static void Main(string[] args){if(args.Length==0)Original.Run();else if(args[0]=="master")SourceMaster.Run();else if(args[0]=="premult")PremultipliedMaster.Run();else CorrectedTail.Run();}}}''';(r/'Program.cs').write_text(cs)
 build=subprocess.run(['/tmp/deadsync-dotnet10/dotnet','build',str(r/'O.csproj'),'--nologo','-v:q'],capture_output=True,text=True)
 if build.returncode:raise RuntimeError(build.stdout+build.stderr)
 original=subprocess.check_output(['/tmp/deadsync-dotnet10/dotnet',str(r/'bin/Debug/net10.0/O.dll')],text=True).splitlines()
 corrected=subprocess.check_output(['/tmp/deadsync-dotnet10/dotnet',str(r/'bin/Debug/net10.0/O.dll'),'corrected-tail'],text=True).splitlines()
 rs='''use deadlib_audio_core::*;use std::sync::Arc;fn main(){for frames in [257usize,2049,4097]{for rate in [0.3f64,1.1,1.35]{for start in [-3i64,0,42]{reset_music_stream_clock_state();reset_music_target_gain();activate_music_track();resume_music_track();let generation=bump_music_map_generation();let epoch=advance_music_asset_epoch();assert!(configure_editor_main(generation,epoch,start,rate));let(stream,handle)=music_transport(48000,2);let(mut sender,mut receiver)=sfx_transport(8);let controls=Arc::new(MixControls::new());controls.set_bus_gain(MixBus::new(4),0.7);controls.set_bus_gain(MixBus::new(5),0.75);let mut render=RenderState::new(handle,controls.clone(),2);render.attach_sfx_retirement(receiver.take_retirement());
'''
 for index,values in enumerate([assist,beat]):
  formula='i*37+ch*5'if index==0 else'i*7+ch*3+11'
  rs+='let data:Vec<i16>=(0..30000).flat_map(|i|(0..2).map(move|ch|((('+formula+')%113)as i16)-56)).collect();let schedule=SfxSchedule::with_source_main(vec!['+','.join(map(str,values))+'],Arc::new(SfxScheduleVoice::new()),generation,epoch).unwrap();let bus=MixBus::new('+str(index+4)+');assert!(sender.try_send(QueuedSfx{data:data.into(),bus,generation:controls.bus_generation(bus),restart_slot:Some(0),pan:SfxPan::Center,target_stream_frame:SfxOnset::Immediate,schedule:Some(schedule),voice:None}).is_ok());'
 rs+='''for callback in 0..3{let mut output=vec![0.;frames*2];render.render(OutputBufferMut::F32(&mut output),CallbackInfo{anchor_nanos:1000+callback,clock:CallbackClockSource::Instant},receiver.try_iter());let mut h=1469598103934665603u64;for v in output{h=(h^u64::from(v.to_bits())).wrapping_mul(1099511628211);}println!("{};{}",editor_main_cursor().unwrap().source_frame,h);}stop_music_track();drop(stream);}}}}'''
 (r/'native.rs').write_text(rs);subprocess.run(['rustc','--edition','2024',str(r/'native.rs'),'--extern','deadlib_audio_core='+str(lib),'-L','dependency='+str(dependency),'-o',str(r/'native')],check=True)
 actual=subprocess.check_output([str(r/'native')],text=True).splitlines()
 assert len(actual)==81
 mismatches=[(i,a,b)for i,(a,b)in enumerate(zip(actual,corrected))if a!=b]
 assert not mismatches,mismatches[:8]
 assert all(actual[i]==original[i]for i in range(0,len(actual),3)), 'fresh callback onsets must match the unmodified original'
 changed=sum(a!=b for a,b in zip(original,corrected));assert changed>0
 print(f'Original real assist+beat PCM/nonempty lists:81 actual callbacks, all fresh callbacks originalexact; all81 corrected-tail diagnostic exact; {changed} original tail-repeat differences explicit')

 master_source=subprocess.check_output(['/tmp/deadsync-dotnet10/dotnet',str(r/'bin/Debug/net10.0/O.dll'),'master'],text=True).splitlines()
 premult_source=subprocess.check_output(['/tmp/deadsync-dotnet10/dotnet',str(r/'bin/Debug/net10.0/O.dll'),'premult'],text=True).splitlines()
 assert len(master_source)==81 and len(premult_source)==81
 master_differences=sum(a!=b for a,b in zip(master_source,premult_source));assert master_differences>0
 print(f'Known separate gain-order gap witnessed in original full callback: {master_differences}/81 master-after-sum vs bus-premultiplication differences (nonunit music+assist+beat, signed cancellation)')
