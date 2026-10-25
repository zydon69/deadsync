// Development-only oracle using pinned libraries and unchanged editor helper
// bodies. Explicit adapters replace UI/container plumbing; this does not run
// the full editor. It is not linked into DeadSync or shipped with the game.
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Reflection;
using Fumen.ChartDefinition;
using StepManiaLibrary;
using StepManiaLibrary.PerformedChart;
using PerformedConfig = StepManiaLibrary.PerformedChart.Config;
using ExpressedNode = StepManiaLibrary.ExpressedChart.ExpressedChart.ChartSearchNode;

if (args.Length != 2)
    throw new ArgumentException("Usage: EditorOracle <StepManiaLibrary checkout> <request.json>");

var options = new JsonSerializerOptions
{
    IncludeFields = true,
    Converters = { new JsonStringEnumConverter(JsonNamingPolicy.CamelCase) },
    WriteIndented = true,
};
var request = JsonDocument.Parse(File.ReadAllBytes(args[1])).RootElement;
var operation = request.GetProperty("Operation").GetString();
if (operation == "NormalizedFile") { Console.WriteLine(JsonSerializer.Serialize(await SourceTimingOracle.Reference.Run(request,args[0]), options)); return; }
if (operation == "Region") { Console.WriteLine(JsonSerializer.Serialize(RegionOracle.Reference.Run(request), options)); return; }
if (operation == "Pause") { Console.WriteLine(JsonSerializer.Serialize(PauseOracle.Reference.Run(request), options)); return; }
if (operation == "Tempo") { Console.WriteLine(JsonSerializer.Serialize(TempoOracle.Reference.Run(request), options)); return; }
if (operation == "Profile") { Console.WriteLine(JsonSerializer.Serialize(ProfilesOracle.Reference.Run(request), options)); return; }
if (operation == "SongSync") { Console.WriteLine(JsonSerializer.Serialize(MetadataOracle.EditorSong.Run(request), options)); return; }
if (operation == "Metadata") { Console.WriteLine(JsonSerializer.Serialize(MetadataOracle.EditorChart.Run(request), options)); return; }
if (operation == "EditorTiming") { Console.WriteLine(JsonSerializer.Serialize(EditorTimingOracle.Reference.Run(request,args[0]), options)); return; }
if (operation == "RegionFile") {
    object EditorState(Chart chart) {
        var timing=new List<object[]>();
        foreach(var e in chart.Layers[0].Events) {
            object[]? entry=e switch {
                Tempo t=>[e.IntegerPosition,"tempo",t.TempoBPM],
                TimeSignature t=>[e.IntegerPosition,"signature",t.Signature.Numerator,t.Signature.Denominator],
                Stop t=>[e.IntegerPosition,t.IsDelay?"delay":"stop",t.LengthSeconds],
                Warp t=>[e.IntegerPosition,"warp",t.LengthIntegerPosition],
                FakeSegment t=>[e.IntegerPosition,"fake",t.LengthIntegerPosition],
                ScrollRate t=>[e.IntegerPosition,"scroll",t.Rate],
                ScrollRateInterpolation _=>[e.IntegerPosition,"speed"],
                Label _=>[e.IntegerPosition,"label"],TickCount _=>[e.IntegerPosition,"tick"],Multipliers _=>[e.IntegerPosition,"combo"],
                _=>null
            };
            if(entry!=null)timing.Add(entry);
        }
        var notes=chart.Layers[0].Events.OfType<LaneNote>().Select(note=>new object[]{note.IntegerPosition,note.Lane,note.SourceType}).ToArray();
        var input=JsonSerializer.SerializeToElement(new{Timing=timing,Notes=notes,Rows=Enumerable.Range(0,33).Select(i=>i*12).ToArray(),NumSteps=notes.Length,LastStep=notes.Max(note=>(int)note[0])});
        return EditorTimingOracle.Reference.Run(input,args[0]);
    }
    async Task<object> ReadRegions(string content) {
        var path=System.IO.Path.Combine(System.IO.Path.GetTempPath(),Guid.NewGuid()+".ssc");
        try {
            await File.WriteAllTextAsync(path,content);
            var song=await new Fumen.Converters.SSCReader(path).LoadAsync(CancellationToken.None);
            return song.Charts.Select(chart=>new {
                Warps=chart.Layers[0].Events.OfType<Warp>().Select(e=>new{Row=e.IntegerPosition,Length=e.LengthIntegerPosition}).ToArray(),
                Fakes=chart.Layers[0].Events.OfType<FakeSegment>().Select(e=>new{Row=e.IntegerPosition,Length=e.LengthIntegerPosition}).ToArray(),
                Notes=chart.Layers[0].Events.OfType<LaneNote>().Select(note=>new{Row=note.IntegerPosition,Lane=note.Lane,Time=note.TimeSeconds}).ToArray(),
                Editor=EditorState(chart)
            }).ToArray();
        } finally {File.Delete(path);}
    }
    Console.WriteLine(JsonSerializer.Serialize(new{Before=await ReadRegions(request.GetProperty("Content").GetString()!),After=await ReadRegions(request.GetProperty("EditedContent").GetString()!)},options));return;
}
if (operation == "PauseFile") {
    async Task<object> ReadPauses(string content) {
        var extension=request.GetProperty("Format").GetString()!;
        var path=System.IO.Path.Combine(System.IO.Path.GetTempPath(),Guid.NewGuid()+"."+extension);
        try {
            await File.WriteAllTextAsync(path,content);
            Fumen.Converters.Reader reader=extension=="ssc"?new Fumen.Converters.SSCReader(path):new Fumen.Converters.SMReader(path);
            var song=await reader.LoadAsync(CancellationToken.None);
            return song.Charts.Select(chart=>new {
                Offset=chart.ChartOffsetFromMusic,
                Pauses=chart.Layers[0].Events.OfType<Stop>().Select(stop=>new{Row=stop.IntegerPosition,Seconds=stop.LengthSeconds,Delay=stop.IsDelay}).ToArray(),
                Notes=chart.Layers[0].Events.OfType<LaneNote>().Select(note=>new{Row=note.IntegerPosition,Lane=note.Lane,Time=note.TimeSeconds}).ToArray()
            }).ToArray();
        } finally {File.Delete(path);}
    }
    Console.WriteLine(JsonSerializer.Serialize(new{Before=await ReadPauses(request.GetProperty("Content").GetString()!),After=await ReadPauses(request.GetProperty("EditedContent").GetString()!)},options));return;
}
if (operation == "TimingFile") {
    async Task<object> Read(string content) {
        var extension=request.GetProperty("Format").GetString()!;
        var path=System.IO.Path.Combine(System.IO.Path.GetTempPath(),Guid.NewGuid()+"."+extension);
        try {
            await File.WriteAllTextAsync(path,content);
            Fumen.Converters.Reader reader=extension=="ssc"?new Fumen.Converters.SSCReader(path):new Fumen.Converters.SMReader(path);
            var song=await reader.LoadAsync(CancellationToken.None);
            return song.Charts.Select(chart=>new {
                Offset=chart.ChartOffsetFromMusic,
                Bpms=chart.Layers[0].Events.OfType<Tempo>().Select(tempo=>new{Row=tempo.IntegerPosition,Bpm=tempo.TempoBPM}).ToArray(),
                Notes=chart.Layers[0].Events.OfType<LaneNote>().Select(note=>new{Row=note.IntegerPosition,Lane=note.Lane,Time=note.TimeSeconds}).ToArray()
            }).ToArray();
        } finally {File.Delete(path);}
    }
    Console.WriteLine(JsonSerializer.Serialize(new{Before=await Read(request.GetProperty("Content").GetString()!),After=await Read(request.GetProperty("EditedContent").GetString()!)},options));return;
}
if (operation == "FileClock")
{
    var extension = request.GetProperty("Format").GetString()!;
    var temp = Path.Combine(Path.GetTempPath(), Guid.NewGuid().ToString() + "." + extension);
    try {
        await File.WriteAllTextAsync(temp, request.GetProperty("Content").GetString()!);
        Fumen.Converters.Reader reader = extension == "ssc" ? new Fumen.Converters.SSCReader(temp) : new Fumen.Converters.SMReader(temp);
        var song = await reader.LoadAsync(CancellationToken.None);
        var fileChart = song.Charts[request.GetProperty("Chart").GetInt32()];
        var fileEvents = fileChart.Layers[0].Events;
        var fileNotes = fileEvents.OfType<LaneNote>().ToArray();
        var filePad = await PadData.LoadPadData("dance-single", Path.Combine(args[0], "StepManiaLibrary", "dance-single.json"));
        var fileGraph = StepGraph.Load(Path.Combine(args[0], "StepManiaLibrary", "dance-single.fsg"), filePad);
        var fileConfig = new StepManiaLibrary.ExpressedChart.Config {
            BracketParsingDetermination = StepManiaLibrary.ExpressedChart.BracketParsingDetermination.UseDefaultMethod,
            DefaultBracketParsingMethod = StepManiaLibrary.ExpressedChart.BracketParsingMethod.Balanced };
        fileConfig.Init();
        var fileExpressed = StepManiaLibrary.ExpressedChart.ExpressedChart.CreateFromSMEvents(fileEvents, fileGraph, fileConfig, 9.0, "Oracle");

        var fileTargetName = request.GetProperty("TargetLayout").GetString()!;
        var fileTargetPad = await PadData.LoadPadData(fileTargetName, Path.Combine(args[0], "StepManiaLibrary", fileTargetName + ".json"));
        var fileTargetGraph = StepGraph.Load(Path.Combine(args[0], "StepManiaLibrary", fileTargetName + ".fsg"), fileTargetPad);
        var filePerformed = JsonSerializer.Deserialize<PerformedConfig>(request.GetProperty("PerformedConfig"), options)!;
        filePerformed.Init();
        var fileRoots = new List<List<GraphNode>> { new() { fileTargetGraph.GetRoot() } };
        for (var tier = 1; tier < fileTargetPad.StartingPositions.Length; tier++)
            fileRoots.Add(fileTargetPad.StartingPositions[tier].Select(pos => fileTargetGraph.FindGraphNode(pos[0], GraphArrowState.Resting, pos[1], GraphArrowState.Resting)).ToList());
        var fileFallbacks = filePad.CanFitWithin(fileTargetPad) ? null : await StepTypeFallbacks.Load(Path.Combine(args[0], "StepManiaLibrary", StepTypeFallbacks.DefaultFallbacksFileName));
        var fileConverted = fileExpressed == null ? null : PerformedChart.CreateFromExpressedChart(fileTargetGraph, filePerformed, fileRoots, fileFallbacks, fileExpressed, request.GetProperty("Seed").GetInt32(), "Oracle");
        Console.WriteLine(JsonSerializer.Serialize(new {
            Success = fileExpressed != null,
            ConvertedNotes = fileConverted?.CreateSMChartEvents().OfType<LaneNote>().Select(note => new { Row = note.IntegerPosition, Lane = note.Lane, Symbol = note.SourceType }).ToArray(),
            Steps = fileExpressed?.StepEvents.Select(e => new { Row = e.Position, Time = e.Time,
                Link = Enumerable.Range(0, 4).SelectMany(i => new int[] {
                    e.LinkInstance.GraphLink.Links[i / 2, i % 2].Valid ? 1 : 0,
                    (int)e.LinkInstance.GraphLink.Links[i / 2, i % 2].Step,
                    (int)e.LinkInstance.GraphLink.Links[i / 2, i % 2].Action,
                    (int)e.LinkInstance.InstanceTypes[i / 2, i % 2] }).ToArray() }).ToArray(),
            Mines = fileExpressed?.MineEvents.Select(m => new { Row = m.Position, Time = m.Time, Lane = m.OriginalArrow,
                Kind = (int)m.Type, Nth = m.ArrowIsNthClosest, Foot = m.FootAssociatedWithPairedNote }).ToArray(),
            Span = new[] { fileEvents.First().TimeSeconds, fileEvents.Last().TimeSeconds },
            Notes = fileNotes.Select(note => new { Row = note.IntegerPosition, Lane = note.Lane, Symbol = note.SourceType,
                Time = note.TimeSeconds, GroupTime = fileEvents.First(e => e.IntegerPosition == note.IntegerPosition).TimeSeconds }).ToArray()
        }, options));
    } finally { File.Delete(temp); }
    return;
}
if (operation == "Clock")
{
    var clockEvents = new List<Event>();
    foreach (var tempo in request.GetProperty("Bpms").EnumerateArray())
        clockEvents.Add(new Tempo(tempo[1].GetDouble()) { IntegerPosition = tempo[0].GetInt32() });
    foreach (var timingName in new[] { "Stops", "Delays", "Warps" })
        if (request.TryGetProperty(timingName, out var segments))
            foreach (var segment in segments.EnumerateArray())
                clockEvents.Add(timingName == "Warps"
                    ? new Warp(segment[1].GetInt32()) { IntegerPosition = segment[0].GetInt32() }
                    : new Stop(segment[1].GetDouble(), timingName == "Delays") { IntegerPosition = segment[0].GetInt32() });
    var probes = request.GetProperty("Rows").EnumerateArray().Select(row =>
        new LaneTapNote { IntegerPosition = row.GetInt32(), Lane = 0, SourceType = "1" }).ToList();
    clockEvents.AddRange(probes);
    clockEvents.Sort(new Fumen.Converters.SMCommon.SMEventComparer());
    Fumen.Converters.SMCommon.SetEventTimeFromRows(clockEvents);
    Console.WriteLine(JsonSerializer.Serialize(new {
        Notes = probes.Select(note => note.TimeSeconds).ToArray(),
        Groups = probes.Select(note => clockEvents.First(e => e.IntegerPosition == note.IntegerPosition).TimeSeconds).ToArray()
    }, options));
    return;
}
if (operation == "PadFit")
{
    var fitNames = request.GetProperty("Layouts").Deserialize<string[]>()!;
    var fitPads = new List<PadData>();
    foreach (var fitName in fitNames) fitPads.Add(await PadData.LoadPadData(fitName, Path.Combine(args[0], "StepManiaLibrary", fitName + ".json")));
    Console.WriteLine(JsonSerializer.Serialize(fitPads.Select(source => fitPads.Select(target => source.CanFitWithin(target)).ToArray()).ToArray(), options));
    return;
}
if (operation == "RandomOrder")
{
    var orderResults = new List<object>();
    foreach (var test in request.GetProperty("Cases").EnumerateArray()) {
        var orderRandom = new Random(test.GetProperty("Seed").GetInt32());
        var calls = 0;
        var values = Enumerable.Range(0, test.GetProperty("Length").GetInt32())
            .OrderBy(_ => { calls++; return orderRandom.Next(); }).ToList();
        orderResults.Add(new { Order = values, Calls = calls, Next = orderRandom.Next() });
    }
    Console.WriteLine(JsonSerializer.Serialize(orderResults, options));
    return;
}
if (operation == "Random")
{
    var random = new Random(request.GetProperty("Seed").GetInt32());
    Console.WriteLine(JsonSerializer.Serialize(Enumerable.Range(0, request.GetProperty("Count").GetInt32()).Select(_ => random.Next()).ToArray(), options));
    return;
}
if (operation == "Configurations")
{
    var results = new List<object>();
    foreach (var test in request.GetProperty("Cases").EnumerateArray())
    {
        var kind = test.GetProperty("Kind").GetString();
        var json = test.GetProperty("Config").GetRawText();
        StepManiaLibrary.Config config = kind switch
        {
            "Performed" => JsonSerializer.Deserialize<PerformedConfig>(json, options)!,
            "Pattern" => JsonSerializer.Deserialize<PatternConfig>(json, options)!,
            "Expressed" => JsonSerializer.Deserialize<StepManiaLibrary.ExpressedChart.Config>(json, options)!,
            _ => throw new ArgumentException("Unknown configuration kind"),
        };
        config.Init();
        var valid = config.Validate();
        object? weights = null;
        if (config is PerformedConfig performed && test.TryGetProperty("Layout", out var layout))
        {
            var chartTypeName = layout.GetString()!;
            var chartTypeValid = Fumen.Converters.SMCommon.TryGetChartType(chartTypeName, out var chartType);
            valid &= performed.ValidateArrowWeights(chartTypeName, chartType, chartTypeValid);
            weights = performed.GetArrowWeightsNormalized(chartTypeName);
        }
        results.Add(new { Name = test.GetProperty("Name").GetString(), Valid = valid,
            Initialized = JsonSerializer.SerializeToElement(config, config.GetType(), options), Weights = weights });
    }
    Console.WriteLine(JsonSerializer.Serialize(results, options));
    return;
}
if (operation == "Fallbacks")
{
    var fallbackConfig = await StepTypeFallbacks.Load(Path.Combine(args[0], "StepManiaLibrary", StepTypeFallbacks.DefaultFallbacksFileName));
    if (fallbackConfig == null) throw new InvalidDataException("Invalid pinned fallbacks");
    var cacheType = typeof(PerformedChart).Assembly.GetType("StepManiaLibrary.PerformedChart.GraphLinkInstanceCache")!;
    var cacheInstance = Activator.CreateInstance(cacheType)!;
    var getLinks = cacheType.GetMethod("GetGraphLinks")!;
    int[] encode(GraphLinkInstance link) => Enumerable.Range(0, 4).SelectMany(i => new[] {
        link.GraphLink.Links[i / 2, i % 2].Valid ? 1 : 0, (int)link.GraphLink.Links[i / 2, i % 2].Step,
        (int)link.GraphLink.Links[i / 2, i % 2].Action, (int)link.InstanceTypes[i / 2, i % 2]
    }).ToArray();
    var fallbackResults = new List<object>();
    foreach (var test in request.GetProperty("Cases").EnumerateArray())
    {
        var source = new GraphLinkInstance();
        for (var f = 0; f < 2; f++)
        {
            if (!test.TryGetProperty(f == 0 ? "Left" : "Right", out var foot)) continue;
            var step = (StepType)foot.GetProperty("Step").GetInt32();
            var data = StepData.Steps[(int)step];
            var portion = data.IsOneArrowBracket ? data.SingleStepFootPortion : 0;
            var action = (FootAction)foot.GetProperty("Action").GetInt32();
            source.GraphLink.Links[f, portion] = new GraphLink.FootArrowState(step, action);
            source.InstanceTypes[f, portion] = (InstanceStepType)foot.GetProperty("Instance").GetInt32();
            if (data.IsBracket)
            {
                source.GraphLink.Links[f, 1] = new GraphLink.FootArrowState(step,
                    foot.TryGetProperty("ToeAction", out var toeAction) ? (FootAction)toeAction.GetInt32() : action);
                source.InstanceTypes[f, 1] = foot.TryGetProperty("ToeInstance", out var toeInstance) ? (InstanceStepType)toeInstance.GetInt32() : source.InstanceTypes[f, portion];
            }
        }
        var enabled = !test.TryGetProperty("Disabled", out var disabled) || !disabled.GetBoolean();
        var links = (List<GraphLinkInstance>)getLinks.Invoke(cacheInstance, new object?[] { source, enabled ? fallbackConfig : null })!;
        fallbackResults.Add(new { Label = test.GetProperty("Label").GetString(), Source = encode(source),
            Links = links.Select(link => new { Link = encode(link),
                BlankFoot = (bool)cacheType.GetMethod("ContainsBlankLink")!.Invoke(cacheInstance, new object[] { source, link })!,
                Removed = (int)cacheType.GetMethod("GetNumStepsRemoved")!.Invoke(cacheInstance, new object[] { source, link })! }).ToArray() });
    }
    Console.WriteLine(JsonSerializer.Serialize(new {
        Lists = Enum.GetValues<StepType>().Select(step => fallbackConfig.GetFallbacks(step).Select(s => (int)s).ToArray()).ToArray(),
        Cases = fallbackResults
    }, options));
    return;
}
if (operation == "Sort")
{
    var sortedCases = new List<object>();
    foreach (var values in request.GetProperty("Cases").EnumerateArray())
    {
        var sortValues = values.EnumerateArray().Select((v, i) => (Key: v.GetInt32(), Index: i)).ToList();
        sortValues.Sort((a, b) => a.Key.CompareTo(b.Key));
        sortedCases.Add(sortValues.Select(v => v.Index).ToArray());
    }
    Console.WriteLine(JsonSerializer.Serialize(sortedCases, options));
    return;
}
if (operation != "Pattern" && operation != "InterpretMatches" && operation != "InterpretPaths" && operation != "InterpretChart" && operation != "Convert" && operation != "PatternContext")
    throw new ArgumentException("Unknown oracle operation");

var name = request.GetProperty("Layout").GetString()!;
var pad = await PadData.LoadPadData(name, Path.Combine(args[0], "StepManiaLibrary", name + ".json"));
var graph = StepGraph.Load(Path.Combine(args[0], "StepManiaLibrary", name + ".fsg"), pad);
if (graph == null) throw new InvalidDataException("Could not load reference graph");
if (operation == "PatternContext")
{
    List<Event> PcNotes(string field) {
        var pcEvents = new List<Event>();
        foreach (var pcItem in request.GetProperty(field).EnumerateArray()) {
            var pcSymbol = pcItem[2].GetString()!;
            LaneNote pcNote = pcSymbol switch { "2" or "4" => new LaneHoldStartNote(), "3" => new LaneHoldEndNote(), "M" or "K" => new LaneNote(), _ => new LaneTapNote() };
            pcNote.SourceType = pcSymbol; pcNote.Lane = pcItem[1].GetInt32(); pcNote.IntegerPosition = pcItem[0].GetInt32();
            pcNote.TimeSeconds = pcNote.IntegerPosition / 120.0 * (request.TryGetProperty("TimeScale", out var pcScale) ? pcScale.GetDouble() : 1.0);
            pcEvents.Add(pcNote);
        }
        pcEvents.Sort(new Fumen.Converters.SMCommon.SMEventComparer()); return pcEvents;
    }
    List<EditorEvent> PcEditorEvents(string notesField, string eventsField) {
        if (request.TryGetProperty(eventsField, out var pcRaw)) return pcRaw.EnumerateArray().Select(pcItem => new EditorEvent(pcItem[0].GetInt32(), pcItem[1].GetInt32(), pcItem[2].GetBoolean())).ToList();
        return request.GetProperty(notesField).EnumerateArray().Where(pcItem => pcItem[2].GetString() != "3")
            .Select(pcItem => new EditorEvent(pcItem[0].GetInt32(), pcItem[1].GetInt32(), pcItem[2].GetString() is "1" or "2" or "4" or "F" or "L"))
            .OrderBy(pcItem => pcItem.Row).ThenBy(pcItem => pcItem.Lane).ToList();
    }
    var pcExpression = new StepManiaLibrary.ExpressedChart.Config {
        BracketParsingDetermination = StepManiaLibrary.ExpressedChart.BracketParsingDetermination.UseDefaultMethod,
        DefaultBracketParsingMethod = StepManiaLibrary.ExpressedChart.BracketParsingMethod.Balanced };
    pcExpression.Init();
    var pcBefore = StepManiaLibrary.ExpressedChart.ExpressedChart.CreateFromSMEvents(PcNotes("BeforeNotes"), graph, pcExpression, 12.0, "Oracle");
    var pcAfter = StepManiaLibrary.ExpressedChart.ExpressedChart.CreateFromSMEvents(PcNotes("AfterNotes"), graph, pcExpression, 12.0, "Oracle");
    if (pcBefore == null || pcAfter == null) { Console.WriteLine("{\"Success\":false}"); return; }
    var pcPerformed = JsonSerializer.Deserialize<PerformedConfig>(request.GetProperty("PerformedConfig"), options)!; pcPerformed.Init();
    var pcPattern = JsonSerializer.Deserialize<PatternConfig>(request.GetProperty("PatternConfig"), options)!; pcPattern.Init();
    var pcStart = request.GetProperty("Start").GetInt32(); var pcEnd = request.GetProperty("End").GetInt32();
    var pcTotal = request.GetProperty("TotalSteps").GetInt32();
    var pcCount = request.GetProperty("NumSteps").GetInt32();
    var pcValues = PatternContextReference.Run(new EditorPatternEvent(pcStart, pcEnd, pcCount, pcPerformed), graph, pcBefore, pcAfter,
        PcEditorEvents("BeforeNotes", "BeforeEditorEvents"), PcEditorEvents("AfterNotes", "AfterEditorEvents"), pcTotal, request.GetProperty("RangeStartsAfterChart").GetBoolean());
    var pcTiming = new List<Event> { new Tempo(150.0) { IntegerPosition = 0 } };
    var pcGenerated = PerformedChart.CreateWithPattern(graph, pcPattern, pcPerformed, pcStart, pcEnd, request.GetProperty("Seed").GetInt32(),
        pcValues.PreviousFoot, pcValues.PreviousTimes, pcValues.PreviousFooting, pcValues.FollowingFooting,
        request.GetProperty("LaneCounts").Deserialize<int[]>()!, pcValues.SameCounts, pcTiming, pcTotal,
        pcValues.LastTransitionSteps, pcValues.LastTransitionLeft, request.GetProperty("Nps").GetDouble(), "Oracle");
    Console.WriteLine(JsonSerializer.Serialize(new { Success = true, Context = pcValues,
        Notes = pcGenerated?.CreateSMChartEvents().OfType<LaneNote>().Select(pcNote => new { Row = pcNote.IntegerPosition, Lane = pcNote.Lane, Symbol = pcNote.SourceType }).ToArray() }, options));
    return;
}
if (operation == "Convert")
{
    StepManiaLibrary.ExpressedChart.ExpressedChart? expressed;
    if (request.TryGetProperty("LinkEvents", out var linkEvents))
    {
        expressed = (StepManiaLibrary.ExpressedChart.ExpressedChart)Activator.CreateInstance(typeof(StepManiaLibrary.ExpressedChart.ExpressedChart),
            BindingFlags.Instance | BindingFlags.NonPublic, null, new object[] { new List<Event>(), "Oracle", graph }, null)!;
        foreach (var item in linkEvents.EnumerateArray())
        {
            var link = new GraphLinkInstance();
            var encoded = item.GetProperty("Link");
            for (var f = 0; f < 2; f++) for (var p = 0; p < 2; p++) {
                var i = f * 8 + p * 4;
                link.GraphLink.Links[f, p] = new GraphLink.FootArrowState((StepType)encoded[i+1].GetInt32(), (FootAction)encoded[i+2].GetInt32(), encoded[i].GetInt32() != 0);
                link.InstanceTypes[f, p] = (InstanceStepType)encoded[i+3].GetInt32();
            }
            expressed.StepEvents.Add(new StepManiaLibrary.ExpressedChart.ExpressedChart.StepEvent(item.GetProperty("Row").GetInt32(), item.GetProperty("Time").GetDouble(), link));
        }
    }
    else
    {
        var sourceName = request.GetProperty("SourceLayout").GetString()!;
        var sourcePad = await PadData.LoadPadData(sourceName, Path.Combine(args[0], "StepManiaLibrary", sourceName + ".json"));
        var sourceGraph = StepGraph.Load(Path.Combine(args[0], "StepManiaLibrary", sourceName + ".fsg"), sourcePad);
        var sourceEvents = new List<Event>();
        foreach (var item in request.GetProperty("Notes").EnumerateArray()) {
            var symbol = item[2].GetString()!;
            LaneNote note = symbol switch { "2" or "4" => new LaneHoldStartNote(), "3" => new LaneHoldEndNote(), "M" or "K" => new LaneNote(), _ => new LaneTapNote() };
            note.SourceType = symbol; note.Lane = item[1].GetInt32(); note.IntegerPosition = item[0].GetInt32(); note.TimeSeconds = note.IntegerPosition / 120.0;
            sourceEvents.Add(note);
        }
        sourceEvents.Sort(new Fumen.Converters.SMCommon.SMEventComparer());
        var config = new StepManiaLibrary.ExpressedChart.Config { BracketParsingDetermination = StepManiaLibrary.ExpressedChart.BracketParsingDetermination.UseDefaultMethod,
            DefaultBracketParsingMethod = StepManiaLibrary.ExpressedChart.BracketParsingMethod.Balanced };
        config.Init();
        expressed = StepManiaLibrary.ExpressedChart.ExpressedChart.CreateFromSMEvents(sourceEvents, sourceGraph, config, 12.0, "Oracle");
    }
    if (expressed == null) { Console.WriteLine("{\"Success\":false,\"Stage\":\"Expression\"}"); return; }
    if (request.TryGetProperty("LinkMines", out var linkMines)) foreach (var mine in linkMines.EnumerateArray())
        expressed.MineEvents.Add(new StepManiaLibrary.ExpressedChart.ExpressedChart.MineEvent(mine.GetProperty("Row").GetInt32(), mine.GetProperty("Time").GetDouble(), mine.GetProperty("Lane").GetInt32()) {
            Type = (MineType)mine.GetProperty("Kind").GetInt32(), ArrowIsNthClosest = mine.GetProperty("Nth").GetInt32(), FootAssociatedWithPairedNote = mine.GetProperty("Foot").GetInt32()
        });
    var convertedConfig = JsonSerializer.Deserialize<PerformedConfig>(request.GetProperty("PerformedConfig"), options)!;
    convertedConfig.Init();
    if (!convertedConfig.Validate()) throw new InvalidDataException("Invalid performed configuration");
    var mode = request.GetProperty("FallbackMode").GetString();
    StepTypeFallbacks? conversionFallbacks = null;
    if (mode == "default") conversionFallbacks = await StepTypeFallbacks.Load(Path.Combine(args[0], "StepManiaLibrary", StepTypeFallbacks.DefaultFallbacksFileName));
    if (mode == "identity") {
        conversionFallbacks = new StepTypeFallbacks();
        conversionFallbacks.FallbacksStrings = Enum.GetValues<StepType>().ToDictionary(step => step, step => new List<string> { step.ToString() });
        typeof(StepTypeFallbacks).GetMethod("Init", BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(conversionFallbacks, null);
    }
    var conversionRoots = new List<List<GraphNode>> { new() { graph.GetRoot() } };
    for (var tier = 1; tier < pad.StartingPositions.Length; tier++)
        conversionRoots.Add(pad.StartingPositions[tier].Select(pos => graph.FindGraphNode(pos[0], GraphArrowState.Resting, pos[1], GraphArrowState.Resting)).ToList());
    var converted = PerformedChart.CreateFromExpressedChart(graph, convertedConfig, conversionRoots, conversionFallbacks, expressed, request.GetProperty("Seed").GetInt32(), "Oracle");
    if (converted == null) { Console.WriteLine("{\"Success\":false,\"Stage\":\"Conversion\"}"); return; }
    var convertedSteps = new List<object>();
    var convertedMines = new List<object>();
    var convertedRoot = ((StepPerformanceNode)converted.GetRootNode()).GraphNodeInstance.Node;
    var convertedTier = conversionRoots.FindIndex(tier => tier.Any(root => root.Equals(convertedRoot)));
    for (var cursor = converted.GetRootNode(); cursor != null; cursor = cursor.Next) {
        if (cursor is MinePerformanceNode mine) convertedMines.Add(new { Row = mine.Position, Time = mine.Time, Lane = mine.Arrow });
        if (cursor is not StepPerformanceNode step) continue;
        convertedSteps.Add(new { Row = step.Position, Time = step.Time, Orientation = (int)step.GraphNodeInstance.Node.Orientation,
            Feet = Enumerable.Range(0, 2).Select(f => Enumerable.Range(0, 2).Select(p => new {
                Lane = step.GraphNodeInstance.Node.State[f, p].Arrow, State = (int)step.GraphNodeInstance.Node.State[f, p].State,
                Valid = step.GraphLinkInstance?.GraphLink.Links[f, p].Valid ?? false,
                Step = (int)(step.GraphLinkInstance?.GraphLink.Links[f, p].Step ?? StepType.SameArrow),
                Action = (int)(step.GraphLinkInstance?.GraphLink.Links[f, p].Action ?? FootAction.Tap),
                Instance = (int)step.GraphNodeInstance.InstanceTypes[f, p]
            }).ToArray()).ToArray() });
    }
    var smEvents = converted.CreateSMChartEvents().OfType<LaneNote>()
        .Select(note => new { Row = note.IntegerPosition, Lane = note.Lane, Symbol = note.SourceType }).ToArray();
    Console.WriteLine(JsonSerializer.Serialize(new { Success = true, Tier = convertedTier, Steps = convertedSteps, Mines = convertedMines, NoteEvents = smEvents }, options));
    return;
}
if (operation == "InterpretChart")
{
    var expressedType = typeof(StepManiaLibrary.ExpressedChart.ExpressedChart);
    var noteEvents = new List<Event>();
    foreach (var item in request.GetProperty("Notes").EnumerateArray())
    {
        var symbol = item[2].GetString()!;
        LaneNote note = symbol switch {
            "2" or "4" => new LaneHoldStartNote(), "3" => new LaneHoldEndNote(),
            "M" or "K" => new LaneNote(), _ => new LaneTapNote()
        };
        note.SourceType = symbol;
        note.Lane = item[1].GetInt32();
        note.IntegerPosition = item[0].GetInt32();
        note.TimeSeconds = note.IntegerPosition / 120.0 * (request.TryGetProperty("TimeScale", out var scale) ? scale.GetDouble() : 1.0);
        noteEvents.Add(note);
    }
    noteEvents.Sort(new Fumen.Converters.SMCommon.SMEventComparer());
    var interpreter = Activator.CreateInstance(expressedType, BindingFlags.Instance | BindingFlags.NonPublic,
        null, new object[] { noteEvents, "Oracle", graph }, null)!;
    var field = expressedType.GetField("BracketParsingMethod", BindingFlags.Instance | BindingFlags.NonPublic)!;
    field.SetValue(interpreter, Enum.Parse(field.FieldType, request.GetProperty("Method").GetString()!));
    bool success;
    if (request.TryGetProperty("ExpressedConfig", out var expressionConfig))
    {
        var config = JsonSerializer.Deserialize<StepManiaLibrary.ExpressedChart.Config>(expressionConfig, options)!;
        config.Init();
        if (!config.Validate()) throw new InvalidDataException("Invalid expressed configuration");
        var complete = StepManiaLibrary.ExpressedChart.ExpressedChart.CreateFromSMEvents(noteEvents, graph, config,
            request.GetProperty("Difficulty").GetDouble(), "Oracle");
        success = complete != null;
        if (success) interpreter = complete!;
    }
    else
    {
        success = (bool)expressedType.GetMethod("Search", BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(interpreter, null)!;
        if (success) expressedType.GetMethod("AddMines", BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(interpreter, null);
    }
    var interpreted = (StepManiaLibrary.ExpressedChart.ExpressedChart)interpreter;
    var trace = new List<ExpressedNode>();
    var result = new List<object>();
    var node = success ? interpreted.GetRootSearchNode() : null;
    while (node != null)
    {
        trace.Add(node);
        result.Add(new { Row = node.Position, Time = node.TimeSeconds, Cost = node.Cost, TotalCost = node.TotalCost,
            OrientationCost = node.TotalOrientationCost, LastFoot = node.LastFootUsedToStepOnArrow,
            Orientation = (int)node.GraphNode.Orientation,
            Feet = Enumerable.Range(0, 2).Select(f => Enumerable.Range(0, 2).Select(p => new {
                Lane = node.GraphNode.State[f, p].Arrow, State = (int)node.GraphNode.State[f, p].State,
                Valid = node.PreviousLink?.GraphLink.Links[f, p].Valid ?? false,
                Step = (int)(node.PreviousLink?.GraphLink.Links[f, p].Step ?? StepType.SameArrow),
                Action = (int)(node.PreviousLink?.GraphLink.Links[f, p].Action ?? FootAction.Tap),
                Instance = (int)(node.PreviousLink?.InstanceTypes[f, p] ?? InstanceStepType.Default)
            }).ToArray()).ToArray() });
        node = node.GetNextNode();
    }
    var actions = MineUtils.GetReleasesAndSteps(trace, pad.NumArrows);
    Console.WriteLine(JsonSerializer.Serialize(new { Success = success, Nodes = result,
        Method = interpreted.GetBracketParsingMethod().ToString(),
        Releases = actions.Item1.Select(a => new { Row = a.Position, Lane = a.Arrow, Foot = a.Foot }).ToArray(),
        Steps = actions.Item2.Select(a => new { Row = a.Position, Lane = a.Arrow, Foot = a.Foot }).ToArray(),
        Mines = interpreted.MineEvents.Select(m => new { Row = m.Position, Time = m.Time, Lane = m.OriginalArrow,
            Kind = (int)m.Type, Nth = m.ArrowIsNthClosest, Foot = m.FootAssociatedWithPairedNote }).ToArray()
    }, options));
    return;
}
if (operation == "InterpretPaths")
{
    var expressedType = typeof(StepManiaLibrary.ExpressedChart.ExpressedChart);
    var stateType = expressedType.GetNestedType("SearchState", BindingFlags.NonPublic)!;
    var match = expressedType.GetMethod("GetLinkInstanceIfStateMatches", BindingFlags.Static | BindingFlags.NonPublic)!;
    var getCost = expressedType.GetMethod("GetCost", BindingFlags.Instance | BindingFlags.NonPublic)!;
    var getOrientation = expressedType.GetMethod("GetOrientationCost", BindingFlags.Instance | BindingFlags.NonPublic)!;
    var prune = expressedType.GetMethod("Prune", BindingFlags.Static | BindingFlags.NonPublic,
        null, new[] { typeof(HashSet<ExpressedNode>) }, null)!;
    var interpreter = Activator.CreateInstance(expressedType, BindingFlags.Instance | BindingFlags.NonPublic,
        null, new object[] { new List<Event>(), "Oracle", graph }, null)!;
    var method = request.GetProperty("Method").GetString()!;
    var field = expressedType.GetField("BracketParsingMethod", BindingFlags.Instance | BindingFlags.NonPublic)!;
    field.SetValue(interpreter, Enum.Parse(field.FieldType, method));
    var root = new StepManiaLibrary.ExpressedChart.ExpressedChart.ChartSearchNode(graph.GetRoot(), 0, 0, null, null, new InstanceStepType[pad.NumArrows]);
    var interpretCurrent = new List<StepManiaLibrary.ExpressedChart.ExpressedChart.ChartSearchNode> { root };
    var ids = new Dictionary<StepManiaLibrary.ExpressedChart.ExpressedChart.ChartSearchNode, int> { [root] = 0 };
    var output = new List<object>();
    object nodeState(GraphNode node) => new {
        Orientation = (int)node.Orientation,
        Feet = Enumerable.Range(0, 2).Select(f => Enumerable.Range(0, 2).Select(p =>
            new { Lane = node.State[f, p].Arrow, State = (int)node.State[f, p].State }).ToArray()).ToArray()
    };
    var position = 0;
    foreach (var frame in request.GetProperty("Frames").EnumerateArray())
    {
        position += 12;
        var state = Array.CreateInstance(stateType, pad.NumArrows);
        var buffer = Array.CreateInstance(stateType, pad.NumArrows);
        var values = frame.GetProperty("State").Deserialize<int[]>()!;
        var instanceTypes = values.Select(v => v is 6 or 7 ? InstanceStepType.Roll : v == 2 ? InstanceStepType.Fake : v == 3 ? InstanceStepType.Lift : InstanceStepType.Default).ToArray();
        for (var a = 0; a < values.Length; a++) state.SetValue(Enum.ToObject(stateType, values[a]), a);
        var interpretNodes = new List<StepManiaLibrary.ExpressedChart.ExpressedChart.ChartSearchNode>();
        var matches = new List<object>();
        var multiple = values.Count(v => v is 1 or 2 or 3 or 4 or 6) > 1;
        foreach (var parent in interpretCurrent)
        foreach (var edge in parent.GraphNode.Links)
        {
            if ((!multiple && (edge.Key.IsJump() || edge.Key.InvolvesBracket()))
                || (frame.GetProperty("Release").GetBoolean() && !edge.Key.IsRelease())) continue;
            foreach (var target in edge.Value)
            {
                var instance = (GraphLinkInstance?)match.Invoke(null, new object[] { state, buffer, target, edge.Key });
                if (instance == null) continue;
                var child = new StepManiaLibrary.ExpressedChart.ExpressedChart.ChartSearchNode(target, position, position / 120.0, parent, instance, instanceTypes);
                var cost = (int)getCost.Invoke(interpreter, new object[] { child, state, new int[pad.NumArrows], new int[pad.NumArrows], pad })!;
                var orientation = (int)getOrientation.Invoke(interpreter, new object[] { child })!;
                child.SetCost(cost, orientation);
                ids[child] = ids.Count;
                interpretNodes.Add(child);
                if (!parent.NextNodes.TryGetValue(edge.Key, out var siblings)) parent.NextNodes[edge.Key] = siblings = new();
                siblings.Add(child);
                matches.Add(new { Id = ids[child], Parent = ids[parent], Node = nodeState(target), Cost = cost, OrientationCost = orientation,
                    TotalCost = child.TotalCost, TotalOrientationCost = child.TotalOrientationCost, LastFoot = child.LastFootUsedToStepOnArrow,
                    PreviousSteps = new[] { child.GetPreviousStepSearchNode(), child.GetPreviousStepSearchNode(2) }.Select(n => n == null ? -1 : ids[n]).ToArray(),
                    Feet = Enumerable.Range(0, 2).Select(f => Enumerable.Range(0, 2).Select(p => new {
                        Valid = edge.Key.Links[f, p].Valid, Step = (int)edge.Key.Links[f, p].Step,
                        Action = (int)edge.Key.Links[f, p].Action, Instance = (int)instance.InstanceTypes[f, p]
                    }).ToArray()).ToArray() });
            }
        }
        var kept = (HashSet<ExpressedNode>)prune.Invoke(null, new object[] { interpretNodes.ToHashSet() })!;
        interpretCurrent = kept.ToList();
        var best = interpretCurrent.FirstOrDefault();
        foreach (var child in interpretCurrent) if (best == null || child.CompareTo(best) < 0) best = child;
        output.Add(new { Row = position, Matches = matches, Retained = interpretCurrent.Select(n => ids[n]).ToArray(), Best = best == null ? -1 : ids[best] });
        if (interpretCurrent.Count == 0) break;
    }
    Console.WriteLine(JsonSerializer.Serialize(new { Root = nodeState(root.GraphNode), Frames = output }, options));
    return;
}
if (operation == "InterpretMatches")
{
    var expressedType = typeof(StepManiaLibrary.ExpressedChart.ExpressedChart);
    var stateType = expressedType.GetNestedType("SearchState", BindingFlags.NonPublic)!;
    var match = expressedType.GetMethod("GetLinkInstanceIfStateMatches", BindingFlags.Static | BindingFlags.NonPublic)!;
    var parent = graph.GetRoot();
    var output = new List<object>();
    object nodeState(GraphNode node) => new {
        Orientation = (int)node.Orientation,
        Feet = Enumerable.Range(0, 2).Select(f => Enumerable.Range(0, 2).Select(p =>
            new { Lane = node.State[f, p].Arrow, State = (int)node.State[f, p].State }).ToArray()).ToArray()
    };
    foreach (var frame in request.GetProperty("Frames").EnumerateArray())
    {
        var state = Array.CreateInstance(stateType, pad.NumArrows);
        var buffer = Array.CreateInstance(stateType, pad.NumArrows);
        var values = frame.GetProperty("State").Deserialize<int[]>()!;
        for (var a = 0; a < values.Length; a++) state.SetValue(Enum.ToObject(stateType, values[a]), a);
        var matches = new List<object>();
        GraphNode? first = null;
        var multiple = values.Count(v => v is 1 or 2 or 3 or 4 or 6) > 1;
        foreach (var edge in parent.Links)
        {
            if ((!multiple && (edge.Key.IsJump() || edge.Key.InvolvesBracket()))
                || (frame.GetProperty("Release").GetBoolean() && !edge.Key.IsRelease())) continue;
            foreach (var target in edge.Value)
            {
                var instance = (GraphLinkInstance?)match.Invoke(null, new object[] { state, buffer, target, edge.Key });
                if (instance == null) continue;
                first ??= target;
                matches.Add(new { Node = nodeState(target), Feet = Enumerable.Range(0, 2).Select(f =>
                    Enumerable.Range(0, 2).Select(p => new {
                        Valid = edge.Key.Links[f, p].Valid, Step = (int)edge.Key.Links[f, p].Step,
                        Action = (int)edge.Key.Links[f, p].Action, Instance = (int)instance.InstanceTypes[f, p]
                    }).ToArray()).ToArray() });
            }
        }
        output.Add(new { Parent = nodeState(parent), Matches = matches });
        if (first == null) break;
        parent = first;
    }
    Console.WriteLine(JsonSerializer.Serialize(output, options));
    return;
}
var performedConfig = JsonSerializer.Deserialize<PerformedConfig>(request.GetProperty("PerformedConfig"), options)!;
performedConfig.Init();
var patternConfig = JsonSerializer.Deserialize<PatternConfig>(request.GetProperty("PatternConfig"), options)!;
patternConfig.Init();
if (!performedConfig.Validate() || !patternConfig.Validate())
    throw new InvalidDataException("Invalid reference configuration");
var events = new List<Event>();
foreach (var tempo in request.GetProperty("Bpms").EnumerateArray())
    events.Add(new Tempo(tempo[1].GetDouble()) { IntegerPosition = tempo[0].GetInt32() });
foreach (var timingName in new[] { "Stops", "Delays", "Warps" })
    if (request.TryGetProperty(timingName, out var segments))
        foreach (var segment in segments.EnumerateArray())
            events.Add(timingName == "Warps"
                ? new Warp(segment[1].GetInt32()) { IntegerPosition = segment[0].GetInt32() }
                : new Stop(segment[1].GetDouble(), timingName == "Delays") { IntegerPosition = segment[0].GetInt32() });
var previousFooting = request.GetProperty("PreviousFooting").Deserialize<int[]>()!;
var followingFooting = request.GetProperty("FollowingFooting").Deserialize<int[]>()!;
var laneCounts = request.TryGetProperty("LaneCounts", out var counts) ? counts.Deserialize<int[]>()! : new int[pad.NumArrows];
var previousTimes = request.TryGetProperty("PreviousTimes", out var times) ? times.Deserialize<double[]>()! : new double[2];
var sameCounts = request.TryGetProperty("PreviousSameCounts", out var same) ? same.Deserialize<int[]>()! : new int[2];
var chart = PerformedChart.CreateWithPattern(graph, patternConfig, performedConfig,
    request.GetProperty("Start").GetInt32(), request.GetProperty("End").GetInt32(),
    request.GetProperty("Seed").GetInt32(), request.GetProperty("PreviousFoot").GetInt32(),
    previousTimes, previousFooting, followingFooting, laneCounts, sameCounts, events,
    request.TryGetProperty("TotalSteps", out var steps) ? steps.GetInt32() : 0,
    request.TryGetProperty("LastTransitionSteps", out var transitionSteps) ? transitionSteps.GetInt32() : 0,
    request.TryGetProperty("LastTransitionLeft", out var transitionLeft) ? transitionLeft.GetBoolean() : null,
    request.TryGetProperty("Nps", out var nps) ? nps.GetDouble() : 0.0, "Rust differential fixture");
if (chart == null)
{
    Console.WriteLine("{\"Success\":false}");
    return;
}
var notes = chart.CreateSMChartEvents().OfType<LaneNote>().Select(note => new { Row = note.IntegerPosition, note.Lane, Kind = note.SourceType }).ToList();
var nodes = new List<object>();
var performanceNodes = new List<StepPerformanceNode>();
var current = chart.GetRootNode().Next;
while (current != null)
{
    if (current is StepPerformanceNode step)
    {
        performanceNodes.Add(step);
        var feet = new List<object>();
        for (var foot = 0; foot < 2; foot++)
        {
            var portion = step.GraphNodeInstance.Node.State[foot, 0];
            feet.Add(new { Lane = portion.Arrow, State = (int)portion.State,
                Valid = step.GraphLinkInstance.GraphLink.Links[foot, 0].Valid,
                Step = (int)step.GraphLinkInstance.GraphLink.Links[foot, 0].Step });
        }
        nodes.Add(new { Row = step.Position, step.Time, Feet = feet });
    }
    current = current.Next;
}
var scores = ScoreTrace.Make(graph, performedConfig, patternConfig, request, performanceNodes);
Console.WriteLine(JsonSerializer.Serialize(new { Success = true, Notes = notes, Nodes = nodes, Scores = scores }, options));

static class ScoreTrace
{
    public static List<object> Make(StepGraph graph, PerformedConfig config, PatternConfig pattern, JsonElement request, List<StepPerformanceNode> path)
    {
        var type = typeof(PerformedChart).GetNestedType("SearchNode", BindingFlags.NonPublic)!;
        var constructor = type.GetConstructors().Single(info => info.GetParameters().Length == 18);
        var fields = new Dictionary<string, string> {
            ["fallback"] = "TotalFallbackStepCost", ["misleading"] = "MisleadingStepCount", ["ambiguous"] = "AmbiguousStepCount",
            ["same_run_over_max"] = "TotalNumSameArrowStepsInARowPerFootOverMax", ["stretch"] = "TotalStretchCost",
            ["facing"] = "TotalFacingCost", ["distance"] = "TotalIndividualStepTravelDistanceCost", ["speed"] = "TotalIndividualStepTravelSpeedCost",
            ["pattern_type"] = "PatternGenerationStepTypeCost", ["lateral"] = "TotalLateralMovementSpeedCost",
            ["early_transition"] = "TotalEarlyTransitionCost", ["late_transition"] = "TotalLateTransitionCost", ["distribution"] = "TotalDistributionCost",
        };
        var rootNode = path[0].Prev is StepPerformanceNode root ? root.GraphNodeInstance.Node : throw new InvalidOperationException();
        var firstFoot = path[0].GraphLinkInstance.GraphLink.IsSingleStep(out _, out var foot) ? foot : throw new InvalidOperationException();
        var times = request.TryGetProperty("PreviousTimes", out var rawTimes) ? rawTimes.Deserialize<double[]>()! : new double[2];
        var counts = request.TryGetProperty("LaneCounts", out var rawCounts) ? rawCounts.Deserialize<int[]>()! : new int[graph.NumArrows];
        var prefix = request.TryGetProperty("PreviousSameCounts", out var rawPrefix) ? rawPrefix.Deserialize<int[]>()! : new int[2];
        var nps = request.TryGetProperty("Nps", out var rawNps) ? rawNps.GetDouble() : 0.0;
        object? transition = null;
        if (request.TryGetProperty("LastTransitionSteps", out var transitionSteps) && transitionSteps.GetInt32() != 0)
            transition = type.GetMethod("MakeTransitionNode")!.Invoke(null, [request.GetProperty("LastTransitionLeft").GetBoolean(), transitionSteps.GetInt32()]);
        object previous = constructor.Invoke([rootNode, new List<GraphLinkInstance>(), null, 0.0, times[firstFoot], 0, null,
            new PerformedChart.PerformanceFootAction[graph.NumArrows], graph, nps, 0.0, config, pattern, counts, times,
            request.TryGetProperty("TotalSteps", out var total) ? total.GetInt32() : 0, transition, null]);
        var getActions = typeof(PerformedChart).GetMethods(BindingFlags.Static | BindingFlags.NonPublic).Single(method =>
            method.Name == "GetActionsForNode" && method.GetParameters()[0].ParameterType == typeof(GraphNode));
        var results = new List<object>();
        var depth = 1;
        foreach (var step in path)
        {
            step.GraphLinkInstance.GraphLink.IsSingleStep(out _, out var steppingFoot);
            var actions = getActions.Invoke(null, [step.GraphNodeInstance.Node, step.GraphLinkInstance.GraphLink, graph.NumArrows]);
            previous = constructor.Invoke([step.GraphNodeInstance.Node, new List<GraphLinkInstance>(), step.GraphLinkInstance,
                0.0, step.Time, depth++, previous, actions, graph, nps, 0.0, config, pattern, null, null, null, null, prefix[steppingFoot]]);
            var result = fields.ToDictionary(pair => pair.Key, pair => type.GetField(pair.Value, BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(previous));
            results.Add(result);
        }
        return results;
    }
}
