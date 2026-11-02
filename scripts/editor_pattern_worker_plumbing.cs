using Fumen;using StepManiaLibrary;
namespace EditorTimingOracle {internal sealed class Editor(string root) {public bool GetStepGraph(string type,out StepGraph graph,bool unused){var pad=PadData.LoadPadData(type,System.IO.Path.Combine(root,"StepManiaLibrary",type+".json")).GetAwaiter().GetResult();graph=StepGraph.Load(System.IO.Path.Combine(root,"StepManiaLibrary",type+".fsg"),pad);return graph!=null;}}
internal static class MainThreadDispatcher{public static void RunOnMainThread(Action action)=>action();}
internal static class Logger{public static List<string> Messages=[];public static void Warn(string s)=>Messages.Add(s);public static void Error(string s)=>Messages.Add(s);public static void Info(string s)=>Messages.Add(s);}
}
namespace StepManiaEditor.AutogenConfig {internal sealed class ExpressedChartConfigManager {public static readonly ExpressedChartConfigManager Instance=new();public StepManiaLibrary.ExpressedChart.Config Value=new();public Holder GetConfig(Guid id)=>new(Value);public record Holder(StepManiaLibrary.ExpressedChart.Config Config);}}
