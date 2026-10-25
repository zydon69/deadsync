// Development-only preference/input notification adapters. ZoomManager itself
// is copied unchanged from the pinned Git object, not reimplemented here.
namespace StepManiaEditor;
internal static class Editor { public enum SpacingMode { ConstantTime, ConstantRow, Variable } }
internal sealed class Preferences {
    public static Preferences Instance { get; } = new();
    public PreferencesScroll PreferencesScroll { get; } = new();
    public PreferencesKeyBinds PreferencesKeyBinds { get; } = new();
}
internal sealed class PreferencesKeyBinds { public int ScrollZoom => 1; public int ScrollSpacing => 2; }
internal sealed class KeyCommandManager(bool zoom,bool spacing) { public bool IsAnyInputDown(int key) => key==1 ? zoom : spacing; }
internal sealed class PreferencesScroll {
    public const string NotificationTimeBasedPpsChanged="time",NotificationRowBasedPprChanged="row",NotificationVariablePpsChanged="variable",NotificationSizeCapChanged="cap",NotificationLimitZoomToSizeChanged="limit";
    private readonly List<Fumen.IObserver<PreferencesScroll>> observers=[];
    private double time=300,row=2,variable=300,cap=1;
    private bool limit;
    public void AddObserver(Fumen.IObserver<PreferencesScroll> value) => observers.Add(value);
    private void Notify(string key) {foreach(var observer in observers) observer.OnNotify(key,this,null!);}
    public double TimeBasedPixelsPerSecond {get=>time;set{time=value;Notify(NotificationTimeBasedPpsChanged);}}
    public double RowBasedPixelsPerRow {get=>row;set{row=value;Notify(NotificationRowBasedPprChanged);}}
    public double VariablePixelsPerSecondAtDefaultBPM {get=>variable;set{variable=value;Notify(NotificationVariablePpsChanged);}}
    public double SizeCap {get=>cap;set{cap=value;Notify(NotificationSizeCapChanged);}}
    public bool LimitZoomToSize {get=>limit;set{limit=value;Notify(NotificationLimitZoomToSizeChanged);}}
    public Editor.SpacingMode SpacingMode {get;set;}=Editor.SpacingMode.ConstantTime;
    public double ScrollInterpolationDuration {get;set;}=0.1;
    public double ZoomMultiplier {get;set;}=1.2;
}
