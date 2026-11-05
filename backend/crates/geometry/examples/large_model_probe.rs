//! Diagnostic full-geometry preview conversion; no files are persisted.
//! Run only with external RAM/CPU/wall limits and isolation. These allowances
//! do not change API defaults or validate production STL, GLTF metre transforms,
//! or quotations.

use std::{fs::File, io::Read, path::PathBuf, process::ExitCode, time::Instant};

use rapidmaker_geometry::{ConversionLimits, PreviewOutput, ThreeMfLimits, source_to_preview_glb};

const MAX_INPUT_BYTES: usize = 96 * 1024 * 1024;

fn probe() -> Result<PreviewOutput, String> {
    let path = PathBuf::from(std::env::args_os().nth(1).ok_or("MissingInput")?);
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("InvalidFilename")?;
    let mut input = Vec::new();
    File::open(&path)
        .map_err(|_| "InputIoError")?
        .take(MAX_INPUT_BYTES as u64 + 1)
        .read_to_end(&mut input)
        .map_err(|_| "InputIoError")?;
    if input.len() > MAX_INPUT_BYTES {
        return Err("InputTooLarge".into());
    }
    source_to_preview_glb(
        filename,
        &input,
        &ConversionLimits {
            max_input_bytes: MAX_INPUT_BYTES,
            max_output_glb_bytes: 128 * 1024 * 1024,
            max_vertices: 4_000_000,
            max_triangles: 8_000_000,
            max_parse_seconds: 25,
            ..ConversionLimits::default()
        },
        &ThreeMfLimits {
            max_entry_count: 512,
            max_uncompressed_bytes: 512 * 1024 * 1024,
            max_entry_bytes: 512 * 1024 * 1024,
        },
    )
    .map_err(|error| format!("{error:?}"))
}

fn main() -> ExitCode {
    let started = Instant::now();
    let result = probe().and_then(|output| {
        let analysis = &output.analysis;
        if analysis
            .dimensions_mm
            .iter()
            .chain(&analysis.bounds_min_mm)
            .chain(&analysis.bounds_max_mm)
            .all(|v| v.is_finite())
        {
            Ok(output)
        } else {
            Err("NonFiniteCoordinate".into())
        }
    });
    match result {
        Ok(output) => {
            let analysis = &output.analysis;
            println!(
                "{{\"status\":\"ok\",\"vertex_count\":{},\"triangle_count\":{},\"dimensions_mm\":{:?},\"bounds_min_mm\":{:?},\"bounds_max_mm\":{:?},\"glb_bytes\":{},\"elapsed_ms\":{}}}",
                analysis.vertex_count,
                analysis.triangle_count,
                analysis.dimensions_mm,
                analysis.bounds_min_mm,
                analysis.bounds_max_mm,
                output.glb.len(),
                started.elapsed().as_millis()
            );
            drop(output);
            ExitCode::SUCCESS
        }
        Err(error) => {
            println!(
                "{{\"status\":\"error\",\"error\":\"{error}\",\"elapsed_ms\":{}}}",
                started.elapsed().as_millis()
            );
            ExitCode::FAILURE
        }
    }
}
