using System.Reflection;
using System.Text.Json;
using StepManiaEditor;
var input=JsonDocument.Parse(File.ReadAllBytes(args[0])).RootElement;
var manager=new ZoomManager();var prefs=Preferences.Instance.PreferencesScroll;
double Internal(string name) {
    var value=typeof(ZoomManager).GetField(name,BindingFlags.Instance|BindingFlags.NonPublic)!.GetValue(manager)!;
    return (double)value.GetType().GetMethod("GetValue")!.Invoke(value,null)!;
}
object Snapshot(bool? captured=null)=>new {SpacingZoom=manager.GetSpacingZoom(),SizeZoom=manager.GetSizeZoom(),SizeCap=manager.GetSizeCap(),Time=Internal("ConstantTimeSpacingData"),Row=Internal("ConstantRowSpacingData"),Variable=Internal("VariableSpacingData"),Captured=captured};
var initial=Snapshot();var stages=new List<object>();
Editor.SpacingMode Mode(string value)=>value switch {"time"=>Editor.SpacingMode.ConstantTime,"row"=>Editor.SpacingMode.ConstantRow,_=>Editor.SpacingMode.Variable};
foreach(var op in input.GetProperty("Operations").EnumerateArray()) {
    bool? captured=null;
    switch(op[0].GetString()) {
        case "zoom":manager.SetZoom(op[1].GetDouble());break;
        case "cap":manager.SetSizeCap(op[1].GetDouble());break;
        case "limit":prefs.LimitZoomToSize=op[1].GetBoolean();break;
        case "duration":prefs.ScrollInterpolationDuration=op[1].GetDouble();break;
        case "multiplier":prefs.ZoomMultiplier=op[1].GetDouble();break;
        case "spacing":
            switch(Mode(op[1].GetString()!)) {
                case Editor.SpacingMode.ConstantTime:prefs.TimeBasedPixelsPerSecond=op[2].GetDouble();break;
                case Editor.SpacingMode.ConstantRow:prefs.RowBasedPixelsPerRow=op[2].GetDouble();break;
                case Editor.SpacingMode.Variable:prefs.VariablePixelsPerSecondAtDefaultBPM=op[2].GetDouble();break;
            }break;
        case "update":manager.Update(op[1].GetDouble());break;
        case "scroll":
            prefs.SpacingMode=Mode(op[5].GetString()!);
            captured=manager.ProcessInput(op[1].GetDouble(),new(op[3].GetBoolean(),op[4].GetBoolean()),op[2].GetSingle());break;
        default:throw new ArgumentException("Unknown zoom operation");
    }
    stages.Add(Snapshot(captured));
}
Console.WriteLine(JsonSerializer.Serialize(new {Initial=initial,Stages=stages},new JsonSerializerOptions {WriteIndented=true}));
