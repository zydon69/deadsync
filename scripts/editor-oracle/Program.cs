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
if (operation != "Pattern")
    throw new ArgumentException("Unknown oracle operation");

var name = request.GetProperty("Layout").GetString()!;
var pad = await PadData.LoadPadData(name, Path.Combine(args[0], "StepManiaLibrary", name + ".json"));
var graph = StepGraph.Load(Path.Combine(args[0], "StepManiaLibrary", name + ".fsg"), pad);
if (graph == null) throw new InvalidDataException("Could not load reference graph");
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
