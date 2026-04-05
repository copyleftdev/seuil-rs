//! String built-in functions for JSONata.

use std::borrow::Cow;

use crate::evaluator::value::{ArrayFlags, FnContext, Value};
use crate::{Error, Result, Span};

use super::{assert_arg, assert_array_of_type, bad_arg, max_args, min_args};

// ---------------------------------------------------------------------------
// $string(arg, prettify?)
// ---------------------------------------------------------------------------

pub fn fn_string<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    max_args!(context, args, 2);

    let input = if args.is_empty() {
        context.input
    } else {
        args[0]
    };

    if input.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }

    // When called with no explicit args and input is Null (no data),
    // JSONata returns undefined.
    if args.is_empty() && input.is_null() {
        return Ok(Value::undefined(context.arena));
    }

    let pretty = args.get(1).copied();
    if let Some(p) = pretty {
        assert_arg!(p.is_undefined() || p.is_bool(), context, 2);
    }

    if input.is_string() {
        return Ok(input);
    }
    if input.is_function() {
        return Ok(Value::string(context.arena, ""));
    }
    if input.is_number() && !input.is_finite() {
        return Err(Error::D3001StringNotFinite(Span::at(context.char_index)));
    }

    let is_pretty = pretty
        .map(|p| matches!(p, Value::Bool(true)))
        .unwrap_or(false);

    if is_pretty {
        let output = input.serialize_strict(true)?;
        Ok(Value::string(context.arena, &output))
    } else {
        let output = input.serialize_strict(false)?;
        Ok(Value::string(context.arena, &output))
    }
}

// ---------------------------------------------------------------------------
// $length(str)
// ---------------------------------------------------------------------------

pub fn fn_length<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);

    let arg = if args.is_empty() {
        // No explicit arg: use context input
        let input = context.input;
        if input.is_undefined() || input.is_null() {
            return Err(Error::T0411ContextNotValid(
                Span::at(context.char_index),
                1,
                context.name.to_string(),
            ));
        }
        if !input.is_string() {
            return Err(Error::T0411ContextNotValid(
                Span::at(context.char_index),
                1,
                context.name.to_string(),
            ));
        }
        input
    } else {
        args[0]
    };

    if arg.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }

    assert_arg!(arg.is_string(), context, 1);
    Ok(Value::number(
        context.arena,
        arg.as_str().chars().count() as f64,
    ))
}

// ---------------------------------------------------------------------------
// $substring(str, start, length?)
// ---------------------------------------------------------------------------

pub fn fn_substring<'a>(
    context: FnContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let string = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    let start = args
        .get(1)
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    let length = args
        .get(2)
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));

    if string.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }

    assert_arg!(string.is_string(), context, 1);
    assert_arg!(start.is_number(), context, 2);

    let string = string.as_str();
    let len = string.chars().count() as isize;
    let mut start_idx = start.as_isize();

    if len + start_idx < 0 {
        start_idx = 0;
    }
    let start_idx = if start_idx < 0 {
        len + start_idx
    } else {
        start_idx
    };

    if length.is_undefined() {
        let sub: String = string.chars().skip(start_idx as usize).collect();
        Ok(Value::string(context.arena, &sub))
    } else {
        assert_arg!(length.is_number(), context, 3);
        let length = length.as_isize();
        if length < 0 {
            Ok(Value::string(context.arena, ""))
        } else {
            let end = (start_idx + length) as usize;
            let sub: String = string
                .chars()
                .skip(start_idx as usize)
                .take(end - start_idx as usize)
                .collect();
            Ok(Value::string(context.arena, &sub))
        }
    }
}

// ---------------------------------------------------------------------------
// $substringBefore(str, chars)
// ---------------------------------------------------------------------------

pub fn fn_substring_before<'a>(
    context: FnContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let string = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    let chars = args
        .get(1)
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));

    if !string.is_string() {
        return Ok(Value::undefined(context.arena));
    }
    if !chars.is_string() {
        return Err(Error::D3010EmptyPattern(Span::at(context.char_index)));
    }

    let s: &str = &string.as_str();
    let c: &str = &chars.as_str();

    if let Some(index) = s.find(c) {
        Ok(Value::string(context.arena, &s[..index]))
    } else {
        Ok(Value::string(context.arena, s))
    }
}

// ---------------------------------------------------------------------------
// $substringAfter(str, chars)
// ---------------------------------------------------------------------------

pub fn fn_substring_after<'a>(
    context: FnContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let string = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    let chars = args
        .get(1)
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));

    if !string.is_string() {
        return Ok(Value::undefined(context.arena));
    }
    if !chars.is_string() {
        return Err(Error::D3010EmptyPattern(Span::at(context.char_index)));
    }

    let s: &str = &string.as_str();
    let c: &str = &chars.as_str();

    if let Some(index) = s.find(c) {
        let after = index + c.len();
        Ok(Value::string(context.arena, &s[after..]))
    } else {
        Ok(Value::string(context.arena, s))
    }
}

// ---------------------------------------------------------------------------
// $uppercase(str)
// ---------------------------------------------------------------------------

pub fn fn_uppercase<'a>(
    context: FnContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let arg = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    if !arg.is_string() {
        return Ok(Value::undefined(context.arena));
    }
    Ok(Value::string(context.arena, &arg.as_str().to_uppercase()))
}

// ---------------------------------------------------------------------------
// $lowercase(str)
// ---------------------------------------------------------------------------

pub fn fn_lowercase<'a>(
    context: FnContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let arg = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    if !arg.is_string() {
        return Ok(Value::undefined(context.arena));
    }
    Ok(Value::string(context.arena, &arg.as_str().to_lowercase()))
}

// ---------------------------------------------------------------------------
// $trim(str)
// ---------------------------------------------------------------------------

pub fn fn_trim<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    let arg = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    if !arg.is_string() {
        return Ok(Value::undefined(context.arena));
    }
    let original = arg.as_str();
    let mut words = original.split_whitespace();
    let trimmed = match words.next() {
        None => String::new(),
        Some(first) => {
            let mut result = String::from(first);
            for word in words {
                result.push(' ');
                result.push_str(word);
            }
            result
        }
    };
    Ok(Value::string(context.arena, &trimmed))
}

// ---------------------------------------------------------------------------
// $pad(str, width, char?)
// ---------------------------------------------------------------------------

pub fn fn_pad<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    let str_value = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    if !str_value.is_string() {
        return Ok(Value::undefined(context.arena));
    }
    let width_value = args
        .get(1)
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    if !width_value.is_number() {
        return Ok(Value::undefined(context.arena));
    }

    let s = str_value.as_str();
    let width_i64 = width_value.as_f64().round() as i64;
    let width = width_i64.unsigned_abs() as usize;
    let is_right = width_i64 > 0;

    let pad_char = args
        .get(2)
        .and_then(|v| v.try_as_str())
        .filter(|c| !c.is_empty())
        .unwrap_or(Cow::Borrowed(" "));

    let pad_len = width.saturating_sub(s.chars().count());
    if pad_len == 0 {
        return Ok(Value::string(context.arena, &s));
    }

    let padding: String = pad_char.chars().cycle().take(pad_len).collect();
    let result = if is_right {
        format!("{}{}", s, padding)
    } else {
        format!("{}{}", padding, s)
    };
    Ok(Value::string(context.arena, &result))
}

// ---------------------------------------------------------------------------
// $contains(str, pattern)
// ---------------------------------------------------------------------------

pub fn fn_contains<'a>(
    context: FnContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    let str_value = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    let token = args
        .get(1)
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));

    if str_value.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }
    assert_arg!(str_value.is_string(), context, 1);

    let s = str_value.as_str();
    let result = match token {
        Value::Regex(ref regex_literal) => regex_literal.get_regex().find(&s).is_some(),
        Value::String(_) => {
            let tok = token.as_str();
            s.contains(&*tok)
        }
        _ => bad_arg!(context, 2),
    };

    Ok(Value::bool_val(context.arena, result))
}

// ---------------------------------------------------------------------------
// $split(str, separator, limit?)
// ---------------------------------------------------------------------------

pub fn fn_split<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    let str_value = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    let separator = args
        .get(1)
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    let limit_value = args
        .get(2)
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));

    if str_value.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }
    assert_arg!(str_value.is_string(), context, 1);

    let s = str_value.as_str();
    let sep_is_regex = matches!(separator, Value::Regex(_));
    if !sep_is_regex && !separator.is_string() {
        bad_arg!(context, 2);
    }

    let limit = if limit_value.is_undefined() {
        None
    } else {
        assert_arg!(limit_value.is_number(), context, 3);
        if limit_value.as_f64() < 0.0 {
            return Err(Error::D3020NegativeLimit(Span::at(context.char_index)));
        }
        Some(limit_value.as_f64() as usize)
    };

    let substrings: Vec<String> = if sep_is_regex {
        let regex = match separator {
            Value::Regex(ref rl) => rl.get_regex(),
            _ => unreachable!(),
        };
        let mut results = Vec::new();
        let mut last_end = 0;
        let effective_limit = limit.unwrap_or(usize::MAX);
        for m in regex.find_iter(&s) {
            if results.len() >= effective_limit {
                break;
            }
            if m.start() > last_end {
                results.push(s[last_end..m.start()].to_string());
            }
            last_end = m.end();
        }
        if results.len() < effective_limit {
            results.push(s[last_end..].to_string());
        }
        results
    } else {
        let sep_str = separator.as_str();
        if sep_str.is_empty() {
            if let Some(limit) = limit {
                s.chars().take(limit).map(|c| c.to_string()).collect()
            } else {
                s.chars().map(|c| c.to_string()).collect()
            }
        } else if let Some(limit) = limit {
            s.split(&*sep_str)
                .take(limit)
                .map(|x| x.to_string())
                .collect()
        } else {
            s.split(&*sep_str).map(|x| x.to_string()).collect()
        }
    };

    let result = Value::array_with_capacity(context.arena, substrings.len(), ArrayFlags::empty());
    for sub in &substrings {
        result.push(Value::string(context.arena, sub));
    }
    Ok(result)
}

// ---------------------------------------------------------------------------
// $join(array, separator?)
// ---------------------------------------------------------------------------

pub fn fn_join<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    min_args!(context, args, 1);
    max_args!(context, args, 2);
    let strings = args[0];

    if strings.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }
    if strings.is_string() {
        return Ok(strings);
    }

    assert_array_of_type!(strings.is_array(), context, 1, "string");

    let separator = args
        .get(1)
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    assert_arg!(
        separator.is_undefined() || separator.is_string(),
        context,
        2
    );

    let sep: Cow<'_, str> = if separator.is_string() {
        separator.as_str()
    } else {
        Cow::Borrowed("")
    };

    let mut result = String::with_capacity(256);
    let total = strings.len();
    for (i, member) in strings.members().enumerate() {
        assert_array_of_type!(member.is_string(), context, 1, "string");
        result.push_str(&member.as_str());
        if i < total - 1 {
            result.push_str(&sep);
        }
    }

    Ok(Value::string(context.arena, &result))
}

// ---------------------------------------------------------------------------
// $replace(str, pattern, replacement, limit?)
// ---------------------------------------------------------------------------

pub fn fn_replace<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    let str_value = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    let pattern = args
        .get(1)
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    let replacement = args
        .get(2)
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    let limit_value = args
        .get(3)
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));

    if str_value.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }
    if pattern.is_string() && pattern.as_str().is_empty() {
        return Err(Error::D3010EmptyPattern(Span::at(context.char_index)));
    }

    assert_arg!(str_value.is_string(), context, 1);

    let s = str_value.as_str();
    let limit = if limit_value.is_undefined() {
        None
    } else {
        assert_arg!(limit_value.is_number(), context, 4);
        if limit_value.as_isize() < 0 {
            return Err(Error::D3011NegativeLimit(Span::at(context.char_index)));
        }
        Some(limit_value.as_isize() as usize)
    };

    // String pattern (simple replace)
    if pattern.is_string() {
        assert_arg!(replacement.is_string(), context, 3);
        let pat = pattern.as_str();
        let rep = replacement.as_str();
        let replaced = if let Some(limit) = limit {
            s.replacen(&*pat, &rep, limit)
        } else {
            s.replace(&*pat, &rep)
        };
        return Ok(Value::string(context.arena, &replaced));
    }

    // Regex pattern
    let regex = match pattern {
        Value::Regex(ref rl) => rl.get_regex(),
        _ => bad_arg!(context, 2),
    };

    // Replacement can be a string or a function
    if replacement.is_function() {
        // Function replacement: fn(match_obj) -> string
        let span = Span::at(context.char_index);
        let mut result = String::new();
        let mut last_end = 0;

        for (count, m) in regex.find_iter(&s).enumerate() {
            if m.range().is_empty() {
                return Err(Error::D1004ZeroLengthMatch(Span::at(context.char_index)));
            }
            if let Some(limit) = limit {
                if count >= limit {
                    break;
                }
            }
            result.push_str(&s[last_end..m.start()]);

            // Build match object like $match returns
            let matched_text = &s[m.start()..m.end()];
            let match_obj = Value::object(context.arena);
            match_obj.insert("match", Value::string(context.arena, matched_text));
            match_obj.insert("index", Value::number(context.arena, m.start() as f64));
            let groups = Value::array(context.arena, ArrayFlags::empty());
            for cap in &m.captures {
                if let Some(ref range) = cap {
                    groups.push(Value::string(context.arena, &s[range.start..range.end]));
                } else {
                    groups.push(Value::null(context.arena));
                }
            }
            match_obj.insert("groups", groups);

            let replaced = (context.apply_fn)(span, context.input, replacement, &[match_obj])?;
            if replaced.is_string() {
                result.push_str(&replaced.as_str());
            } else {
                return Err(Error::D3012InvalidReplacementType(Span::at(
                    context.char_index,
                )));
            }
            last_end = m.end();
        }
        result.push_str(&s[last_end..]);
        return Ok(Value::string(context.arena, &result));
    }

    assert_arg!(replacement.is_string(), context, 3);
    let rep_str = replacement.as_str();

    let mut result = String::new();
    let mut last_end = 0;

    for (count, m) in regex.find_iter(&s).enumerate() {
        if m.range().is_empty() {
            return Err(Error::D1004ZeroLengthMatch(Span::at(context.char_index)));
        }
        if let Some(limit) = limit {
            if count >= limit {
                break;
            }
        }
        result.push_str(&s[last_end..m.start()]);

        // Expand backreferences in replacement: $0, $1, $2, ..., $$
        let expanded = expand_backreferences(&rep_str, &m, &s);
        result.push_str(&expanded);
        last_end = m.end();
    }
    result.push_str(&s[last_end..]);

    Ok(Value::string(context.arena, &result))
}

// ---------------------------------------------------------------------------
// $match(str, pattern, limit?)
// ---------------------------------------------------------------------------

pub fn fn_match<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    let value = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    if value.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }
    assert_arg!(value.is_string(), context, 1);

    let pattern = args.get(1).copied();
    let pattern = match pattern {
        Some(p) => p,
        None => return Err(Error::D3010EmptyPattern(Span::at(context.char_index))),
    };

    let regex = match pattern {
        Value::Regex(ref rl) => rl.get_regex(),
        _ => return Err(Error::D3010EmptyPattern(Span::at(context.char_index))),
    };

    let limit = args.get(2).and_then(|v| {
        if v.is_number() {
            Some(v.as_f64() as usize)
        } else {
            None
        }
    });

    let input_str = value.as_str();
    let arena = context.arena;
    let effective_limit = limit.unwrap_or(usize::MAX);

    let matches_arr = Value::array(arena, ArrayFlags::empty());

    for (i, m) in regex.find_iter(&input_str).enumerate() {
        if i >= effective_limit {
            break;
        }

        let matched_text = &input_str[m.start()..m.end()];
        let match_obj = Value::object(arena);
        match_obj.insert("match", Value::string(arena, matched_text));
        match_obj.insert("index", Value::number(arena, m.start() as f64));

        // Build capture groups array (skip first element which is the full match)
        let groups = Value::array(arena, ArrayFlags::empty());
        for cap in &m.captures {
            if let Some(ref range) = cap {
                groups.push(Value::string(arena, &input_str[range.start..range.end]));
            } else {
                groups.push(Value::null(arena));
            }
        }
        match_obj.insert("groups", groups);
        matches_arr.push(match_obj);
    }

    // JSONata semantics: no matches → undefined, one match → unwrap object, multiple → array
    if matches_arr.is_empty() {
        Ok(Value::undefined(arena))
    } else if matches_arr.len() == 1 && limit.is_none() {
        Ok(matches_arr
            .get_member(0)
            .unwrap_or_else(|| Value::undefined(arena)))
    } else {
        Ok(matches_arr)
    }
}

// ---------------------------------------------------------------------------
// $base64encode(str)
// ---------------------------------------------------------------------------

pub fn fn_base64_encode<'a>(
    context: FnContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);
    let arg = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    if arg.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }
    assert_arg!(arg.is_string(), context, 1);

    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(arg.as_str().as_bytes());
    Ok(Value::string(context.arena, &encoded))
}

// ---------------------------------------------------------------------------
// $base64decode(str)
// ---------------------------------------------------------------------------

pub fn fn_base64_decode<'a>(
    context: FnContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);
    let arg = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    if arg.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }
    assert_arg!(arg.is_string(), context, 1);

    use base64::Engine;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(arg.as_str().as_bytes())
        .map_err(|e| Error::D3137Error(e.to_string()))?;
    let decoded_str = String::from_utf8(decoded).map_err(|e| Error::D3137Error(e.to_string()))?;
    Ok(Value::string(context.arena, &decoded_str))
}

// ---------------------------------------------------------------------------
// Regex backreference expansion
// ---------------------------------------------------------------------------

/// Expand backreference patterns in a replacement string.
/// $0 = full match, $1..$N = capture groups, $$ = literal '$'
/// Supports multi-digit group refs: $12 matches group 12 if it exists,
/// otherwise tries $1 followed by literal "2".
/// Non-existent single-digit groups produce empty string.
fn expand_backreferences(replacement: &str, m: &regress::Match, input: &str) -> String {
    let mut result = String::with_capacity(replacement.len());
    let bytes = replacement.as_bytes();
    let num_captures = m.captures.len(); // number of capture groups (not counting full match)
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'$' && i + 1 < bytes.len() {
            let next = bytes[i + 1];
            if next == b'$' {
                // $$ -> literal $
                result.push('$');
                i += 2;
            } else if next.is_ascii_digit() {
                // Collect all consecutive digits after $
                let digit_start = i + 1;
                let mut digit_end = digit_start;
                while digit_end < bytes.len() && bytes[digit_end].is_ascii_digit() {
                    digit_end += 1;
                }
                let digits = &replacement[digit_start..digit_end];

                // Try the longest possible group number, then fall back
                // to shorter prefixes. E.g., $12: try group 12, else try group 1 + "2"
                let mut matched = false;
                let mut try_len = digits.len();
                while try_len > 0 {
                    let try_num: usize = digits[..try_len].parse().unwrap_or(0);
                    if try_num <= num_captures {
                        // Use m.group() which handles 0=full match, 1..=N=captures
                        if let Some(range) = m.group(try_num) {
                            result.push_str(&input[range.start..range.end]);
                        }
                        // Append remaining digits as literal text
                        result.push_str(&digits[try_len..]);
                        matched = true;
                        break;
                    }
                    try_len -= 1;
                }
                if !matched {
                    // Single-digit $N where N > num_captures: consume $N, output empty
                    // (JSONata behavior: non-existent group refs produce empty string)
                    if digits.len() == 1 {
                        // Just consume $N and output nothing
                    } else {
                        // Multi-digit: consume only $+first_digit, rest are literal
                        // e.g. $18 with only 0 captures: $1->empty, "8" literal
                        result.push_str(&digits[1..]);
                    }
                }
                i = digit_end;
            } else {
                result.push('$');
                i += 1;
            }
        } else {
            result.push(bytes[i] as char);
            i += 1;
        }
    }

    result
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
    fn test_length() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let s = Value::string(&arena, "hello");
        let result = fn_length(c, &[s]).unwrap();
        assert_eq!(result.as_f64(), 5.0);
    }

    #[test]
    fn test_uppercase_lowercase() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let s = Value::string(&arena, "Hello World");
        let up = fn_uppercase(c.clone(), &[s]).unwrap();
        assert_eq!(up.as_str().as_ref(), "HELLO WORLD");
        let lo = fn_lowercase(c, &[s]).unwrap();
        assert_eq!(lo.as_str().as_ref(), "hello world");
    }

    #[test]
    fn test_trim() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let s = Value::string(&arena, "  hello   world  ");
        let result = fn_trim(c, &[s]).unwrap();
        assert_eq!(result.as_str().as_ref(), "hello world");
    }

    #[test]
    fn test_substring() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let s = Value::string(&arena, "hello");
        let start = Value::number(&arena, 1.0);
        let len = Value::number(&arena, 3.0);
        let result = fn_substring(c, &[s, start, len]).unwrap();
        assert_eq!(result.as_str().as_ref(), "ell");
    }

    #[test]
    fn test_contains_string() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let s = Value::string(&arena, "hello world");
        let tok = Value::string(&arena, "world");
        let result = fn_contains(c, &[s, tok]).unwrap();
        assert_eq!(result.as_bool(), true);
    }

    #[test]
    fn test_split_basic() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let s = Value::string(&arena, "a,b,c");
        let sep = Value::string(&arena, ",");
        let result = fn_split(c, &[s, sep]).unwrap();
        assert!(result.is_array());
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_join() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let arr = Value::array(&arena, ArrayFlags::empty());
        arr.push(Value::string(&arena, "a"));
        arr.push(Value::string(&arena, "b"));
        arr.push(Value::string(&arena, "c"));
        let sep = Value::string(&arena, "-");
        let result = fn_join(c, &[arr, sep]).unwrap();
        assert_eq!(result.as_str().as_ref(), "a-b-c");
    }

    #[test]
    fn test_replace_string() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let s = Value::string(&arena, "hello world");
        let pat = Value::string(&arena, "world");
        let rep = Value::string(&arena, "rust");
        let result = fn_replace(c, &[s, pat, rep]).unwrap();
        assert_eq!(result.as_str().as_ref(), "hello rust");
    }

    #[test]
    fn test_base64_roundtrip() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let s = Value::string(&arena, "hello");
        let encoded = fn_base64_encode(c.clone(), &[s]).unwrap();
        let decoded = fn_base64_decode(c, &[encoded]).unwrap();
        assert_eq!(decoded.as_str().as_ref(), "hello");
    }

    #[test]
    fn test_pad() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let s = Value::string(&arena, "hi");
        let w = Value::number(&arena, 5.0);
        let result = fn_pad(c, &[s, w]).unwrap();
        assert_eq!(result.as_str().as_ref(), "hi   ");
    }

    #[test]
    fn test_substring_before_after() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let s = Value::string(&arena, "hello-world");
        let sep = Value::string(&arena, "-");
        let before = fn_substring_before(c.clone(), &[s, sep]).unwrap();
        assert_eq!(before.as_str().as_ref(), "hello");
        let after = fn_substring_after(c, &[s, sep]).unwrap();
        assert_eq!(after.as_str().as_ref(), "world");
    }
}
