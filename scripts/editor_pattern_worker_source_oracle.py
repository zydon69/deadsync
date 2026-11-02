#!/usr/bin/env python3
"""Run the complete pinned original asynchronous pattern action and real engine.
Only editor containers, source config accessors and synchronous dispatcher are adapted.
"""
import argparse, hashlib, importlib.util, json, shutil, subprocess, tempfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--repo-root',type=Path,default=Path(__file__).resolve().parents[1]);p.add_argument('--editor-root',type=Path,required=True);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--diagnose-single-enumeration',action='store_true');a=p.parse_args()
GA='48e18f71655d0f0d86144c63239331833501fda2';LIB='bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8';FUMEN='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
for root,pin in [(a.editor_root,GA),(a.reference_root,LIB),(a.reference_root/'Fumen',FUMEN)]:assert subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip()==pin
source=subprocess.check_output(['git','-C',str(a.editor_root),'show',f'{GA}:StepManiaEditor/EditorActions/ActionAutoGeneratePatterns.cs'])
worker=source.decode('utf-8-sig');assert worker.count('namespace StepManiaEditor;')==1
script_dir=Path(__file__).resolve().parent
branch=(script_dir/'editor_pattern_worker_branch.cs').read_text();plumbing=(script_dir/'editor_pattern_worker_plumbing.cs').read_text()
spec=importlib.util.spec_from_file_location('generator',a.repo_root/'scripts/editor-generate-fixtures.py');generator=importlib.util.module_from_spec(spec);spec.loader.exec_module(generator)
def replace(s,old,new):
    assert s.count(old)==1,old[:100]
    return s.replace(old,new)
a.output.mkdir(parents=True,exist_ok=True);results=[]
with tempfile.TemporaryDirectory(prefix='deadsync-original-pattern-worker-') as folder:
    root=Path(folder);project=root/'scripts/editor-oracle';project.mkdir(parents=True);(project/'obj').mkdir()
    for path in (a.repo_root/'scripts/editor-oracle').iterdir():
        if path.is_file():shutil.copy2(path,project/path.name)
    generator.ROOT=root
    for name in dir(generator):
        if name.startswith('prepare_'):getattr(generator,name)(a.editor_root)
    adapter=project/'obj/EditorTimingReference.cs';s=adapter.read_text()
    s=replace(s,'protected abstract void UndoImplementation();','protected abstract void UndoImplementation(); public TaskCompletionSource Completion=new();protected void OnDone()=>Completion.TrySetResult();')
    s=replace(s,'public static EditorEvent CreateEvent(EventConfig config)=>new EditorAttackEvent(config.Chart,config.Attack,[]);','public static EditorEvent CreateEvent(EventConfig config){if(config.Generated==null)return new EditorAttackEvent(config.Chart,config.Attack,[]);var note=new EditorNoteEvent(config.Chart,(LaneNote)config.Generated);note.SetRowDependencies(config.Chart.RateAlteringEvents.ForEvent(config.Generated).GetChartTimeFromPosition(note.GetRow()),0,4,false);note.SetIsFakeDueToRow(config.Chart.IsEventInFake(note));return note;}')
    s=replace(s,'public Attack Attack=attack;','public Attack Attack=attack;public Event? Generated;public static EventConfig CreateConfig(EditorChart chart,Event data,bool unused)=>new(chart,new Attack()){Generated=data};')
    s=replace(s,'IEquatable<EditorPatternEvent> {','IEquatable<EditorPatternEvent>,IComparable<EditorPatternEvent> {public int CompareTo(EditorPatternEvent? other)=>GetRow().CompareTo(other!.GetRow());public int RandomSeed;public bool IgnorePrecedingDistribution;public double ChartRow=>GetRow();public int GetRowDuration()=>Length;public string GetMiscEventText()=>"Pattern";public PatternHolder GetPatternConfig()=>new(PatternConfig);public StepManiaLibrary.PerformedChart.PatternConfig PatternConfig=new();public record PatternHolder(StepManiaLibrary.PerformedChart.PatternConfig Config);')
    s=replace(s,'public Totals GetStepTotals()=>new(this);','public string ChartType="dance-single";public double Rating=9;public Guid ExpressedChartConfig;public List<Event> GetSmTimingEvents()=>RateAlteringEvents.Select(e=>e.Data).Where(DoesEventAffectTiming).ToList();public Totals GetStepTotals()=>new(this);')
    s=replace(s,'PatternDeletionReference.DeleteEventsOverlappingPatterns(chart,markers);\n            var engine','if(!request.TryGetProperty("ActualWorker",out _))PatternDeletionReference.DeleteEventsOverlappingPatterns(chart,markers);\n            var engine')
    s=replace(s,'            schedule=request.TryGetProperty("BatchPatterns",out _)?engine.Generate(request,referenceRoot):engine.Run();',branch+'else schedule=request.TryGetProperty("BatchPatterns",out _)?engine.Generate(request,referenceRoot):engine.Run();')
    adapter.write_text(s);(project/'WorkerPlumbing.cs').write_text(plumbing)
    original='using ActionDeletePatternNotes=EditorTimingOracle.PatternDeletionReference;\n'+worker.replace('namespace StepManiaEditor;','namespace EditorTimingOracle;')
    (project/'OriginalWorker.cs').write_text(original)
    subprocess.run([str(a.dotnet),'build',str(project/'EditorOracle.csproj'),'-c','Release',f'-p:ReferenceRoot={a.reference_root}','--nologo','-v','quiet'],check=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
    requests=list(sorted((a.repo_root/'crates/deadsync-editor/tests/fixtures/grooveauthor').glob('pattern-batch-*.request.json')))
    timing_requests=[]
    for candidate in sorted((a.repo_root/'crates/deadsync-editor/tests/fixtures/grooveauthor').glob('editor-timing-*.request.json')):
        data=json.loads(candidate.read_text())
        if isinstance(data,dict) and 'PatternConfig' in data and 'Region' not in data and any(event[1]=='tempo' for event in data['Timing']):timing_requests.append(candidate)
    for request, single in [(r,False) for r in requests]+[(next(r for r in requests if r.name=='pattern-batch-adjacent.request.json'),True)]+[(r,False) for r in timing_requests]:
        data=json.loads(request.read_text())
        name='pattern-single-adjacent.request.json' if single else request.name
        if single:data['Schedule']=data['Schedule'][:1]
        timing=request in timing_requests
        if timing:
            projection=data.pop('Projection');region=projection['Region'];inclusive=data.get('Inclusive',[True,True])
            data['Schedule']=[[region[0],region[1]-region[0],projection['Spacing'],inclusive[0],inclusive[1]]]
            data['PerformedConfig']=data.pop('Conversion')['PerformedConfig'];data['WorkerSeed']=42;data['WorkerExpressionBalanced']=True
            data['BatchPatterns']=True
        data['ActualWorker']=True;temp=root/name;temp.write_text(json.dumps(data))
        actual=json.loads(subprocess.check_output([str(a.dotnet),str(project/'bin/Release/net10.0/EditorOracle.dll'),str(a.reference_root),str(temp)]))
        assert actual['NoteData']==actual['Redo'],request.name
        assert actual['Before']==actual['Undo'],request.name
        target=a.output/name.replace('.request.json','.expected.json');target.write_text(json.dumps(actual,indent=2)+'\n')
        old=json.loads(request.with_name(request.name.replace('request','expected')).read_text())
        results.append({'Request':name,'DerivedFirstMarkerOnly':single,'DerivedProjectionToSingleMarker':timing,'RequestSha256':hashlib.sha256(request.read_bytes()).hexdigest(),'ResultSha256':hashlib.sha256(target.read_bytes()).hexdigest(),'MatchesReconstructedWorker':None if single or timing else actual['NoteData']==old['NoteData']})
    diagnostic=[]
    if a.diagnose_single_enumeration:
        assert original.count('Patterns.AddRange(allPatterns);')==1
        variant=original.replace('Patterns.AddRange(allPatterns);','// diagnostic: second enumeration omitted')
        (project/'OriginalWorker.cs').write_text(variant)
        subprocess.run([str(a.dotnet),'build',str(project/'EditorOracle.csproj'),'-c','Release',f'-p:ReferenceRoot={a.reference_root}','--nologo','-v','quiet'],check=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
        for request in requests:
            actual=json.loads(subprocess.check_output([str(a.dotnet),str(project/'bin/Release/net10.0/EditorOracle.dll'),str(a.reference_root),str(root/request.name)]))
            old=json.loads(request.with_name(request.name.replace('request','expected')).read_text())
            assert actual['NoteData']==old['NoteData'],request.name
            diagnostic.append({'Request':request.name,'SingleEnumerationMatchesReconstructedWorker':True})
        (a.output/'single-enumeration-diagnostic.json').write_text(json.dumps({'VariantCompiledWorkerSha256':hashlib.sha256(variant.encode()).hexdigest(),'Change':'Only Patterns.AddRange(allPatterns) omitted in diagnostic copy, never in original oracle or native correction.','Cases':diagnostic},indent=2)+'\n')
    provenance={'EditorCommit' :GA,'LibraryCommit':LIB,'FumenCommit':FUMEN,'OriginalWorkerSha256':hashlib.sha256(source).hexdigest(),'CompiledWorkerSha256':hashlib.sha256(original.encode()).hexdigest(),'ContainerAdapterSha256':hashlib.sha256(s.encode()).hexdigest(),'BranchSha256':hashlib.sha256(branch.encode()).hexdigest(),'PlumbingSha256':hashlib.sha256(plumbing.encode()).hexdigest(),'Cases':results,'LibraryAssemblySha256':hashlib.sha256((a.reference_root/'StepManiaLibrary/bin/Release/net10.0/StepManiaLibrary.dll').read_bytes()).hexdigest(),'FumenAssemblySha256':hashlib.sha256((a.reference_root/'Fumen/Fumen/bin/Release/net10.0/Fumen.dll').read_bytes()).hexdigest(),'DotnetSdk':subprocess.check_output([str(a.dotnet),'--version'],text=True).strip(),'Scope':'Full original class unchanged except namespace and deletion type alias. Real original expression/performed search, scheduling, constructor duplication, helpers, Do/Undo/Redo. Existing editor containers and source-extracted clocks/deletion helpers; config accessors, note factory row-dependency adapter, graph-loader host, completion observation, synchronous dispatcher. Not full editor threads/UI, metadata writer, original event factory, actual Fumen ActionQueue or physical thread interleaving.'}
    (a.output/'provenance.json').write_text(json.dumps(provenance,indent=2)+'\n')
print(f'Executed {len(results)} original worker cases; {sum(r["MatchesReconstructedWorker"] is False for r in results)} outputs differ from reconstructed worker; Undo/Redo exact.')
