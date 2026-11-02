# Independent generation source: profile catalog lifecycle

Confirmed native workflow gap after independent source selection was added.
Source UIAutogenChart.cs:78–93 owns SourceChart independently of chart focus;
its performed selector at115–144 reads global PerformedChartConfigManager and
LastSelectedAutogenPerformedChartConfig. It requires no focused chart view.
The action at161–166 uses the independent SourceChart and selected performed
configuration.

Native Generation row5 reached open_conversion_profiles_for, which captured
Workspace::conversion_profile_task with the focused chart (workspace.chart).
With no active view this index is the no-chart sentinel: catalog capture fails,
even though generation row7 has a valid selected source. With independent focus,
the catalog and staging were unnecessarily bound to the focused chart. Merely
changing the F9 check would not fix initial capture or final staging.

The UI now captures a stable chosen-source GUID and SessionBinding at catalog
opening. Task capture, poll/ordering, F9 sorting, profile editing/copy/export
(which already use profile_catalog_chart), and staging use that same owner.
Changing focus or closing the view does not retarget it. Removed/foreign/stale
catalogs refuse, preserving drafts and documents. Other catalog contexts keep
their existing owners, including the distinct chosen-expression target.
Core focused Workspace APIs remain available and strict.

Two native workflows cover focusedA/sourceB and no-active-view/sourceB:
actual Generation row5 key opens the worker; real profile selector/F9/order
preserves chosen GUID; staging updates generation preferences without chart
bytes/Undo; actual generation captures sourceB, Apply adds the chart, Undo
restores the original focus/no-view state and bytes. A separate test changes
focus after capture, closes the view, and checks successful staging, then
verifies stale and foreign owner refusals. Tests are authored, not run by this
sidecar agent; parent owns Cargo execution.

The existing type-group deletion fixture is repaired through the actual native
open_chart_deletion → process_chart_order(host) → deletion_key(Apply) flow.
It therefore prepares the song-timing fallback via the current production
barrier rather than bypassing the required cultural ordering or weakening the
core guard.
