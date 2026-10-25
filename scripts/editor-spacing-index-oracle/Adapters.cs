// Scalar event/search adapters. Actual Fumen red-black tree and unchanged
// original FindBest/Ensure members perform the selection.
using Fumen;
using static StepManiaEditor.EventTreeUtils;
namespace StepManiaEditor;
internal static class Editor {public enum SpacingMode{ConstantTime,ConstantRow,Variable}}
internal sealed class Preferences {public static Preferences Instance {get;}=new();public PreferencesScroll PreferencesScroll {get;}=new();}
internal sealed class PreferencesScroll {public Editor.SpacingMode SpacingMode;}
internal interface IReadOnlyEditorPosition {double ChartTime{get;}double ChartPosition{get;}}
internal sealed record Position(double ChartTime,double ChartPosition):IReadOnlyEditorPosition;
internal sealed class EditorChart{}
internal sealed record EventConfig(double Value,bool Time){public static EventConfig CreateSearchEventConfigWithOnlyTime(EditorChart _,double value)=>new(value,true);public static EventConfig CreateSearchEventConfigWithOnlyRow(EditorChart _,double value)=>new(value,false);}
internal class EditorEvent:IComparable<EditorEvent>{
 public double Row,Time;public int Index;public bool SearchRow,SearchTime;
 public double GetChartPosition()=>Row;public double GetChartTime()=>Time;
 public static EditorEvent CreateEvent(EventConfig config)=>new EditorRateAlteringEvent {Row=config.Time?0:config.Value,Time=config.Time?config.Value:0,SearchRow=!config.Time,SearchTime=config.Time};
 public int CompareTo(EditorEvent other){
  if(ReferenceEquals(this,other))return 0;
  if(SearchRow || other.SearchRow)return Row.CompareTo(other.Row);
  if(SearchTime || other.SearchTime)return Time.CompareTo(other.Time);
  var row=Row.CompareTo(other.Row);return row!=0?row:Index.CompareTo(other.Index);
 }
}
internal sealed class EditorRateAlteringEvent:EditorEvent,IComparable<EditorRateAlteringEvent>{public int CompareTo(EditorRateAlteringEvent other)=>base.CompareTo(other);}
internal sealed partial class RateAlteringEventTree:RedBlackTree<EditorRateAlteringEvent>{private readonly EditorChart Chart=new();}
