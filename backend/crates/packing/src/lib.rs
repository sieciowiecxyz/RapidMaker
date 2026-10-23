//! Local, uncalibrated packing estimates. Client-entered geometry is never verified geometry.
use serde::{Deserialize, Serialize};

const PADDING_MM: u32 = 4;
const MAX_PARCELS: usize = 30;
// Uncalibrated safety reserve for cushioning/irregular shapes, in addition to
// per-part padding. Volume alone never proves that parts can be packed.
const MAX_FILL_PERCENT: u64 = 67;

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Part {
    pub id: String,
    pub quantity: u32,
    #[serde(default)]
    pub analysis_id: Option<String>,
    pub dimensions_mm: Option<[u32; 3]>,
    pub weight_g: Option<u32>,
}
#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    Courier,
    Locker,
    #[serde(alias = "collection")]
    Pickup,
}
#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EstimateRequest {
    pub parts: Vec<Part>,
    pub delivery: Delivery,
}
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct Placement {
    pub part_id: String,
    /// Position and occupied dimensions include protective padding, in millimetres.
    pub position_mm: [u32; 3],
    pub dimensions_mm: [u32; 3],
}
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct Parcel {
    pub box_id: &'static str,
    pub outer_mm: [u32; 3],
    pub weight_g: u32,
    pub placements: Vec<Placement>,
}
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct Estimate {
    pub status: &'static str,
    pub parcels: Vec<Parcel>,
    pub packaging_grosze: u32,
    pub reason: Option<&'static str>,
    pub geometry_source: &'static str,
}
impl Estimate {
    fn manual(reason: &'static str) -> Self {
        Self {
            status: "manual_review",
            parcels: vec![],
            packaging_grosze: 0,
            reason: Some(reason),
            geometry_source: "customer_estimate",
        }
    }
}
#[derive(Clone, Copy)]
struct Carton {
    id: &'static str,
    outer: [u32; 3],
    tare: u32,
    cost: u32,
}
const CARTONS: [Carton; 4] = [
    Carton {
        id: "S",
        outer: [180, 130, 90],
        tare: 160,
        cost: 250,
    },
    Carton {
        id: "M",
        outer: [300, 220, 150],
        tare: 280,
        cost: 400,
    },
    Carton {
        id: "L",
        outer: [400, 300, 250],
        tare: 450,
        cost: 600,
    },
    Carton {
        id: "XL",
        outer: [600, 400, 400],
        tare: 750,
        cost: 900,
    },
];
#[derive(Clone)]
struct Unit {
    id: String,
    size: [u32; 3],
    weight: u32,
}
struct Space {
    position: [u32; 3],
    size: [u32; 3],
}
fn orientations([x, y, z]: [u32; 3]) -> [[u32; 3]; 6] {
    [
        [x, y, z],
        [x, z, y],
        [y, x, z],
        [y, z, x],
        [z, x, y],
        [z, y, x],
    ]
}
fn volume(size: [u32; 3]) -> u64 {
    size.into_iter().map(u64::from).product()
}
fn fill(carton: Carton, units: &[Unit]) -> (Parcel, Vec<usize>) {
    let interior = carton.outer.map(|d| d - 10);
    let volume_limit = volume(interior) * MAX_FILL_PERCENT / 100;
    let mut used_volume = 0;
    let mut spaces = vec![Space {
        position: [0; 3],
        size: interior,
    }];
    let mut parcel = Parcel {
        box_id: carton.id,
        outer_mm: carton.outer,
        weight_g: carton.tare,
        placements: vec![],
    };
    let mut assigned = vec![];
    for (index, unit) in units.iter().enumerate() {
        // 20 kg includes carton and a conservative 10% packing allowance.
        if parcel.weight_g + unit.weight + unit.weight.div_ceil(10) > 20_000
            || used_volume + volume(unit.size) > volume_limit
        {
            continue;
        }
        let found = spaces.iter().enumerate().find_map(|(i, space)| {
            orientations(unit.size)
                .into_iter()
                .find(|size| (0..3).all(|axis| size[axis] <= space.size[axis]))
                .map(|size| (i, size))
        });
        let Some((space_index, size)) = found else {
            continue;
        };
        let space = spaces.swap_remove(space_index);
        // Three disjoint remainder boxes: no overlap and no volume-only assumptions.
        for axis in 0..3 {
            let mut remainder = space.size;
            let mut position = space.position;
            remainder[axis] -= size[axis];
            position[axis] += size[axis];
            remainder[..axis].copy_from_slice(&size[..axis]);
            if remainder.iter().all(|&d| d > 0) {
                spaces.push(Space {
                    position,
                    size: remainder,
                });
            }
        }
        parcel.weight_g += unit.weight + unit.weight.div_ceil(10);
        used_volume += volume(size);
        parcel.placements.push(Placement {
            part_id: unit.id.clone(),
            position_mm: space.position,
            dimensions_mm: size,
        });
        assigned.push(index);
    }
    (parcel, assigned)
}

pub fn estimate(request: EstimateRequest) -> Estimate {
    let locker = matches!(request.delivery, Delivery::Locker);
    // POC: lockers use our S/M/L cartons, one parcel per order. The provider
    // still validates the actual parcel; this is not a universal carrier limit.
    let cartons = if locker { &CARTONS[..3] } else { &CARTONS[..] };
    if request.parts.is_empty() || request.parts.len() > 20 {
        return Estimate::manual("Dodaj od 1 do 20 różnych części.");
    }
    let mut total = 0_u32;
    let mut ids = std::collections::HashSet::new();
    let mut units = vec![];
    for part in request.parts {
        if part.id.is_empty()
            || part.id.len() > 100
            || !ids.insert(part.id.clone())
            || part.quantity == 0
            || part.quantity > 1000
        {
            return Estimate::manual("Nieprawidłowa część lub liczba sztuk (1–1000).");
        }
        total += part.quantity;
        if total > 2000 {
            return Estimate::manual("Duże zamówienie wymaga ręcznej oceny logistyki.");
        }
        // Collection does not require a parcel or a carrier. Production validation remains separate.
        if matches!(request.delivery, Delivery::Pickup) {
            continue;
        }
        let (Some(dimensions), Some(weight)) = (part.dimensions_mm, part.weight_g) else {
            return Estimate::manual(
                "Do wyceny dostawy potrzebna jest analiza wymiarów i masy modeli. Możesz wybrać odbiór osobisty lub poprosić o wycenę.",
            );
        };
        if dimensions.iter().any(|&d| d == 0 || d > 2000) || weight == 0 || weight > 100_000 {
            return Estimate::manual("Nieprawidłowe wymiary lub masa części.");
        }
        let size = dimensions.map(|d| d + PADDING_MM * 2);

        if !cartons.iter().any(|c| {
            orientations(size)
                .iter()
                .any(|s| (0..3).all(|a| s[a] <= c.outer[a] - 10))
                && c.tare + weight + weight.div_ceil(10) <= 20_000
                && volume(size) <= volume(c.outer.map(|d| d - 10)) * MAX_FILL_PERCENT / 100
        }) {
            return Estimate::manual(
                "Część przekracza bezpieczny gabaryt lub masę paczki; ustalimy dostawę indywidualnie.",
            );
        }
        for _ in 0..part.quantity {
            units.push(Unit {
                id: part.id.clone(),
                size,
                weight,
            });
        }
    }
    let mut result = Estimate {
        status: "estimated",
        parcels: vec![],
        packaging_grosze: 0,
        reason: None,
        geometry_source: "customer_estimate",
    };
    units.sort_by_key(|unit| {
        std::cmp::Reverse(unit.size.iter().map(|&d| u64::from(d)).product::<u64>())
    });
    while !units.is_empty() {
        if locker && !result.parcels.is_empty() {
            return Estimate::manual(
                "Ta partia wymaga kilku paczek. Wybierz kuriera lub odbiór osobisty.",
            );
        }
        if result.parcels.len() == MAX_PARCELS {
            return Estimate::manual(
                "Zamówienie wymaga ponad 30 paczek. Porównamy transport paletowy i dostawę zbiorczą w indywidualnej wycenie; dostępny jest też odbiór osobisty.",
            );
        }
        // Prefer more pieces per parcel, then cheaper packaging. The old
        // packaging-cost/piece score could favor many small, separately charged
        // parcels. Actual carrier prices still decide between available offers;
        // this bounded greedy heuristic does not claim a global cost optimum.
        let (carton, parcel, assigned) = cartons
            .iter()
            .copied()
            .filter_map(|carton| {
                let (parcel, assigned) = fill(carton, &units);
                if assigned.is_empty() {
                    None
                } else {
                    Some((carton, parcel, assigned))
                }
            })
            .min_by_key(|(carton, _, assigned)| (std::cmp::Reverse(assigned.len()), carton.cost))
            .expect("each validated unit fits at least one carton");
        let mut index = 0;
        let mut assigned = assigned.into_iter().peekable();
        units.retain(|_| {
            let keep = assigned.peek() != Some(&index);
            if !keep {
                assigned.next();
            }
            index += 1;
            keep
        });
        result.packaging_grosze += carton.cost;
        result.parcels.push(parcel);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    fn part(id: &str, quantity: u32, size: [u32; 3], weight: u32) -> Part {
        Part {
            id: id.into(),
            quantity,
            analysis_id: None,
            dimensions_mm: Some(size),
            weight_g: Some(weight),
        }
    }
    fn request(parts: Vec<Part>) -> EstimateRequest {
        EstimateRequest {
            parts,
            delivery: Delivery::Courier,
        }
    }
    fn verify(plan: &Estimate, expected: usize) {
        assert_eq!(plan.status, "estimated", "{:?}", plan.reason);
        assert_eq!(
            plan.parcels
                .iter()
                .map(|p| p.placements.len())
                .sum::<usize>(),
            expected
        );
        for parcel in &plan.parcels {
            assert!(parcel.weight_g <= 20_000);
            let occupied: u64 = parcel
                .placements
                .iter()
                .map(|p| volume(p.dimensions_mm))
                .sum();
            assert!(occupied <= volume(parcel.outer_mm.map(|d| d - 10)) * MAX_FILL_PERCENT / 100);
            for (i, a) in parcel.placements.iter().enumerate() {
                assert!(
                    (0..3)
                        .all(|d| a.position_mm[d] + a.dimensions_mm[d] <= parcel.outer_mm[d] - 10)
                );
                for b in &parcel.placements[i + 1..] {
                    assert!(
                        (0..3).any(
                            |d| a.position_mm[d] + a.dimensions_mm[d] <= b.position_mm[d]
                                || b.position_mm[d] + b.dimensions_mm[d] <= a.position_mm[d]
                        ),
                        "overlapping pieces"
                    );
                }
            }
        }
    }
    #[test]
    fn thousand_pieces_need_more_than_one_parcel_and_cost_more() {
        let one = estimate(request(vec![part("a", 1, [50, 50, 50], 20)]));
        let bulk = estimate(request(vec![part("a", 1000, [50, 50, 50], 20)]));
        verify(&one, 1);
        verify(&bulk, 1000);
        assert!(bulk.parcels.len() > one.parcels.len());
        assert!(bulk.packaging_grosze > one.packaging_grosze);
    }
    #[test]
    fn mixed_parts_are_assigned_without_overlap() {
        let plan = estimate(request(vec![
            part("a", 12, [45, 70, 20], 100),
            part("b", 8, [80, 20, 30], 80),
        ]));
        verify(&plan, 20);
        assert_eq!(
            plan.parcels
                .iter()
                .flat_map(|p| &p.placements)
                .filter(|p| p.part_id == "b")
                .count(),
            8
        );
    }
    #[test]
    fn chooses_one_larger_carton_over_multiple_small_cartons() {
        let plan = estimate(request(vec![part("a", 200, [10, 10, 10], 2)]));
        verify(&plan, 200);
        assert_eq!(plan.parcels.len(), 1);
        assert_eq!(plan.parcels[0].box_id, "M");
    }
    #[test]
    fn void_reserve_upsizes_a_physically_fitting_part() {
        // Fits S geometrically with padding, but consumes more than 67% of
        // its interior. Use M rather than promising inadequate protection.
        let plan = estimate(request(vec![part("a", 1, [150, 100, 65], 100)]));
        verify(&plan, 1);
        assert_eq!(plan.parcels[0].box_id, "M");
    }
    #[test]
    fn rotations_and_protection_are_respected() {
        verify(
            &estimate(request(vec![part("long", 1, [60, 150, 80], 50)])),
            1,
        );
        assert_eq!(
            estimate(request(vec![part("too_long", 1, [590, 20, 20], 50)])).status,
            "manual_review"
        );
    }
    #[test]
    fn mass_splits_parcels_and_single_overweight_is_manual() {
        let plan = estimate(request(vec![part("a", 4, [10, 10, 10], 10_000)]));
        verify(&plan, 4);
        assert_eq!(plan.parcels.len(), 4);
        assert_eq!(
            estimate(request(vec![part("a", 1, [10, 10, 10], 20_000)])).status,
            "manual_review"
        );
    }
    #[test]
    fn missing_geometry_and_invalid_input_do_not_produce_payable_plan() {
        let mut missing = part("a", 1, [10, 10, 10], 1);
        missing.dimensions_mm = None;
        for req in [
            request(vec![missing]),
            request(vec![part("a", 0, [1, 1, 1], 1)]),
            request(vec![part("a", 1001, [1, 1, 1], 1)]),
            request(vec![part("a", 1, [0, 1, 1], 1)]),
            request(vec![part("a", 1, [u32::MAX, 1, 1], 1)]),
            request(vec![]),
            request(vec![
                part("same", 1, [1, 1, 1], 1),
                part("same", 1, [1, 1, 1], 1),
            ]),
        ] {
            let plan = estimate(req);
            assert_eq!(plan.status, "manual_review");
            assert!(plan.parcels.is_empty());
        }
    }
    #[test]
    fn parcel_cap_never_returns_partial_plan() {
        let plan = estimate(request(vec![part("a", 1000, [500, 300, 300], 1000)]));
        assert_eq!(plan.status, "manual_review");
        assert!(plan.parcels.is_empty());
    }
    #[test]
    fn collection_does_not_need_geometry_or_carrier() {
        let plan = estimate(EstimateRequest {
            parts: vec![Part {
                id: "a".into(),
                quantity: 1000,
                analysis_id: None,
                dimensions_mm: None,
                weight_g: None,
            }],
            delivery: Delivery::Pickup,
        });
        verify(&plan, 0);
        assert_eq!(plan.packaging_grosze, 0);
    }
    #[test]
    fn json_rejects_negative_fractional_and_unknown_fields() {
        for json in [
            r#"{"parts":[{"id":"a","quantity":-1}],"delivery":"courier"}"#,
            r#"{"parts":[{"id":"a","quantity":1,"weight_g":1.1}],"delivery":"courier"}"#,
            r#"{"parts":[],"delivery":"courier","price_grosze":1}"#,
        ] {
            assert!(serde_json::from_str::<EstimateRequest>(json).is_err());
        }
    }
}
