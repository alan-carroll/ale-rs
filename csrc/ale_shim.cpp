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
#include <ale/common/Log.hpp>
#include <ale/version.hpp>

struct AleShim {
  ale::ALEInterface ale;
  /* Reused across calls so the grayscale path allocates only on the first
   * frame; ALE's applyPaletteGrayscale resizes, which is a no-op afterwards. */
  std::vector<unsigned char> grayscale;
  std::string last_error;
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

int ale_shim_load_rom(AleShim *shim, const char *path) {
  return guard(shim, [&] { shim->ale.loadROM(std::filesystem::path(path)); });
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
    shim->ale.getScreenGrayscale(shim->grayscale);
    if (out_len != nullptr) {
      *out_len = shim->grayscale.size();
    }
    pixels = shim->grayscale.data();
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
