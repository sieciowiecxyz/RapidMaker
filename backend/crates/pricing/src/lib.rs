//! Existing POC tariff only; real slicer-based costs and calibration are pending.
use serde::Serialize;

pub const SETUP_GROSZE: u32 = 1500;
const UNIT_GROSZE: u32 = 2400;

#[derive(Serialize)]
pub struct PrintLine {
    pub model_id: String,
    pub quantity: u32,
    pub unit_grosze: u32,
    pub total_grosze: u32,
}

pub fn price_line(model_id: String, quantity: u32) -> PrintLine {
    PrintLine {
        model_id,
        quantity,
        unit_grosze: UNIT_GROSZE,
        total_grosze: quantity.saturating_mul(UNIT_GROSZE),
    }
}
