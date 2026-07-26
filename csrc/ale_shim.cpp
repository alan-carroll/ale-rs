/* C ABI over ale::ALEInterface. See ale_shim.h for the contract.
 *
 * Copyright (C) 2026 Alan Carroll.
 * Released under the GNU General Public License version 2.
 */
#include "ale_shim.h"

#include <cstring>
#include <exception>
#include <filesystem>
#include <memory>
#include <optional>
#include <stdexcept>
#include <string>
#include <vector>

#include <ale/ale_interface.hpp>
#include <ale/common/ColourPalette.hpp>
#include <ale/common/Log.hpp>
#include <ale/emucore/OSystem.hxx>
#include <ale/version.hpp>

/* Palette fast path (see the kernels at the bottom of this file). ALE's
 * applyPalette* loops decompose a 32-bit palette entry with shifts for every
 * one of 33,600 pixels; on Apple Silicon that costs about a fifth of
 * emulating the frame itself. Both conversions here run instead over a
 * byte-table snapshot of the active palette: NEON tbl kernels on aarch64
 * (~3-5x), plain lookup loops everywhere else (~1.7-2x). Output is
 * byte-identical to ALE's loops either way. */
#if defined(__ARM_NEON)
#define ALE_SHIM_NEON_PALETTE 1
#include <arm_neon.h>
#endif

struct AleShim {
  ale::ALEInterface ale;
  /* Reused across calls so the grayscale path allocates only on the first
   * frame; a vector resize to the same length is a no-op afterwards. */
  std::vector<unsigned char> grayscale;
  /* The RGB twin of `grayscale`, kept separate so a caller holding one borrow
   * cannot have it rewritten by a call for the other palette. */
  std::vector<unsigned char> rgb;
  std::string last_error;
  /* Snapshot of the active palette as plain tables: byte planes in the layout
   * the NEON kernels gather from, plus the same triples padded to four bytes
   * (r,g,b,0 in memory) for the portable kernel's single-store writes.
   * Invalidated by loadROM — the only call that changes the palette
   * (it re-reads the ROM's display format and the `palette` setting) — and
   * rebuilt lazily by the next conversion. */
  bool palette_luts_valid = false;
  uint8_t gray_lut[256];
  uint8_t r_lut[256];
  uint8_t g_lut[256];
  uint8_t b_lut[256];
  uint32_t rgbx_lut[256];
};

namespace {

/* Every entry point funnels through this so no C++ exception crosses the ABI
 * boundary. `...` is caught too: a foreign exception unwinding into Rust is
 * undefined behaviour regardless of its type. */
template <typename Body>
int guard(AleShim *shim, Body body) {
  if (shim == nullptr) {
    return ALE_SHIM_ERR;
  }
  try {
    shim->last_error.clear();
    body();
    return ALE_SHIM_OK;
  } catch (const std::exception &error) {
    shim->last_error = error.what();
  } catch (...) {
    shim->last_error = "unknown C++ exception from ALE";
  }
  return ALE_SHIM_ERR;
}

/* Rebuild the LUT snapshot by running the identity byte sequence through
 * ALE's *own* conversion loops. Both loops are pointwise — each output pixel
 * is a pure function of its input byte and the palette table — so a table
 * built this way reproduces them exactly, without this file knowing anything
 * about ALE's palette layout (the grayscale loop's `m_palette[*p + 1]`
 * included). */
void rebuild_palette_luts(AleShim *shim) {
  uint8_t identity[256];
  for (int i = 0; i < 256; ++i) {
    identity[i] = static_cast<uint8_t>(i);
  }
  uint8_t rgb[256 * 3];
  ale::ColourPalette &palette = shim->ale.theOSystem->colourPalette();
  palette.applyPaletteRGB(rgb, identity, 256);
  palette.applyPaletteGrayscale(shim->gray_lut, identity, 256);
  for (int i = 0; i < 256; ++i) {
    shim->r_lut[i] = rgb[i * 3 + 0];
    shim->g_lut[i] = rgb[i * 3 + 1];
    shim->b_lut[i] = rgb[i * 3 + 2];
    /* Assembled through memory rather than shifts so the in-memory byte
     * order is r,g,b,0 on any endianness. */
    const uint8_t rgbx[4] = {rgb[i * 3 + 0], rgb[i * 3 + 1], rgb[i * 3 + 2],
                             0};
    std::memcpy(&shim->rgbx_lut[i], rgbx, 4);
  }
  shim->palette_luts_valid = true;
}

/* Portable kernels, used wherever the NEON ones are not compiled. Still
 * faster than ALE's loops: grayscale becomes a byte-to-byte table walk
 * instead of a 32-bit load plus mask per pixel, and RGB writes each triple as
 * one four-byte store from `rgbx_lut` — the fourth byte lands on the next
 * pixel's slot an instant before that pixel overwrites it, with the final
 * pixel stored bytewise because past it lies memory we do not own. */
void apply_grayscale_portable(const uint8_t *src, size_t size,
                              const uint8_t lut[256], uint8_t *dst) {
  for (size_t i = 0; i < size; ++i) {
    dst[i] = lut[src[i]];
  }
}

void apply_rgb_portable(const AleShim *shim, const uint8_t *src, size_t size,
                        uint8_t *dst) {
  if (size == 0) {
    return;
  }
  for (size_t i = 0; i + 1 < size; ++i) {
    const uint32_t rgbx = shim->rgbx_lut[src[i]];
    std::memcpy(dst + i * 3, &rgbx, 4);
  }
  const size_t last = size - 1;
  dst[last * 3 + 0] = shim->r_lut[src[last]];
  dst[last * 3 + 1] = shim->g_lut[src[last]];
  dst[last * 3 + 2] = shim->b_lut[src[last]];
}

#if defined(ALE_SHIM_NEON_PALETTE)

/* A full 256-entry lookup from four 64-byte tbl tables. vqtbl4q returns 0 for
 * an out-of-range index, and the wrapped subtractions push every index
 * outside a table's 64-entry window out of range, so the four partial
 * results OR into the complete lookup. */
inline uint8x16_t gather256(const uint8x16x4_t tables[4], uint8x16_t x) {
  const uint8x16_t x1 = vsubq_u8(x, vdupq_n_u8(64));
  const uint8x16_t x2 = vsubq_u8(x, vdupq_n_u8(128));
  const uint8x16_t x3 = vsubq_u8(x, vdupq_n_u8(192));
  return vorrq_u8(
      vorrq_u8(vqtbl4q_u8(tables[0], x), vqtbl4q_u8(tables[1], x1)),
      vorrq_u8(vqtbl4q_u8(tables[2], x2), vqtbl4q_u8(tables[3], x3)));
}

void load_tables(uint8x16x4_t tables[4], const uint8_t lut[256]) {
  for (int i = 0; i < 4; ++i) {
    tables[i] = vld1q_u8_x4(lut + 64 * i);
  }
}

void apply_grayscale_neon(const uint8_t *src, size_t size,
                          const uint8_t lut[256], uint8_t *dst) {
  uint8x16x4_t tables[4];
  load_tables(tables, lut);
  size_t i = 0;
  for (; i + 16 <= size; i += 16) {
    vst1q_u8(dst + i, gather256(tables, vld1q_u8(src + i)));
  }
  /* The standard screen is 210*160 = 33,600 pixels, a multiple of 16, so
   * this tail is normally empty; it exists so no screen size is wrong. */
  for (; i < size; ++i) {
    dst[i] = lut[src[i]];
  }
}

void apply_rgb_neon(const AleShim *shim, const uint8_t *src, size_t size,
                    uint8_t *dst) {
  uint8x16x4_t r[4], g[4], b[4];
  load_tables(r, shim->r_lut);
  load_tables(g, shim->g_lut);
  load_tables(b, shim->b_lut);
  size_t i = 0;
  for (; i + 16 <= size; i += 16) {
    const uint8x16_t x = vld1q_u8(src + i);
    /* st3 interleaves the three gathered planes back into r,g,b triples. */
    const uint8x16x3_t planes = {
        gather256(r, x), gather256(g, x), gather256(b, x)};
    vst3q_u8(dst + i * 3, planes);
  }
  for (; i < size; ++i) {
    dst[i * 3 + 0] = shim->r_lut[src[i]];
    dst[i * 3 + 1] = shim->g_lut[src[i]];
    dst[i * 3 + 2] = shim->b_lut[src[i]];
  }
}

#endif  // ALE_SHIM_NEON_PALETTE

}  // namespace

extern "C" {

AleShim *ale_shim_new(void) {
  try {
    return new AleShim();
  } catch (...) {
    return nullptr;
  }
}

void ale_shim_free(AleShim *shim) { delete shim; }

const char *ale_shim_last_error(const AleShim *shim) {
  return shim == nullptr ? "" : shim->last_error.c_str();
}

int ale_shim_supported_rom(AleShim *shim, const char *path, char *out,
                           size_t capacity, size_t *out_len) {
  return guard(shim, [&] {
    const std::optional<std::string> name =
        ale::ALEInterface::isSupportedROM(std::filesystem::path(path));
    if (!name.has_value()) {
      /* Thrown so `guard` records it the same way it records ALE's own
       * failures; the caller sees one uniform error channel. */
      throw std::runtime_error(
          "ROM is not a supported cartridge (no MD5 match in ALE's table)");
    }
    if (out_len != nullptr) {
      *out_len = name->size();
    }
    if (out != nullptr && capacity > 0) {
      const size_t copied =
          name->size() < capacity - 1 ? name->size() : capacity - 1;
      std::memcpy(out, name->data(), copied);
      out[copied] = '\0';
    }
  });
}

int ale_shim_set_int(AleShim *shim, const char *key, int value) {
  return guard(shim, [&] { shim->ale.setInt(key, value); });
}

int ale_shim_set_float(AleShim *shim, const char *key, float value) {
  return guard(shim, [&] { shim->ale.setFloat(key, value); });
}

int ale_shim_get_int(AleShim *shim, const char *key, int *out_value) {
  return guard(shim, [&] {
    const int value = shim->ale.getInt(key);
    if (out_value != nullptr) {
      *out_value = value;
    }
  });
}

int ale_shim_get_float(AleShim *shim, const char *key, float *out_value) {
  return guard(shim, [&] {
    const float value = shim->ale.getFloat(key);
    if (out_value != nullptr) {
      *out_value = value;
    }
  });
}

int ale_shim_load_rom(AleShim *shim, const char *path) {
  return guard(shim, [&] {
    shim->ale.loadROM(std::filesystem::path(path));
    /* Reached only when loadROM did not throw. loadROM re-reads the ROM's
     * display format and may swap the palette, so the snapshot is stale. */
    shim->palette_luts_valid = false;
  });
}

int ale_shim_reset_game(AleShim *shim) {
  return guard(shim, [&] { shim->ale.reset_game(); });
}

int ale_shim_act(AleShim *shim, int action, float strength, int *out_reward) {
  return guard(shim, [&] {
    const ale::reward_t reward =
        shim->ale.act(static_cast<ale::Action>(action), strength);
    if (out_reward != nullptr) {
      *out_reward = reward;
    }
  });
}

int ale_shim_game_over(AleShim *shim, int with_truncation, int *out_flag) {
  return guard(shim, [&] {
    const bool over = shim->ale.game_over(with_truncation != 0);
    if (out_flag != nullptr) {
      *out_flag = over ? 1 : 0;
    }
  });
}

int ale_shim_game_truncated(AleShim *shim, int *out_flag) {
  return guard(shim, [&] {
    const bool truncated = shim->ale.game_truncated();
    if (out_flag != nullptr) {
      *out_flag = truncated ? 1 : 0;
    }
  });
}

int ale_shim_lives(AleShim *shim, int *out_lives) {
  return guard(shim, [&] {
    const int lives = shim->ale.lives();
    if (out_lives != nullptr) {
      *out_lives = lives;
    }
  });
}

int ale_shim_frame_number(AleShim *shim, int *out_frame) {
  return guard(shim, [&] {
    const int frame = shim->ale.getFrameNumber();
    if (out_frame != nullptr) {
      *out_frame = frame;
    }
  });
}

int ale_shim_episode_frame_number(AleShim *shim, int *out_frame) {
  return guard(shim, [&] {
    const int frame = shim->ale.getEpisodeFrameNumber();
    if (out_frame != nullptr) {
      *out_frame = frame;
    }
  });
}

int ale_shim_minimal_action_set(AleShim *shim, int *out, size_t capacity,
                                size_t *out_len) {
  return guard(shim, [&] {
    const ale::ActionVect actions = shim->ale.getMinimalActionSet();
    if (out_len != nullptr) {
      *out_len = actions.size();
    }
    if (out != nullptr) {
      const size_t copied = actions.size() < capacity ? actions.size() : capacity;
      for (size_t index = 0; index < copied; ++index) {
        out[index] = static_cast<int>(actions[index]);
      }
    }
  });
}

const uint8_t *ale_shim_screen(AleShim *shim, size_t *out_height,
                               size_t *out_width) {
  const uint8_t *pixels = nullptr;
  const int status = guard(shim, [&] {
    const ale::ALEScreen &screen = shim->ale.getScreen();
    if (out_height != nullptr) {
      *out_height = screen.height();
    }
    if (out_width != nullptr) {
      *out_width = screen.width();
    }
    pixels = static_cast<const uint8_t *>(screen.getArray());
  });
  return status == ALE_SHIM_OK ? pixels : nullptr;
}

const uint8_t *ale_shim_screen_grayscale(AleShim *shim, size_t *out_len) {
  const uint8_t *pixels = nullptr;
  const int status = guard(shim, [&] {
    const ale::ALEScreen &screen = shim->ale.getScreen();
    const size_t size = screen.height() * screen.width();
    if (!shim->palette_luts_valid) {
      rebuild_palette_luts(shim);
    }
    shim->grayscale.resize(size);
#if defined(ALE_SHIM_NEON_PALETTE)
    apply_grayscale_neon(screen.getArray(), size, shim->gray_lut,
                         shim->grayscale.data());
#else
    apply_grayscale_portable(screen.getArray(), size, shim->gray_lut,
                             shim->grayscale.data());
#endif
    if (out_len != nullptr) {
      *out_len = shim->grayscale.size();
    }
    pixels = shim->grayscale.data();
  });
  return status == ALE_SHIM_OK ? pixels : nullptr;
}

const uint8_t *ale_shim_screen_rgb(AleShim *shim, size_t *out_len) {
  const uint8_t *pixels = nullptr;
  const int status = guard(shim, [&] {
    const ale::ALEScreen &screen = shim->ale.getScreen();
    const size_t size = screen.height() * screen.width();
    if (!shim->palette_luts_valid) {
      rebuild_palette_luts(shim);
    }
    shim->rgb.resize(size * 3);
#if defined(ALE_SHIM_NEON_PALETTE)
    apply_rgb_neon(shim, screen.getArray(), size, shim->rgb.data());
#else
    apply_rgb_portable(shim, screen.getArray(), size, shim->rgb.data());
#endif
    if (out_len != nullptr) {
      *out_len = shim->rgb.size();
    }
    pixels = shim->rgb.data();
  });
  return status == ALE_SHIM_OK ? pixels : nullptr;
}

const char *ale_shim_version(void) { return ALE_VERSION; }

void ale_shim_set_logger_mode(int mode) {
  ale::Logger::mode resolved = ale::Logger::Error;
  if (mode == 0) {
    resolved = ale::Logger::Info;
  } else if (mode == 1) {
    resolved = ale::Logger::Warning;
  }
  ale::Logger::setMode(resolved);
}

}  // extern "C"
