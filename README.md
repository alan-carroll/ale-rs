# ale-rs

A small Rust binding for the [Arcade Learning Environment][ale]'s C++
`ALEInterface`, for driving Atari 2600 emulation.

Upstream ships no C wrapper as of v0.12.0 (the old `ale_c_wrapper.h` is gone),
so this crate carries its own `extern "C"` shim (`csrc/ale_shim.cpp`) over about
a dozen `ALEInterface` calls and wraps it in a safe Rust API.

```rust
use ale_rs::Ale;
use std::path::Path;

let mut ale = Ale::new()?;
ale.set_int("random_seed", 12345)?;
ale.set_float("repeat_action_probability", 0.25)?;
ale.set_int("max_num_frames_per_episode", 108_000)?;
ale.load_rom(Path::new("/path/to/bowling.bin"))?;
ale.reset_game()?;

let actions = ale.minimal_action_set()?;
let reward = ale.act(actions[0], 1.0)?;
let terminated = ale.game_over(false)?;
let truncated = ale.game_truncated()?;
let screen = ale.screen()?; // borrowed, 210 * 160 palette indices
```

## Building

ALE is a C++ library you build separately. Point `ALE_ROOT` at a CMake install
prefix holding `include/ale/ale_interface.hpp` and `lib/libale.a`:

```bash
git clone https://github.com/Farama-Foundation/Arcade-Learning-Environment.git
cd Arcade-Learning-Environment && git checkout v0.12.0 && cd ..
cmake -S Arcade-Learning-Environment -B ale-build -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_PYTHON_LIB=OFF -DBUILD_VECTOR_LIB=OFF -DBUILD_VECTOR_XLA_LIB=OFF \
  -DBUILD_WASM_LIB=OFF -DSDL_SUPPORT=OFF \
  -DCMAKE_INSTALL_PREFIX="$PWD/ale-install"
cmake --build ale-build --parallel --target install
export ALE_ROOT="$PWD/ale-install"
```

With those options zlib is the only external dependency.

**`ALE_ROOT` is optional.** Unset, the crate still builds; `ale_rs::is_linked()`
returns `false` and every `Ale::new()` returns `AleError::NotLinked`, so a
consumer's test suite can skip instead of failing on a machine without a C++
toolchain. `build.rs` drives `$CXX` and `$AR` directly, so the crate has no
dependencies at all.

No ROMs are distributed here. `load_rom` takes a path; supply your own.

The test suite's happy-path smoke is gated the same way: it runs only when
`ALE_TEST_ROM` points at a supported cartridge, and skips otherwise. The
upstream source checkout ships `tests/resources/tetris.bin` — 2600 homebrew
present in ALE's MD5 table — which works:

```bash
ALE_TEST_ROM="$PWD/Arcade-Learning-Environment/tests/resources/tetris.bin" cargo test
```

## Three unrecoverable ALE paths this crate stands in front of

ALE does not always signal failure recoverably, and a library that let those
through would take a caller's whole process down:

- `ALEInterface::loadROM` (and the `loadSettings` it calls) end their failure
  branches with `std::exit(1)` — missing file, unreadable file, unrecognized
  cartridge. `load_rom` runs `ALEInterface::isSupportedROM` first, which reports
  the same conditions recoverably, and only then loads. This is *stricter* than
  `ale-py`, which merely warns on an MD5 mismatch.
- `ALEState::apply_action`'s `default:` arm is `std::exit(-1)`. `act` rejects
  any value outside `0..=17` (the `PLAYER_A_*` range) before the call. Values
  inside that range but illegal for the loaded ROM are folded to NOOP by ALE
  itself, exactly as under `ale-py`.
- Until `loadROM` runs, `ALEInterface` holds its `environment` as a null
  `unique_ptr`, and every post-ROM method (`act`, `reset_game`, `getScreen`,
  …) dereferences it unconditionally — a segfault, which no exception guard
  can catch. The wrapper tracks whether a ROM has loaded and returns
  `AleError::NoRomLoaded` instead.

One branch remains uncovered: a cartridge whose MD5 *is* known but whose console
Stella then fails to create still exits.

## Threading

`Ale` is `Send` but not `Sync` and not `Clone`. The `unsafe impl Send` is
justified by single-owner semantics — ALE holds its emulator state in
per-instance `unique_ptr`s — and the one process-global observed, the logger
mode, is reachable only through the free-standing `set_logger_mode`. That
justification is worth testing rather than trusting: step N instances
round-robin in one process and check the streams against the same N run in
isolation.

Constructing the first `Ale` in a process quiets ALE's logger to `Error` unless
`set_logger_mode` was called first, because `ALEInterface`'s constructor prints
a banner at `Info`.

[ale]: https://github.com/Farama-Foundation/Arcade-Learning-Environment
