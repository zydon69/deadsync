using StepManiaLibrary.PerformedChart;using Fumen;using Fumen.ChartDefinition;using StepManiaLibrary;using static Fumen.Converters.SMCommon;
namespace StepManiaEditor {
abstract class UndoableAction(bool a,bool b){public TaskCompletionSource Completion=new();protected void OnDone()=>Completion.TrySetResult();public abstract bool AffectsFile();protected abstract void DoImplementation();protected abstract void UndoImplementation();public void Run()=>DoImplementation();public void Undo()=>UndoImplementation();}
static class MainThreadDispatcher{public static void RunOnMainThread(Action action)=>action();}
static class Logger{public static List<string> Messages=[];public static void Error(string s)=>Messages.Add(s);}
static class ActionQueue{public static QueueOwner Instance=new();public class QueueOwner{public void SetHasUnsavedChanges(){}}}
static class ImGuiUtils{public static string GetPrettyEnumString(ChartType type)=>type.ToString();}
class Editor(string root,EditorChart original){public List<EditorChart> Added=[];public EditorChart Focus=original;Dictionary<ChartType,StepGraph> graphs=[];
 public EditorChart GetFocusedChart()=>Focus;public void AddChart(EditorChart chart,bool focus){Added.Add(chart);if(focus)Focus=chart;}public void DeleteChart(EditorChart chart,EditorChart previous){Added.Remove(chart);Focus=previous;}
 public bool GetStepGraph(ChartType type,out StepGraph graph,bool unused){if(graphs.TryGetValue(type,out graph))return true;var name=ChartTypeString(type);var pad=PadData.LoadPadData(name,System.IO.Path.Combine(root,"StepManiaLibrary",name+".json")).GetAwaiter().GetResult();graph=StepGraph.Load(System.IO.Path.Combine(root,"StepManiaLibrary",name+".fsg"),pad);graphs[type]=graph;return graph!=null;}
 public bool GetStepGraphRootNodes(ChartType type,out List<List<GraphNode>> nodes){GetStepGraph(type,out var graph,true);nodes=[new(){graph.GetRoot()}];for(int tier=1;tier<graph.PadData.StartingPositions.Length;tier++)nodes.Add(graph.PadData.StartingPositions[tier].Select(pos=>graph.FindGraphNode(pos[0],GraphArrowState.Resting,pos[1],GraphArrowState.Resting)).ToList());return true;}
 public StepTypeFallbacks GetStepTypeFallbacks()=>null;
}
}
namespace StepManiaEditor.AutogenConfig {
class ExpressedChartConfigManager {public static Guid DefaultExpressedChartDynamicConfigGuid=new("a19d532e-b0ce-4759-ad1c-02ecbbdf2efd");public static ExpressedChartConfigManager Instance=new();public record Holder(StepManiaLibrary.ExpressedChart.Config Config);public Holder GetConfig(Guid id){var c=new StepManiaLibrary.ExpressedChart.Config{BracketParsingDetermination=StepManiaLibrary.ExpressedChart.BracketParsingDetermination.UseDefaultMethod,DefaultBracketParsingMethod=StepManiaLibrary.ExpressedChart.BracketParsingMethod.NoBrackets};c.Init();return new(c);}}
class PatternConfigManager {public static PatternConfigManager Instance=new();public object GetConfig(Guid id)=>new();public static string DefaultPatternConfigSixteenthsName="Sixteenths";public static Guid DefaultPatternConfigSixteenthsGuid=new("03de57bd-4329-4de6-a722-a171c11fdd16");}
class PerformedChartConfigManager {public static PerformedChartConfigManager Instance=new();public object GetConfig(Guid id)=>new();public static string DefaultPerformedChartPatternBalancedConfigName="Balanced";public static Guid DefaultPerformedChartPatternBalancedGuid=new("41a7f76e-596a-4fbe-bf6e-6a77a898e4a1");}
}
