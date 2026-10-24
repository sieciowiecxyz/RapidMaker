#include "rapidmaker-geometry/src/step.rs.h"
#include <BRepMesh_IncrementalMesh.hxx>
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
    TopExp_Explorer solids(shape, TopAbs_SOLID);
    if (!solids.More()) {
      return {StepStatus::Unsupported, {}};
    }
    solids.Next();
    if (solids.More()) {
      return {StepStatus::Unsupported, {}};
    }
    BRepMesh_IncrementalMesh mesher(shape, 0.01, false, 0.5, false);
    if (!mesher.IsDone()) {
      throw std::runtime_error("STEP tessellation failed");
    }
    StepMesh result{StepStatus::Ok, {}};
    for (TopExp_Explorer faces(shape, TopAbs_FACE); faces.More();
         faces.Next()) {
      const auto face = TopoDS::Face(faces.Current());
      TopLoc_Location location;
      const auto mesh = BRep_Tool::Triangulation(face, location);
      if (mesh.IsNull()) {
        throw std::runtime_error("Missing STEP face mesh");
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
