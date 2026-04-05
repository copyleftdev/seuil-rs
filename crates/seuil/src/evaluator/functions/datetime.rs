//! Date/time built-in functions for JSONata.
//!
//! `$now`, `$millis`, `$fromMillis`, `$toMillis` all depend on the
//! `Environment` trait for deterministic simulation testing.
//! Since `FnContext` does not carry the environment, these are stubs
//! that the evaluator will override at call sites.

use crate::evaluator::value::{FnContext, Value};
use crate::{Error, Result};

use super::max_args;

// ---------------------------------------------------------------------------
// $now(picture?, timezone?)
// ---------------------------------------------------------------------------

/// Stub -- the evaluator intercepts $now to use Environment.now_millis().
pub fn fn_now<'a>(_context: FnContext<'a, '_>, _args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    // TODO: Wire up to Environment.now_millis() + chrono formatting.
    Err(Error::D3137Error(
        "$now requires evaluator context (environment stub)".to_string(),
    ))
}

// ---------------------------------------------------------------------------
// $millis()
// ---------------------------------------------------------------------------

/// Stub -- the evaluator intercepts $millis to use Environment.now_millis().
pub fn fn_millis<'a>(
    _context: FnContext<'a, '_>,
    _args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    // TODO: Wire up to Environment.now_millis().
    Err(Error::D3137Error(
        "$millis requires evaluator context (environment stub)".to_string(),
    ))
}

// ---------------------------------------------------------------------------
// $uuid()
// ---------------------------------------------------------------------------

/// Stub -- the evaluator intercepts $uuid to use Environment.random_uuid().
pub fn fn_uuid<'a>(_context: FnContext<'a, '_>, _args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    Err(Error::D3137Error(
        "$uuid requires evaluator context (environment stub)".to_string(),
    ))
}

// ---------------------------------------------------------------------------
// $fromMillis(millis, picture?, timezone?)
// ---------------------------------------------------------------------------

pub fn fn_from_millis<'a>(
    context: FnContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 3);

    let millis = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    if millis.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }

    if !millis.is_number() {
        return Err(Error::T0410ArgumentNotValid(
            crate::Span::at(context.char_index),
            1,
            context.name.to_string(),
        ));
    }

    // Validate arg2 (picture) if present
    let picture_arg = args.get(1).copied();
    if let Some(p) = picture_arg {
        if !p.is_undefined() && !p.is_null() && !p.is_string() {
            return Err(Error::T0410ArgumentNotValid(
                crate::Span::at(context.char_index),
                2,
                context.name.to_string(),
            ));
        }
    }

    // Validate arg3 (timezone) if present
    let tz_arg = args.get(2).copied();
    if let Some(tz) = tz_arg {
        if !tz.is_undefined() && !tz.is_null() && !tz.is_string() {
            return Err(Error::T0410ArgumentNotValid(
                crate::Span::at(context.char_index),
                3,
                context.name.to_string(),
            ));
        }
    }

    let ms = millis.as_f64() as i64;

    use chrono::{FixedOffset, TimeZone, Utc};

    let Some(utc_timestamp) = Utc.timestamp_millis_opt(ms).single() else {
        return Err(Error::T0410ArgumentNotValid(
            crate::Span::at(context.char_index),
            1,
            context.name.to_string(),
        ));
    };

    // Determine timezone offset
    let tz_str = tz_arg.and_then(|v| {
        if v.is_string() {
            Some(v.as_str().to_string())
        } else {
            None
        }
    });

    let offset: FixedOffset = if let Some(ref tz) = tz_str {
        match crate::datetime::parse_timezone_offset(tz) {
            Some(o) => o,
            None => FixedOffset::east_opt(0).unwrap(),
        }
    } else {
        FixedOffset::east_opt(0).unwrap()
    };

    let date_with_offset = utc_timestamp.with_timezone(&offset);

    // Get picture string
    let picture_str = picture_arg.and_then(|v| {
        if v.is_string() {
            Some(v.as_str().to_string())
        } else {
            None
        }
    });

    if let Some(ref pic) = picture_str {
        // Use custom formatting
        let formatted = crate::datetime::format_custom_date(&date_with_offset, pic)?;
        return Ok(Value::string(context.arena, &formatted));
    }

    // No picture string: use ISO 8601 default
    let iso = date_with_offset.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    // When timezone is provided but no picture, we need to show the offset in the ISO string.
    // to_rfc3339_opts with 'true' uses Z for UTC, otherwise shows offset.
    // However, if tz is explicitly "0000", Utc -> Z is correct.
    // If tz is provided and non-UTC, the offset is embedded by to_rfc3339_opts automatically.
    Ok(Value::string(context.arena, &iso))
}

// ---------------------------------------------------------------------------
// $toMillis(timestamp, picture?)
// ---------------------------------------------------------------------------

pub fn fn_to_millis<'a>(
    context: FnContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 2);

    let timestamp = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    if timestamp.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }

    if !timestamp.is_string() {
        return Err(Error::T0410ArgumentNotValid(
            crate::Span::at(context.char_index),
            1,
            context.name.to_string(),
        ));
    }

    // Validate arg2 (picture) if present
    let picture_arg = args.get(1).copied();
    if let Some(p) = picture_arg {
        if !p.is_undefined() && !p.is_null() && !p.is_string() {
            return Err(Error::T0410ArgumentNotValid(
                crate::Span::at(context.char_index),
                2,
                context.name.to_string(),
            ));
        }
    }

    let ts_str = timestamp.as_str();
    if ts_str.is_empty() {
        return Ok(Value::undefined(context.arena));
    }

    // Get picture string
    let picture_str = picture_arg.and_then(|v| {
        if v.is_string() {
            Some(v.as_str().to_string())
        } else {
            None
        }
    });

    if let Some(ref pic) = picture_str {
        // If picture is "Hello" or other literal-only string, try parsing anyway
        // parse_custom_format may return Ok(None) for unrecognized or Err for errors
        match crate::datetime::parse_custom_format(&ts_str, pic) {
            Ok(Some(ms)) => return Ok(Value::number(context.arena, ms as f64)),
            Ok(None) => return Ok(Value::undefined(context.arena)),
            Err(e) => return Err(e),
        }
    }

    // No picture string: parse ISO 8601
    match crate::datetime::parse_custom_format(&ts_str, "") {
        Ok(Some(ms)) => Ok(Value::number(context.arena, ms as f64)),
        Ok(None) => {
            // Failed to parse -- this is an error for $toMillis without picture
            Err(Error::D3110InvalidDateTimeString(ts_str.to_string()))
        }
        Err(e) => Err(e),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Span;
    use bumpalo::Bump;

    fn dummy_apply_fn<'a>(
        _span: Span,
        _input: &'a Value<'a>,
        _proc: &'a Value<'a>,
        _args: &[&'a Value<'a>],
    ) -> crate::Result<&'a Value<'a>> {
        Err(crate::Error::D3137Error("dummy apply_fn".to_string()))
    }

    fn ctx(arena: &Bump) -> FnContext<'_, '_> {
        FnContext {
            name: "test",
            char_index: 0,
            input: Value::undefined(arena),
            arena,
            apply_fn: &dummy_apply_fn,
        }
    }

    #[test]
    fn test_from_millis_basic() {
        let arena = Bump::new();
        let c = ctx(&arena);
        // 2017-01-01T00:00:00.000Z in millis = 1483228800000
        let ms = Value::number(&arena, 1_483_228_800_000.0);
        let result = fn_from_millis(c, &[ms]).unwrap();
        assert!(result.is_string());
        let s = result.as_str();
        assert!(s.contains("2017-01-01"));
    }

    #[test]
    fn test_from_millis_with_picture() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let ms = Value::number(&arena, 1521801216617.0);
        let pic = Value::string(&arena, "[Y0001]-[M01]-[D01]");
        let result = fn_from_millis(c, &[ms, pic]).unwrap();
        assert!(result.is_string());
        assert_eq!(result.as_str().as_ref(), "2018-03-23");
    }

    #[test]
    fn test_from_millis_with_timezone() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let ms = Value::number(&arena, 1521801216617.0);
        let pic = Value::string(&arena, "[Y]-[M01]-[D01]T[H01]:[m]:[s].[f001][Z0101t]");
        let tz = Value::string(&arena, "+0100");
        let result = fn_from_millis(c, &[ms, pic, tz]).unwrap();
        assert!(result.is_string());
        assert_eq!(result.as_str().as_ref(), "2018-03-23T11:33:36.617+0100");
    }

    #[test]
    fn test_to_millis_roundtrip() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let ts = Value::string(&arena, "2017-01-01T00:00:00.000Z");
        let result = fn_to_millis(c, &[ts]).unwrap();
        assert!(result.is_number());
        assert_eq!(result.as_f64(), 1_483_228_800_000.0);
    }

    #[test]
    fn test_to_millis_with_picture() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let ts = Value::string(&arena, "2018");
        let pic = Value::string(&arena, "[Y1]");
        let result = fn_to_millis(c, &[ts, pic]).unwrap();
        assert!(result.is_number());
        assert_eq!(result.as_f64(), 1514764800000.0);
    }

    #[test]
    fn test_to_millis_invalid_iso() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let ts = Value::string(&arena, "foo");
        let result = fn_to_millis(c, &[ts]);
        assert!(result.is_err());
    }

    #[test]
    fn test_now_stub() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let result = fn_now(c, &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_millis_stub() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let result = fn_millis(c, &[]);
        assert!(result.is_err());
    }
}
