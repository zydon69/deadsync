// Development-only scalar and conversion adapters. The spacing helper source
// is copied unchanged. Linear rate conversions are explicit fixture inputs;
// this runner does not emulate the real EditorChart timeline.
namespace StepManiaEditor;
internal static class Editor {
 public enum SpacingMode { ConstantTime, ConstantRow, Variable }
 public static bool Reverse;
 public static double GetScrollDirectionSign()=>Reverse ? -1.0 : 1.0;
}
internal sealed class Preferences {
 public static Preferences Instance { get; }=new();
 public PreferencesScroll PreferencesScroll { get; }=new();
}
internal sealed class PreferencesScroll {
 public const double DefaultVariablePixelsPerSecondAtDefaultBPM=300;
 public Editor.SpacingMode SpacingMode;
 public double TimeBasedPixelsPerSecond=300,RowBasedPixelsPerRow=2,VariablePixelsPerSecondAtDefaultBPM=300;
}
internal sealed class EditorChart {}
internal class EditorEvent {
 public double Time,Row;
 public double GetChartTime()=>Time;
 public double GetChartPosition()=>Row;
 public double GetRow()=>Row;
}
internal sealed class EditorRateAlteringEvent : EditorEvent {
 public double Tempo,SecondsPerRow,RowsPerSecond,Scroll;
 public double GetTempo()=>Tempo;
 public double GetSecondsPerRow()=>SecondsPerRow;
 public double GetRowsPerSecond()=>RowsPerSecond;
 public double GetScrollRate()=>Scroll;
 public double GetChartTimeFromPosition(double row)=>Time+(row-Row)*SecondsPerRow;
 public double GetChartPositionFromTime(double time)=>Row+(time-Time)/SecondsPerRow;
}
internal interface IChartRegion {
 double GetChartTime();double GetChartPosition();double GetChartTimeDurationForRegion();double GetChartPositionDurationForRegion();bool IsRegionSelection();double GetRegionY();
}
internal sealed class Region : IChartRegion {
 public double Time,Row,TimeDuration,RowDuration,Y;public bool Selection;
 public double GetChartTime()=>Time;public double GetChartPosition()=>Row;public double GetChartTimeDurationForRegion()=>TimeDuration;public double GetChartPositionDurationForRegion()=>RowDuration;public bool IsRegionSelection()=>Selection;public double GetRegionY()=>Y;
}
