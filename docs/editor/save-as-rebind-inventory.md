# Save As relocation: state and stamp inventory

This is an implementation inventory for the unapplied native/rebind patches,
not validation of their runtime behavior. Existing storage/history21 tests remain
separate and unchanged by these extension modules.

| State | Namespace | Policy |
|---|---|---|
| Session current/saved/content, Entry before/after stamps | Session revisions | Injective total remap; saved MAX sentinel stays a sentinel |
| Workspace.structural_focus keys + before_revision | Session revisions | Preallocate remapped map before output, preserve every entry including detached stamps |
| ActiveChartViews owner | Session identity/content token | Preserve owner identity; rebind under media-only proof |
| ViewState position.token + clock_token | Content tokens | Validate every old token; retain exact row/time and Clock Arc because media-only proof leaves timing unchanged |
| Views focused GUID/order/dedicated/selection_anchor/music_time | User view state | Preserve, no clear/reopen/time inversion |
| State.focus_position | Exact fractional/negative/warp position | Rebind token only, preserve independent numeric coordinates |
| State.profile_preference_history keys | Session revisions | Include all keys before planning; preallocate total remapped map with same snapshots |
| State.profile_creations keys | Session revisions | Same total remap; retain kind/GUID payloads |
| PreferenceSnapshot.revision, State.profile_preference_revision/serial | Independent preference serials | Preserve values; these are not Session stamps |
| State.profile_history | External profile path keys/receipts | Preserve; no replay and no song-path rebasing |
| Workspace note/event clipboards, attack clipboard | Detached copied data, no live Session token | Preserve data |
| Workspace row/snap/player/tool/selection endpoints/clipboard/autoadvance | User editing state | Preserve; do not reconstruct Workspace |
| Paused/playing decoder fields | Generation + content stamp, audio path | Stop explicitly before preparing destination; no restart from stale preview |
| Marker/spacing/sample/attack/view-render/minimap caches | Source Arc + revision or content binding | Invalidate via remapped stamps and refresh; caches are not histories |
| Waveform key/job | Source/revision/chart/path | Refresh new path/key; stale result stays rejected by existing key checks |
| Image request identity | Native document request counter | Advance identity after path publication |
| Chart list/order caches/requests | Content binding + language revision | Reject/invalidate stale binding, keep unrelated UI preference serial |
| Generation/catalog/preview jobs | RevisionToken | Block Save As while active; do not bless their stale work |
| Property/timing/note/event/profile drafts | Live revision/content token | Save As is entered only from ordinary editor/menu after drafts are resolved; do not silently accept stale drafts |
| Pending lane input | Live draft/held note intent | Refuse Save As while draft/deletion intent is present |
| AutoRecovery worker | Old path/baseline + RevisionToken | Refuse entry/publication while worker active; suspend new jobs while modal, install new path ownership after publication |
| Pack settings draft/pending durability | Separate pack file baseline | Refuse Save As until resolved; only clean derived pack/list cache may be refreshed for new path |
| SaveAs pending durability | Published ContentToken + destination/parent inode/bytes | Retain receipt across editing; guard Close/Open/Recover, expose Retry and SaveCurrent separately |

Current ActiveChartViews has no additional revision-indexed history collection:
its complete fields are owner, entries, focused and music_time. Any future such
field must be included in this inventory and total mapping before integration.

The prepared API accepts external Session stamp collections before finalizing
the relocation map. Workspace contributes all structural-focus keys/before
values; State contributes the two profile maps. Missing keys are errors, never
filtered away. All new stamps are greater than the largest original stamp across
Session and these controllers. Overflow fails before writing. The recovered
saved_revision=u64::MAX is excluded from the real stamp set.

The media-only receipt has private construction and exact before/after content
identity checks. It cannot be used as a general-purpose rebind after arbitrary
timing edits or across Sessions. Native drafts/jobs are not converted using it.
