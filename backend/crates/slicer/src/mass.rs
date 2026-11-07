//! Exact, bounded decimal masses from Bambu JSON; no per-plate milligram rounding.
pub(super) fn milligrams<'a>(batches: impl Iterator<Item = (&'a [String], u32)>) -> Option<u64> {
    let (mut total, mut denominator) = (0_u128, 1_u128);
    for (tokens, repeats) in batches {
        if tokens.is_empty() || repeats == 0 {
            return None;
        }
        for token in tokens {
            let (value, scale) = decimal(token)?;
            let common = denominator.max(scale);
            total = total.checked_mul(common / denominator)?.checked_add(
                value
                    .checked_mul(common / scale)?
                    .checked_mul(u128::from(repeats))?,
            )?;
            denominator = common;
        }
    }
    if total == 0 {
        return None;
    }
    u64::try_from(total.checked_mul(1000)?.div_ceil(denominator)).ok()
}

fn decimal(token: &str) -> Option<(u128, u128)> {
    // JSON syntax is already validated by RawValue. Bound work and exponent math.
    if token.len() > 128 || token.starts_with('-') {
        return None;
    }
    let (mantissa, exponent) = match token.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => (mantissa, exponent.parse::<i32>().ok()?),
        None => (token, 0),
    };
    let fractional = mantissa.split_once('.').map_or(0, |(_, f)| f.len());
    let scale = i32::try_from(fractional).ok()?.checked_sub(exponent)?;
    let value = mantissa.replace('.', "").parse::<u128>().ok()?;
    if value == 0 {
        return None;
    }
    let power = 10_u128.checked_pow(scale.unsigned_abs())?;
    if scale < 0 {
        Some((value.checked_mul(power)?, 1))
    } else {
        Some((value, power))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_decimals_round_only_after_all_filaments_batches_and_repeats() {
        for (tokens, repeats, expected) in [
            (vec!["32.112"], 1, 32_112),
            (vec!["0.0004", "0.0004"], 1, 1),
            (vec!["0.0004"], 3, 2),
            (vec!["4E-4", "0.00020"], 2, 2),
            (vec!["1.25e+1"], 1, 12_500),
            (vec!["3.7206485271453857"], 1, 3721),
        ] {
            let tokens: Vec<String> = tokens.into_iter().map(str::to_owned).collect();
            assert_eq!(
                milligrams(std::iter::once((tokens.as_slice(), repeats))),
                Some(expected)
            );
        }
        let tokens = ["0.0004".to_owned()];
        assert_eq!(
            milligrams([(tokens.as_slice(), 1), (tokens.as_slice(), 1)].into_iter()),
            Some(1)
        );
    }

    #[test]
    fn missing_out_of_range_or_excess_precision_mass_fails_closed() {
        for token in [
            "0",
            "-1",
            "1e-39",
            "1e39",
            "1e2147483647",
            "1e-2147483648",
            "340282366920938463463374607431768211456",
            "340282366920938463463374607431768211455",
        ] {
            assert_eq!(
                milligrams(std::iter::once(([token.to_owned()].as_slice(), 1))),
                None,
                "{token}"
            );
        }
        assert_eq!(decimal(&"0".repeat(129)), None);
        assert_eq!(milligrams(std::iter::once(([].as_slice(), 1))), None);
        assert_eq!(
            milligrams(std::iter::once((["1".to_owned()].as_slice(), 0))),
            None
        );
        assert_eq!(milligrams(std::iter::empty()), None);
    }
}
