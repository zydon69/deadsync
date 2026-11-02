//! Pulse objects and their mainloop are confined to the render worker. The
//! async API permits querying negotiated attributes before startup is ACKed.
use super::{PaBufferAttr, PaSampleSpec, PulseOutputPrep, load_library, load_symbol};
use libloading::Library;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr;
use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

type Handle = *mut c_void;
struct Api {
    _library: Library,
    mainloop_new: unsafe extern "C" fn() -> Handle,
    mainloop_free: unsafe extern "C" fn(Handle),
    mainloop_get_api: unsafe extern "C" fn(Handle) -> Handle,
    mainloop_iterate: unsafe extern "C" fn(Handle, c_int, *mut c_int) -> c_int,
    context_new: unsafe extern "C" fn(Handle, *const c_char) -> Handle,
    context_connect: unsafe extern "C" fn(Handle, *const c_char, u32, *const c_void) -> c_int,
    context_get_state: unsafe extern "C" fn(*const c_void) -> c_int,
    context_errno: unsafe extern "C" fn(*const c_void) -> c_int,
    context_disconnect: unsafe extern "C" fn(Handle),
    context_unref: unsafe extern "C" fn(Handle),
    stream_new:
        unsafe extern "C" fn(Handle, *const c_char, *const PaSampleSpec, *const c_void) -> Handle,
    stream_connect_playback: unsafe extern "C" fn(
        Handle,
        *const c_char,
        *const PaBufferAttr,
        u32,
        *const c_void,
        Handle,
    ) -> c_int,
    stream_get_state: unsafe extern "C" fn(*const c_void) -> c_int,
    stream_get_buffer_attr: unsafe extern "C" fn(*const c_void) -> *const PaBufferAttr,
    stream_get_sample_spec: unsafe extern "C" fn(*const c_void) -> *const PaSampleSpec,
    stream_writable_size: unsafe extern "C" fn(*const c_void) -> usize,
    stream_write: unsafe extern "C" fn(
        Handle,
        *const c_void,
        usize,
        Option<unsafe extern "C" fn(Handle)>,
        i64,
        c_int,
    ) -> c_int,
    stream_get_latency: unsafe extern "C" fn(Handle, *mut u64, *mut c_int) -> c_int,
    stream_disconnect: unsafe extern "C" fn(Handle) -> c_int,
    stream_unref: unsafe extern "C" fn(Handle),
    strerror: unsafe extern "C" fn(c_int) -> *const c_char,
}
static API: OnceLock<Result<Api, String>> = OnceLock::new();
fn api() -> Result<&'static Api, String> {
    API.get_or_init(|| {
        let library = load_library(&["libpulse.so.0", "libpulse.so"])?;
        // SAFETY: Symbol types below match the PulseAudio C API declarations.
        // Api retains the loaded Library for the lifetime of every function pointer.
        unsafe {
            Ok(Api {
                mainloop_new: load_symbol(&library, b"pa_mainloop_new\0")?,
                mainloop_free: load_symbol(&library, b"pa_mainloop_free\0")?,
                mainloop_get_api: load_symbol(&library, b"pa_mainloop_get_api\0")?,
                mainloop_iterate: load_symbol(&library, b"pa_mainloop_iterate\0")?,
                context_new: load_symbol(&library, b"pa_context_new\0")?,
                context_connect: load_symbol(&library, b"pa_context_connect\0")?,
                context_get_state: load_symbol(&library, b"pa_context_get_state\0")?,
                context_errno: load_symbol(&library, b"pa_context_errno\0")?,
                context_disconnect: load_symbol(&library, b"pa_context_disconnect\0")?,
                context_unref: load_symbol(&library, b"pa_context_unref\0")?,
                stream_new: load_symbol(&library, b"pa_stream_new\0")?,
                stream_connect_playback: load_symbol(&library, b"pa_stream_connect_playback\0")?,
                stream_get_state: load_symbol(&library, b"pa_stream_get_state\0")?,
                stream_get_buffer_attr: load_symbol(&library, b"pa_stream_get_buffer_attr\0")?,
                stream_get_sample_spec: load_symbol(&library, b"pa_stream_get_sample_spec\0")?,
                stream_writable_size: load_symbol(&library, b"pa_stream_writable_size\0")?,
                stream_write: load_symbol(&library, b"pa_stream_write\0")?,
                stream_get_latency: load_symbol(&library, b"pa_stream_get_latency\0")?,
                stream_disconnect: load_symbol(&library, b"pa_stream_disconnect\0")?,
                stream_unref: load_symbol(&library, b"pa_stream_unref\0")?,
                strerror: load_symbol(&library, b"pa_strerror\0")?,
                _library: library,
            })
        }
    })
    .as_ref()
    .map_err(Clone::clone)
}
pub(super) fn available() -> bool {
    api().is_ok()
}

pub(super) struct PulseConnection {
    api: &'static Api,
    mainloop: Handle,
    context: Handle,
    stream: Handle,
    pub(super) period_frames: u32,
    pub(super) buffer_frames: u32,
}
impl PulseConnection {
    pub(super) fn open(prep: &PulseOutputPrep, stop: &AtomicBool) -> Result<Self, String> {
        let api = api()?;
        // No handle escapes this thread; Drop unwinds every partially built state.
        let mut connection = Self {
            api,
            mainloop: ptr::null_mut(),
            context: ptr::null_mut(),
            stream: ptr::null_mut(),
            period_frames: 0,
            buffer_frames: 0,
        };
        // SAFETY: mainloop_new has no arguments; its nullable owned result is checked below.
        unsafe {
            connection.mainloop = (api.mainloop_new)();
        }
        if connection.mainloop.is_null() {
            return Err("Pulse mainloop allocation failed".into());
        }
        // SAFETY: mainloop is non-null and owned here; the static C name outlives context_new.
        unsafe {
            connection.context = (api.context_new)(
                (api.mainloop_get_api)(connection.mainloop),
                c"deadsync".as_ptr(),
            );
        }
        if connection.context.is_null() {
            return Err("Pulse context allocation failed".into());
        }
        // SAFETY: context is live; null server/API arguments request Pulse defaults.
        if unsafe { (api.context_connect)(connection.context, ptr::null(), 0, ptr::null()) } < 0 {
            return Err(connection.error("context connect"));
        }
        connection.wait_for(
            stop,
            // SAFETY: the live context is confined to this mainloop worker.
            || unsafe { (api.context_get_state)(connection.context) },
            4,
            5,
        )?;
        let spec = PaSampleSpec {
            format: 3,
            rate: prep.sample_rate_hz,
            channels: prep.channels as u8,
        };
        // SAFETY: context is ready; spec is a valid borrowed sample specification copied by Pulse.
        unsafe {
            connection.stream =
                (api.stream_new)(connection.context, c"Gameplay".as_ptr(), &spec, ptr::null());
        }
        if connection.stream.is_null() {
            return Err(connection.error("stream allocation"));
        }
        let attr = PaBufferAttr {
            maxlength: u32::MAX,
            tlength: super::frames_to_bytes(prep.buffer_frames, prep.channels),
            prebuf: u32::MAX,
            minreq: super::frames_to_bytes(prep.period_frames, prep.channels),
            fragsize: u32::MAX,
        };
        // Auto timing + interpolation + requested latency; no global server mutation.
        // SAFETY: stream is live, attr is valid, and null optional arguments request defaults.
        if unsafe {
            (api.stream_connect_playback)(
                connection.stream,
                ptr::null(),
                &attr,
                0x200a,
                ptr::null(),
                ptr::null_mut(),
            )
        } < 0
        {
            return Err(connection.error("stream connect"));
        }
        connection.wait_for(
            stop,
            // SAFETY: stream is live and all mainloop access is on this worker.
            || unsafe { (api.stream_get_state)(connection.stream) },
            2,
            3,
        )?;
        // SAFETY: ready stream owns this borrowed spec until it is disconnected; no mutation intervenes.
        let actual_spec = unsafe { (api.stream_get_sample_spec)(connection.stream).as_ref() }
            .ok_or_else(|| "Pulse negotiated sample specification missing".to_string())?;
        if actual_spec.format != spec.format
            || actual_spec.rate != spec.rate
            || actual_spec.channels != spec.channels
        {
            return Err("Pulse negotiated output format changed".into());
        }
        // SAFETY: ready stream owns these attributes; the optional pointer is checked before access.
        let actual = unsafe { (api.stream_get_buffer_attr)(connection.stream).as_ref() }
            .ok_or_else(|| "Pulse negotiated buffer attributes missing".to_string())?;
        let (period, total) = checked_attributes(actual, prep.channels)?;
        connection.period_frames = period;
        connection.buffer_frames = total;
        Ok(connection)
    }
    fn error(&self, operation: &str) -> String {
        // SAFETY: error() is used only after a non-null context has been constructed.
        let code = unsafe { (self.api.context_errno)(self.context) };
        // SAFETY: strerror accepts any Pulse error code and returns static or null storage.
        let text = unsafe { (self.api.strerror)(code) };
        let detail = if text.is_null() {
            "unknown error".into()
        } else {
            // SAFETY: non-null pa_strerror strings are static, NUL-terminated C strings.
            unsafe { CStr::from_ptr(text) }.to_string_lossy()
        };
        format!("Pulse {operation} failed: {detail}")
    }
    fn iterate(&self) -> Result<(), String> {
        let mut result = 0;
        // SAFETY: mainloop is live and worker-confined; result is valid writable storage.
        if unsafe { (self.api.mainloop_iterate)(self.mainloop, 0, &mut result) } < 0 {
            return Err(self.error("mainloop"));
        }
        Ok(())
    }
    fn wait_for(
        &self,
        stop: &AtomicBool,
        state: impl Fn() -> c_int,
        ready: c_int,
        first_failed: c_int,
    ) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if stop.load(Ordering::Acquire) {
                return Err("Pulse startup cancelled".into());
            }
            self.iterate()?;
            let current = state();
            if current == ready {
                return Ok(());
            }
            if current >= first_failed {
                return Err(self.error("startup state"));
            }
            if Instant::now() >= deadline {
                return Err("Pulse startup timed out".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    pub(super) fn write_i16(&self, data: &[i16], stop: &AtomicBool) -> Result<(), String> {
        let bytes = std::mem::size_of_val(data);
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if stop.load(Ordering::Acquire) {
                return Ok(());
            }
            self.iterate()?;
            // SAFETY: self owns a live stream; mainloop access remains worker-confined.
            if unsafe { (self.api.stream_get_state)(self.stream) } != 2 {
                return Err(self.error("write state"));
            }
            // SAFETY: stream is ready; this query borrows it without retaining Rust pointers.
            let writable = unsafe { (self.api.stream_writable_size)(self.stream) };
            if writable == usize::MAX {
                return Err(self.error("writable query"));
            }
            if writable >= bytes {
                break;
            }
            if Instant::now() >= deadline {
                return Err("Pulse writable wait timed out".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        // SAFETY: data covers exactly bytes readable bytes. A null free callback
        // asks Pulse to copy samples before returning; no borrowed slice escapes.
        if unsafe { (self.api.stream_write)(self.stream, data.as_ptr().cast(), bytes, None, 0, 0) }
            < 0
        {
            return Err(self.error("write"));
        }
        Ok(())
    }
    pub(super) fn latency_nanos(&self) -> Result<u64, String> {
        self.iterate()?;
        let mut micros = 0;
        let mut negative = 0;
        // SAFETY: stream is live; both output pointers refer to initialized writable locals.
        if unsafe { (self.api.stream_get_latency)(self.stream, &mut micros, &mut negative) } < 0 {
            return Err(self.error("latency"));
        }
        if negative != 0 {
            return Err("Pulse reports negative output latency".into());
        }
        micros
            .checked_mul(1000)
            .ok_or_else(|| "Pulse latency overflow".into())
    }
}
impl Drop for PulseConnection {
    fn drop(&mut self) {
        // SAFETY: non-null handles are exclusively owned and freed once, on the
        // allocation worker, in stream/context/mainloop dependency order. Api keeps
        // the shared library loaded throughout every destructor call.
        unsafe {
            if !self.stream.is_null() {
                (self.api.stream_disconnect)(self.stream);
                (self.api.stream_unref)(self.stream);
            }
            if !self.context.is_null() {
                (self.api.context_disconnect)(self.context);
                (self.api.context_unref)(self.context);
            }
            if !self.mainloop.is_null() {
                (self.api.mainloop_free)(self.mainloop);
            }
        }
    }
}

fn checked_attributes(attr: &PaBufferAttr, channels: usize) -> Result<(u32, u32), String> {
    let frame_bytes = u32::try_from(channels)
        .ok()
        .and_then(|channels| channels.checked_mul(2))
        .filter(|bytes| *bytes != 0)
        .ok_or_else(|| "invalid Pulse channel count".to_string())?;
    if attr.minreq == 0
        || attr.minreq == u32::MAX
        || attr.tlength == u32::MAX
        || attr.minreq % frame_bytes != 0
        || attr.tlength % frame_bytes != 0
    {
        return Err("Pulse negotiated attributes are missing or not frame aligned".into());
    }
    let period = attr.minreq / frame_bytes;
    let total = attr.tlength / frame_bytes;
    if period > 8192 || total < period {
        return Err("Pulse negotiated attributes exceed render quantum bounds".into());
    }
    Ok((period, total))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn attr(minreq: u32, tlength: u32) -> PaBufferAttr {
        PaBufferAttr {
            minreq,
            tlength,
            maxlength: u32::MAX,
            prebuf: 0,
            fragsize: u32::MAX,
        }
    }
    #[test]
    fn negotiated_attributes_are_read_not_reconstructed_from_requests() {
        assert_eq!(
            checked_attributes(&attr(1024, 6144), 2).unwrap(),
            (256, 1536)
        );
        assert!(checked_attributes(&attr(u32::MAX, 6144), 2).is_err());
        assert!(checked_attributes(&attr(1023, 6144), 2).is_err());
        assert!(checked_attributes(&attr(0, 6144), 2).is_err());
        assert!(checked_attributes(&attr(1024, 512), 2).is_err());
    }
}
