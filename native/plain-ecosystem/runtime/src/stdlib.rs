//! Standard library functions callable from Plain (documented in docs/STDLIB.md).
//!
//! Every function takes and returns Plain values. Failures are returned as error values
//! (never panics), so `TRY`/`CATCH` and error propagation work the same as for the core runtime.
use super::*;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

const MAX_STRING_BYTES: usize = 100_000_000;
const MAX_SLEEP_MS: i64 = 3_600_000;
const MAX_JSON_DEPTH: usize = 64;
const MAX_JSON_INPUT_BYTES: usize = 100_000_000;
const MAX_JSON_VALUES: usize = 1_000_000;

// ---------------------------------------------------------------------------------- helpers

fn new_list(items: Vec<*mut PlainValue>) -> *mut PlainValue {
    let out = value(K_LIST);
    unsafe { (*out).ptr = Box::into_raw(Box::new(List { items })) as *mut c_void; }
    out
}

fn new_dict(items: HashMap<String, *mut PlainValue>) -> *mut PlainValue {
    let out = value(K_DICT);
    unsafe { (*out).ptr = Box::into_raw(Box::new(Dict { items })) as *mut c_void; }
    out
}

fn list_items<'a>(v: *mut PlainValue) -> Option<&'a Vec<*mut PlainValue>> {
    if v.is_null() { return None; }
    unsafe { if (*v).kind == K_LIST { Some(&(*((*v).ptr as *mut List)).items) } else { None } }
}

fn dict_ref<'a>(v: *mut PlainValue) -> Option<&'a Dict> {
    if v.is_null() { return None; }
    unsafe { if (*v).kind == K_DICT { Some(&*((*v).ptr as *mut Dict)) } else { None } }
}

fn text_of(v: *mut PlainValue) -> Option<String> {
    if is_string_value(v) { Some(read_string(v)) } else { None }
}

fn int_of(v: *mut PlainValue) -> Option<i64> {
    if v.is_null() { return None; }
    unsafe { if (*v).kind == K_INT { Some((*v).i) } else { None } }
}

fn is_number(v: *mut PlainValue) -> bool {
    !v.is_null() && unsafe { matches!((*v).kind, K_INT | K_FLOAT) }
}

/// Dictionary keys without the internal `__` keys, sorted.
fn visible_keys(d: &Dict) -> Vec<&String> {
    let mut keys: Vec<&String> = d.items.keys().filter(|k| !k.starts_with("__")).collect();
    keys.sort();
    keys
}

// ---------------------------------------------------------------------------------- strings

#[no_mangle] pub extern "C" fn plain_trim(s: *mut PlainValue) -> *mut PlainValue {
    if is_err(s) { return s; }
    match text_of(s) {
        Some(text) => string_value(K_STRING, text.trim()),
        None => type_error("TRIM needs a string"),
    }
}

#[no_mangle] pub extern "C" fn plain_replace(s: *mut PlainValue, from: *mut PlainValue, to: *mut PlainValue) -> *mut PlainValue {
    if is_err(s) { return s; }
    match (text_of(s), text_of(from), text_of(to)) {
        (Some(text), Some(old), Some(new)) => {
            if old.is_empty() { return string_value(K_STRING, &text); }
            string_value(K_STRING, &text.replace(old.as_str(), new.as_str()))
        }
        _ => type_error("REPLACE needs three strings"),
    }
}

#[no_mangle] pub extern "C" fn plain_substring(s: *mut PlainValue, start: *mut PlainValue, length: *mut PlainValue) -> *mut PlainValue {
    if is_err(s) { return s; }
    match (text_of(s), int_of(start), int_of(length)) {
        (Some(text), Some(first), Some(count)) => {
            if first < 0 || count < 0 { return type_error("SUBSTRING start and length must not be negative"); }
            string_value(K_STRING, &text.chars().skip(first as usize).take(count as usize).collect::<String>())
        }
        _ => type_error("SUBSTRING needs a string and two integers"),
    }
}

#[no_mangle] pub extern "C" fn plain_index_of(haystack: *mut PlainValue, needle: *mut PlainValue) -> *mut PlainValue {
    if is_err(haystack) { return haystack; }
    if is_err(needle) { return needle; }
    if let Some(items) = list_items(haystack) {
        let position = items.iter().position(|item| values_equal(*item, needle, 0));
        return plain_int(position.map(|p| p as i64).unwrap_or(-1));
    }
    match (text_of(haystack), text_of(needle)) {
        (Some(text), Some(part)) => match text.find(part.as_str()) {
            Some(byte) => plain_int(text[..byte].chars().count() as i64),
            None => plain_int(-1),
        },
        _ => type_error("INDEX_OF needs a list, or two strings"),
    }
}

#[no_mangle] pub extern "C" fn plain_reverse(v: *mut PlainValue) -> *mut PlainValue {
    if is_err(v) { return v; }
    if let Some(items) = list_items(v) { return new_list(items.iter().rev().cloned().collect()); }
    match text_of(v) {
        Some(text) => string_value(K_STRING, &text.chars().rev().collect::<String>()),
        None => type_error("REVERSE needs a list or a string"),
    }
}

#[no_mangle] pub extern "C" fn plain_repeat_string(s: *mut PlainValue, count: *mut PlainValue) -> *mut PlainValue {
    if is_err(s) { return s; }
    match (text_of(s), int_of(count)) {
        (Some(text), Some(times)) => {
            if times < 0 { return type_error("REPEAT_STRING count must not be negative"); }
            if (text.len() as u128) * (times as u128) > MAX_STRING_BYTES as u128 { return type_error("REPEAT_STRING result is too large"); }
            string_value(K_STRING, &text.repeat(times as usize))
        }
        _ => type_error("REPEAT_STRING needs a string and an integer"),
    }
}

// ---------------------------------------------------------------------------------- conversion and types

#[no_mangle] pub extern "C" fn plain_to_string(v: *mut PlainValue) -> *mut PlainValue {
    string_value(K_STRING, &value_to_string(v))
}

#[no_mangle] pub extern "C" fn plain_to_int(v: *mut PlainValue) -> *mut PlainValue {
    if is_err(v) { return v; }
    if v.is_null() { return type_error("TO_INT cannot convert null"); }
    unsafe {
        match (*v).kind {
            K_INT => plain_int((*v).i),
            K_FLOAT => {
                let f = (*v).f;
                if f.is_finite() { plain_int(f.trunc() as i64) } else { type_error("TO_INT cannot convert NaN or infinity") }
            }
            K_BOOL => plain_int(((*v).b != 0) as i64),
            K_STRING | K_BORROWED_STRING => {
                let text = read_string(v);
                match text.trim().parse::<i64>() {
                    Ok(n) => plain_int(n),
                    Err(_) => type_error(&format!("cannot convert \"{}\" to an integer", text)),
                }
            }
            _ => type_error("TO_INT needs a number, a boolean or a string"),
        }
    }
}

#[no_mangle] pub extern "C" fn plain_to_float(v: *mut PlainValue) -> *mut PlainValue {
    if is_err(v) { return v; }
    if v.is_null() { return type_error("TO_FLOAT cannot convert null"); }
    unsafe {
        match (*v).kind {
            K_INT => plain_float((*v).i as f64),
            K_FLOAT => v,
            K_BOOL => plain_float(if (*v).b != 0 { 1.0 } else { 0.0 }),
            K_STRING | K_BORROWED_STRING => {
                let text = read_string(v);
                match text.trim().parse::<f64>() {
                    Ok(f) if f.is_finite() => plain_float(f),
                    _ => type_error(&format!("cannot convert \"{}\" to a float", text)),
                }
            }
            _ => type_error("TO_FLOAT needs a number, a boolean or a string"),
        }
    }
}

/// Names the type of a value. For an object it is the class name.
#[no_mangle] pub extern "C" fn plain_type_of(v: *mut PlainValue) -> *mut PlainValue {
    if v.is_null() { return string_value(K_STRING, "null"); }
    let name = unsafe {
        match (*v).kind {
            K_INT => "int".to_string(),
            K_FLOAT => "float".to_string(),
            K_BOOL => "bool".to_string(),
            K_STRING | K_BORROWED_STRING => "string".to_string(),
            K_LIST => "list".to_string(),
            K_DICT => match dict_ref(v).and_then(|d| d.items.get("__class__").copied()) {
                Some(tag) => read_string(tag),
                None => "dict".to_string(),
            },
            K_ERROR => "error".to_string(),
            K_HANDLE => "handle".to_string(),
            K_LINE_ITER => "iterator".to_string(),
            _ => "unknown".to_string(),
        }
    };
    string_value(K_STRING, &name)
}

#[no_mangle] pub extern "C" fn plain_is_error_value(v: *mut PlainValue) -> *mut PlainValue {
    plain_bool(is_err(v) as i8)
}

// ---------------------------------------------------------------------------------- math

fn float_result(v: *mut PlainValue, f: fn(f64) -> f64) -> *mut PlainValue {
    if is_err(v) { return v; }
    match num(v) { Ok(x) => plain_float(f(x)), Err(e) => e }
}

#[no_mangle] pub extern "C" fn plain_exp(v: *mut PlainValue) -> *mut PlainValue { float_result(v, f64::exp) }
#[no_mangle] pub extern "C" fn plain_tan(v: *mut PlainValue) -> *mut PlainValue { float_result(v, f64::tan) }
#[no_mangle] pub extern "C" fn plain_pi() -> *mut PlainValue { plain_float(std::f64::consts::PI) }

#[no_mangle] pub extern "C" fn plain_log(v: *mut PlainValue) -> *mut PlainValue {
    if is_err(v) { return v; }
    match num(v) {
        Ok(x) => if x > 0.0 { plain_float(x.ln()) } else { type_error("LOG needs a positive number") },
        Err(e) => e,
    }
}

#[no_mangle] pub extern "C" fn plain_round(v: *mut PlainValue) -> *mut PlainValue {
    if is_err(v) { return v; }
    if !v.is_null() && unsafe { (*v).kind == K_INT } { return plain_int(unsafe { (*v).i }); }
    match num(v) {
        Ok(x) => if x.is_finite() { plain_int(x.round() as i64) } else { type_error("ROUND cannot round NaN or infinity") },
        Err(e) => e,
    }
}

#[no_mangle] pub extern "C" fn plain_sign(v: *mut PlainValue) -> *mut PlainValue {
    if is_err(v) { return v; }
    if !v.is_null() && unsafe { (*v).kind == K_INT } { return plain_int(unsafe { (*v).i }.signum()); }
    match num(v) {
        Ok(x) => {
            if x.is_nan() { type_error("SIGN of NaN is undefined") }
            else if x > 0.0 { plain_int(1) }
            else if x < 0.0 { plain_int(-1) }
            else { plain_int(0) }
        }
        Err(e) => e,
    }
}

/// The smaller (`want_less`) or larger of two numbers or two strings; returns the original value.
fn pick(a: *mut PlainValue, b: *mut PlainValue, want_less: bool) -> *mut PlainValue {
    match order_of(a, b) {
        Ok(Some(order)) => {
            let a_wins = if want_less { order != std::cmp::Ordering::Greater } else { order != std::cmp::Ordering::Less };
            if a_wins { a } else { b }
        }
        Ok(None) => {
            // NaN is involved: prefer the other operand.
            let a_is_nan = !a.is_null() && unsafe { (*a).kind == K_FLOAT && (*a).f.is_nan() };
            if a_is_nan { b } else { a }
        }
        Err(e) => e,
    }
}

#[no_mangle] pub extern "C" fn plain_min(a: *mut PlainValue, b: *mut PlainValue) -> *mut PlainValue { pick(a, b, true) }
#[no_mangle] pub extern "C" fn plain_max(a: *mut PlainValue, b: *mut PlainValue) -> *mut PlainValue { pick(a, b, false) }

#[no_mangle] pub extern "C" fn plain_clamp(x: *mut PlainValue, low: *mut PlainValue, high: *mut PlainValue) -> *mut PlainValue {
    match order_of(low, high) {
        Ok(Some(std::cmp::Ordering::Greater)) => return type_error("CLAMP needs low <= high"),
        Err(e) => return e,
        _ => {}
    }
    let at_least_low = pick(x, low, false);
    pick(at_least_low, high, true)
}

// ---------------------------------------------------------------------------------- random numbers

static RNG_STATE: AtomicU64 = AtomicU64::new(0);
const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

fn next_random() -> u64 {
    if RNG_STATE.load(AtomicOrdering::Relaxed) == 0 {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x1234_5678_9ABC_DEF1);
        RNG_STATE.store(seed | 1, AtomicOrdering::Relaxed);
    }
    // splitmix64: a fixed seed always gives the same sequence.
    let state = RNG_STATE.fetch_add(GOLDEN, AtomicOrdering::Relaxed).wrapping_add(GOLDEN);
    let mut z = state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Makes the random sequence reproducible.
#[no_mangle] pub extern "C" fn plain_seed(n: *mut PlainValue) -> *mut PlainValue {
    match int_of(n) {
        Some(seed) => {
            RNG_STATE.store((seed as u64 ^ 0xD1B5_4A32_D192_ED03) | 1, AtomicOrdering::Relaxed);
            plain_int(seed)
        }
        None => type_error("SEED needs an integer"),
    }
}

/// A float in [0, 1).
#[no_mangle] pub extern "C" fn plain_random() -> *mut PlainValue {
    plain_float((next_random() >> 11) as f64 / (1u64 << 53) as f64)
}

/// An integer in [low, high], both ends included.
#[no_mangle] pub extern "C" fn plain_random_int(low: *mut PlainValue, high: *mut PlainValue) -> *mut PlainValue {
    match (int_of(low), int_of(high)) {
        (Some(a), Some(b)) => {
            if a > b { return type_error("RANDOM_INT needs low <= high"); }
            let span = (b as i128 - a as i128 + 1) as u128;
            let offset = (next_random() as u128) % span;
            plain_int((a as i128 + offset as i128) as i64)
        }
        _ => type_error("RANDOM_INT needs two integers"),
    }
}

// ---------------------------------------------------------------------------------- collections

#[no_mangle] pub extern "C" fn plain_sort(v: *mut PlainValue) -> *mut PlainValue {
    if is_err(v) { return v; }
    let items: Vec<*mut PlainValue> = match list_items(v) {
        Some(items) => items.clone(),
        None => return type_error("SORT needs a list"),
    };
    if items.iter().all(|x| is_string_value(*x)) {
        let mut keyed: Vec<(String, *mut PlainValue)> = items.iter().map(|x| (read_string(*x), *x)).collect();
        keyed.sort_by(|a, b| a.0.cmp(&b.0));
        return new_list(keyed.into_iter().map(|(_, p)| p).collect());
    }
    if !items.iter().all(|x| is_number(*x)) { return type_error("SORT needs a list of numbers or a list of strings"); }
    if items.iter().any(|x| unsafe { (**x).kind == K_FLOAT }) {
        let mut keyed: Vec<(f64, *mut PlainValue)> = items
            .iter()
            .map(|x| unsafe { if (**x).kind == K_FLOAT { ((**x).f, *x) } else { ((**x).i as f64, *x) } })
            .collect();
        keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
        new_list(keyed.into_iter().map(|(_, p)| p).collect())
    } else {
        let mut keyed: Vec<(i64, *mut PlainValue)> = items.iter().map(|x| unsafe { ((**x).i, *x) }).collect();
        keyed.sort_by_key(|k| k.0);
        new_list(keyed.into_iter().map(|(_, p)| p).collect())
    }
}

/// `SLICE(x, start, end)` is the part of a list or string from `start` up to (not including) `end`.
#[no_mangle] pub extern "C" fn plain_slice(v: *mut PlainValue, start: *mut PlainValue, end: *mut PlainValue) -> *mut PlainValue {
    if is_err(v) { return v; }
    let (first, last) = match (int_of(start), int_of(end)) {
        (Some(a), Some(b)) if a >= 0 && b >= 0 => (a as usize, b as usize),
        (Some(_), Some(_)) => return type_error("SLICE indexes must not be negative"),
        _ => return type_error("SLICE needs integer indexes"),
    };
    if let Some(items) = list_items(v) {
        let stop = last.min(items.len());
        let begin = first.min(stop);
        return new_list(items[begin..stop].to_vec());
    }
    match text_of(v) {
        Some(text) => string_value(K_STRING, &text.chars().skip(first).take(last.saturating_sub(first)).collect::<String>()),
        None => type_error("SLICE needs a list or a string"),
    }
}

#[no_mangle] pub extern "C" fn plain_keys(v: *mut PlainValue) -> *mut PlainValue {
    if is_err(v) { return v; }
    match dict_ref(v) {
        Some(d) => new_list(visible_keys(d).into_iter().map(|k| string_value(K_STRING, k)).collect()),
        None => type_error("KEYS needs a dictionary"),
    }
}

#[no_mangle] pub extern "C" fn plain_values(v: *mut PlainValue) -> *mut PlainValue {
    if is_err(v) { return v; }
    match dict_ref(v) {
        Some(d) => new_list(visible_keys(d).into_iter().map(|k| d.items[k]).collect()),
        None => type_error("VALUES needs a dictionary"),
    }
}

#[no_mangle] pub extern "C" fn plain_has_key(v: *mut PlainValue, key: *mut PlainValue) -> *mut PlainValue {
    if is_err(v) { return v; }
    match dict_ref(v) {
        Some(d) => plain_bool(d.items.contains_key(&value_to_string(key)) as i8),
        None => type_error("HAS_KEY needs a dictionary"),
    }
}

/// Removes a key; returns TRUE when the key existed.
#[no_mangle] pub extern "C" fn plain_delete_key(v: *mut PlainValue, key: *mut PlainValue) -> *mut PlainValue {
    if is_err(v) { return v; }
    if v.is_null() { return type_error("DELETE_KEY needs a dictionary"); }
    unsafe {
        if (*v).kind != K_DICT { return type_error("DELETE_KEY needs a dictionary"); }
        let d = &mut *((*v).ptr as *mut Dict);
        plain_bool(d.items.remove(&value_to_string(key)).is_some() as i8)
    }
}

#[no_mangle] pub extern "C" fn plain_sum(v: *mut PlainValue) -> *mut PlainValue {
    if is_err(v) { return v; }
    let items = match list_items(v) { Some(items) => items, None => return type_error("SUM needs a list") };
    if !items.iter().all(|x| is_number(*x)) { return type_error("SUM needs a list of numbers"); }
    if items.iter().any(|x| unsafe { (**x).kind == K_FLOAT }) {
        let total: f64 = items.iter().map(|x| unsafe { if (**x).kind == K_FLOAT { (**x).f } else { (**x).i as f64 } }).sum();
        plain_float(total)
    } else {
        plain_int(items.iter().fold(0i64, |acc, x| acc.wrapping_add(unsafe { (**x).i })))
    }
}

fn extreme(v: *mut PlainValue, want_less: bool, name: &str) -> *mut PlainValue {
    if is_err(v) { return v; }
    let items = match list_items(v) { Some(items) => items, None => return type_error(&format!("{} needs a list", name)) };
    let mut best = match items.first() { Some(first) => *first, None => return type_error(&format!("{} needs a non-empty list", name)) };
    for item in items.iter().skip(1) {
        best = pick(best, *item, want_less);
        if is_err(best) { return best; }
    }
    best
}

#[no_mangle] pub extern "C" fn plain_list_min(v: *mut PlainValue) -> *mut PlainValue { extreme(v, true, "LIST_MIN") }
#[no_mangle] pub extern "C" fn plain_list_max(v: *mut PlainValue) -> *mut PlainValue { extreme(v, false, "LIST_MAX") }

/// The distinct items of a list, in first-seen order (quadratic time: meant for modest lists).
#[no_mangle] pub extern "C" fn plain_unique(v: *mut PlainValue) -> *mut PlainValue {
    if is_err(v) { return v; }
    let items = match list_items(v) { Some(items) => items, None => return type_error("UNIQUE needs a list") };
    let mut kept: Vec<*mut PlainValue> = Vec::new();
    for item in items {
        if !kept.iter().any(|k| values_equal(*k, *item, 0)) { kept.push(*item); }
    }
    new_list(kept)
}

/// A shallow copy of a list or dictionary (other values are returned as they are).
#[no_mangle] pub extern "C" fn plain_copy(v: *mut PlainValue) -> *mut PlainValue {
    if let Some(items) = list_items(v) { return new_list(items.clone()); }
    if let Some(d) = dict_ref(v) { return new_dict(d.items.clone()); }
    v
}

#[no_mangle] pub extern "C" fn plain_concat(a: *mut PlainValue, b: *mut PlainValue) -> *mut PlainValue {
    if is_err(a) { return a; }
    if is_err(b) { return b; }
    match (list_items(a), list_items(b)) {
        (Some(x), Some(y)) => {
            let mut items = x.clone();
            items.extend(y.iter().cloned());
            new_list(items)
        }
        _ => type_error("CONCAT needs two lists"),
    }
}

// ---------------------------------------------------------------------------------- files, time and the process

#[no_mangle] pub extern "C" fn plain_sleep(ms: *mut PlainValue) -> *mut PlainValue {
    match int_of(ms) {
        Some(n) if (0..=MAX_SLEEP_MS).contains(&n) => {
            let _ = std::io::stdout().flush();
            std::thread::sleep(std::time::Duration::from_millis(n as u64));
            plain_int(n)
        }
        Some(_) => type_error("SLEEP needs 0 to 3600000 milliseconds"),
        None => type_error("SLEEP needs an integer number of milliseconds"),
    }
}

#[no_mangle] pub extern "C" fn plain_file_exists(path: *mut PlainValue) -> *mut PlainValue {
    match text_of(path) {
        Some(p) => plain_bool(std::path::Path::new(&p).exists() as i8),
        None => type_error("FILE_EXISTS needs a string path"),
    }
}

#[no_mangle] pub extern "C" fn plain_append_file(path: *mut PlainValue, contents: *mut PlainValue) -> *mut PlainValue {
    match (text_of(path), text_of(contents)) {
        (Some(p), Some(text)) => match std::fs::OpenOptions::new().create(true).append(true).open(&p) {
            Ok(mut file) => match file.write_all(text.as_bytes()) {
                Ok(()) => plain_bool(1),
                Err(e) => type_error(&format!("APPEND_FILE failed: {}", e)),
            },
            Err(e) => type_error(&format!("APPEND_FILE failed: {}", e)),
        },
        _ => type_error("APPEND_FILE needs two strings"),
    }
}

/// The command-line arguments after the program name.
#[no_mangle] pub extern "C" fn plain_args() -> *mut PlainValue {
    new_list(std::env::args_os().skip(1).map(|a| string_value(K_STRING, &a.to_string_lossy())).collect())
}

/// Ends the program with an exit status (output is flushed first).
#[no_mangle] pub extern "C" fn plain_exit(code: *mut PlainValue) -> *mut PlainValue {
    let status = int_of(code).unwrap_or(1);
    let _ = std::io::stdout().flush();
    std::process::exit(status as i32)
}

// ---------------------------------------------------------------------------------- JSON

fn json_string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn json_encode(v: *mut PlainValue, depth: usize, out: &mut String) -> std::result::Result<(), String> {
    if depth > MAX_JSON_DEPTH { return Err("JSON_ENCODE: the value is nested too deeply".to_string()); }
    if v.is_null() { out.push_str("null"); return Ok(()); }
    unsafe {
        match (*v).kind {
            K_INT => out.push_str(&(*v).i.to_string()),
            K_FLOAT => {
                let f = (*v).f;
                if f.is_finite() { out.push_str(&format!("{:?}", f)); } else { out.push_str("null"); }
            }
            K_BOOL => out.push_str(if (*v).b != 0 { "true" } else { "false" }),
            K_STRING | K_BORROWED_STRING => json_string(&read_string(v), out),
            K_LIST => {
                out.push('[');
                for (index, item) in (&*((*v).ptr as *mut List)).items.iter().enumerate() {
                    if index > 0 { out.push(','); }
                    json_encode(*item, depth + 1, out)?;
                }
                out.push(']');
            }
            K_DICT => {
                let d = &*((*v).ptr as *mut Dict);
                out.push('{');
                for (index, key) in visible_keys(d).into_iter().enumerate() {
                    if index > 0 { out.push(','); }
                    json_string(key, out);
                    out.push(':');
                    json_encode(d.items[key], depth + 1, out)?;
                }
                out.push('}');
            }
            K_ERROR => return Err(format!("JSON_ENCODE cannot encode an error: {}", read_string(v))),
            _ => return Err("JSON_ENCODE cannot encode this kind of value".to_string()),
        }
    }
    Ok(())
}

/// Converts a value to JSON text. Dictionary keys are sorted; objects lose their hidden class tag.
#[no_mangle] pub extern "C" fn plain_json_encode(v: *mut PlainValue) -> *mut PlainValue {
    if is_err(v) { return v; }
    let mut out = String::new();
    match json_encode(v, 0, &mut out) {
        Ok(()) => string_value(K_STRING, &out),
        Err(message) => type_error(&message),
    }
}

struct JsonParser<'a> {
    bytes: &'a [u8],
    pos: usize,
    values: usize,
}

impl<'a> JsonParser<'a> {
    fn fail<T>(&self, message: &str) -> std::result::Result<T, String> {
        Err(format!("JSON_DECODE: {} at position {}", message, self.pos))
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.bytes.len() && matches!(self.bytes[self.pos], b' ' | b'\t' | b'\n' | b'\r') {
            self.pos += 1;
        }
    }

    fn parse_value(&mut self, depth: usize) -> std::result::Result<*mut PlainValue, String> {
        if depth > MAX_JSON_DEPTH { return self.fail("nesting is too deep"); }
        self.values = self.values.saturating_add(1);
        if self.values > MAX_JSON_VALUES { return self.fail("document contains too many values"); }
        self.skip_whitespace();
        match self.bytes.get(self.pos).copied() {
            None => self.fail("unexpected end of input"),
            Some(b'{') => self.parse_object(depth),
            Some(b'[') => self.parse_array(depth),
            Some(b'"') => {
                let text = self.parse_string()?;
                Ok(string_value(K_STRING, &text))
            }
            Some(b't') => self.parse_word("true", plain_bool(1)),
            Some(b'f') => self.parse_word("false", plain_bool(0)),
            Some(b'n') => self.parse_word("null", ptr::null_mut()),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.parse_number(),
            Some(_) => self.fail("unexpected character"),
        }
    }

    fn parse_word(&mut self, word: &str, result: *mut PlainValue) -> std::result::Result<*mut PlainValue, String> {
        if self.bytes[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(result)
        } else {
            self.fail("invalid literal")
        }
    }

    fn parse_number(&mut self) -> std::result::Result<*mut PlainValue, String> {
        let start = self.pos;
        if self.bytes.get(self.pos) == Some(&b'-') { self.pos += 1; }
        // JSON integer: either exactly zero, or a non-zero digit followed by digits.
        match self.bytes.get(self.pos).copied() {
            Some(b'0') => {
                self.pos += 1;
                if self.bytes.get(self.pos).map_or(false, |c| c.is_ascii_digit()) {
                    self.pos = start;
                    return self.fail("leading zeros are not allowed in numbers");
                }
            }
            Some(b'1'..=b'9') => {
                self.pos += 1;
                while self.bytes.get(self.pos).map_or(false, |c| c.is_ascii_digit()) { self.pos += 1; }
            }
            _ => { self.pos = start; return self.fail("invalid number"); }
        }
        let mut is_float = false;
        if self.bytes.get(self.pos) == Some(&b'.') {
            is_float = true;
            self.pos += 1;
            let fraction_start = self.pos;
            while self.bytes.get(self.pos).map_or(false, |c| c.is_ascii_digit()) { self.pos += 1; }
            if self.pos == fraction_start { self.pos = start; return self.fail("fraction requires at least one digit"); }
        }
        if matches!(self.bytes.get(self.pos).copied(), Some(b'e') | Some(b'E')) {
            is_float = true;
            self.pos += 1;
            if matches!(self.bytes.get(self.pos).copied(), Some(b'+') | Some(b'-')) { self.pos += 1; }
            let exponent_start = self.pos;
            while self.bytes.get(self.pos).map_or(false, |c| c.is_ascii_digit()) { self.pos += 1; }
            if self.pos == exponent_start { self.pos = start; return self.fail("exponent requires at least one digit"); }
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos]).unwrap_or("");
        if !is_float {
            if let Ok(n) = text.parse::<i64>() { return Ok(plain_int(n)); }
        }
        match text.parse::<f64>() {
            Ok(f) if f.is_finite() => Ok(plain_float(f)),
            _ => { self.pos = start; self.fail("number is outside the supported range") }
        }
    }

    fn parse_hex4(&mut self) -> std::result::Result<u32, String> {
        if self.pos + 4 > self.bytes.len() { return self.fail("incomplete \\u escape"); }
        let digits = std::str::from_utf8(&self.bytes[self.pos..self.pos + 4]).unwrap_or("");
        if !digits.chars().all(|c| c.is_ascii_hexdigit()) { return self.fail("invalid \\u escape"); }
        match u32::from_str_radix(digits, 16) {
            Ok(code) => {
                self.pos += 4;
                Ok(code)
            }
            Err(_) => self.fail("invalid \\u escape"),
        }
    }

    fn parse_string(&mut self) -> std::result::Result<String, String> {
        self.pos += 1; // opening quote
        let mut out = String::new();
        loop {
            let c = match self.bytes.get(self.pos).copied() {
                Some(c) => c,
                None => return self.fail("unterminated string"),
            };
            self.pos += 1;
            match c {
                b'"' => return Ok(out),
                b'\\' => {
                    let escape = match self.bytes.get(self.pos).copied() {
                        Some(e) => e,
                        None => return self.fail("unterminated escape"),
                    };
                    self.pos += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let first = self.parse_hex4()?;
                            let code = if (0xD800..0xDC00).contains(&first) && self.bytes[self.pos..].starts_with(b"\\u") {
                                self.pos += 2;
                                let second = self.parse_hex4()?;
                                if (0xDC00..0xE000).contains(&second) {
                                    0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00)
                                } else {
                                    return self.fail("invalid surrogate pair");
                                }
                            } else {
                                first
                            };
                            out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                        }
                        _ => return self.fail("invalid escape"),
                    }
                }
                c if c < 0x20 => return self.fail("control character in string"),
                _ => {
                    // Copy one complete UTF-8 sequence (the input came from a Rust string, so it is valid).
                    let start = self.pos - 1;
                    let mut end = self.pos;
                    while end < self.bytes.len() && (self.bytes[end] & 0xC0) == 0x80 { end += 1; }
                    out.push_str(std::str::from_utf8(&self.bytes[start..end]).unwrap_or("\u{FFFD}"));
                    self.pos = end;
                }
            }
        }
    }

    fn parse_array(&mut self, depth: usize) -> std::result::Result<*mut PlainValue, String> {
        self.pos += 1; // [
        let mut items: Vec<*mut PlainValue> = Vec::new();
        self.skip_whitespace();
        if self.bytes.get(self.pos) == Some(&b']') {
            self.pos += 1;
            return Ok(new_list(items));
        }
        loop {
            items.push(self.parse_value(depth + 1)?);
            self.skip_whitespace();
            match self.bytes.get(self.pos).copied() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(new_list(items));
                }
                _ => return self.fail("expected , or ] in an array"),
            }
        }
    }

    fn parse_object(&mut self, depth: usize) -> std::result::Result<*mut PlainValue, String> {
        self.pos += 1; // {
        let mut items: HashMap<String, *mut PlainValue> = HashMap::new();
        self.skip_whitespace();
        if self.bytes.get(self.pos) == Some(&b'}') {
            self.pos += 1;
            return Ok(new_dict(items));
        }
        loop {
            self.skip_whitespace();
            if self.bytes.get(self.pos) != Some(&b'"') { return self.fail("expected a string key"); }
            let key = self.parse_string()?;
            self.skip_whitespace();
            if self.bytes.get(self.pos) != Some(&b':') { return self.fail("expected : after a key"); }
            self.pos += 1;
            let item = self.parse_value(depth + 1)?;
            items.insert(key, item);
            self.skip_whitespace();
            match self.bytes.get(self.pos).copied() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(new_dict(items));
                }
                _ => return self.fail("expected , or } in an object"),
            }
        }
    }
}

/// Parses JSON text into lists, dictionaries, numbers, booleans, strings and null.
#[no_mangle] pub extern "C" fn plain_json_decode(text: *mut PlainValue) -> *mut PlainValue {
    if is_err(text) { return text; }
    let source = match text_of(text) {
        Some(s) => s,
        None => return type_error("JSON_DECODE needs a string"),
    };
    if source.len() > MAX_JSON_INPUT_BYTES {
        return type_error("JSON_DECODE: input exceeds the 100 MB limit");
    }
    let mut parser = JsonParser { bytes: source.as_bytes(), pos: 0, values: 0 };
    match parser.parse_value(0) {
        Ok(result) => {
            parser.skip_whitespace();
            if parser.pos == parser.bytes.len() {
                result
            } else {
                type_error(&format!("JSON_DECODE: unexpected trailing characters at position {}", parser.pos))
            }
        }
        Err(message) => type_error(&message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// The random generator is global, so tests that use it must not run at the same time.
    static RNG_LOCK: Mutex<()> = Mutex::new(());

    fn int(n: i64) -> *mut PlainValue { plain_int(n) }
    fn float(f: f64) -> *mut PlainValue { plain_float(f) }
    fn text(s: &str) -> *mut PlainValue { string_value(K_STRING, s) }
    fn as_text(v: *mut PlainValue) -> String { assert!(is_string_value(v), "expected a string, got {}", value_to_string(v)); read_string(v) }
    fn as_int(v: *mut PlainValue) -> i64 { unsafe { assert_eq!((*v).kind, K_INT, "expected an int, got {}", value_to_string(v)); (*v).i } }
    fn as_float(v: *mut PlainValue) -> f64 { unsafe { assert_eq!((*v).kind, K_FLOAT, "expected a float, got {}", value_to_string(v)); (*v).f } }
    fn as_bool(v: *mut PlainValue) -> bool { unsafe { assert_eq!((*v).kind, K_BOOL, "expected a bool, got {}", value_to_string(v)); (*v).b != 0 } }
    fn list_of(items: &[*mut PlainValue]) -> *mut PlainValue { new_list(items.to_vec()) }
    fn shown(v: *mut PlainValue) -> String { value_to_string(v) }
    fn dict_of(pairs: &[(&str, *mut PlainValue)]) -> *mut PlainValue {
        let mut items: Vec<*mut PlainValue> = Vec::new();
        for (key, item) in pairs { items.push(text(key)); items.push(*item); }
        plain_dict_new(pairs.len() as i64, items.as_mut_ptr())
    }

    #[test]
    fn string_functions() {
        assert_eq!(as_text(plain_trim(text("  hi \n"))), "hi");
        assert_eq!(as_text(plain_replace(text("a-b-c"), text("-"), text("+"))), "a+b+c");
        assert_eq!(as_text(plain_replace(text("abc"), text(""), text("x"))), "abc");
        assert_eq!(as_text(plain_substring(text("héllo"), int(1), int(3))), "éll");
        assert_eq!(as_text(plain_substring(text("abc"), int(2), int(99))), "c");
        assert!(is_err(plain_substring(text("abc"), int(-1), int(1))));
        assert_eq!(as_int(plain_index_of(text("héllo"), text("llo"))), 2);
        assert_eq!(as_int(plain_index_of(text("abc"), text("z"))), -1);
        assert_eq!(as_int(plain_index_of(list_of(&[int(5), int(7)]), int(7))), 1);
        assert_eq!(as_text(plain_reverse(text("abc"))), "cba");
        assert_eq!(shown(plain_reverse(list_of(&[int(1), int(2)]))), "[2, 1]");
        assert_eq!(as_text(plain_repeat_string(text("ab"), int(3))), "ababab");
        assert!(is_err(plain_repeat_string(text("ab"), int(-1))));
        assert!(is_err(plain_trim(int(1))));
    }

    #[test]
    fn conversions_and_types() {
        assert_eq!(as_int(plain_to_int(text("  42 "))), 42);
        assert!(is_err(plain_to_int(text("4x"))));
        assert_eq!(as_int(plain_to_int(float(3.9))), 3);
        assert_eq!(as_int(plain_to_int(float(-3.9))), -3);
        assert_eq!(as_int(plain_to_int(plain_bool(1))), 1);
        assert_eq!(as_float(plain_to_float(text("2.5"))), 2.5);
        assert_eq!(as_float(plain_to_float(int(2))), 2.0);
        assert!(is_err(plain_to_float(text("nan"))));
        assert_eq!(as_text(plain_to_string(int(7))), "7");
        assert_eq!(as_text(plain_type_of(int(1))), "int");
        assert_eq!(as_text(plain_type_of(float(1.0))), "float");
        assert_eq!(as_text(plain_type_of(text("x"))), "string");
        assert_eq!(as_text(plain_type_of(list_of(&[]))), "list");
        assert_eq!(as_text(plain_type_of(dict_of(&[]))), "dict");
        assert_eq!(as_text(plain_type_of(type_error("x"))), "error");
        assert_eq!(as_text(plain_type_of(ptr::null_mut())), "null");
        let object = plain_new_instance();
        plain_dict_set(object, text("__class__"), text("Dog"));
        assert_eq!(as_text(plain_type_of(object)), "Dog");
        assert!(as_bool(plain_is_error_value(type_error("x"))));
        assert!(!as_bool(plain_is_error_value(int(1))));
    }

    #[test]
    fn math_functions() {
        assert_eq!(as_int(plain_round(float(2.5))), 3);
        assert_eq!(as_int(plain_round(float(-2.5))), -3);
        assert_eq!(as_int(plain_round(int(9_007_199_254_740_993))), 9_007_199_254_740_993);
        assert_eq!(as_int(plain_sign(int(-9))), -1);
        assert_eq!(as_int(plain_sign(float(0.0))), 0);
        assert_eq!(as_float(plain_log(float(1.0))), 0.0);
        assert!(is_err(plain_log(int(0))));
        assert!((as_float(plain_exp(int(1))) - std::f64::consts::E).abs() < 1e-12);
        assert!((as_float(plain_pi()) - std::f64::consts::PI).abs() < 1e-15);
        assert!(as_float(plain_tan(float(0.0))).abs() < 1e-15);
    }

    #[test]
    fn min_max_and_clamp() {
        assert_eq!(as_int(plain_min(int(2), int(5))), 2);
        assert_eq!(as_float(plain_min(int(2), float(1.5))), 1.5);
        assert_eq!(as_text(plain_max(text("a"), text("b"))), "b");
        assert!(is_err(plain_min(int(1), text("a"))));
        assert_eq!(as_int(plain_max(int(2), float(f64::NAN))), 2);
        assert_eq!(as_int(plain_clamp(int(15), int(0), int(10))), 10);
        assert_eq!(as_int(plain_clamp(int(-4), int(0), int(10))), 0);
        assert_eq!(as_int(plain_clamp(int(5), int(0), int(10))), 5);
        assert!(is_err(plain_clamp(int(1), int(5), int(0))));
    }

    #[test]
    fn random_numbers_are_reproducible_and_in_range() {
        let _guard = RNG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        plain_seed(int(42));
        let first = (as_int(plain_random_int(int(0), int(1000))), as_int(plain_random_int(int(0), int(1000))));
        plain_seed(int(42));
        let second = (as_int(plain_random_int(int(0), int(1000))), as_int(plain_random_int(int(0), int(1000))));
        assert_eq!(first, second);
        for _ in 0..200 {
            let f = as_float(plain_random());
            assert!((0.0..1.0).contains(&f));
            let n = as_int(plain_random_int(int(-3), int(3)));
            assert!((-3..=3).contains(&n));
        }
        assert_eq!(as_int(plain_random_int(int(7), int(7))), 7);
        assert!(is_err(plain_random_int(int(5), int(1))));
        assert!(is_err(plain_seed(text("x"))));
    }

    #[test]
    fn sorting_and_slicing() {
        assert_eq!(shown(plain_sort(list_of(&[int(3), int(1), int(2)]))), "[1, 2, 3]");
        assert_eq!(shown(plain_sort(list_of(&[float(2.5), int(1), int(3)]))), "[1, 2.5, 3]");
        assert_eq!(shown(plain_sort(list_of(&[text("b"), text("a")]))), "[a, b]");
        assert_eq!(shown(plain_sort(list_of(&[]))), "[]");
        assert!(is_err(plain_sort(list_of(&[int(1), text("a")]))));
        assert!(is_err(plain_sort(int(1))));
        let original = list_of(&[int(2), int(1)]);
        let sorted = plain_sort(original);
        assert_eq!(shown(original), "[2, 1]"); // SORT returns a new list
        assert_eq!(shown(sorted), "[1, 2]");
        assert_eq!(shown(plain_slice(list_of(&[int(1), int(2), int(3), int(4)]), int(1), int(3))), "[2, 3]");
        assert_eq!(shown(plain_slice(list_of(&[int(1)]), int(5), int(2))), "[]");
        assert_eq!(as_text(plain_slice(text("héllo"), int(1), int(99))), "éllo");
        assert!(is_err(plain_slice(text("abc"), int(-1), int(2))));
    }

    #[test]
    fn dictionary_functions() {
        let d = dict_of(&[("b", int(2)), ("a", int(1))]);
        assert_eq!(shown(plain_keys(d)), "[a, b]");
        assert_eq!(shown(plain_values(d)), "[1, 2]");
        assert!(as_bool(plain_has_key(d, text("a"))));
        assert!(!as_bool(plain_has_key(d, text("z"))));
        assert!(as_bool(plain_delete_key(d, text("a"))));
        assert!(!as_bool(plain_has_key(d, text("a"))));
        assert!(!as_bool(plain_delete_key(d, text("a"))));
        let object = plain_new_instance();
        plain_dict_set(object, text("__class__"), text("Dog"));
        plain_dict_set(object, text("name"), text("Rex"));
        assert_eq!(shown(plain_keys(object)), "[name]"); // the class tag is hidden
        assert!(is_err(plain_keys(int(1))));
    }

    #[test]
    fn list_functions() {
        assert_eq!(as_int(plain_sum(list_of(&[int(1), int(2), int(3)]))), 6);
        assert_eq!(as_float(plain_sum(list_of(&[int(1), float(0.5)]))), 1.5);
        assert_eq!(as_int(plain_sum(list_of(&[]))), 0);
        assert!(is_err(plain_sum(list_of(&[text("a")]))));
        assert_eq!(as_int(plain_list_min(list_of(&[int(4), int(2), int(9)]))), 2);
        assert_eq!(as_int(plain_list_max(list_of(&[int(4), int(2), int(9)]))), 9);
        assert!(is_err(plain_list_max(list_of(&[]))));
        assert_eq!(shown(plain_unique(list_of(&[int(1), int(2), int(1), int(3), int(2)]))), "[1, 2, 3]");
        let original = list_of(&[int(1)]);
        let copy = plain_copy(original);
        plain_append(copy, int(2));
        assert_eq!(shown(original), "[1]");
        assert_eq!(shown(copy), "[1, 2]");
        assert_eq!(shown(plain_concat(list_of(&[int(1)]), list_of(&[int(2), int(3)]))), "[1, 2, 3]");
        assert!(is_err(plain_concat(list_of(&[int(1)]), int(2))));
    }

    #[test]
    fn files_time_and_process() {
        assert!(as_bool(plain_file_exists(text("Cargo.toml"))));
        assert!(!as_bool(plain_file_exists(text("definitely/not/here.txt"))));
        let path = std::env::temp_dir().join(format!("plain_stdlib_test_{}.txt", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let path_text = path.to_string_lossy().to_string();
        assert!(as_bool(plain_append_file(text(&path_text), text("one\n"))));
        assert!(as_bool(plain_append_file(text(&path_text), text("two\n"))));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "one\ntwo\n");
        let _ = std::fs::remove_file(&path);
        assert!(is_err(plain_append_file(text("/definitely/not/a/dir/file.txt"), text("x"))));
        assert_eq!(as_int(plain_sleep(int(1))), 1);
        assert!(is_err(plain_sleep(int(-1))));
        assert!(is_err(plain_sleep(int(4_000_000))));
        unsafe { assert_eq!((*plain_args()).kind, K_LIST); }
    }

    #[test]
    fn json_encoding() {
        let value = dict_of(&[("b", list_of(&[plain_bool(1), ptr::null_mut(), text("x")])), ("a", int(1))]);
        assert_eq!(as_text(plain_json_encode(value)), "{\"a\":1,\"b\":[true,null,\"x\"]}");
        assert_eq!(as_text(plain_json_encode(float(1.5))), "1.5");
        assert_eq!(as_text(plain_json_encode(float(1.0))), "1.0");
        assert_eq!(as_text(plain_json_encode(float(f64::NAN))), "null");
        assert_eq!(as_text(plain_json_encode(text("a\"b\n\u{1}"))), "\"a\\\"b\\n\\u0001\"");
        assert!(is_err(plain_json_encode(type_error("boom"))));
        let object = plain_new_instance();
        plain_dict_set(object, text("__class__"), text("Dog"));
        plain_dict_set(object, text("name"), text("Rex"));
        assert_eq!(as_text(plain_json_encode(object)), "{\"name\":\"Rex\"}");
    }

    #[test]
    fn json_decoding_round_trips() {
        let source = "{\"a\": [1, 2.5, \"x\\u00e9\", true, null], \"b\": {\"c\": -3}}";
        let decoded = plain_json_decode(text(source));
        assert!(!is_err(decoded), "{}", shown(decoded));
        assert_eq!(as_text(plain_json_encode(decoded)), "{\"a\":[1,2.5,\"xé\",true,null],\"b\":{\"c\":-3}}");
        assert_eq!(as_int(plain_json_decode(text(" 42 "))), 42);
        assert_eq!(as_float(plain_json_decode(text("1e3"))), 1000.0);
        assert_eq!(as_text(plain_json_decode(text("\"\\ud83d\\ude00\""))), "😀");
        assert_eq!(shown(plain_json_decode(text("[]"))), "[]");
        assert_eq!(shown(plain_json_decode(text("{}"))), "{}");
    }

    #[test]
    fn json_decoding_reports_errors() {
        for bad in ["[1,", "{} x", "nul", "{\"a\" 1}", "[1 2]", "\"abc", "-", "{'a':1}", "", "01", "-01", "1.", ".1", "1e", "1e+", "+1", "--1"] {
            let result = plain_json_decode(text(bad));
            assert!(is_err(result), "{:?} should be rejected", bad);
            assert!(read_string(result).starts_with("JSON_DECODE"), "{}", read_string(result));
        }
        let deep = "[".repeat(200);
        assert!(read_string(plain_json_decode(text(&deep))).contains("too deep"));
        assert!(is_err(plain_json_decode(int(1))));
    }
}
