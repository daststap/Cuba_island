//! Calculator for the launcher: `2+2*2` → `6` (PLAN.md stage 4).

/// Characters an expression may contain; anything else means "not math".
fn is_expression_char(c: char) -> bool {
    c.is_ascii_digit() || " .,+-*/^%()".contains(c)
}

/// Whether the query looks like arithmetic: digits, at least one operator, nothing else.
pub fn looks_like_expression(query: &str) -> bool {
    let q = query.trim();
    !q.is_empty()
        && q.chars().all(is_expression_char)
        && q.chars().any(|c| c.is_ascii_digit())
        && q.chars().any(|c| "+-*/^%".contains(c))
        // "-5" alone is a number, not a calculation
        && q.trim_start_matches('-').chars().any(|c| "+-*/^%".contains(c))
}

/// Rewrites the user's input for evalexpr: decimal commas become dots, and every
/// number becomes a float so `7/2` is `3.5`, not integer division.
fn normalize(query: &str) -> String {
    let mut out = String::with_capacity(query.len() + 8);
    let mut number = String::new();
    let flush = |number: &mut String, out: &mut String| {
        if !number.is_empty() {
            out.push_str(number);
            if !number.contains('.') {
                out.push_str(".0");
            }
            number.clear();
        }
    };
    for c in query.chars() {
        match c {
            '0'..='9' => number.push(c),
            '.' | ',' if !number.is_empty() => number.push('.'),
            _ => {
                flush(&mut number, &mut out);
                out.push(c);
            }
        }
    }
    flush(&mut number, &mut out);
    out
}

/// Evaluates an expression; `None` if it is not valid math or not a finite number.
pub fn evaluate(query: &str) -> Option<f64> {
    if !looks_like_expression(query) {
        return None;
    }
    let value = evalexpr::eval_number(&normalize(query.trim())).ok()?;
    value.is_finite().then_some(value)
}

/// `6`, `3.5`, `0.3333333333` — integers without a fraction, at most 10 decimals.
pub fn format_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        return format!("{value:.0}");
    }
    let text = format!("{value:.10}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_with_precedence() {
        assert_eq!(evaluate("2+2*2"), Some(6.0));
        assert_eq!(evaluate("(2+2)*2"), Some(8.0));
        assert_eq!(evaluate("2^10"), Some(1024.0));
    }

    #[test]
    fn divides_as_floats_and_accepts_decimal_commas() {
        assert_eq!(evaluate("7/2"), Some(3.5));
        assert_eq!(evaluate("1,5*2"), Some(3.0));
        assert_eq!(
            evaluate("0.1+0.2").map(format_number).as_deref(),
            Some("0.3")
        );
    }

    #[test]
    fn rejects_non_math() {
        assert_eq!(evaluate("chrome"), None);
        assert_eq!(evaluate("42"), None);
        assert_eq!(evaluate("-5"), None);
        assert_eq!(evaluate("2+"), None);
        assert_eq!(evaluate("1/0"), None);
        assert!(!looks_like_expression("c++"));
    }

    #[test]
    fn formats_results() {
        assert_eq!(format_number(6.0), "6");
        assert_eq!(format_number(-12.0), "-12");
        assert_eq!(format_number(3.5), "3.5");
        assert_eq!(format_number(1.0 / 3.0), "0.3333333333");
    }
}
