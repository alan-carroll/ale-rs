/* Minimal C ABI over ale::ALEInterface.
 *
 * Upstream ships no C wrapper as of v0.12.0 (the old ale_c_wrapper.h was
 * removed), so this is ours. Every entry point catches C++ exceptions at the
 * boundary: unwinding into Rust is undefined behaviour. Functions that can
 * fail return a status code and leave a message retrievable with
 * ale_shim_last_error.
 *
 * Copyright (C) 2026 Alan Carroll.
 * Released under the GNU General Public License version 2; ALE itself is
 * GPL-2.0-only and this shim links it.
 */
#ifndef ALE_SHIM_H
#define ALE_SHIM_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct AleShim AleShim;

/* 0 on success, 1 on failure. On failure ale_shim_last_error describes it. */
#define ALE_SHIM_OK 0
#define ALE_SHIM_ERR 1

/* Returns NULL if the interface could not be constructed. */
AleShim *ale_shim_new(void);
void ale_shim_free(AleShim *shim);

/* Message from the most recent failed call on this handle. Never NULL; the
 * empty string means "no failure recorded". Valid until the next call on the
 * same handle. */
const char *ale_shim_last_error(const AleShim *shim);

/* Pre-flight for ale_shim_load_rom, and the reason it exists: ALEInterface::
 * loadROM does not throw on a bad ROM, it calls std::exit(1). isSupportedROM
 * answers the same question by throwing (missing/unreadable) or returning
 * nullopt (MD5 does not match any known cartridge), both of which are
 * recoverable. Status is ALE_SHIM_ERR for every rejection, with the reason in
 * ale_shim_last_error. On success the ROM's canonical name is copied into
 * `out` (truncated to `capacity`) and its full length written to `out_len`.
 *
 * This is a static ALE call and needs no loaded ROM, but it takes a handle so
 * failures have somewhere to record a message. */
int ale_shim_supported_rom(AleShim *shim, const char *path, char *out,
                           size_t capacity, size_t *out_len);

int ale_shim_set_int(AleShim *shim, const char *key, int value);
int ale_shim_set_float(AleShim *shim, const char *key, float value);

/* Read a setting back out of the live interface, so a caller can verify what it
 * configured rather than trusting its own record of it. Settings live in an
 * ALEInterface-owned Settings object built by the constructor, so — like the
 * two setters above and unlike every accessor below — these need no loaded ROM.
 * Writes through the out parameter; the return value is the status code. */
int ale_shim_get_int(AleShim *shim, const char *key, int *out_value);
int ale_shim_get_float(AleShim *shim, const char *key, float *out_value);

int ale_shim_load_rom(AleShim *shim, const char *path);
int ale_shim_reset_game(AleShim *shim);

/* ale::reward_t is int (common/Constants.h). Writes the reward through
 * out_reward. */
int ale_shim_act(AleShim *shim, int action, float strength, int *out_reward);

/* Write 1/0 through the out parameter; the return value is the status code. */
int ale_shim_game_over(AleShim *shim, int with_truncation, int *out_flag);
int ale_shim_game_truncated(AleShim *shim, int *out_flag);

int ale_shim_lives(AleShim *shim, int *out_lives);
int ale_shim_frame_number(AleShim *shim, int *out_frame);
int ale_shim_episode_frame_number(AleShim *shim, int *out_frame);

/* Copies at most `capacity` action values into `out` and writes the full set
 * size through `out_len`, so a caller can size its buffer from a first call
 * with capacity 0. */
int ale_shim_minimal_action_set(AleShim *shim, int *out, size_t capacity,
                                size_t *out_len);

/* Borrowed pointer into ALE's own palette-index screen buffer (ALEScreen::
 * getArray). Valid until the next act/reset on this handle. NULL on failure. */
const uint8_t *ale_shim_screen(AleShim *shim, size_t *out_height,
                               size_t *out_width);

/* Applies the grayscale palette into a buffer owned by this handle and returns
 * a borrowed pointer to it. Valid until the next call to this function on the
 * same handle. NULL on failure. The buffer is reused across calls, so steady
 * state allocates nothing. */
const uint8_t *ale_shim_screen_grayscale(AleShim *shim, size_t *out_len);

/* As ale_shim_screen_grayscale, but the RGB palette: three bytes per pixel,
 * row-major, so out_len is 3 * height * width. This is the exact buffer
 * ale_py's render() returns in rgb_array mode (env.py calls getScreenRGB and
 * applies no further transform). Its own reused handle-owned buffer, so it does
 * not disturb the grayscale one. */
const uint8_t *ale_shim_screen_rgb(AleShim *shim, size_t *out_len);

/* Compiled-in ALE_VERSION of the linked library. */
const char *ale_shim_version(void);

/* Process-wide log verbosity. ALE's logger mode is a process-global, which is
 * why this is deliberately not a per-handle call. 0=Info, 1=Warning, 2=Error. */
void ale_shim_set_logger_mode(int mode);

#ifdef __cplusplus
}
#endif

#endif /* ALE_SHIM_H */
