//! ICU collation for the editor's CurrentCulture ordering. This uses the same
//! default UCollator configuration as .NET's CompareOptions.None implementation.
//! ICU/CLDR versions can differ between hosts; record the collation version in
//! differential diagnostics rather than promising a platform-independent order.
#[path = "editor_collation/log_clock.rs"]
mod log_clock;
use deadsync_editor::chart_order::Snapshot;
use libloading::Library;
pub(crate) use log_clock::LogClock;
use std::{
    cmp::Ordering,
    collections::HashMap,
    ffi::{CString, c_char, c_void},
    fmt,
    ptr::NonNull,
};

/// Stateless main-thread boundary, invoked only when the screen has queued a
/// chart-navigation request. No native handle is stored in screen state.
pub(crate) struct NativeChartOrder;
impl deadsync_editor::chart_order::Orderer for NativeChartOrder {
    fn order(&mut self, snapshot: &Snapshot) -> Result<Vec<usize>, deadsync_editor::Error> {
        let language = deadsync_theme_simply_love::i18n::current_locale();
        Collator::for_language(&language)
            .and_then(|collator| collator.ordered_snapshot(snapshot))
            .map_err(|error| deadsync_editor::Error::Unsupported(error.to_string()))
    }
}

type Open = unsafe extern "C" fn(*const c_char, *mut i32) -> *mut c_void;
type Close = unsafe extern "C" fn(*mut c_void);
type Compare = unsafe extern "C" fn(*const c_void, *const u16, i32, *const u16, i32) -> i32;
type SortKey = unsafe extern "C" fn(*const c_void, *const u16, i32, *mut u8, i32) -> i32;
type Version = unsafe extern "C" fn(*const c_void, *mut u8);

#[derive(Debug)]
pub(crate) struct Error(String);
impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy)]
struct Functions {
    open: Open,
    close: Close,
    compare: Compare,
    sort_key: SortKey,
    version: Version,
    number_open: NumberOpen,
    number_close: NumberClose,
    number_attribute: NumberAttribute,
    number_format: NumberFormat,
}
impl Functions {
    /// ICU's stable C ABI uses these exact prototypes. Resolve every function
    /// from one library and one symbol suffix, never mix ICU major versions.
    unsafe fn load(library: &Library, suffix: &str) -> Option<Self> {
        unsafe fn get<T: Copy>(library: &Library, name: &str, suffix: &str) -> Option<T> {
            let symbol = format!("{name}{suffix}\0");
            // SAFETY: caller chooses the ABI type corresponding to this ICU name;
            // the Library owner outlives all copied function pointers.
            unsafe { library.get::<T>(symbol.as_bytes()) }
                .ok()
                .map(|value| *value)
        }
        // SAFETY: each symbol's signature is the documented ICU C API signature.
        unsafe {
            Some(Self {
                open: get(library, "ucol_open", suffix)?,
                close: get(library, "ucol_close", suffix)?,
                compare: get(library, "ucol_strcoll", suffix)?,
                sort_key: get(library, "ucol_getSortKey", suffix)?,
                version: get(library, "ucol_getVersion", suffix)?,
                number_open: get(library, "unum_open", suffix)?,
                number_close: get(library, "unum_close", suffix)?,
                number_attribute: get(library, "unum_setAttribute", suffix)?,
                number_format: get(library, "unum_formatDouble", suffix)?,
            })
        }
    }
}

/// Owns the UCollator and library together. No Send/Sync claim is made for the
/// opaque handle; native editor interaction uses this on its owning UI thread.
pub(crate) struct Collator {
    handle: NonNull<c_void>,
    functions: Functions,
    _library: Library,
    culture: String,
    symbol_suffix: String,
    collation_version: [u8; 4],
}
impl Collator {
    pub(crate) fn for_language(language: &str) -> Result<Self, Error> {
        Self::open(&culture_for_language(language)?)
    }
    pub(crate) fn open(culture: &str) -> Result<Self, Error> {
        let culture = checked_culture(culture)?;
        let name = CString::new(culture.as_str())
            .map_err(|_| Error("Invalid collation culture".into()))?;
        for candidate in candidates() {
            let Ok(library) = load_library(&candidate) else {
                continue;
            };
            // ICU builds either export unversioned symbols (Apple/Windows) or
            // suffix all C symbols with the same ICU major version (Linux).
            let mut suffixes = vec![String::new()];
            suffixes.extend((50..=255).rev().map(|version| format!("_{version}")));
            for suffix in suffixes {
                // SAFETY: only known platform ICU libraries are loaded, with
                // signatures checked together by Functions::load.
                let Some(functions) = (unsafe { Functions::load(&library, &suffix) }) else {
                    continue;
                };
                let mut status = 0;
                // SAFETY: NUL-terminated culture and valid status output pointer.
                let pointer = unsafe { (functions.open)(name.as_ptr(), &mut status) };
                let Some(handle) = NonNull::new(pointer) else {
                    return Err(Error(format!(
                        "ICU could not open culture {culture} (status {status})"
                    )));
                };
                // ICU uses positive error codes and negative success warnings.
                // Valid en-US/fr-FR collators can legitimately report fallback
                // warnings because they inherit root or parent tailoring.
                if status > 0 {
                    // SAFETY: handle was allocated by this library's ucol_open.
                    unsafe { (functions.close)(handle.as_ptr()) };
                    return Err(Error(format!(
                        "ICU could not open culture {culture} (status {status})"
                    )));
                }
                let mut collation_version = [0; 4];
                // SAFETY: handle is live and ICU writes exactly four bytes.
                unsafe { (functions.version)(handle.as_ptr(), collation_version.as_mut_ptr()) };
                return Ok(Self {
                    handle,
                    functions,
                    _library: library,
                    culture,
                    symbol_suffix: suffix,
                    collation_version,
                });
            }
        }
        Err(Error(
            "Native ICU collation is unavailable on this system".into(),
        ))
    }
    pub(crate) fn culture(&self) -> &str {
        &self.culture
    }
    pub(crate) fn collation_version(&self) -> [u8; 4] {
        self.collation_version
    }
    pub(crate) fn compare(&self, left: &str, right: &str) -> Result<Ordering, Error> {
        let left = utf16(left)?;
        let right = utf16(right)?;
        // SAFETY: both UTF-16 arrays are valid for their explicitly checked
        // i32 lengths. Interior NULs remain significant input code units.
        let result = unsafe {
            (self.functions.compare)(
                self.handle.as_ptr(),
                left.as_ptr(),
                left.len() as i32,
                right.as_ptr(),
                right.len() as i32,
            )
        };
        Ok(result.cmp(&0))
    }
    pub(crate) fn sort_key(&self, text: &str) -> Result<Vec<u8>, Error> {
        let text = utf16(text)?;
        // SAFETY: ICU's documented preflight uses a null target with capacity 0.
        let required = unsafe {
            (self.functions.sort_key)(
                self.handle.as_ptr(),
                text.as_ptr(),
                text.len() as i32,
                std::ptr::null_mut(),
                0,
            )
        };
        if required <= 0 {
            return Err(Error("ICU returned an invalid sort key length".into()));
        }
        let mut result = Vec::new();
        result
            .try_reserve_exact(required as usize)
            .map_err(|_| Error("Cannot allocate ICU sort key".into()))?;
        result.resize(required as usize, 0);
        // SAFETY: target has exactly the preflighted size; input and collator
        // are unchanged. ICU sort-key size includes its terminating zero byte.
        let written = unsafe {
            (self.functions.sort_key)(
                self.handle.as_ptr(),
                text.as_ptr(),
                text.len() as i32,
                result.as_mut_ptr(),
                required,
            )
        };
        if written != required || result.last() != Some(&0) {
            return Err(Error(
                "ICU sort key changed between preflight and writing".into(),
            ));
        }
        Ok(result)
    }
    /// Precompute all name/description sort keys before sorting. Comparison
    /// then allocates nothing and cannot fail midway through Rust's sort.
    pub(crate) fn ordered_snapshot(&self, snapshot: &Snapshot) -> Result<Vec<usize>, Error> {
        let mut keys: HashMap<&str, Vec<u8>> = HashMap::new();
        for key in snapshot.keys() {
            for text in [&key.name, &key.description] {
                if !keys.contains_key(text.as_str()) {
                    keys.insert(text, self.sort_key(text)?);
                }
            }
        }
        snapshot
            .indices_with(|left, right| keys[left].cmp(&keys[right]))
            .map_err(|error| Error(error.to_string()))
    }
}
impl Drop for Collator {
    fn drop(&mut self) {
        // SAFETY: this is the unique owner of this open UCollator. The library
        // field remains alive throughout Drop and is unloaded afterwards.
        unsafe { (self.functions.close)(self.handle.as_ptr()) };
    }
}
fn utf16(text: &str) -> Result<Vec<u16>, Error> {
    let count = text.encode_utf16().count();
    i32::try_from(count).map_err(|_| Error("Text exceeds ICU's signed length limit".into()))?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Error("Cannot allocate ICU input text".into()))?;
    result.extend(text.encode_utf16());
    Ok(result)
}
fn checked_culture(value: &str) -> Result<String, Error> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(Error("Invalid collation culture".into()));
    }
    Ok(value.replace('_', "-"))
}
/// The editor intentionally follows the configured UI language. These neutral
/// bundled languages map to the same explicit cultures used by the oracle;
/// existing regional tags retain their region rather than being truncated.
fn culture_for_language(language: &str) -> Result<String, Error> {
    match language {
        "en" => Ok("en-US".into()),
        "fr" => Ok("fr-FR".into()),
        "sv" => Ok("sv-SE".into()),
        _ => checked_culture(language),
    }
}
#[cfg(target_os = "macos")]
fn candidates() -> Vec<String> {
    vec!["/usr/lib/libicucore.dylib".into()]
}
#[cfg(target_os = "linux")]
fn candidates() -> Vec<String> {
    std::iter::once("libicui18n.so".into())
        .chain(
            (50..=255)
                .rev()
                .map(|version| format!("libicui18n.so.{version}")),
        )
        .collect()
}
#[cfg(target_os = "windows")]
fn candidates() -> Vec<String> {
    ["icu.dll", "icuin.dll"]
        .into_iter()
        .map(str::to_owned)
        .chain(
            (50..=255)
                .rev()
                .map(|version| format!("icuin{version}.dll")),
        )
        .collect()
}
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn candidates() -> Vec<String> {
    Vec::new()
}
fn load_library(path: &str) -> Result<Library, libloading::Error> {
    #[cfg(target_os = "windows")]
    {
        // SAFETY: load ICU only from the Windows system directory, excluding
        // the working directory and other application-controlled search paths.
        unsafe { libloading::os::windows::Library::load_with_flags(path, 0x00000800) }
            .map(Into::into)
    }
    #[cfg(not(target_os = "windows"))]
    {
        // SAFETY: path is a fixed system ICU candidate. Linux resolves its
        // installed SONAME through the platform dynamic loader as .NET does.
        unsafe { Library::new(path) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadsync_editor::{
        document::{Document, Format},
        history::Session,
    };
    #[test]
    fn system_icu_matches_actual_dotnet_culture_pairs_and_sort_keys() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../deadsync-editor/tests/fixtures/grooveauthor/chart-order.reference.json"
        ))
        .unwrap();
        let strings: Vec<_> = fixture["strings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect();
        assert_eq!(fixture["cultures"].as_array().unwrap().len(), 3);
        for culture in fixture["cultures"].as_array().unwrap() {
            let name = culture["culture"].as_str().unwrap();
            let collator = Collator::open(name).unwrap_or_else(|error| panic!("{name}: {error}"));
            let keys: Vec<_> = strings
                .iter()
                .map(|text| collator.sort_key(text).unwrap())
                .collect();
            assert_eq!(collator.culture(), name);
            assert_ne!(collator.collation_version(), [0; 4]);
            for (a, left) in strings.iter().enumerate() {
                for (b, right) in strings.iter().enumerate() {
                    let expected = culture["text_comparisons"][a][b].as_i64().unwrap().cmp(&0);
                    assert_eq!(
                        collator.compare(left, right).unwrap(),
                        expected,
                        "{name} {left:?}/{right:?}, collation {:?}",
                        collator.collation_version()
                    );
                    assert_eq!(
                        keys[a].cmp(&keys[b]),
                        expected,
                        "sort key {name} {left:?}/{right:?}"
                    );
                }
            }
        }
    }
    #[test]
    fn configured_cultures_produce_contrasting_live_chart_order_without_source_edits() {
        let mut source = String::from(
            "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:kb7-single;#NOTES:1000000;",
        );
        for name in ["zebra", "Ångström", "éclair", ""] {
            source += &format!(
                "#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#CHARTNAME:{name};#NOTES:1000;"
            );
        }
        let mut session = Session::new(Document::parse(source.as_bytes(), Format::Ssc).unwrap());
        let snapshot = Snapshot::resident(&mut session).unwrap();
        assert_eq!(snapshot.source_indices(), [1, 2, 3, 4]);
        let identities = session.chart_ids().to_vec();
        for (language, expected) in [
            ("en", vec![2, 3, 1, 4]),
            ("fr", vec![2, 3, 1, 4]),
            ("sv", vec![3, 1, 2, 4]),
        ] {
            let collator = Collator::for_language(language).unwrap();
            assert_eq!(collator.ordered_snapshot(&snapshot).unwrap(), expected);
            assert_eq!(session.serialize().unwrap(), source.as_bytes());
            assert_eq!(session.chart_ids(), identities);
            assert!(!session.is_dirty());
        }
    }
    #[test]
    fn handles_are_independent_and_utf16_lengths_do_not_truncate_embedded_nuls() {
        let survivor = Collator::for_language("fr").unwrap();
        for _ in 0..16 {
            let other = Collator::for_language("sv").unwrap();
            assert_eq!(
                other.compare("Ångström", "zebra").unwrap(),
                Ordering::Greater
            );
        }
        assert_eq!(
            survivor.compare("Ångström", "zebra").unwrap(),
            Ordering::Less
        );
        assert_eq!(survivor.compare("a\0z", "a\0b").unwrap(), Ordering::Greater);
        assert_eq!(survivor.compare("a\u{301}", "á").unwrap(), Ordering::Equal);
        for language in ["", "fr;en", "../fr", "fr\0FR", "fr FR"] {
            assert!(Collator::for_language(language).is_err());
        }
        assert_eq!(culture_for_language("pt-BR").unwrap(), "pt-BR");
        assert_eq!(culture_for_language("fr_FR").unwrap(), "fr-FR");
        assert!(load_library("/deadsync-nonexistent-system-icu-library").is_err());
    }
}
/// Profile ordering uses source names+abbreviations and the host's real ICU.
pub(crate) struct NativeProfileOrder;
impl deadsync_editor::generation::profile_catalog::Orderer for NativeProfileOrder {
    fn order(
        &mut self,
        catalog: &deadsync_editor::generation::profile_catalog::Catalog,
    ) -> Result<
        Vec<(
            deadsync_editor::generation::profiles::Kind,
            Vec<(deadsync_editor::generation::metadata::Guid, String)>,
        )>,
        deadsync_editor::Error,
    > {
        let locale = deadsync_theme_simply_love::i18n::current_locale();
        let collator = Collator::for_language(&locale).map_err(profile_error)?;
        let mut result = Vec::new();
        for kind in deadsync_editor::generation::profile_catalog::KINDS {
            let mut rows = catalog
                .entries(kind)
                .iter()
                .map(|entry| {
                    let text = entry.source_text(&mut |number| {
                        collator.fixed2(number).map_err(profile_error)
                    })?;
                    let key = collator.sort_key(&text).map_err(profile_error)?;
                    Ok((entry.builtin.is_none(), key, entry.guid.clone(), text))
                })
                .collect::<Result<Vec<_>, deadsync_editor::Error>>()?;
            rows.sort_by(|a, b| (&a.0, &a.1, &a.2).cmp(&(&b.0, &b.1, &b.2)));
            result.push((
                kind,
                rows.into_iter()
                    .map(|(_, _, id, text)| (id, text))
                    .collect(),
            ));
        }
        Ok(result)
    }
}
fn profile_error(error: Error) -> deadsync_editor::Error {
    deadsync_editor::Error::Unsupported(error.to_string())
}
type NumberOpen =
    unsafe extern "C" fn(i32, *const u16, i32, *const c_char, *mut c_void, *mut i32) -> *mut c_void;
type NumberClose = unsafe extern "C" fn(*mut c_void);
type NumberAttribute = unsafe extern "C" fn(*mut c_void, i32, i32);
type NumberFormat =
    unsafe extern "C" fn(*const c_void, f64, *mut u16, i32, *mut c_void, *mut i32) -> i32;
impl Collator {
    fn fixed2(&self, number: f64) -> Result<String, Error> {
        if !number.is_finite() {
            return Err(Error("Nonfinite profile summary value".into()));
        }
        let locale = CString::new(self.culture.as_str())
            .map_err(|_| Error("Invalid summary culture".into()))?;
        let mut status = 0;
        // SAFETY: exact ICU C ABI; formatter is confined to this call and closed on every path.
        let handle = unsafe {
            (self.functions.number_open)(
                1,
                std::ptr::null(),
                0,
                locale.as_ptr(),
                std::ptr::null_mut(),
                &mut status,
            )
        };
        if handle.is_null() || status > 0 {
            if !handle.is_null() {
                unsafe {
                    (self.functions.number_close)(handle);
                }
            }
            return Err(Error(format!("ICU number formatter open failed: {status}")));
        }
        struct OwnedNumber {
            handle: *mut c_void,
            close: NumberClose,
        }
        impl Drop for OwnedNumber {
            fn drop(&mut self) {
                unsafe {
                    (self.close)(self.handle);
                }
            }
        }
        let handle = OwnedNumber {
            handle,
            close: self.functions.number_close,
        };
        unsafe {
            (self.functions.number_attribute)(handle.handle, 1, 0);
            (self.functions.number_attribute)(handle.handle, 6, 2);
            (self.functions.number_attribute)(handle.handle, 7, 2);
            (self.functions.number_attribute)(handle.handle, 11, 4);
        }
        let mut buffer = vec![0u16; 512];
        status = 0;
        let length = unsafe {
            (self.functions.number_format)(
                handle.handle,
                number,
                buffer.as_mut_ptr(),
                buffer.len() as i32,
                std::ptr::null_mut(),
                &mut status,
            )
        };
        if status > 0 || length < 0 || length as usize > buffer.len() {
            return Err(Error(format!(
                "ICU profile summary formatting failed: {status}"
            )));
        }
        String::from_utf16(&buffer[..length as usize])
            .map_err(|_| Error("ICU returned invalid UTF16 summary".into()))
    }
}
#[cfg(test)]
mod profile_tests {
    use super::*;
    #[test]
    fn profile_fixed2_uses_real_host_number_formats_and_current_culture_order() {
        for (culture, number) in [("en-US", "1.25"), ("fr-FR", "1,25"), ("sv-SE", "1,25")] {
            let collator = Collator::open(culture).unwrap();
            assert_eq!(collator.fixed2(1.25).unwrap(), number);
            assert_eq!(
                collator.fixed2(1234.5).unwrap(),
                if culture != "en-US" {
                    "1234,50"
                } else {
                    "1234.50"
                }
            );
            let strings = ["zebra", "éclair", "Éclair", "eclair", " Ångström "];
            let mut keys = strings
                .iter()
                .map(|text| (*text, collator.sort_key(text).unwrap()))
                .collect::<Vec<_>>();
            keys.sort_by(|a, b| a.1.cmp(&b.1));
            for adjacent in keys.windows(2) {
                assert_ne!(
                    collator.compare(adjacent[0].0, adjacent[1].0).unwrap(),
                    std::cmp::Ordering::Greater
                );
            }
        }
    }
}
