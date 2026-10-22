//! Integer cost calculation; the old checkout tariff remains until its adapter is replaced.
use serde::{Deserialize, Serialize};

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

/// All rates are net PLN grosze. No guessed purchase prices or implicit defaults.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Rates {
    pub version: String,
    pub setup_grosze: u32,
    pub handling_per_plate_grosze: u32,
    pub machine_per_hour_grosze: u32,
    pub energy_per_kwh_grosze: u32,
    pub printer_watts: u32,
    pub loss_bps: u32,
    pub margin_bps: u32,
    pub vat_bps: u32,
    pub minimum_net_grosze: u32,
}

#[derive(Debug, Serialize)]
pub struct Price {
    pub filament_grosze: u32,
    pub machine_grosze: u32,
    pub energy_grosze: u32,
    pub handling_grosze: u32,
    pub setup_grosze: u32,
    pub loss_grosze: u32,
    pub cost_net_grosze: u32,
    pub net_grosze: u32,
    pub vat_grosze: u32,
    pub gross_grosze: u32,
}

impl Rates {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.version.is_empty()
            || self.version.len() > 128
            || self.margin_bps == 0
            || self.margin_bps >= 10_000
            || self.loss_bps > 10_000
            || self.vat_bps > 10_000
            || self.printer_watts > 10_000
        {
            return Err("Invalid pricing configuration");
        }
        Ok(())
    }
}

/// Inputs are measured totals across all plates, not per-copy guesses.
/// Setup is charged once and plate handling follows the actual batch plan.
pub fn calculate(
    seconds: u64,
    filament_milligrams: u64,
    plates: u32,
    purchase_grosze_per_kg: u32,
    rates: &Rates,
) -> Result<Price, &'static str> {
    rates.validate()?;
    if !(1..=1000).contains(&plates)
        || seconds == 0
        || seconds > 365 * 24 * 3600
        || filament_milligrams == 0
        || filament_milligrams > 1_000_000_000
        || purchase_grosze_per_kg == 0
    {
        return Err("Invalid slicer measurement, plate count or purchase price");
    }
    let ceil = |n: u128, d: u128| n.div_ceil(d);
    let time = u128::from(seconds);
    let filament = ceil(
        u128::from(filament_milligrams) * u128::from(purchase_grosze_per_kg),
        1_000_000,
    );
    let machine = ceil(time * u128::from(rates.machine_per_hour_grosze), 3600);
    let energy = ceil(
        time * u128::from(rates.printer_watts) * u128::from(rates.energy_per_kwh_grosze),
        3_600_000,
    );
    let handling = u128::from(plates) * u128::from(rates.handling_per_plate_grosze);
    let base = filament + machine + energy + handling + u128::from(rates.setup_grosze);
    let loss = ceil(base * u128::from(rates.loss_bps), 10_000);
    let cost = base + loss;
    let net = ceil(cost * 10_000, u128::from(10_000 - rates.margin_bps))
        .max(u128::from(rates.minimum_net_grosze));
    let vat = ceil(net * u128::from(rates.vat_bps), 10_000);
    let amount = |v| u32::try_from(v).map_err(|_| "Price exceeds supported range");
    Ok(Price {
        filament_grosze: amount(filament)?,
        machine_grosze: amount(machine)?,
        energy_grosze: amount(energy)?,
        handling_grosze: amount(handling)?,
        setup_grosze: rates.setup_grosze,
        loss_grosze: amount(loss)?,
        cost_net_grosze: amount(cost)?,
        net_grosze: amount(net)?,
        vat_grosze: amount(vat)?,
        gross_grosze: amount(net + vat)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rates() -> Rates {
        Rates {
            version: "test".into(),
            setup_grosze: 1000,
            handling_per_plate_grosze: 100,
            machine_per_hour_grosze: 200,
            energy_per_kwh_grosze: 120,
            printer_watts: 150,
            loss_bps: 500,
            margin_bps: 2000,
            vat_bps: 2300,
            minimum_net_grosze: 0,
        }
    }

    #[test]
    fn measured_costs_scale_without_repeating_order_setup() {
        let rates = rates();
        let one = calculate(3600, 100_000, 1, 8000, &rates).unwrap();
        assert_eq!(
            (one.filament_grosze, one.machine_grosze, one.energy_grosze),
            (800, 200, 18)
        );
        for quantity in [2, 10, 100] {
            let price = calculate(
                3600 * u64::from(quantity),
                100_000 * u64::from(quantity),
                quantity,
                8000,
                &rates,
            )
            .unwrap();
            assert_eq!(price.setup_grosze, 1000);
            assert!(price.net_grosze > price.cost_net_grosze);
            assert!(price.gross_grosze > one.gross_grosze);
            assert!(price.gross_grosze < one.gross_grosze * quantity);
        }
        let shared = calculate(6000, 200_000, 1, 8000, &rates).unwrap();
        let separate = calculate(7200, 200_000, 2, 8000, &rates).unwrap();
        assert_eq!(shared.handling_grosze, 100);
        assert_eq!(shared.filament_grosze, 1600);
        assert!(shared.net_grosze < separate.net_grosze);
        assert!(
            calculate(3600, 200_000, 1, 8000, &rates)
                .unwrap()
                .net_grosze
                > one.net_grosze
        );
    }

    #[test]
    fn missing_prices_invalid_margins_and_overflow_cannot_issue_prices() {
        assert!(calculate(1, 1, 1, 0, &rates()).is_err());
        let mut rates = rates();
        rates.margin_bps = 10_000;
        assert!(calculate(1, 1, 1, 100, &rates).is_err());
        rates.margin_bps = 9999;
        rates.machine_per_hour_grosze = u32::MAX;
        assert!(calculate(604_800, 10_000_000, 1000, u32::MAX, &rates).is_err());
    }
}
