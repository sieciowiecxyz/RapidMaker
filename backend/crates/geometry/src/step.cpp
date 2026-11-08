#include "rapidmaker-geometry/src/step.rs.h"
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRepGProp.hxx>
#include <GProp_GProps.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <Precision.hxx>
#include <cmath>
#include <vector>
#include <BRep_Tool.hxx>
#include <Message.hxx>
#include <Message_Messenger.hxx>
#include <STEPControl_Reader.hxx>
#include <Standard_Failure.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <mutex>
#include <sstream>
#include <stdexcept>
#include <utility>

namespace rapidmaker {
StepMesh tessellate_step(rust::Slice<const std::uint8_t> input,
                         std::size_t max_triangles, std::size_t max_vertices) {
  // OCCT's STEP controller and default messenger have process-global state.
  static std::mutex mutex;
  const std::lock_guard<std::mutex> lock(mutex);
  try {
    // Keep customer geometry and OCCT diagnostics out of the worker JSON
    // stream.
    Message::DefaultMessenger()->ChangePrinters().Clear();
    std::istringstream stream(std::string(input.begin(), input.end()));
    STEPControl_Reader reader;
    if (reader.ReadStream("rapidmaker.step", stream) != IFSelect_RetDone) {
      throw std::runtime_error("Invalid STEP");
    }
    if (reader.NbRootsForTransfer() != 1 || reader.TransferRoots() != 1) {
      return {StepStatus::Unsupported, {}};
    }
    const auto shape = reader.OneShape();
    if (shape.IsNull()) {
      throw std::runtime_error("Empty STEP");
    }
    std::vector<TopoDS_Shape> solids;
    for (TopExp_Explorer item(shape, TopAbs_SOLID); item.More(); item.Next()) {
      if (solids.size() == 32) return {StepStatus::Unsupported, {}};
      solids.push_back(item.Current());
    }
    if (solids.empty()) return {StepStatus::Unsupported, {}};
    if (solids.size() > 1) {
      for (std::size_t i = 0; i < solids.size(); ++i) {
        if (!BRepCheck_Analyzer(solids[i]).IsValid()) return {StepStatus::Unsupported, {}};
        for (std::size_t j = 0; j < i; ++j) {
          if (solids[i].IsSame(solids[j])) return {StepStatus::Unsupported, {}};
          BRepExtrema_DistShapeShape separation(solids[i], solids[j]);
          if (!separation.IsDone() || separation.InnerSolution() ||
              !std::isfinite(separation.Value()) || separation.Value() <= Precision::Confusion()) {
            return {StepStatus::Unsupported, {}};
          }
        }
      }
    }
    BRepMesh_IncrementalMesh mesher(shape, 0.01, false, 0.5, false);
    if (!mesher.IsDone()) {
      throw std::runtime_error("STEP tessellation failed");
    }
    StepMesh result{StepStatus::Ok, {}};
    double unmeshed_area = 0.0;
    for (TopExp_Explorer faces(shape, TopAbs_FACE); faces.More();
         faces.Next()) {
      const auto face = TopoDS::Face(faces.Current());
      TopLoc_Location location;
      const auto mesh = BRep_Tool::Triangulation(face, location);
      if (mesh.IsNull()) {
        if (!BRepCheck_Analyzer(face).IsValid()) throw std::runtime_error("Missing STEP face mesh");
        for (const auto &solid : solids) {
          if (!BRepCheck_Analyzer(solid).IsValid()) throw std::runtime_error("Missing STEP face mesh");
        }
        GProp_GProps properties;
        BRepGProp::SurfaceProperties(face, properties, false, false);
        unmeshed_area += std::abs(properties.Mass());
        if (!std::isfinite(unmeshed_area) || unmeshed_area > Precision::SquareConfusion()) {
          throw std::runtime_error("Missing STEP face mesh");
        }
        continue;
      }
      for (int i = 1; i <= mesh->NbTriangles(); ++i) {
        if (result.points.size() / 3 >= max_triangles) {
          return {StepStatus::TooManyTriangles, {}};
        }
        if (max_vertices < 3 || result.points.size() > max_vertices - 3) {
          return {StepStatus::TooManyVertices, {}};
        }
        int a, b, c;
        mesh->Triangle(i).Get(a, b, c);
        if (face.Orientation() == TopAbs_REVERSED) {
          std::swap(b, c);
        }
        for (int index : {a, b, c}) {
          if (index < 1 || index > mesh->NbNodes()) {
            throw std::runtime_error("Invalid STEP mesh index");
          }
          const auto point =
              mesh->Node(index).Transformed(location.Transformation());
          result.points.push_back({point.X(), point.Y(), point.Z()});
        }
      }
    }
    return result;
  } catch (const Standard_Failure &) {
    // Standard_Failure is not std::exception; CXX cannot translate it directly.
    // Do not propagate OCCT diagnostics (which may contain customer geometry).
    throw std::runtime_error("Invalid STEP geometry");
  }
}
} // namespace rapidmaker
