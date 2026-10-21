//! The price table (0.24.0, `usage_v1`): USD per million tokens of each
//! kind, by model family. A row's `model` matches a model whose name
//! contains it; the longest match wins, then `*`. Flotilla seeds one row
//! per family, marked `estimate`, and the owner's `set_usage_prices`
//! replaces the table.

use serde_json::{json, Value};

use crate::db::{Tokens, UsagePrice};

/// The most rows a table may have.
pub const MAX_PRICES: usize = 64;

/// The price `model` is billed at, if any row matches it.
pub fn price_for<'a>(prices: &'a [UsagePrice], model: &str) -> Option<&'a UsagePrice> {
    let model = model.to_ascii_lowercase();
    if let Some(exact) = prices.iter().find(|p| p.model.eq_ignore_ascii_case(&model)) {
        return Some(exact);
    }
    prices
        .iter()
        .filter(|p| p.model != "*" && model.contains(&p.model.to_ascii_lowercase()))
        .max_by_key(|p| p.model.len())
        .or_else(|| prices.iter().find(|p| p.model == "*"))
}

/// What `tokens` of `model` cost, estimated from the table.
pub fn cost(prices: &[UsagePrice], model: &str, tokens: &Tokens) -> f64 {
    let Some(p) = price_for(prices, model) else {
        return 0.0;
    };
    let per = |n: u64, usd: f64| n as f64 * usd / 1_000_000.0;
    per(tokens.input, p.input)
        + per(tokens.output, p.output)
        + per(tokens.cache_read, p.cache_read)
        + per(tokens.cache_write, p.cache_write)
}

pub fn view(p: &UsagePrice) -> Value {
    json!({
        "model": p.model,
        "input": p.input,
        "output": p.output,
        "cache_read": p.cache_read,
        "cache_write": p.cache_write,
        "estimate": p.estimate,
    })
}

/// The table a `set_usage_prices` request carries, checked.
pub fn parse(prices: &Value) -> Result<Vec<UsagePrice>, String> {
    let Some(rows) = prices.as_array() else {
        return Err("prices must be a list".into());
    };
    if rows.len() > MAX_PRICES {
        return Err(format!("at most {MAX_PRICES} prices"));
    }
    let mut out: Vec<UsagePrice> = Vec::new();
    for row in rows {
        let model = row["model"].as_str().unwrap_or("").trim().to_string();
        if model.is_empty() || model.len() > 64 {
            return Err("each price names a model, at most 64 characters".into());
        }
        if out.iter().any(|p| p.model.eq_ignore_ascii_case(&model)) {
            return Err(format!("{model} is listed twice"));
        }
        let usd = |field: &str| -> Result<f64, String> {
            row[field]
                .as_f64()
                .filter(|n| n.is_finite() && *n >= 0.0)
                .ok_or_else(|| format!("{model}: {field} must be a number of dollars, 0 or more"))
        };
        out.push(UsagePrice {
            input: usd("input")?,
            output: usd("output")?,
            cache_read: usd("cache_read")?,
            cache_write: usd("cache_write")?,
            model,
            estimate: false,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn price(model: &str, input: f64) -> UsagePrice {
        UsagePrice {
            model: model.into(),
            input,
            output: input * 5.0,
            cache_read: input / 10.0,
            cache_write: input * 1.25,
            estimate: true,
        }
    }

    #[test]
    fn the_longest_family_wins_then_the_fallback() {
        let prices = [price("opus", 4.0), price("opus-4", 5.0), price("*", 1.0)];
        assert_eq!(price_for(&prices, "claude-opus-5-5").unwrap().input, 4.0);
        assert_eq!(price_for(&prices, "claude-opus-4-8").unwrap().input, 5.0);
        assert_eq!(price_for(&prices, "something-else").unwrap().input, 1.0);
        assert_eq!(price_for(&[price("opus", 4.0)], "gpt-5"), None);
    }

    #[test]
    fn a_million_tokens_cost_the_price() {
        let prices = [price("sonnet", 2.0)];
        let tokens = Tokens {
            input: 1_000_000,
            output: 1_000_000,
            cache_read: 1_000_000,
            cache_write: 0,
        };
        let usd = cost(&prices, "claude-sonnet-5-5", &tokens);
        assert!((usd - 12.2).abs() < 1e-9, "{usd}");
    }

    #[test]
    fn a_table_is_checked() {
        assert!(parse(&json!([{"model": "opus", "input": -1, "output": 1,
            "cache_read": 1, "cache_write": 1}]))
        .is_err());
        assert!(parse(&json!([{"model": "", "input": 1}])).is_err());
        let ok = parse(&json!([{"model": "opus", "input": 4, "output": 20,
            "cache_read": 0.2, "cache_write": 5}]))
        .unwrap();
        assert_eq!(ok[0].output, 20.0);
        assert!(!ok[0].estimate);
    }
}
