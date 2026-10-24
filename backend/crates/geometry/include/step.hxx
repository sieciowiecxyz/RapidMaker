#pragma once
#include "rust/cxx.h"

namespace rapidmaker {
struct StepMesh;
StepMesh tessellate_step(rust::Slice<const std::uint8_t> input,
                         std::size_t max_triangles, std::size_t max_vertices);
} // namespace rapidmaker
