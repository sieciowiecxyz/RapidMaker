#[cxx::bridge(namespace = "rapidmaker")]
pub(crate) mod ffi {
    enum StepStatus {
        Ok,
        TooManyTriangles,
        TooManyVertices,
        Unsupported,
    }

    struct StepPoint {
        x: f64,
        y: f64,
        z: f64,
    }

    struct StepMesh {
        status: StepStatus,
        points: Vec<StepPoint>,
    }

    // SAFETY: CXX owns the returned vectors and validates the borrowed slice ABI.
    // The bridge retains no pointers, bounds extraction, translates OCCT failures
    // to CXX Result and runs under the existing disposable parser sandbox.
    unsafe extern "C++" {
        include!("step.hxx");
        fn tessellate_step(
            input: &[u8],
            max_triangles: usize,
            max_vertices: usize,
        ) -> Result<StepMesh>;
    }
}
