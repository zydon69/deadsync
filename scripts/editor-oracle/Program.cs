// Development-only differential oracle. All generation and cost decisions are
// executed by the pinned, unmodified StepManiaLibrary project referenced by
// ReferenceRoot. This tool is not linked into DeadSync or shipped with the game.
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
if (operation != "Pattern" && operation != "InterpretMatches" && operation != "InterpretPaths" && operation != "InterpretChart")
    throw new ArgumentException("Unknown oracle operation");

var name = request.GetProperty("Layout").GetString()!;
var pad = await PadData.LoadPadData(name, Path.Combine(args[0], "StepManiaLibrary", name + ".json"));
var graph = StepGraph.Load(Path.Combine(args[0], "StepManiaLibrary", name + ".fsg"), pad);
if (graph == null) throw new InvalidDataException("Could not load reference graph");
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
        note.TimeSeconds = note.IntegerPosition / 120.0;
        noteEvents.Add(note);
    }
    noteEvents.Sort(new Fumen.Converters.SMCommon.SMEventComparer());
    var interpreter = Activator.CreateInstance(expressedType, BindingFlags.Instance | BindingFlags.NonPublic,
        null, new object[] { noteEvents, "Oracle", graph }, null)!;
    var field = expressedType.GetField("BracketParsingMethod", BindingFlags.Instance | BindingFlags.NonPublic)!;
    field.SetValue(interpreter, Enum.Parse(field.FieldType, request.GetProperty("Method").GetString()!));
    var success = (bool)expressedType.GetMethod("Search", BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(interpreter, null)!;
    var result = new List<object>();
    var node = ((StepManiaLibrary.ExpressedChart.ExpressedChart)interpreter).GetRootSearchNode();
    while (node != null)
    {
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
    Console.WriteLine(JsonSerializer.Serialize(new { Success = success, Nodes = result }, options));
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
