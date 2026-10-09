//! Android: AAudio through the raw `ndk-sys` bindings (`docs/adr/0020`).
//!
//! AAudio has no duplex stream. As Oboe's `FullDuplexStream` does, this opens an output stream
//! whose data callback drives everything, plus an input stream with no callback that the output
//! callback drains with a non-blocking read. The output side's frame count is the stream clock, so
//! input and output share one clock by construction (`docs/PLATFORM_AUDIO.md` §1).

use std::ffi::c_void;
use std::ptr::{self, NonNull};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering, fence};

use ndk_sys as aaudio;

use crate::{
    AudioBackend, AudioCallback, AudioError, BackendReport, CallbackInfo, GrantedPath, InputPreset,
    Result, SAMPLE_RATES, StreamConfig, StreamHandle, StreamTimestamp,
};

/// How long `close` waits for AAudio to confirm a stop before closing anyway.
const STOP_TIMEOUT_NS: i64 = 500_000_000;

/// Input reads discarded on the first callback, at most. The microphone starts before the speaker,
/// so a backlog builds up that would otherwise sit between them as latency.
const MAX_DRAIN_READS: usize = 32;

impl InputPreset {
    fn to_raw(self) -> aaudio::aaudio_input_preset_t {
        match self {
            Self::Unprocessed => raw(aaudio::AAUDIO_INPUT_PRESET_UNPROCESSED),
            Self::VoiceRecognition => raw(aaudio::AAUDIO_INPUT_PRESET_VOICE_RECOGNITION),
            Self::Other(code) => code,
        }
    }

    fn from_raw(code: aaudio::aaudio_input_preset_t) -> Self {
        if code == raw(aaudio::AAUDIO_INPUT_PRESET_UNPROCESSED) {
            Self::Unprocessed
        } else if code == raw(aaudio::AAUDIO_INPUT_PRESET_VOICE_RECOGNITION) {
            Self::VoiceRecognition
        } else {
            Self::Other(code)
        }
    }
}

impl GrantedPath {
    fn of(stream: *mut aaudio::AAudioStream) -> Self {
        // SAFETY: called only with open streams. `get*` calls are thread-safe in AAudio.
        let (mode, sharing) = unsafe {
            (
                aaudio::AAudioStream_getPerformanceMode(stream),
                aaudio::AAudioStream_getSharingMode(stream),
            )
        };
        Self {
            low_latency: mode == raw(aaudio::AAUDIO_PERFORMANCE_MODE_LOW_LATENCY),
            exclusive: sharing == raw(aaudio::AAUDIO_SHARING_MODE_EXCLUSIVE),
        }
    }
}

/// The production Android backend: one duplex stream over AAudio (`docs/adr/0020`).
///
/// Every stream runs at the device's native sample rate. The rate in the requested
/// [`StreamConfig`] is ignored, because asking for any other rate is the most common way to lose
/// the low-latency path (`docs/PLATFORM_AUDIO.md` §2). [`actual_config`](AudioBackend::actual_config)
/// reports the rate granted, and the callback is told it on every block.
pub struct AAudioBackend {
    preset: InputPreset,
    stream: Option<Running>,
}

impl AAudioBackend {
    /// A closed backend that will open the microphone with `preset`.
    #[must_use]
    pub fn new(preset: InputPreset) -> Self {
        Self {
            preset,
            stream: None,
        }
    }

    /// The input preset the device actually applied, which may not be the one requested. `None`
    /// when closed or when the stream has no input.
    #[must_use]
    pub fn obtained_input_preset(&self) -> Option<InputPreset> {
        self.stream.as_ref().and_then(|running| running.preset)
    }

    /// The output stream's burst: the smallest block the device moves at once. `None` when closed.
    #[must_use]
    pub fn frames_per_burst(&self) -> Option<u32> {
        let running = self.stream.as_ref()?;
        // SAFETY: the output stream is open for as long as `running` exists.
        let burst = unsafe { aaudio::AAudioStream_getFramesPerBurst(running.output.ptr()) };
        u32::try_from(burst).ok()
    }

    /// The path the output and, if there is one, the input stream were given. `None` when closed.
    #[must_use]
    pub fn granted_paths(&self) -> Option<(GrantedPath, Option<GrantedPath>)> {
        let running = self.stream.as_ref()?;
        let input = running.input.map(|input| GrantedPath::of(input.as_ptr()));
        Some((GrantedPath::of(running.output.ptr()), input))
    }

    /// Output underruns and input overruns the platform has counted since the stream opened.
    /// `None` when closed. Read on the control side; AAudio counts them for us.
    #[must_use]
    pub fn xruns(&self) -> Option<u32> {
        let running = self.stream.as_ref()?;
        // SAFETY: both streams are open for as long as `running` exists. getXRunCount only reads
        // a counter, so calling it beside the audio thread is allowed.
        let output = unsafe { aaudio::AAudioStream_getXRunCount(running.output.ptr()) };
        let input = running.input.map_or(0, |input| {
            // SAFETY: as above; `input` is owned by the callback state, which outlives `running`'s
            // use here because only `close` frees it.
            unsafe { aaudio::AAudioStream_getXRunCount(input.as_ptr()) }
        });
        u32::try_from(output.max(0))
            .ok()?
            .checked_add(u32::try_from(input.max(0)).ok()?)
    }
}

impl Drop for AAudioBackend {
    fn drop(&mut self) {
        // Errors cannot be reported from a drop. `close` is the way to see them.
        let _ = self.close();
    }
}

impl AudioBackend for AAudioBackend {
    fn name(&self) -> &'static str {
        "aaudio"
    }

    fn open(
        &mut self,
        config: StreamConfig,
        callback: Box<dyn AudioCallback>,
    ) -> Result<StreamHandle> {
        if self.stream.is_some() {
            return Err(AudioError::AlreadyOpen);
        }
        config.validate()?;
        let running = Running::open(config, callback, self.preset)?;
        let handle = running.shared.handle.clone();
        self.stream = Some(running);
        Ok(handle)
    }

    fn actual_config(&self) -> Option<StreamConfig> {
        self.stream.as_ref().map(|running| running.config)
    }

    fn timestamp(&self) -> Option<StreamTimestamp> {
        self.stream
            .as_ref()
            .and_then(|running| running.shared.stamp.read())
    }

    fn report(&self) -> BackendReport {
        let paths = self.granted_paths();
        BackendReport {
            xruns: self.xruns(),
            frames_per_burst: self.frames_per_burst(),
            input_preset: self.obtained_input_preset(),
            output_path: paths.map(|(output, _)| output),
            input_path: paths.and_then(|(_, input)| input),
        }
    }

    fn close(&mut self) -> Result<()> {
        match self.stream.take() {
            Some(mut running) => running.shutdown(),
            None => Ok(()),
        }
    }
}

/// One open duplex stream and everything its callbacks point at.
struct Running {
    /// Closed before `state` is freed, so no callback can still see it. A placeholder until open
    /// succeeds, guarded by `closed`.
    output: Stream,
    /// The data callback's state, leaked to AAudio as its user data. Freed by `shutdown`.
    state: Option<NonNull<CallbackState>>,
    /// The input stream, owned by `state`. Kept here only to read its counters.
    input: Option<NonNull<aaudio::AAudioStream>>,
    /// What the error callback points at. Outlives the output stream, which is closed first.
    shared: Arc<Shared>,
    config: StreamConfig,
    preset: Option<InputPreset>,
    closed: bool,
}

// SAFETY: `Running` is not `Send` only because of its two raw pointers.
// - `state` is dereferenced only in `shutdown`, after the output stream that calls into it has
//   been closed, so moving `Running` to another thread cannot race the audio thread.
// - `input` is used only for `AAudioStream_get*` calls, which AAudio documents as thread-safe.
// Everything else it holds (`Stream`, `Arc<Shared>`, plain data) is `Send`.
unsafe impl Send for Running {}

impl Running {
    fn open(
        requested: StreamConfig,
        callback: Box<dyn AudioCallback>,
        preset: InputPreset,
    ) -> Result<Self> {
        let shared = Arc::new(Shared::default());
        let input_samples = requested.max_block_frames * requested.input_channels;
        let state = NonNull::from(Box::leak(Box::new(CallbackState {
            callback,
            input: None,
            input_buffer: vec![0.0; input_samples].into_boxed_slice(),
            config: requested,
            shared: Arc::clone(&shared),
            next_frame: 0,
            last_host_ns: 0,
            drained: false,
        })));
        // From here, `state` is freed on every error path by dropping the half-built `Running`.
        let mut running = Self {
            output: Stream::dangling(),
            state: Some(state),
            input: None,
            shared,
            config: requested,
            preset: None,
            closed: true,
        };

        // Output first: its granted rate is the rate the input has to match.
        let output = Builder::new()?
            .direction(aaudio::AAUDIO_DIRECTION_OUTPUT)
            .common(requested.output_channels)?
            .usage()
            .data_callback(state)
            .error_callback(&running.shared)
            .open("open the output stream")?;
        running.output = output;
        running.closed = false;
        let rate = granted_rate(&running.output)?;
        expect_channels(
            &running.output,
            requested.output_channels,
            "the speaker refused the requested channel count",
        )?;
        // Two bursts: the smallest buffer that survives one late callback without an underrun.
        // SAFETY: the output stream is open.
        let burst = unsafe { aaudio::AAudioStream_getFramesPerBurst(running.output.ptr()) };
        // SAFETY: as above. A refused size leaves AAudio's own default, which is still valid.
        unsafe { aaudio::AAudioStream_setBufferSizeInFrames(running.output.ptr(), burst * 2) };

        let input = if requested.input_channels == 0 {
            None
        } else {
            let input = Builder::new()?
                .direction(aaudio::AAUDIO_DIRECTION_INPUT)
                .common(requested.input_channels)?
                .sample_rate(rate)?
                .input_preset(preset)
                .error_callback(&running.shared)
                .open("open the input stream")?;
            expect_channels(
                &input,
                requested.input_channels,
                "the microphone refused the requested channel count",
            )?;
            if granted_rate(&input)? != rate {
                return Err(AudioError::UnsupportedConfiguration(
                    "the microphone cannot run at the speaker's rate".to_owned(),
                ));
            }
            // SAFETY: the input stream is open.
            let obtained = unsafe { aaudio::AAudioStream_getInputPreset(input.ptr()) };
            running.preset = Some(InputPreset::from_raw(obtained));
            running.input = Some(input.non_null());
            Some(input)
        };

        let granted = StreamConfig {
            sample_rate: rate,
            ..requested
        };
        running.config = granted;
        // SAFETY: no callback runs before `requestStart` below, so this thread still has the only
        // access to `state`.
        let callback_state = unsafe { &mut *state.as_ptr() };
        callback_state.config = granted;
        callback_state.input = input;

        // The microphone starts first, so the first output callback already has input to read.
        if let Some(input) = &callback_state.input {
            // SAFETY: the input stream is open and not yet started.
            check("start the input stream", unsafe {
                aaudio::AAudioStream_requestStart(input.ptr())
            })?;
        }
        // SAFETY: the output stream is open; its callback state is fully built.
        check("start the output stream", unsafe {
            aaudio::AAudioStream_requestStart(running.output.ptr())
        })?;
        Ok(running)
    }

    /// Stop and close both streams, then free the callback state. Safe to call twice.
    ///
    /// The order is what makes it sound: the output stream is stopped and closed first, so no data
    /// callback can be running when the input stream it reads is closed and its state is freed.
    fn shutdown(&mut self) -> Result<()> {
        let mut result = Ok(());
        let mut output_closed = true;
        if !self.closed {
            self.closed = true;
            result = stop(&self.output, "stop the output stream");
            let close = self.output.close();
            output_closed = close.is_ok();
            result = result.and(close);
        }
        let Some(state) = self.state.take() else {
            return result;
        };
        if !output_closed {
            // AAudio refuses to close a stream it is still calling back from, so a failed close
            // means the callback may yet run. Freeing its state would be a use-after-free;
            // leaking it, input stream included, is the safe failure.
            self.input = None;
            return result;
        }
        // SAFETY: `state` came from `Box::leak` in `open`, and the output stream that called into
        // it is closed, so this is the only reference left.
        let mut state = unsafe { Box::from_raw(state.as_ptr()) };
        self.input = None;
        if let Some(mut input) = state.input.take() {
            result = result.and(stop(&input, "stop the input stream"));
            result = result.and(input.close());
        }
        // Dropping the state drops the callback, as the `close` contract requires.
        drop(state);
        result
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

/// What the data callback owns. Touched only by the audio thread once the stream starts.
struct CallbackState {
    callback: Box<dyn AudioCallback>,
    input: Option<Stream>,
    /// `max_block_frames * input_channels` samples, allocated at open.
    input_buffer: Box<[f32]>,
    config: StreamConfig,
    shared: Arc<Shared>,
    next_frame: u64,
    last_host_ns: u64,
    drained: bool,
}

impl CallbackState {
    /// One AAudio callback: cut into blocks of at most `max_block_frames`, each handed to the
    /// engine with its slice of the microphone. Real-time thread: no allocation or panic.
    ///
    /// `output_stream` is the stream that called back. Only this thread may ask it for a
    /// timestamp, because `getTimestamp` is the one AAudio getter that is not thread-safe.
    fn render(&mut self, output_stream: *mut aaudio::AAudioStream, output: &mut [f32]) {
        let started_ns = now_ns();
        let StreamConfig {
            sample_rate,
            max_block_frames,
            input_channels,
            output_channels,
        } = self.config;
        // The first callback also drains the microphone's backlog, so its duration says nothing
        // about steady state and is left out of the worst case.
        let first = !self.drained;
        if first {
            self.drained = true;
            if let Some(input) = &self.input {
                drain(
                    input,
                    &mut self.input_buffer,
                    max_block_frames,
                    &self.shared.handle,
                );
            }
        }
        let anchor = presentation_anchor(output_stream);

        let mut offset_frames: u64 = 0;
        for block in output.chunks_mut(max_block_frames * output_channels) {
            let frames = block.len() / output_channels;
            let input = &mut self.input_buffer[..frames * input_channels];
            if let Some(stream) = &self.input {
                match read_input(stream, input, frames) {
                    Input::Full => {}
                    Input::Short => self.shared.handle.record_input_underrun(),
                    Input::Broken => self.shared.handle.mark_disconnected(),
                }
            }
            // When the block's first frame reaches the speaker (AUDIO_ENGINE.md §6), extrapolated
            // from the frame AAudio last reported as presented. Until it has presented one, the
            // best estimate is when the block is rendered. Kept strictly rising either way.
            let host_time_ns = anchor
                .map_or_else(
                    || started_ns.saturating_add(frames_to_ns(offset_frames, sample_rate)),
                    |anchor| anchor.presented_at(self.next_frame, sample_rate),
                )
                .max(self.last_host_ns.saturating_add(1));
            let timestamp = StreamTimestamp {
                frame: self.next_frame,
                host_time_ns,
            };
            let info = CallbackInfo {
                frames,
                sample_rate,
                input_channels,
                output_channels,
                timestamp,
            };
            self.callback.process(input, block, &info);
            self.shared.handle.record(frames);
            self.shared.stamp.publish(timestamp);
            self.next_frame += frames as u64;
            self.last_host_ns = host_time_ns;
            offset_frames += frames as u64;
        }
        if !first {
            self.shared
                .handle
                .record_duration(now_ns().saturating_sub(started_ns));
        }
    }
}

/// A frame of the output stream and the host time at which it reached the speaker.
#[derive(Clone, Copy)]
struct Anchor {
    frame: i64,
    presented_ns: i64,
}

impl Anchor {
    /// When `frame` reaches the speaker, by extrapolating from this anchor at `sample_rate`.
    fn presented_at(self, frame: u64, sample_rate: u32) -> u64 {
        let ahead = i128::from(frame) - i128::from(self.frame);
        let ns =
            i128::from(self.presented_ns) + ahead * 1_000_000_000 / i128::from(sample_rate.max(1));
        u64::try_from(ns.max(0)).unwrap_or(u64::MAX)
    }
}

/// The output's latest presented frame, or `None` before it has presented any. Audio thread only.
fn presentation_anchor(stream: *mut aaudio::AAudioStream) -> Option<Anchor> {
    let (mut frame, mut presented_ns) = (0_i64, 0_i64);
    // SAFETY: `stream` is the open output stream that is calling back, and this is its callback
    // thread, the only one that calls `getTimestamp` on it. Both out-pointers are valid.
    let result = unsafe {
        aaudio::AAudioStream_getTimestamp(
            stream,
            libc::CLOCK_MONOTONIC,
            &raw mut frame,
            &raw mut presented_ns,
        )
    };
    (result == aaudio::AAUDIO_OK).then_some(Anchor {
        frame,
        presented_ns,
    })
}

/// What one non-blocking microphone read produced.
#[derive(PartialEq, Eq)]
enum Input {
    /// Every frame asked for.
    Full,
    /// Fewer frames than asked for; the rest is silence.
    Short,
    /// The input stream failed, typically because the microphone went away. All silence.
    Broken,
}

/// Read one block of microphone input without waiting, padding with silence whatever did not
/// arrive. A failed read is how the input stream reports that its device is gone, since its error
/// callback is not guaranteed to fire before the next read.
fn read_input(stream: &Stream, input: &mut [f32], frames: usize) -> Input {
    let wanted = i32::try_from(frames).unwrap_or(0);
    // SAFETY: `input` holds `frames` frames at the stream's channel count and float format, both
    // checked at open. A zero timeout makes AAudio return at once with whatever it has. Only the
    // audio thread reads this stream.
    let got =
        unsafe { aaudio::AAudioStream_read(stream.ptr(), input.as_mut_ptr().cast(), wanted, 0) };
    let Ok(got) = usize::try_from(got) else {
        input.fill(0.0);
        return Input::Broken;
    };
    let got = got.min(frames);
    if got == frames {
        return Input::Full;
    }
    let channels = input.len() / frames.max(1);
    input[got * channels..].fill(0.0);
    Input::Short
}

/// Discard whatever the microphone has buffered, so input reaches the callback with the least
/// delay. Bounded, because a device that delivers faster than this reads must not hang the stream.
/// The read that comes up short is where the backlog ended, so it is not an underrun.
fn drain(stream: &Stream, buffer: &mut [f32], max_block_frames: usize, handle: &StreamHandle) {
    for _ in 0..MAX_DRAIN_READS {
        match read_input(stream, buffer, max_block_frames) {
            Input::Full => {}
            Input::Short => return,
            Input::Broken => {
                handle.mark_disconnected();
                return;
            }
        }
    }
}

/// The AAudio data callback.
unsafe extern "C" fn on_audio(
    stream: *mut aaudio::AAudioStream,
    user_data: *mut c_void,
    audio_data: *mut c_void,
    num_frames: i32,
) -> aaudio::aaudio_data_callback_result_t {
    // SAFETY: `user_data` is the `CallbackState` leaked in `Running::open`. It is freed only after
    // the output stream has stopped and closed, and AAudio never runs two data callbacks at once,
    // so this is the only reference while it lives.
    let state = unsafe { &mut *user_data.cast::<CallbackState>() };
    let frames = usize::try_from(num_frames).unwrap_or(0);
    let samples = frames * state.config.output_channels;
    // SAFETY: AAudio hands an interleaved buffer of `num_frames` frames at the stream's channel
    // count, in the float format requested at open, valid for the length of this call.
    let output = unsafe { std::slice::from_raw_parts_mut(audio_data.cast::<f32>(), samples) };
    // Traps any allocation on this thread in debug builds; compiled out of release.
    assert_no_alloc::assert_no_alloc(|| state.render(stream, output));
    raw(aaudio::AAUDIO_CALLBACK_RESULT_CONTINUE)
}

/// The AAudio error callback, registered on both streams. AAudio may call it on its own thread or
/// on the data-callback thread; either way it only stores an atomic.
unsafe extern "C" fn on_error(
    _stream: *mut aaudio::AAudioStream,
    user_data: *mut c_void,
    _error: aaudio::aaudio_result_t,
) {
    // SAFETY: `user_data` is the `Shared` inside `Running::shared`'s `Arc`, which lives until after
    // both streams are closed, and AAudio stops calling back once a stream is.
    let shared = unsafe { &*user_data.cast::<Shared>() };
    // Any error means the stream has stopped for good. AAudio forbids reopening from here.
    shared.handle.mark_disconnected();
}

/// State read from other threads while the stream runs.
#[derive(Default)]
struct Shared {
    handle: StreamHandle,
    stamp: StampCell,
}

/// The latest block's timestamp, readable from any thread without tearing the pair apart.
///
/// A sequence lock: the audio thread is the only writer and never waits; a reader retries in the
/// rare case it raced a write. `seq` is zero until the first block, odd while one is being written.
#[derive(Default)]
struct StampCell {
    seq: AtomicU64,
    frame: AtomicU64,
    host_time_ns: AtomicU64,
}

impl StampCell {
    fn publish(&self, stamp: StreamTimestamp) {
        let seq = self.seq.load(Ordering::Relaxed);
        self.seq.store(seq.wrapping_add(1), Ordering::Relaxed);
        fence(Ordering::Release);
        self.frame.store(stamp.frame, Ordering::Relaxed);
        self.host_time_ns
            .store(stamp.host_time_ns, Ordering::Relaxed);
        self.seq.store(seq.wrapping_add(2), Ordering::Release);
    }

    /// Control side only. Spins briefly, then yields: if the audio thread was preempted
    /// mid-publish, it needs this core back to finish.
    fn read(&self) -> Option<StreamTimestamp> {
        for attempt in 0_u32.. {
            if attempt >= 64 {
                std::thread::yield_now();
            }
            let before = self.seq.load(Ordering::Acquire);
            if before == 0 {
                return None;
            }
            let stamp = StreamTimestamp {
                frame: self.frame.load(Ordering::Relaxed),
                host_time_ns: self.host_time_ns.load(Ordering::Relaxed),
            };
            fence(Ordering::Acquire);
            if before.is_multiple_of(2) && self.seq.load(Ordering::Relaxed) == before {
                return Some(stamp);
            }
            std::hint::spin_loop();
        }
        None
    }
}

/// An open AAudio stream with exactly one owner, closed when dropped.
struct Stream(NonNull<aaudio::AAudioStream>);

// SAFETY: an AAudio stream is not tied to the thread that opened it. Each `Stream` has one owner,
// and the calls AAudio does not allow concurrently (read, stop, close) are only made by that
// owner: the input stream by the audio thread, the output stream by the control side.
unsafe impl Send for Stream {}

impl Stream {
    /// A placeholder that is never passed to AAudio. `Running` holds one until its output opens,
    /// guarded by its `closed` flag.
    fn dangling() -> Self {
        Self(NonNull::dangling())
    }

    fn ptr(&self) -> *mut aaudio::AAudioStream {
        self.0.as_ptr()
    }

    fn non_null(&self) -> NonNull<aaudio::AAudioStream> {
        self.0
    }

    /// Close now and report the result, instead of leaving it to `Drop`, which cannot.
    fn close(&mut self) -> Result<()> {
        let stream = std::mem::replace(self, Self::dangling());
        // SAFETY: `stream` is open; replacing it with the placeholder means it is closed once.
        let result = unsafe { aaudio::AAudioStream_close(stream.ptr()) };
        std::mem::forget(stream);
        check("close the stream", result)
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        if self.0 != NonNull::dangling() {
            // SAFETY: a non-placeholder `Stream` is an open stream with no other owner.
            unsafe { aaudio::AAudioStream_close(self.ptr()) };
        }
    }
}

/// A stream builder, deleted when dropped.
struct Builder(NonNull<aaudio::AAudioStreamBuilder>);

impl Builder {
    fn new() -> Result<Self> {
        let mut builder = ptr::null_mut();
        // SAFETY: AAudio writes a new builder through a valid out-pointer.
        check("create a stream builder", unsafe {
            aaudio::AAudio_createStreamBuilder(&raw mut builder)
        })?;
        NonNull::new(builder).map(Self).ok_or(AudioError::Platform {
            operation: "create a stream builder",
            code: 0,
        })
    }

    fn ptr(&self) -> *mut aaudio::AAudioStreamBuilder {
        self.0.as_ptr()
    }

    fn direction(self, direction: u32) -> Self {
        // SAFETY: the builder is live. Setters only record a request; `openStream` validates it.
        unsafe { aaudio::AAudioStreamBuilder_setDirection(self.ptr(), raw(direction)) };
        self
    }

    /// What every stream here shares: float samples, low latency, exclusive where the device
    /// allows it. AAudio falls back to shared mode by itself when exclusive is refused.
    fn common(self, channels: usize) -> Result<Self> {
        let channels = i32::try_from(channels)
            .map_err(|_| AudioError::InvalidConfig("channel count does not fit AAudio"))?;
        // SAFETY: the builder is live.
        unsafe {
            aaudio::AAudioStreamBuilder_setFormat(self.ptr(), aaudio::AAUDIO_FORMAT_PCM_FLOAT);
            aaudio::AAudioStreamBuilder_setChannelCount(self.ptr(), channels);
            aaudio::AAudioStreamBuilder_setPerformanceMode(
                self.ptr(),
                raw(aaudio::AAUDIO_PERFORMANCE_MODE_LOW_LATENCY),
            );
            aaudio::AAudioStreamBuilder_setSharingMode(
                self.ptr(),
                raw(aaudio::AAUDIO_SHARING_MODE_EXCLUSIVE),
            );
        }
        Ok(self)
    }

    fn usage(self) -> Self {
        // SAFETY: the builder is live.
        unsafe {
            aaudio::AAudioStreamBuilder_setUsage(self.ptr(), raw(aaudio::AAUDIO_USAGE_MEDIA));
            aaudio::AAudioStreamBuilder_setContentType(
                self.ptr(),
                raw(aaudio::AAUDIO_CONTENT_TYPE_MUSIC),
            );
        }
        self
    }

    fn sample_rate(self, rate: u32) -> Result<Self> {
        let rate = i32::try_from(rate)
            .map_err(|_| AudioError::InvalidConfig("sample rate does not fit AAudio"))?;
        // SAFETY: the builder is live.
        unsafe { aaudio::AAudioStreamBuilder_setSampleRate(self.ptr(), rate) };
        Ok(self)
    }

    fn input_preset(self, preset: InputPreset) -> Self {
        // SAFETY: the builder is live. API 28+, which is the floor (docs/adr/0019).
        unsafe { aaudio::AAudioStreamBuilder_setInputPreset(self.ptr(), preset.to_raw()) };
        self
    }

    fn data_callback(self, state: NonNull<CallbackState>) -> Self {
        // SAFETY: the builder is live, and `state` outlives every stream this builder opens
        // (`Running::shutdown` frees it only after closing the output stream).
        unsafe {
            aaudio::AAudioStreamBuilder_setDataCallback(
                self.ptr(),
                Some(on_audio),
                state.as_ptr().cast(),
            );
        };
        self
    }

    fn error_callback(self, shared: &Arc<Shared>) -> Self {
        // SAFETY: the builder is live, and `shared` is kept alive by `Running` until after the
        // output stream is closed. AAudio only reads through the pointer, as `&Shared`.
        unsafe {
            aaudio::AAudioStreamBuilder_setErrorCallback(
                self.ptr(),
                Some(on_error),
                Arc::as_ptr(shared).cast_mut().cast(),
            );
        };
        self
    }

    fn open(self, operation: &'static str) -> Result<Stream> {
        let mut stream = ptr::null_mut();
        // SAFETY: the builder is live, and AAudio writes the new stream through a valid
        // out-pointer.
        check(operation, unsafe {
            aaudio::AAudioStreamBuilder_openStream(self.ptr(), &raw mut stream)
        })?;
        NonNull::new(stream)
            .map(Stream)
            .ok_or(AudioError::Platform { operation, code: 0 })
    }
}

impl Drop for Builder {
    fn drop(&mut self) {
        // SAFETY: the builder is live and owned only by `self`. Streams it opened stay open.
        unsafe { aaudio::AAudioStreamBuilder_delete(self.ptr()) };
    }
}

fn granted_rate(stream: &Stream) -> Result<u32> {
    // SAFETY: the stream is open.
    let rate = unsafe { aaudio::AAudioStream_getSampleRate(stream.ptr()) };
    u32::try_from(rate)
        .ok()
        .filter(|rate| SAMPLE_RATES.contains(rate))
        .ok_or(AudioError::Platform {
            operation: "read the granted sample rate",
            code: rate,
        })
}

fn expect_channels(stream: &Stream, wanted: usize, refusal: &'static str) -> Result<()> {
    // SAFETY: the stream is open.
    let got = unsafe { aaudio::AAudioStream_getChannelCount(stream.ptr()) };
    if usize::try_from(got).ok() == Some(wanted) {
        Ok(())
    } else {
        Err(AudioError::UnsupportedConfiguration(refusal.to_owned()))
    }
}

/// Request a stop and wait until AAudio confirms it, so that closing cannot race a callback.
///
/// A stream that is already stopped, was never started, or was disconnected by the platform is
/// stopped as far as anyone can tell, and AAudio says to close such a stream regardless.
fn stop(stream: &Stream, operation: &'static str) -> Result<()> {
    // SAFETY: the stream is open, and only its owner (this thread) stops or waits on it.
    match check(operation, unsafe {
        aaudio::AAudioStream_requestStop(stream.ptr())
    }) {
        Ok(()) | Err(AudioError::Disconnected) => {}
        Err(error) => return Err(error),
    }
    let running = [
        raw(aaudio::AAUDIO_STREAM_STATE_STARTING),
        raw(aaudio::AAUDIO_STREAM_STATE_STARTED),
        raw(aaudio::AAUDIO_STREAM_STATE_STOPPING),
    ];
    // SAFETY: as above; `getState` only reads.
    let mut state = unsafe { aaudio::AAudioStream_getState(stream.ptr()) };
    // Bounded: each wait times out, and a stream does not pass through these states forever.
    for _ in 0..running.len() {
        if !running.contains(&state) {
            return Ok(());
        }
        let mut next = state;
        // SAFETY: as above. No other thread waits on or closes this stream.
        let waited = unsafe {
            aaudio::AAudioStream_waitForStateChange(
                stream.ptr(),
                state,
                &raw mut next,
                STOP_TIMEOUT_NS,
            )
        };
        match check("wait for the stream to stop", waited) {
            Ok(()) => state = next,
            // A stream the platform disconnected has stopped as far as closing it is concerned.
            Err(AudioError::Disconnected) => return Ok(()),
            Err(error) => return Err(error),
        }
    }
    if running.contains(&state) {
        Err(AudioError::Platform {
            operation: "wait for the stream to stop",
            code: state,
        })
    } else {
        Ok(())
    }
}

/// Turn an AAudio result code into this crate's error.
fn check(operation: &'static str, code: aaudio::aaudio_result_t) -> Result<()> {
    if code >= 0 {
        Ok(())
    } else if code == aaudio::AAUDIO_ERROR_DISCONNECTED {
        Err(AudioError::Disconnected)
    } else {
        Err(AudioError::Platform { operation, code })
    }
}

/// AAudio's enum constants are generated as `u32`, while every setter takes `i32`. All of them
/// are small, so the conversion never fails; an impossible one becomes a value AAudio rejects.
fn raw(constant: u32) -> i32 {
    i32::try_from(constant).unwrap_or(i32::MIN)
}

/// Nanoseconds from `frames` at `sample_rate`.
fn frames_to_ns(frames: u64, sample_rate: u32) -> u64 {
    let ns = u128::from(frames) * 1_000_000_000 / u128::from(sample_rate.max(1));
    u64::try_from(ns).unwrap_or(u64::MAX)
}

/// The monotonic host clock, in nanoseconds.
///
/// Called on the audio thread. `clock_gettime(CLOCK_MONOTONIC)` is served by the kernel's vDSO on
/// Android's 64-bit ABIs, so it reads shared memory without entering the kernel. That is the one
/// timing call the real-time rules allow (`docs/adr/0020`).
fn now_ns() -> u64 {
    let mut now = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: clock_gettime writes one timespec through a valid pointer, and CLOCK_MONOTONIC
    // always exists, so it cannot fail.
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &raw mut now) };
    let seconds = u64::try_from(now.tv_sec).unwrap_or(0);
    let nanos = u64::try_from(now.tv_nsec).unwrap_or(0);
    seconds.saturating_mul(1_000_000_000).saturating_add(nanos)
}
