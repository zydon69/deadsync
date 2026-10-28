# Windows Save As: identity and durability boundary

## Prepared, not integrated or validated

`patches/editor-save-as-windows-identity.patch` extends the existing adapter; it does not introduce a second writer. Its new undeclared Windows-only module is `crates/deadsync-editor/src/storage/save_as/windows_identity.rs`. Apply after the baseline history/storage patches. Rebind/native layers remain separate. No Cargo run or Windows execution has been performed for this increment.

The target dependency is windows-sys 0.61.2, already represented in Cargo.lock, with Foundation and Storage_FileSystem only. Rust stable MetadataExt's volume_serial_number/file_index remain unstable APIs, so they cannot supply the required portable identity here.

## Identity

GetFileInformationByHandleEx(FileIdInfo) retrieves a volume serial number (64 bits) and a complete FILE_ID_128. Comparison never falls back to path, canonical string, timestamp, size, or file contents. The ID bytes are mapped injectively into u128; Unix inode becomes u128 without information loss. Directory queries open a real directory handle using FILE_FLAG_BACKUP_SEMANTICS and verify its type. Kernel failures fail preparation/guards closed.

Three Windows-only tests are prepared: independently opened hard links have equal identity; identical bytes replacing a retained old object have a different identity; replacing the parent directory changes identity, and missing/regular-file parents fail. The old object is retained or renamed in replacement tests to avoid ID reuse invalidating the scenario. These tests require Windows and a filesystem supporting the documented FileIdInfo operation. Unsupported filesystems return an error, never weaker identity.

## Durability is deliberately still incomplete

The current shared sync_directory implementation is a no-op outside Unix. Merely enabling Windows identity would therefore incorrectly report Durable. The separate patch replaces the Save As-local sync callback with a platform-specific callback: Unix keeps the existing directory sync, while other platforms return Unsupported. On Windows a successfully published output therefore becomes WrittenPendingDurability, remains dirty, retains its receipt, and retry reports the same unsupported boundary without rewriting bytes. It must not be advertised as a fully finished Windows save workflow.

The next required increment is a proven Windows publication barrier. Investigate a same-volume rename with a documented write-through contract (including existing-destination replacement and parent identity revalidation), rather than assuming File::sync_all before rename or a directory FlushFileBuffers is equivalent to Unix directory fsync. The existing tempfile persist operation alone has not been proven to provide that barrier. Do not change global sync_directory semantics or mark a pending output clean to conceal the missing barrier.

## Official references

- [Microsoft FILE_ID_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_id_info): complete file ID plus volume identity.
- [Microsoft GetFileInformationByHandleEx](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getfileinformationbyhandleex): query result/failure contract.
- [Microsoft directory handles](https://learn.microsoft.com/en-us/windows/win32/fileio/obtaining-a-handle-to-a-directory): BACKUP_SEMANTICS and supported directory operations.
- [Rust MetadataExt](https://doc.rust-lang.org/std/os/windows/fs/trait.MetadataExt.html): unstable identity methods.
- [windows-sys 0.61.2 declaration](https://docs.rs/windows-sys/0.61.2/windows_sys/Win32/Storage/FileSystem/fn.GetFileInformationByHandleEx.html).

The historical 21 baseline tests remain pending parent validation; these three additional Windows tests are not included in that number. Patch application check passed on the current tree; compilation, runtime, filesystem durability and power-loss behavior are unverified.

## Second layer: write-through publication prepared

`patches/editor-save-as-windows-publish.patch` is applied after the identity layer, with the new undeclared `windows_publish.rs` beside `windows_identity.rs`. It replaces only the Windows production persist route: synchronize the temp handle with File::sync_all (FlushFileBuffers), then MoveFileExW with WRITE_THROUGH, adding REPLACE_EXISTING only for an explicitly reviewed existing target. No COPY_ALLOWED and no DELAY_UNTIL_REBOOT are accepted. Both names share the reviewed canonical parent; cross-volume copy/delete is not a fallback. Unicode paths are encoded as UTF-16 with a terminator, rejecting embedded NUL.

A successful callback is the publication barrier itself, so the later commit callback may acknowledge that completed Windows barrier without invoking the non-Unix directory-sync no-op. No generally successful Windows directory-fsync helper is invented. The pending-durability identity-only layer is an intermediate safety boundary, not the proposed final Windows workflow. Receipts originating from that intermediate layer cannot be silently upgraded by retry without a proved publication barrier.

Microsoft documents that WRITE_THROUGH waits for the disk move and explicitly describes flush completion for copy/delete operations. The same-volume design relies on the documented disk-move completion contract; the docs are not a claim of compare-and-swap with uncooperative writers or a hardware-independent proof against power loss. Existing review guards remain necessary. A direct kernel rename API should not be labeled a transactional operation; no transaction API is used.

Three additional Windows-only publication tests are prepared: absent destination plus reviewed replacement with exact bytes/handle identity; no-clobber collision and directory replacement refusal preserving the existing file; NUL path refusal before output. They do not emulate Windows on macOS. Run both new modules on real Windows after parent synchronizes the direct target dependency in Cargo.lock, using --locked. Existing Unix history/storage tests remain unchanged; Windows lifecycle/full adapter tests and real filesystem execution remain required before advertising Windows support.

- [Microsoft MoveFileExW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw): WRITE_THROUGH, REPLACE_EXISTING, disallowed copy/reboot flags and error semantics.
- [Microsoft FlushFileBuffers](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-flushfilebuffers): file-buffer synchronization, distinct from directory namespace publication.

No shared source or Cargo manifest has been changed by either patch preparation. Identity patch application check passed; the writer layer was generated from that applied patch on temporary copies only. All six Windows tests are pending compilation and execution.
