//! In-process Rust binding for the Arcade Learning Environment's C++
//! `ALEInterface`.
//!
//! # Optional linkage
//!
//! ALE is a C++ library that must be built separately. Point `ALE_ROOT` at a
//! CMake install prefix containing `include/ale/ale_interface.hpp` and
//! `lib/libale.a`:
//!
//! ```text
//! cmake -S <ale-source> -B build -DCMAKE_BUILD_TYPE=Release \
//!   -DBUILD_PYTHON_LIB=OFF -DBUILD_VECTOR_LIB=OFF -DSDL_SUPPORT=OFF \
//!   -DCMAKE_INSTALL_PREFIX=<prefix>
//! cmake --build build --target install
//! export ALE_ROOT=<prefix>
//! ```
//!
//! With `ALE_ROOT` unset the crate still builds. [`is_linked`] then returns
//! `false` and every [`Ale::new`] reports [`AleError::NotLinked`], so a caller's
//! test suite can skip rather than fail on a machine without a C++ toolchain.
//!
//! # Licence
//!
//! ALE is GPL-2.0-only

#[cfg(ale_linked)]
use std::ffi::CStr;
use std::ffi::{CString, c_char, c_float, c_int, c_void};
use std::fmt;
use std::path::Path;
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

/// Declarations of the C shim.
///
/// Under `not(ale_linked)` these become Rust stubs with the same signatures
/// rather than `extern` declarations, so the crate links on a machine with no
/// ALE. Nothing reaches a stub: [`Ale::new`] fails with [`AleError::NotLinked`]
/// first, and it is the only way to obtain the handle every other call needs.
mod ffi {
    #[cfg_attr(not(ale_linked), allow(unused_imports))]
    use super::{c_char, c_float, c_int, c_void};

    #[cfg(ale_linked)]
    unsafe extern "C" {
        pub fn ale_shim_new() -> *mut c_void;
        pub fn ale_shim_free(shim: *mut c_void);
        pub fn ale_shim_last_error(shim: *const c_void) -> *const c_char;
        pub fn ale_shim_supported_rom(
            shim: *mut c_void,
            path: *const c_char,
            out: *mut c_char,
            capacity: usize,
            out_len: *mut usize,
        ) -> c_int;
        pub fn ale_shim_set_int(shim: *mut c_void, key: *const c_char, value: c_int) -> c_int;
        pub fn ale_shim_set_float(shim: *mut c_void, key: *const c_char, value: c_float) -> c_int;
        pub fn ale_shim_load_rom(shim: *mut c_void, path: *const c_char) -> c_int;
        pub fn ale_shim_reset_game(shim: *mut c_void) -> c_int;
        pub fn ale_shim_act(
            shim: *mut c_void,
            action: c_int,
            strength: c_float,
            out_reward: *mut c_int,
        ) -> c_int;
        pub fn ale_shim_game_over(
            shim: *mut c_void,
            with_truncation: c_int,
            out_flag: *mut c_int,
        ) -> c_int;
        pub fn ale_shim_game_truncated(shim: *mut c_void, out_flag: *mut c_int) -> c_int;
        pub fn ale_shim_lives(shim: *mut c_void, out_lives: *mut c_int) -> c_int;
        pub fn ale_shim_frame_number(shim: *mut c_void, out_frame: *mut c_int) -> c_int;
        pub fn ale_shim_episode_frame_number(shim: *mut c_void, out_frame: *mut c_int) -> c_int;
        pub fn ale_shim_minimal_action_set(
            shim: *mut c_void,
            out: *mut c_int,
            capacity: usize,
            out_len: *mut usize,
        ) -> c_int;
        pub fn ale_shim_screen(
            shim: *mut c_void,
            out_height: *mut usize,
            out_width: *mut usize,
        ) -> *const u8;
        pub fn ale_shim_screen_grayscale(shim: *mut c_void, out_len: *mut usize) -> *const u8;
        pub fn ale_shim_version() -> *const c_char;
        pub fn ale_shim_set_logger_mode(mode: c_int);
    }

    /// Same signatures, no linkage. Every one is unreachable: they all take the
    /// `*mut c_void` handle, and the only source of one is [`super::Ale::new`],
    /// which fails with `NotLinked` before constructing anything. Only the
    /// shim entry points reached from code outside a `cfg(ale_linked)` block
    /// need a stub, which is why this list is shorter than the extern block.
    #[cfg(not(ale_linked))]
    mod unlinked {
        use super::{c_char, c_float, c_int, c_void};

        const UNLINKED: &str = "ale-rs shim called without ALE linked; Ale::new should have \
                                returned AleError::NotLinked first";

        pub unsafe fn ale_shim_supported_rom(
            _shim: *mut c_void,
            _path: *const c_char,
            _out: *mut c_char,
            _capacity: usize,
            _out_len: *mut usize,
        ) -> c_int {
            unreachable!("{UNLINKED}")
        }
        pub unsafe fn ale_shim_set_int(
            _shim: *mut c_void,
            _key: *const c_char,
            _value: c_int,
        ) -> c_int {
            unreachable!("{UNLINKED}")
        }
        pub unsafe fn ale_shim_set_float(
            _shim: *mut c_void,
            _key: *const c_char,
            _value: c_float,
        ) -> c_int {
            unreachable!("{UNLINKED}")
        }
        pub unsafe fn ale_shim_load_rom(_shim: *mut c_void, _path: *const c_char) -> c_int {
            unreachable!("{UNLINKED}")
        }
        pub unsafe fn ale_shim_reset_game(_shim: *mut c_void) -> c_int {
            unreachable!("{UNLINKED}")
        }
        pub unsafe fn ale_shim_act(
            _shim: *mut c_void,
            _action: c_int,
            _strength: c_float,
            _out_reward: *mut c_int,
        ) -> c_int {
            unreachable!("{UNLINKED}")
        }
        pub unsafe fn ale_shim_game_over(
            _shim: *mut c_void,
            _with_truncation: c_int,
            _out_flag: *mut c_int,
        ) -> c_int {
            unreachable!("{UNLINKED}")
        }
        pub unsafe fn ale_shim_game_truncated(_shim: *mut c_void, _out_flag: *mut c_int) -> c_int {
            unreachable!("{UNLINKED}")
        }
        pub unsafe fn ale_shim_lives(_shim: *mut c_void, _out_lives: *mut c_int) -> c_int {
            unreachable!("{UNLINKED}")
        }
        pub unsafe fn ale_shim_frame_number(_shim: *mut c_void, _out_frame: *mut c_int) -> c_int {
            unreachable!("{UNLINKED}")
        }
        pub unsafe fn ale_shim_episode_frame_number(
            _shim: *mut c_void,
            _out_frame: *mut c_int,
        ) -> c_int {
            unreachable!("{UNLINKED}")
        }
        pub unsafe fn ale_shim_minimal_action_set(
            _shim: *mut c_void,
            _out: *mut c_int,
            _capacity: usize,
            _out_len: *mut usize,
        ) -> c_int {
            unreachable!("{UNLINKED}")
        }
    }

    #[cfg(not(ale_linked))]
    pub use unlinked::*;
}

/// Whether this build linked ALE.
///
/// `false` when `ALE_ROOT` was unset or incomplete at build time. Callers use
/// this to skip ALE-dependent tests rather than fail them.
pub const fn is_linked() -> bool {
    cfg!(ale_linked)
}

/// `ALE_VERSION` compiled into the linked library, or `None` when unlinked.
///
/// This is the build identity of the C++ library actually in the binary, read
/// from `ale/version.hpp` at compile time — not a value this crate declares.
pub fn linked_version() -> Option<&'static str> {
    #[cfg(ale_linked)]
    {
        // SAFETY: `ale_shim_version` returns a pointer to a string literal
        // compiled into the shim, so it is non-null, NUL-terminated, and
        // 'static.
        let version = unsafe { CStr::from_ptr(ffi::ale_shim_version()) };
        version.to_str().ok()
    }
    #[cfg(not(ale_linked))]
    {
        None
    }
}

/// Set once by [`set_logger_mode`] so [`Ale::new`]'s default-quiet behaviour
/// never overrides an explicit caller choice.
static LOGGER_MODE_CHOSEN: AtomicBool = AtomicBool::new(false);
static LOGGER_DEFAULT: Once = Once::new();

/// Process-wide log verbosity for ALE's C++ logger.
///
/// ALE's logger mode is a process-global (`Logger::current_mode` in
/// `src/ale/common/Log.cpp`), so this is not a per-instance setting and is not
/// exposed on [`Ale`]. It is a no-op when unlinked.
///
/// Calling this at all — with any mode — also opts out of the default described
/// on [`Ale::new`].
pub fn set_logger_mode(mode: LoggerMode) {
    LOGGER_MODE_CHOSEN.store(true, Ordering::Release);
    apply_logger_mode(mode);
}

fn apply_logger_mode(mode: LoggerMode) {
    #[cfg(ale_linked)]
    // SAFETY: passing a plain integer to a function that only writes a
    // process-global enum.
    unsafe {
        ffi::ale_shim_set_logger_mode(mode as c_int);
    }
    #[cfg(not(ale_linked))]
    let _ = mode;
}

/// Verbosity levels of ALE's C++ logger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoggerMode {
    Info = 0,
    Warning = 1,
    Error = 2,
}

/// Everything that can go wrong at this boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AleError {
    /// This build has no ALE linked; see the crate docs for `ALE_ROOT`.
    NotLinked,
    /// `ALEInterface`'s constructor failed or allocation failed.
    ConstructionFailed,
    /// A string argument contained an interior NUL and cannot cross the C ABI.
    InteriorNul(&'static str),
    /// A path argument was not valid UTF-8.
    NonUtf8Path,
    /// An action value outside `0..=17`, the `PLAYER_A_*` range.
    ///
    /// Rejected in Rust because ALE's `ALEState::apply_action` ends its switch
    /// with `std::exit(-1)`, so letting one through would kill the process.
    ActionOutOfRange(i32),
    /// A method that needs a loaded ROM was called before [`Ale::load_rom`]
    /// succeeded.
    ///
    /// Rejected in Rust because `ALEInterface` holds its `environment` as a
    /// null `unique_ptr` until `loadROM` runs, and every post-ROM method
    /// dereferences it unconditionally — a segfault, not an exception, so the
    /// shim's guard cannot catch it.
    NoRomLoaded,
    /// The C++ side threw; this is `what()`, or a generic message for a
    /// non-`std::exception` throw.
    Ale(String),
}

impl fmt::Display for AleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotLinked => write!(
                f,
                "ALE is not linked into this build (set ALE_ROOT to a CMake install prefix)"
            ),
            Self::ConstructionFailed => write!(f, "could not construct an ALEInterface"),
            Self::InteriorNul(what) => write!(f, "{what} contains an interior NUL byte"),
            Self::NonUtf8Path => write!(f, "ROM path is not valid UTF-8"),
            Self::ActionOutOfRange(action) => write!(
                f,
                "ALE action value {action} is outside the PLAYER_A range 0..=17"
            ),
            Self::NoRomLoaded => write!(f, "no ROM is loaded (call load_rom first)"),
            Self::Ale(message) => write!(f, "ALE error: {message}"),
        }
    }
}

impl std::error::Error for AleError {}

/// One emulator instance.
///
/// Not `Clone` and not `Sync`: the handle is a unique owner of a C++ object
/// with no internal synchronization. It **is** [`Send`] — see the `unsafe impl`
/// below for the justification and the caveat it carries.
pub struct Ale {
    #[cfg_attr(not(ale_linked), allow(dead_code))]
    handle: *mut c_void,
    /// Set once [`Ale::load_rom`] succeeds, and never cleared: ALE's
    /// `environment` member stays valid from then on, even if a later load
    /// fails. Every method that would dereference it checks this first.
    rom_loaded: bool,
}

// SAFETY: `Ale` owns its `AleShim` exclusively — the field is private, the type
// is neither `Clone` nor `Copy`, and no method hands out the raw pointer — so
// no two threads can reach one instance without an intervening `&mut`. The C++
// state behind it is per-instance: `ALEInterface` holds `theOSystem`,
// `theSettings`, `romSettings`, and `environment` as per-instance
// `unique_ptr`s. The one process-global observed in a scan of `ale_interface.*`
// is `Logger::current_mode`, which this crate touches only through the
// free-standing [`set_logger_mode`] and never per instance.
//
// That scan is indicative, not exhaustive, so this justification is *tested*
// rather than merely asserted: the consuming test suite steps N instances
// round-robin in one process and requires byte-identical streams against the
// same N run in isolation.
//
// Deliberately no `Sync`: `&Ale` would permit concurrent `screen()` reads
// against ALE's own mutable buffer.
unsafe impl Send for Ale {}

impl fmt::Debug for Ale {
    /// Deliberately opaque: the only state is a raw handle, and printing its
    /// address would make `Debug` output non-deterministic across runs.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Ale")
    }
}

impl Drop for Ale {
    fn drop(&mut self) {
        #[cfg(ale_linked)]
        // SAFETY: `handle` came from `ale_shim_new`, is freed exactly once
        // here, and is never handed out or copied.
        unsafe {
            ffi::ale_shim_free(self.handle);
        }
    }
}

/// Borrowed view of ALE's palette-index screen.
///
/// `pixels` points into the emulator's own buffer (`ALEScreen::getArray`); no
/// copy is made. The borrow of [`Ale`] is what keeps it valid — the next
/// `act`/`reset_game` needs `&mut Ale` and so cannot run while this is alive.
#[derive(Clone, Copy, Debug)]
pub struct Screen<'a> {
    pub pixels: &'a [u8],
    pub height: usize,
    pub width: usize,
}

impl Ale {
    /// Construct an emulator instance.
    ///
    /// Returns [`AleError::NotLinked`] when this build has no ALE linked, which
    /// is the case callers should skip on rather than fail.
    ///
    /// **Side effect:** unless [`set_logger_mode`] was called first, the first
    /// construction in a process quiets ALE's logger to
    /// [`LoggerMode::Error`]. `ALEInterface`'s constructor writes a two-line
    /// banner at `Info` (`ale_interface.cpp:100`), so without this every worker
    /// thread would print one into the caller's stdout. It runs under a
    /// [`Once`], so concurrent constructions do not race on the global.
    pub fn new() -> Result<Self, AleError> {
        if !LOGGER_MODE_CHOSEN.load(Ordering::Acquire) {
            LOGGER_DEFAULT.call_once(|| apply_logger_mode(LoggerMode::Error));
        }
        #[cfg(ale_linked)]
        {
            // SAFETY: no arguments; returns either a fresh owned handle or
            // null, which is checked.
            let handle = unsafe { ffi::ale_shim_new() };
            if handle.is_null() {
                return Err(AleError::ConstructionFailed);
            }
            Ok(Self {
                handle,
                rom_loaded: false,
            })
        }
        #[cfg(not(ale_linked))]
        {
            Err(AleError::NotLinked)
        }
    }

    /// Set an integer setting, e.g. `random_seed` or
    /// `max_num_frames_per_episode`.
    ///
    /// Settings that affect ROM initialization must be set *before*
    /// [`Self::load_rom`]; ALE re-reads them there.
    pub fn set_int(&mut self, key: &str, value: i32) -> Result<(), AleError> {
        let key = c_string(key, "setting key")?;
        self.checked(|handle| {
            // SAFETY: `handle` is our live handle; `key` outlives the call.
            unsafe { ffi::ale_shim_set_int(handle, key.as_ptr(), value) }
        })
    }

    /// Set a float setting, e.g. `repeat_action_probability`.
    pub fn set_float(&mut self, key: &str, value: f32) -> Result<(), AleError> {
        let key = c_string(key, "setting key")?;
        self.checked(|handle| {
            // SAFETY: as `set_int`.
            unsafe { ffi::ale_shim_set_float(handle, key.as_ptr(), value) }
        })
    }

    /// Whether ALE recognizes the file at `path` as a supported cartridge.
    ///
    /// `Ok(name)` gives ALE's canonical ROM name. Errors cover a missing file,
    /// an unreadable one, and a file whose MD5 matches no cartridge in ALE's
    /// table.
    pub fn supported_rom(&mut self, path: &Path) -> Result<String, AleError> {
        let path = path.to_str().ok_or(AleError::NonUtf8Path)?;
        let path = c_string(path, "ROM path")?;
        let mut buffer = vec![0u8; 128];
        let mut len = 0usize;
        // SAFETY: `buffer` is a live allocation of exactly `buffer.len()`
        // bytes and that length is passed as the capacity, so the shim's
        // bounded copy cannot overrun it.
        self.checked(|handle| unsafe {
            ffi::ale_shim_supported_rom(
                handle,
                path.as_ptr(),
                buffer.as_mut_ptr().cast::<c_char>(),
                buffer.len(),
                &raw mut len,
            )
        })?;
        let end = buffer.iter().position(|byte| *byte == 0).unwrap_or(0);
        buffer.truncate(end);
        Ok(String::from_utf8_lossy(&buffer).into_owned())
    }

    /// Load a ROM, (re)initializing the emulator with the current settings.
    ///
    /// Every rejection is decided **before** ALE sees the path. That is not
    /// defensive style, it is required: `ALEInterface::loadROM` and the
    /// `loadSettings` it calls end their failure branches with `std::exit(1)`
    /// (four call sites in `ale_interface.cpp`), which no exception guard can
    /// intercept and which would take the whole training process down. So this
    /// runs [`Self::supported_rom`] first — it answers the same question by
    /// throwing or returning "unsupported", both recoverable — and only calls
    /// `loadROM` once the ROM is known-good.
    ///
    /// The residual is one branch `isSupportedROM` does not cover: a cartridge
    /// whose MD5 *is* in ALE's table but whose console Stella then fails to
    /// create (`ale_interface.cpp:92`). Nothing this side of the ABI can
    /// intercept that.
    ///
    /// Note this is *stricter* than `ale-py`, which only warns on an MD5
    /// mismatch and plays on. Here a mismatched ROM is rejected.
    pub fn load_rom(&mut self, path: &Path) -> Result<(), AleError> {
        self.supported_rom(path)?;
        let path = path.to_str().ok_or(AleError::NonUtf8Path)?;
        let path = c_string(path, "ROM path")?;
        self.checked(|handle| {
            // SAFETY: as `set_int`.
            unsafe { ffi::ale_shim_load_rom(handle, path.as_ptr()) }
        })?;
        self.rom_loaded = true;
        Ok(())
    }

    /// The guard in front of ALE's third unrecoverable path: until `loadROM`
    /// runs, `ALEInterface::environment` is a null `unique_ptr` that every
    /// post-ROM method dereferences — a segfault the shim's exception guard
    /// cannot catch. Rejected here instead.
    fn require_rom(&self) -> Result<(), AleError> {
        if self.rom_loaded {
            Ok(())
        } else {
            Err(AleError::NoRomLoaded)
        }
    }

    /// Start a new episode.
    pub fn reset_game(&mut self) -> Result<(), AleError> {
        self.require_rom()?;
        // SAFETY: as `set_int`.
        self.checked(|handle| unsafe { ffi::ale_shim_reset_game(handle) })
    }

    /// Emulate one frame with `action` held, returning that frame's reward.
    ///
    /// `action` is an ALE action *value* (the `Action` enum), not an index into
    /// the minimal action set — map it through [`Self::minimal_action_set`]
    /// first if that is what you hold. `strength` is the paddle strength; `1.0`
    /// is what the discrete Gymnasium path passes.
    ///
    /// Values outside `0..=17` are rejected here with
    /// [`AleError::ActionOutOfRange`] rather than passed down. ALE noops any
    /// *illegal-for-this-ROM* action in that range
    /// (`StellaEnvironment::noopIllegalActions`), but a value above it falls
    /// through to the `default:` arm of `ALEState::apply_action`, which calls
    /// `std::exit(-1)`. An action that is in range but not in this ROM's
    /// minimal set is therefore silently a NOOP, exactly as it is under
    /// `ale-py`; callers wanting that rejected should check
    /// [`Self::minimal_action_set`] themselves.
    pub fn act(&mut self, action: i32, strength: f32) -> Result<i32, AleError> {
        if !(0..=17).contains(&action) {
            return Err(AleError::ActionOutOfRange(action));
        }
        self.require_rom()?;
        let mut reward = 0;
        // SAFETY: `&mut reward` is a valid, aligned, live `c_int` for the call.
        self.checked(|handle| unsafe {
            ffi::ale_shim_act(handle, action, strength, &raw mut reward)
        })?;
        Ok(reward)
    }

    /// Whether the episode has ended.
    ///
    /// `with_truncation` folds the frame-cap truncation into the answer;
    /// Gymnasium's `AtariEnv.step` passes `false` here and reads
    /// [`Self::game_truncated`] separately.
    pub fn game_over(&mut self, with_truncation: bool) -> Result<bool, AleError> {
        self.require_rom()?;
        self.flag(|handle, out| {
            // SAFETY: as `act`.
            unsafe { ffi::ale_shim_game_over(handle, c_int::from(with_truncation), out) }
        })
    }

    /// Whether the episode hit `max_num_frames_per_episode`.
    ///
    /// Always `false` unless that setting was set: it is Gymnasium's
    /// registration, not ALE itself, that supplies the 108,000-frame cap.
    pub fn game_truncated(&mut self) -> Result<bool, AleError> {
        self.require_rom()?;
        // SAFETY: as `act`.
        self.flag(|handle, out| unsafe { ffi::ale_shim_game_truncated(handle, out) })
    }

    /// Remaining lives, as ALE's game rules report them.
    pub fn lives(&mut self) -> Result<i32, AleError> {
        self.require_rom()?;
        // SAFETY: as `act`.
        self.scalar(|handle, out| unsafe { ffi::ale_shim_lives(handle, out) })
    }

    /// Frames emulated since the ROM was loaded.
    pub fn frame_number(&mut self) -> Result<i32, AleError> {
        self.require_rom()?;
        // SAFETY: as `act`.
        self.scalar(|handle, out| unsafe { ffi::ale_shim_frame_number(handle, out) })
    }

    /// Frames emulated since the current episode began.
    pub fn episode_frame_number(&mut self) -> Result<i32, AleError> {
        self.require_rom()?;
        // SAFETY: as `act`.
        self.scalar(|handle, out| unsafe { ffi::ale_shim_episode_frame_number(handle, out) })
    }

    /// The ROM's minimal action set, in ALE's order.
    ///
    /// This is the same vector `getMinimalActionSet` returns, so a policy index
    /// maps to an ALE action value by indexing it — which is exactly what
    /// Gymnasium's `AtariEnv.step` does with `self._action_set[action]`.
    pub fn minimal_action_set(&mut self) -> Result<Vec<i32>, AleError> {
        self.require_rom()?;
        let mut len = 0usize;
        // SAFETY: a null `out` with capacity 0 is the documented size query.
        self.checked(|handle| unsafe {
            ffi::ale_shim_minimal_action_set(handle, std::ptr::null_mut(), 0, &raw mut len)
        })?;
        let mut actions = vec![0; len];
        let mut written = 0usize;
        // SAFETY: `actions` has exactly `len` elements and `len` is passed as
        // the capacity, so the shim cannot write past the end.
        self.checked(|handle| unsafe {
            ffi::ale_shim_minimal_action_set(handle, actions.as_mut_ptr(), len, &raw mut written)
        })?;
        actions.truncate(written.min(len));
        Ok(actions)
    }

    /// Borrow ALE's palette-index screen without copying it.
    ///
    /// Each byte is a palette index in `0..=255`, row-major over
    /// `height * width`. This is the same buffer `ale.getScreen()` exposes to
    /// Python.
    pub fn screen(&mut self) -> Result<Screen<'_>, AleError> {
        self.require_rom()?;
        #[cfg(ale_linked)]
        {
            let mut height = 0usize;
            let mut width = 0usize;
            // SAFETY: both out-parameters are live locals; the returned pointer
            // is checked for null before use.
            let pixels =
                unsafe { ffi::ale_shim_screen(self.handle, &raw mut height, &raw mut width) };
            if pixels.is_null() {
                return Err(self.last_error());
            }
            // SAFETY: `ALEScreen::getArray` points at `arraySize()` =
            // `height * width` bytes of the emulator's own `std::vector<pixel_t>`
            // (ale_screen.hpp). The lifetime is tied to `&mut self`, and every
            // call that can reallocate or overwrite it (`act`, `reset_game`,
            // `load_rom`) also takes `&mut self`, so this borrow cannot outlive
            // the buffer's validity.
            let pixels = unsafe { std::slice::from_raw_parts(pixels, height * width) };
            Ok(Screen {
                pixels,
                height,
                width,
            })
        }
        #[cfg(not(ale_linked))]
        {
            Err(AleError::NotLinked)
        }
    }

    /// Borrow the grayscale screen ALE produces for `obs_type="grayscale"`.
    ///
    /// These are the exact bytes Gymnasium's `AtariEnv._get_obs` returns for
    /// that observation type — it calls `getScreenGrayscale` and applies no
    /// further transform. The buffer belongs to this instance and is reused, so
    /// only the first call allocates.
    pub fn screen_grayscale(&mut self) -> Result<&[u8], AleError> {
        self.require_rom()?;
        #[cfg(ale_linked)]
        {
            let mut len = 0usize;
            // SAFETY: `len` is a live local; the returned pointer is checked.
            let pixels = unsafe { ffi::ale_shim_screen_grayscale(self.handle, &raw mut len) };
            if pixels.is_null() {
                return Err(self.last_error());
            }
            // SAFETY: the shim resized its own `std::vector` to exactly `len`
            // bytes and returned `data()`. The `&mut self` borrow keeps the
            // next call to this function — the only thing that rewrites the
            // buffer — from overlapping this slice.
            Ok(unsafe { std::slice::from_raw_parts(pixels, len) })
        }
        #[cfg(not(ale_linked))]
        {
            Err(AleError::NotLinked)
        }
    }

    #[cfg(ale_linked)]
    fn last_error(&self) -> AleError {
        // SAFETY: `ale_shim_last_error` never returns null for a live handle
        // and the string is valid until the next call on this handle, which
        // cannot happen while `&self` is borrowed here.
        let message = unsafe { CStr::from_ptr(ffi::ale_shim_last_error(self.handle)) };
        AleError::Ale(message.to_string_lossy().into_owned())
    }

    #[cfg(ale_linked)]
    fn checked(&mut self, call: impl FnOnce(*mut c_void) -> c_int) -> Result<(), AleError> {
        if call(self.handle) == 0 {
            Ok(())
        } else {
            Err(self.last_error())
        }
    }

    #[cfg(not(ale_linked))]
    fn checked(&mut self, _call: impl FnOnce(*mut c_void) -> c_int) -> Result<(), AleError> {
        Err(AleError::NotLinked)
    }

    fn scalar(
        &mut self,
        call: impl FnOnce(*mut c_void, *mut c_int) -> c_int,
    ) -> Result<i32, AleError> {
        let mut value = 0;
        self.checked(|handle| call(handle, &raw mut value))?;
        Ok(value)
    }

    fn flag(
        &mut self,
        call: impl FnOnce(*mut c_void, *mut c_int) -> c_int,
    ) -> Result<bool, AleError> {
        Ok(self.scalar(call)? != 0)
    }
}

fn c_string(value: &str, what: &'static str) -> Result<CString, AleError> {
    CString::new(value).map_err(|_| AleError::InteriorNul(what))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linked_version_agrees_with_linkage() {
        assert_eq!(is_linked(), linked_version().is_some());
    }

    #[test]
    fn unlinked_builds_report_not_linked_rather_than_panicking() {
        if is_linked() {
            return;
        }
        assert_eq!(Ale::new().unwrap_err(), AleError::NotLinked);
    }

    #[test]
    fn interior_nul_in_a_setting_key_is_rejected_before_the_abi_boundary() {
        let Ok(mut ale) = Ale::new() else {
            return;
        };
        assert_eq!(
            ale.set_int("random\0seed", 1).unwrap_err(),
            AleError::InteriorNul("setting key")
        );
    }

    /// The point is not the error variant, it is that the test binary is still
    /// alive to make an assertion: ALE's own path here is `std::exit(1)`.
    #[test]
    fn a_missing_rom_is_an_error_not_a_process_exit() {
        let Ok(mut ale) = Ale::new() else {
            return;
        };
        let error = ale
            .load_rom(Path::new("/definitely/not/a/rom.bin"))
            .unwrap_err();
        assert!(matches!(error, AleError::Ale(_)), "unexpected: {error:?}");
    }

    /// A readable file that is not an Atari cartridge takes the other
    /// `std::exit(1)` branch inside ALE. `isSupportedROM` gets there first.
    #[test]
    fn a_file_that_is_not_a_cartridge_is_an_error_not_a_process_exit() {
        let Ok(mut ale) = Ale::new() else {
            return;
        };
        let path = std::env::temp_dir().join("ale_rs_not_a_rom.bin");
        std::fs::write(&path, b"this is not an atari cartridge").unwrap();
        let error = ale.load_rom(&path).unwrap_err();
        let _ = std::fs::remove_file(&path);
        assert!(matches!(error, AleError::Ale(_)), "unexpected: {error:?}");
    }

    /// Until `loadROM` runs, `ALEInterface::environment` is a null
    /// `unique_ptr`, and every one of these methods dereferences it — a
    /// segfault, not an exception. As with the `std::exit` tests, the point is
    /// that the test binary is still alive to make an assertion.
    #[test]
    fn pre_rom_calls_are_errors_not_segfaults() {
        let Ok(mut ale) = Ale::new() else {
            return;
        };
        assert_eq!(ale.reset_game().unwrap_err(), AleError::NoRomLoaded);
        assert_eq!(ale.act(0, 1.0).unwrap_err(), AleError::NoRomLoaded);
        assert_eq!(ale.game_over(false).unwrap_err(), AleError::NoRomLoaded);
        assert_eq!(ale.game_truncated().unwrap_err(), AleError::NoRomLoaded);
        assert_eq!(ale.lives().unwrap_err(), AleError::NoRomLoaded);
        assert_eq!(ale.frame_number().unwrap_err(), AleError::NoRomLoaded);
        assert_eq!(
            ale.episode_frame_number().unwrap_err(),
            AleError::NoRomLoaded
        );
        assert_eq!(ale.minimal_action_set().unwrap_err(), AleError::NoRomLoaded);
        assert_eq!(ale.screen().unwrap_err(), AleError::NoRomLoaded);
        assert_eq!(ale.screen_grayscale().unwrap_err(), AleError::NoRomLoaded);
    }

    /// `ALEState::apply_action`'s `default:` arm is `std::exit(-1)`; 18 is the
    /// first value past `PLAYER_A_DOWNLEFTFIRE` that `noopIllegalActions` does
    /// not fold to a NOOP.
    #[test]
    fn an_out_of_range_action_is_rejected_before_reaching_ale() {
        let Ok(mut ale) = Ale::new() else {
            return;
        };
        for action in [-1, 18, 40, 99] {
            assert_eq!(
                ale.act(action, 1.0).unwrap_err(),
                AleError::ActionOutOfRange(action)
            );
        }
    }
}
