// The active index is an explicit fixture input. This adapter deliberately
// does not claim to test the original event-tree FindBest selection policy.
namespace StepManiaEditor;
internal sealed class SearchPosition { public double ChartTime,ChartPosition; }
internal sealed class SearchZoom { public double Value; public double GetSpacingZoom()=>Value; }
internal sealed class SearchRateList(EditorRateAlteringEvent[] values,int active) {
 public SearchEnumerator FindBest(SearchPosition _) => values.Length==0 ? null : new(values,active-1);
}
internal sealed class SearchEnumerator(EditorRateAlteringEvent[] values,int index) {
 private int Index=index;
 public EditorRateAlteringEvent Current=>values[Index];
 public bool MoveNext(){if(Index+1>=values.Length)return false;Index++;return true;}
 public bool MovePrev(){if(Index<=0)return false;Index--;return true;}
}
internal sealed partial class ActiveChartSearch {
 public EditorChart Chart;
 public SearchPosition Position;
 public SearchZoom ZoomManager;
 public double FocalPointScreenSpaceY,Interpolated;
 public double GetCurrentInterpolatedScrollRate()=>Interpolated;
 public double GetScrollDirectionSign()=>Editor.GetScrollDirectionSign();
}
