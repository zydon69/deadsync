"""Execute full pinned RenderMusicAndTicks+helpers versus actual core RenderState.
Real asset assist/beat PCM and nonempty source onsets, source master-after-sum.
Four gain tuples include nonunit factors, signed cancellation and zero gains.
PCM input is CONDITIONED on the existing decoder i16 boundary (assist24 drops
8 bits); no claim of raw Fumen PCM parsing parity. Resampling is float end-to-end.
The only source diagnostic changes physical tail tracking; original stays intact.
"""
from pathlib import Path
import subprocess,tempfile,wave
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
def fumen_method(path,sig):
 text=subprocess.check_output(['git','-C',str(opts.fumen_root),'show','0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89:'+path],text=True)
 a=text.index(sig);b=text.index('{',a)+1;depth=1
 while depth:depth+=(text[b]=='{')-(text[b]=='}');b+=1
 return text[a:b]
resample_text=fumen_method('Fumen/Sound/SoundManager.cs','public static float[] ResampleFloat(')
hermite_text=fumen_method('Fumen/Interpolation.cs','public static float HermiteInterpolate(')
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
 r=Path(tmp);
 for name in ['assist','beat']:
  with wave.open(str(ROOT/('assets/sounds/editor_'+name+'_tick.wav')),'rb') as wav:
   assert wav.getsampwidth()in(2,3) and wav.getnchannels()==2 and wav.getframerate()==44100
   width=wav.getsampwidth();raw=wav.readframes(wav.getnframes())
   # Current decoder's PCM24->i16 boundary is intentionally conditional here.
   quantized=b''.join((int.from_bytes(raw[i:i+width],'little',signed=True)>>(8 if width==3 else 0)).to_bytes(2,'little',signed=True)for i in range(0,len(raw),width))
   (r/(name+'.pcm')).write_bytes(quantized)
 lib=r/'libdeadlib_audio_core.rlib';dependency=ROOT/'target/debug/deps';rtrb=max(dependency.glob('librtrb-*.rlib'),key=lambda p:p.stat().st_mtime)
 subprocess.run(['rustc','--edition=2024','--crate-name','deadlib_audio_core','--crate-type','rlib',str(opts.native_root/'crates/deadlib-audio-core/src/lib.rs'),'--extern','rtrb='+str(rtrb),'-L','dependency='+str(dependency),'-o',str(lib)],check=True)
 (r/'O.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><TargetFramework>net10.0</TargetFramework><OutputType>Exe</OutputType><AllowUnsafeBlocks>true</AllowUnsafeBlocks><WarningLevel>0</WarningLevel></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>'+dll+'</HintPath></Reference></ItemGroup></Project>')
 (r/'SoundPlaybackState.cs').write_text(ga('SoundPlaybackState.cs'));(r/'TimeStretcher.cs').write_text(ga('TimeStretcher.cs'))
 def cls(name,body):return '''class '''+name+'''{Lock Lock=new();SoundPlaybackState MusicData,AssistTickData,BeatTickData;List<long>NextAssistTickStartMusicSamples=new(){'''+','.join(map(str,assist))+'''},NextBeatTickStartMusicSamples=new(){'''+','.join(map(str,beat))+'''};long SampleIndex;double MusicRate;const double MinMusicRate=.25,MaxMusicRate=4;float[]TimeStretchBuffer;uint SampleRate=48000;bool UseAssistTick=true,UseBeatTick=true;float MainVolume=.61f,MusicVolume=.43f,AssistTickVolume=.71f,BeatTickVolume=.79f;
'''+body+'''
static ulong Hash(float[] d){ulong h=1469598103934665603;foreach(var v in d)h=unchecked((h^(uint)BitConverter.SingleToInt32Bits(v))*1099511628211);return h;}
unsafe public static void Run(){foreach(var frames in new[]{257,2049,4097})foreach(var rate in new[]{.3,1.1,1.35})foreach(var start in new long[]{-3,0,42}){var o=new '''+name+'''();o.SetMusicRate(rate);o.MainVolume=Gains.Get(0);o.MusicVolume=Gains.Get(1);o.AssistTickVolume=Gains.Get(2);o.BeatTickVolume=Gains.Get(3);o.SampleIndex=start;var music=new float[80000];for(int m=0;m<music.Length;m++)music[m]=((m*19)%89-44)/32768f;o.MusicData=new(new(music));o.MusicData.StartPlaying(start);var a=Clip.Read("assist");var b=Clip.Read("beat");o.AssistTickData=new(new(a));o.BeatTickData=new(new(b));for(int cb=0;cb<3;cb++){var output=new float[frames*2];int channels=2;fixed(float* ptr=output)o.RenderMusicAndTicks(ptr,(uint)frames,ref channels);Console.WriteLine($"{o.SampleIndex};{Hash(output)}");}}}}
'''
 master=cls('SourceMaster',methods).replace('MainVolume=.61f,MusicVolume=.43f,AssistTickVolume=.71f,BeatTickVolume=.79f','MainVolume=.61f,MusicVolume=.43f,AssistTickVolume=.71f,BeatTickVolume=.79f').replace('var music=new float[80000];for(int m=0;m<music.Length;m++)music[m]=((m*19)%89-44)/32768f;o.MusicData=new(new(music));','var music=new float[80000];for(int m=0;m<music.Length;m++)music[m]=((m*19)%89-44)/32768f;o.MusicData=new(new(music));')
 premult=master.replace('SourceMaster','PremultipliedMaster').replace('MainVolume=.61f,MusicVolume=.43f,AssistTickVolume=.71f,BeatTickVolume=.79f','MainVolume=1,MusicVolume=.43f*.61f,AssistTickVolume=.71f*.61f,BeatTickVolume=.79f*.61f')
 loader='class Gains{public static int Case;static float[,] V=new float[,]{{.61f,.43f,.71f,.79f},{.37f,.83f,.29f,.53f},{.7f,0f,0f,.79f},{0f,0f,0f,0f}};public static float Get(int i)=>V[Case,i];}class Clip{public static float[] Read(string name){var bytes=System.IO.File.ReadAllBytes("'+str(r)+'/"+name+".pcm");var raw=new float[bytes.Length/2];for(int i=0;i<raw.Length;i++)raw[i]=BitConverter.ToInt16(bytes,i*2)/32768f;return Clip.ResampleFloat(raw,2,44100,48000,CancellationToken.None);}}'
 loader='class Interpolation{'+hermite_text+'}'+loader.replace('class Clip{','class Clip{'+resample_text)
 cs='''using System;using System.Collections.Generic;using System.Threading;using Fumen;namespace StepManiaEditor{
class EditorSound:IDisposable{public const string NotificationSampleDataChanged="changed";float[]data;public EditorSound(float[]pcm){data=pcm;}public(int,float[])GetSampleData()=>(2,data);public bool IsLoaded()=>data!=null;public void Dispose(){}}
'''+loader+cls('Original',methods)+cls('CorrectedTail',fixed_methods)+master+premult+'''class P{static void Main(string[] args){for(int i=0;i<4;i++){Gains.Case=i;if(args.Length==0)Original.Run();else CorrectedTail.Run();}}}}''';(r/'Program.cs').write_text(cs)
 build=subprocess.run([opts.dotnet,'build',str(r/'O.csproj'),'--nologo','-v:q'],capture_output=True,text=True)
 if build.returncode:raise RuntimeError(build.stdout+build.stderr)
 original=subprocess.check_output([opts.dotnet,str(r/'bin/Debug/net10.0/O.dll')],text=True).splitlines()
 corrected=subprocess.check_output([opts.dotnet,str(r/'bin/Debug/net10.0/O.dll'),'corrected-tail'],text=True).splitlines()
 helper=(opts.native_root/'crates/deadlib-audio-decode/src/editor_ticks.rs').read_text();helper='use std::sync::atomic::{AtomicBool,Ordering};use crate::hermite::interpolate;\n'+helper[helper.index('pub fn resample_float('):];(r/'float_resample.rs').write_text(helper)
 rs='''use deadlib_audio_core::*;use std::sync::Arc;#[path="'''+str(opts.native_root/'crates/deadlib-audio-core/src/hermite.rs')+'''"]mod hermite;#[path="'''+str(r/'float_resample.rs')+'''"]mod float_resample;fn clip(path:&str)->Arc<[f32]>{let b=std::fs::read(path).unwrap();let raw:Vec<f32>=b.chunks_exact(2).map(|c|f32::from(i16::from_le_bytes([c[0],c[1]]))*(1.0f32/32768.)).collect();float_resample::resample_float(&raw,2,44100,48000,&std::sync::atomic::AtomicBool::new(false)).unwrap().into()}fn main(){for gains in [[0.61f32,0.43,0.71,0.79],[0.37,0.83,0.29,0.53],[0.7,0.,0.,0.79],[0.,0.,0.,0.]]{for frames in [257usize,2049,4097]{for rate in [0.3f64,1.1,1.35]{for start in [-3i64,0,42]{reset_music_stream_clock_state();reset_music_target_gain();activate_music_track();resume_music_track();let generation=bump_music_map_generation();let epoch=advance_music_asset_epoch();assert!(configure_editor_main(generation,epoch,start,rate));let(mut stream,handle)=music_transport(48000,2);let(mut sender,mut receiver)=sfx_transport(8);let controls=Arc::new(MixControls::new());assert!(controls.begin_editor_gain_override(17,EditorMixGains{main:gains[0],music:gains[1],assist:gains[2],beat:gains[3]}));let music:Vec<f32>=(0..80000i32).map(|m|((m*19)%89-44)as f32/32768.).collect();stream.writer.try_publish_preview_source(PreviewSource{editor_main:true,samples:music.into(),channels:2,sample_rate_hz:48000,generation,asset_epoch:epoch,initial_frame:start,looping:false}).unwrap();stream.writer.publish_preview_generation_ready(generation,epoch);let mut render=RenderState::new(handle,controls.clone(),2);render.attach_sfx_retirement(receiver.take_retirement());
'''
 for index,values in enumerate([assist,beat]):
  name='assist'if index==0 else'beat'
  rs+='let pcm=clip("'+str(r/(name+'.pcm'))+'");let schedule=SfxSchedule::with_source_main_float(vec!['+','.join(map(str,values))+'],Arc::new(SfxScheduleVoice::new()),generation,epoch,pcm,2).unwrap();let bus=MixBus::new('+str(index+4)+');assert!(sender.try_send(QueuedSfx{data:Arc::from([]),bus,generation:controls.bus_generation(bus),restart_slot:Some(0),pan:SfxPan::Center,target_stream_frame:SfxOnset::Immediate,schedule:Some(schedule),voice:None}).is_ok());'
 rs+='''for callback in 0..3{let mut output=vec![0.;frames*2];render.render(OutputBufferMut::F32(&mut output),CallbackInfo{anchor_nanos:1000+callback,clock:CallbackClockSource::Instant},receiver.try_iter());let mut h=1469598103934665603u64;for v in output{h=(h^u64::from(v.to_bits())).wrapping_mul(1099511628211);}println!("{};{}",editor_main_cursor().unwrap().source_frame,h);}stop_music_track();drop(stream);}}}}}'''
 (r/'native.rs').write_text(rs);subprocess.run(['rustc','--edition','2024',str(r/'native.rs'),'--extern','deadlib_audio_core='+str(lib),'-L','dependency='+str(dependency),'-o',str(r/'native')],check=True)
 actual=subprocess.check_output([str(r/'native')],text=True).splitlines()
 assert len(actual)==324
 mismatches=[(i,a,b)for i,(a,b)in enumerate(zip(actual,corrected))if a!=b]
 assert not mismatches,mismatches[:8]
 assert all(actual[i]==original[i]for i in range(0,len(actual),3)), 'fresh callback onsets must match the unmodified original'
 changed=sum(a!=b for a,b in zip(original,corrected));assert changed>0
 print(f'Float editor-only, actual WAV assist+beat / Fumen resample / four gain tuples incl zero, signed cancellation:324 actual callbacks, all fresh callbacks originalexact; all324 corrected-tail diagnostic exact; {changed} original tail-repeat differences explicit')
