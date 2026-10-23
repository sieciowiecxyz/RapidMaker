//! Integer costs and aggregate order pricing from measured slicer results.
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct PrintLine {
    pub model_id: String,
    pub quantity: u32,
    pub unit_grosze: u32,
    pub total_grosze: u32,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
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

pub struct OrderPart<'a> {
    pub model_id: &'a str,
    pub quantity: u32,
    pub price: &'a Price,
}

pub struct OrderPrice {
    pub price: Price,
    pub lines: Vec<PrintLine>,
    /// Customer-facing gross allocation, not the configured net setup cost.
    pub setup_grosze: u32,
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
    finish(filament, machine, energy, handling, rates)
}

/// Combine measured costs; per-model setup, minimum and markup are discarded.
/// The gross total is apportioned by cumulative cost weights so rounding cannot
/// create or lose money. A minimum with no cost base goes to the first line.
pub fn combine(parts: &[OrderPart<'_>], rates: &Rates) -> Result<OrderPrice, &'static str> {
    rates.validate()?;
    if parts.is_empty()
        || parts.len() > 1000
        || parts
            .iter()
            .any(|part| part.model_id.is_empty() || !(1..=1000).contains(&part.quantity))
    {
        return Err("Invalid order parts");
    }
    let sum =
        |field: fn(&Price) -> u32| parts.iter().map(|part| u128::from(field(part.price))).sum();
    let price = finish(
        sum(|p| p.filament_grosze),
        sum(|p| p.machine_grosze),
        sum(|p| p.energy_grosze),
        sum(|p| p.handling_grosze),
        rates,
    )?;
    let weights: Vec<u128> = parts
        .iter()
        .map(|part| {
            let p = part.price;
            u128::from(p.filament_grosze)
                + u128::from(p.machine_grosze)
                + u128::from(p.energy_grosze)
                + u128::from(p.handling_grosze)
        })
        .collect();
    let base = weights.iter().sum::<u128>() + u128::from(rates.setup_grosze);
    let gross = u128::from(price.gross_grosze);
    let mut cumulative = u128::from(rates.setup_grosze);
    let setup_grosze = (gross * cumulative).checked_div(base).unwrap_or(0) as u32;
    let mut allocated = setup_grosze;
    let lines = parts
        .iter()
        .zip(weights)
        .map(|(part, weight)| {
            cumulative += weight;
            let through_line = (gross * cumulative).checked_div(base).unwrap_or(gross) as u32;
            let total_grosze = through_line - allocated;
            allocated = through_line;
            PrintLine {
                model_id: part.model_id.to_owned(),
                quantity: part.quantity,
                unit_grosze: total_grosze.div_ceil(part.quantity),
                total_grosze,
            }
        })
        .collect();
    Ok(OrderPrice {
        price,
        lines,
        setup_grosze,
    })
}

fn finish(
    filament: u128,
    machine: u128,
    energy: u128,
    handling: u128,
    rates: &Rates,
) -> Result<Price, &'static str> {
    let base = filament + machine + energy + handling + u128::from(rates.setup_grosze);
    let loss = (base * u128::from(rates.loss_bps)).div_ceil(10_000);
    let cost = base + loss;
    let net = (cost * 10_000)
        .div_ceil(u128::from(10_000 - rates.margin_bps))
        .max(u128::from(rates.minimum_net_grosze));
    let vat = (net * u128::from(rates.vat_bps)).div_ceil(10_000);
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

    #[test]
    fn order_combines_costs_before_setup_minimum_margin_and_vat() {
        let mut rates = rates();
        rates.minimum_net_grosze = 10_000;
        let first = calculate(3600, 100_000, 1, 8000, &rates).unwrap();
        let second = calculate(7200, 200_000, 2, 8000, &rates).unwrap();
        let parts = [
            OrderPart {
                model_id: "first",
                quantity: 1,
                price: &first,
            },
            OrderPart {
                model_id: "second",
                quantity: 2,
                price: &second,
            },
        ];
        let order = combine(&parts, &rates).unwrap();
        let measured_together = calculate(10800, 300_000, 3, 8000, &rates).unwrap();
        assert_eq!(order.price.gross_grosze, measured_together.gross_grosze);
        assert_eq!(order.price.setup_grosze, rates.setup_grosze);
        assert_eq!(order.price.loss_grosze, measured_together.loss_grosze);
        assert!(order.price.gross_grosze < first.gross_grosze + second.gross_grosze);
        assert!(
            order.lines[1]
                .total_grosze
                .abs_diff(2 * order.lines[0].total_grosze)
                <= 1
        );
        assert_eq!(
            order.setup_grosze + order.lines.iter().map(|p| p.total_grosze).sum::<u32>(),
            order.price.gross_grosze
        );
    }

    #[test]
    fn allocations_preserve_cents_and_zero_cost_minimum_without_overflow() {
        let mut rates = rates();
        let mut price = calculate(1, 1, 1, 1, &rates).unwrap();
        for setup in [0, 1, 10] {
            rates.setup_grosze = setup;
            for filament in [0, 1, 7] {
                price.filament_grosze = filament;
                price.machine_grosze = 0;
                price.energy_grosze = 0;
                price.handling_grosze = 0;
                rates.minimum_net_grosze = 101;
                let parts = [
                    OrderPart {
                        model_id: "first",
                        quantity: 3,
                        price: &price,
                    },
                    OrderPart {
                        model_id: "second",
                        quantity: 2,
                        price: &price,
                    },
                ];
                let order = combine(&parts, &rates).unwrap();
                assert_eq!(
                    order.setup_grosze + order.lines.iter().map(|p| p.total_grosze).sum::<u32>(),
                    order.price.gross_grosze
                );
                if setup == 0 && filament == 0 {
                    assert_eq!(order.lines[0].total_grosze, order.price.gross_grosze);
                    assert_eq!(order.lines[1].total_grosze, 0);
                }
            }
        }
        price.filament_grosze = u32::MAX;
        assert!(
            combine(
                &[OrderPart {
                    model_id: "overflow",
                    quantity: 1,
                    price: &price
                }],
                &rates
            )
            .is_err()
        );
        assert!(combine(&[], &rates).is_err());
        price.filament_grosze = 1;
        assert!(
            combine(
                &[OrderPart {
                    model_id: "invalid",
                    quantity: 0,
                    price: &price
                }],
                &rates
            )
            .is_err()
        );
    }
}
