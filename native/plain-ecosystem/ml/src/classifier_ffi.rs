//! Functions that let Plain programs build, train, use, save and load neural-network classifiers.
//!
//! ```text
//! EXTERN "plain_ml" FUNCTION plain_nn_create(layers AS any, seed AS int) GIVES any
//! EXTERN "plain_ml" FUNCTION plain_nn_train(net AS any, inputs AS any, labels AS any, options AS any) GIVES any
//! EXTERN "plain_ml" FUNCTION plain_nn_predict(net AS any, input AS any) GIVES any
//! LET net = plain_nn_create([2, 8, 2], 7)
//! SAY plain_nn_train(net, [[0, 0], [0, 1], [1, 0], [1, 1]], [0, 1, 1, 0], {epochs: 400, learning_rate: 0.05})
//! SAY plain_nn_predict(net, [1, 0])
//! ```
use crate::{c_string, ffi_error, Classifier, Mlp, TrainConfig};
use plain_runtime::{
    plain_bool, plain_dict_get, plain_float, plain_handle, plain_handle_clear, plain_handle_ptr, plain_int,
    plain_length, plain_list_get, plain_list_new, value_text, PlainValue, K_DICT, K_ERROR, K_FLOAT, K_INT, K_LIST,
};
use std::os::raw::c_void;

const TAG_CLASSIFIER: i64 = 0x504C_4E4E;
const MAX_SAMPLES: usize = 1_000_000;
const MAX_TOTAL_FEATURES: usize = 10_000_000;
const BAD_NETWORK: &str = "ffi error: invalid or released network";

fn fail(message: &str) -> *mut PlainValue {
    ffi_error(&format!("ffi error: {}", message))
}

fn is_kind(v: *mut PlainValue, kind: i32) -> bool {
    !v.is_null() && unsafe { (*v).kind == kind }
}

fn number(v: *mut PlainValue) -> Option<f64> {
    if v.is_null() {
        return None;
    }
    unsafe {
        match (*v).kind {
            K_INT => Some((*v).i as f64),
            K_FLOAT => Some((*v).f),
            _ => None,
        }
    }
}

fn integer(v: *mut PlainValue) -> Option<i64> {
    if is_kind(v, K_INT) {
        Some(unsafe { (*v).i })
    } else {
        None
    }
}

fn list_length(v: *mut PlainValue) -> Option<usize> {
    if !is_kind(v, K_LIST) {
        return None;
    }
    integer(plain_length(v)).map(|n| n.max(0) as usize)
}

fn list_item(v: *mut PlainValue, index: usize) -> *mut PlainValue {
    plain_list_get(v, plain_int(index as i64))
}

fn float_vector(v: *mut PlainValue, what: &str) -> Result<Vec<f64>, String> {
    let count = list_length(v).ok_or_else(|| format!("{} must be a list of numbers", what))?;
    if count > MAX_SAMPLES {
        return Err(format!("{} is too long", what));
    }
    let mut out = Vec::with_capacity(count);
    for index in 0..count {
        match number(list_item(v, index)) {
            Some(x) => out.push(x),
            None => return Err(format!("{} must contain only numbers", what)),
        }
    }
    Ok(out)
}

fn matrix(v: *mut PlainValue, what: &str) -> Result<Vec<Vec<f64>>, String> {
    let count = list_length(v).ok_or_else(|| format!("{} must be a list of lists of numbers", what))?;
    if count > MAX_SAMPLES {
        return Err(format!("{} has too many rows", what));
    }
    let mut rows = Vec::with_capacity(count);
    let mut total_features = 0usize;
    for index in 0..count {
        let row = float_vector(list_item(v, index), what)?;
        total_features = total_features.checked_add(row.len())
            .ok_or_else(|| format!("{} contains too many features", what))?;
        if total_features > MAX_TOTAL_FEATURES {
            return Err(format!("{} exceeds the total feature limit of {}", what, MAX_TOTAL_FEATURES));
        }
        rows.push(row);
    }
    Ok(rows)
}

fn integer_vector(v: *mut PlainValue, what: &str) -> Result<Vec<usize>, String> {
    let count = list_length(v).ok_or_else(|| format!("{} must be a list of integers", what))?;
    if count > MAX_SAMPLES {
        return Err(format!("{} is too long", what));
    }
    let mut out = Vec::with_capacity(count);
    for index in 0..count {
        match integer(list_item(v, index)) {
            Some(x) if x >= 0 => out.push(x as usize),
            _ => return Err(format!("{} must contain only non-negative integers", what)),
        }
    }
    Ok(out)
}

/// A value from an options dictionary, or `None` when the dictionary or the key is missing.
fn option(options: *mut PlainValue, key: &str) -> Option<*mut PlainValue> {
    if !is_kind(options, K_DICT) {
        return None;
    }
    let found = plain_dict_get(options, c_string(key));
    if is_kind(found, K_ERROR) {
        None
    } else {
        Some(found)
    }
}

fn network<'a>(v: *mut PlainValue) -> Option<&'a Classifier> {
    let p = plain_handle_ptr(v, TAG_CLASSIFIER) as *const Classifier;
    if p.is_null() {
        None
    } else {
        Some(unsafe { &*p })
    }
}

fn into_handle(model: Classifier) -> *mut PlainValue {
    plain_handle(Box::into_raw(Box::new(model)) as *mut c_void, TAG_CLASSIFIER)
}

/// `plain_nn_create(layers, seed)`: `layers` is a list of layer widths such as `[2, 8, 2]`.
#[no_mangle]
pub extern "C" fn plain_nn_create(layers: *mut PlainValue, seed: *mut PlainValue) -> *mut PlainValue {
    let sizes = match integer_vector(layers, "layers") {
        Ok(sizes) => sizes,
        Err(message) => return fail(&message),
    };
    let seed = integer(seed).unwrap_or(1) as u64;
    match Classifier::new(&sizes, seed) {
        Ok(model) => into_handle(model),
        Err(error) => fail(&error.to_string()),
    }
}

/// `plain_nn_train(net, inputs, labels, options)` returns the mean loss of the last epoch.
/// `options` may contain `epochs`, `learning_rate`, `batch_size` and `seed`.
#[no_mangle]
pub extern "C" fn plain_nn_train(model: *mut PlainValue, inputs: *mut PlainValue, labels: *mut PlainValue, options: *mut PlainValue) -> *mut PlainValue {
    let net = match network(model) {
        Some(net) => net,
        None => return ffi_error(BAD_NETWORK),
    };
    let rows = match matrix(inputs, "inputs") {
        Ok(rows) => rows,
        Err(message) => return fail(&message),
    };
    let targets = match integer_vector(labels, "labels") {
        Ok(targets) => targets,
        Err(message) => return fail(&message),
    };
    let mut config = TrainConfig::default();
    if let Some(v) = option(options, "epochs") {
        match integer(v) {
            Some(n) if n > 0 => config.epochs = n as usize,
            _ => return fail("epochs must be a positive integer"),
        }
    }
    if let Some(v) = option(options, "learning_rate") {
        match number(v) {
            Some(x) if x > 0.0 => config.learning_rate = x,
            _ => return fail("learning_rate must be a positive number"),
        }
    }
    if let Some(v) = option(options, "batch_size") {
        match integer(v) {
            Some(n) if n > 0 => config.batch_size = n as usize,
            _ => return fail("batch_size must be a positive integer"),
        }
    }
    if let Some(v) = option(options, "seed") {
        match integer(v) {
            Some(n) => config.seed = n as u64,
            None => return fail("seed must be an integer"),
        }
    }
    match net.train(&rows, &targets, &config) {
        Ok(loss) => plain_float(loss),
        Err(error) => fail(&error.to_string()),
    }
}

/// The most likely class (an integer) for one input list.
#[no_mangle]
pub extern "C" fn plain_nn_predict(model: *mut PlainValue, input: *mut PlainValue) -> *mut PlainValue {
    let net = match network(model) {
        Some(net) => net,
        None => return ffi_error(BAD_NETWORK),
    };
    let features = match float_vector(input, "input") {
        Ok(features) => features,
        Err(message) => return fail(&message),
    };
    match net.predict(&features) {
        Ok(class) => plain_int(class as i64),
        Err(error) => fail(&error.to_string()),
    }
}

/// A list with the probability of every class for one input list.
#[no_mangle]
pub extern "C" fn plain_nn_probabilities(model: *mut PlainValue, input: *mut PlainValue) -> *mut PlainValue {
    let net = match network(model) {
        Some(net) => net,
        None => return ffi_error(BAD_NETWORK),
    };
    let features = match float_vector(input, "input") {
        Ok(features) => features,
        Err(message) => return fail(&message),
    };
    match net.probabilities(&features) {
        Ok(probabilities) => {
            let mut items: Vec<*mut PlainValue> = probabilities.iter().map(|p| plain_float(*p)).collect();
            plain_list_new(items.len() as i64, items.as_mut_ptr())
        }
        Err(error) => fail(&error.to_string()),
    }
}

/// Fraction of samples classified correctly (a float between 0 and 1).
#[no_mangle]
pub extern "C" fn plain_nn_accuracy(model: *mut PlainValue, inputs: *mut PlainValue, labels: *mut PlainValue) -> *mut PlainValue {
    let net = match network(model) {
        Some(net) => net,
        None => return ffi_error(BAD_NETWORK),
    };
    let rows = match matrix(inputs, "inputs") {
        Ok(rows) => rows,
        Err(message) => return fail(&message),
    };
    let targets = match integer_vector(labels, "labels") {
        Ok(targets) => targets,
        Err(message) => return fail(&message),
    };
    match net.accuracy(&rows, &targets) {
        Ok(value) => plain_float(value),
        Err(error) => fail(&error.to_string()),
    }
}

#[no_mangle]
pub extern "C" fn plain_nn_parameter_count(model: *mut PlainValue) -> *mut PlainValue {
    match network(model) {
        Some(net) => plain_int(net.mlp.parameter_count() as i64),
        None => ffi_error(BAD_NETWORK),
    }
}

/// Writes the architecture and all weights to a file.
#[no_mangle]
pub extern "C" fn plain_nn_save(model: *mut PlainValue, path: *mut PlainValue) -> *mut PlainValue {
    let net = match network(model) {
        Some(net) => net,
        None => return ffi_error(BAD_NETWORK),
    };
    let path = match value_text(path) {
        Some(path) => path,
        None => return fail("the path must be a string"),
    };
    match net.mlp.save(&path) {
        Ok(()) => plain_bool(1),
        Err(error) => fail(&error.to_string()),
    }
}

/// Reads a network written by `plain_nn_save`.
#[no_mangle]
pub extern "C" fn plain_nn_load(path: *mut PlainValue) -> *mut PlainValue {
    let path = match value_text(path) {
        Some(path) => path,
        None => return fail("the path must be a string"),
    };
    match Mlp::load(&path) {
        Ok(mlp) => into_handle(Classifier { mlp }),
        Err(error) => fail(&error.to_string()),
    }
}

/// Releases a network. The handle becomes invalid, so freeing twice is reported instead of crashing.
#[no_mangle]
pub extern "C" fn plain_nn_free(model: *mut PlainValue) -> *mut PlainValue {
    let p = plain_handle_clear(model, TAG_CLASSIFIER) as *mut Classifier;
    if p.is_null() {
        return ffi_error("ffi error: network already released or invalid");
    }
    unsafe { drop(Box::from_raw(p)) };
    plain_bool(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use plain_runtime::{plain_dict_new, plain_is_error};

    fn list_of(mut items: Vec<*mut PlainValue>) -> *mut PlainValue {
        plain_list_new(items.len() as i64, items.as_mut_ptr())
    }
    fn ints(values: &[i64]) -> *mut PlainValue {
        list_of(values.iter().map(|v| plain_int(*v)).collect())
    }
    fn floats(values: &[f64]) -> *mut PlainValue {
        list_of(values.iter().map(|v| plain_float(*v)).collect())
    }
    fn rows(values: &[[f64; 2]]) -> *mut PlainValue {
        list_of(values.iter().map(|r| floats(r)).collect())
    }
    fn options(pairs: &[(&str, *mut PlainValue)]) -> *mut PlainValue {
        let mut items: Vec<*mut PlainValue> = Vec::new();
        for (key, value) in pairs {
            items.push(c_string(key));
            items.push(*value);
        }
        plain_dict_new(pairs.len() as i64, items.as_mut_ptr())
    }
    fn is_error(v: *mut PlainValue) -> bool {
        plain_is_error(v) == 1
    }
    fn float_value(v: *mut PlainValue) -> f64 {
        unsafe {
            assert_eq!((*v).kind, K_FLOAT);
            (*v).f
        }
    }
    fn int_value(v: *mut PlainValue) -> i64 {
        unsafe {
            assert_eq!((*v).kind, K_INT);
            (*v).i
        }
    }

    const XOR_INPUTS: [[f64; 2]; 4] = [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]];

    #[test]
    fn plain_programs_can_train_use_save_load_and_free_a_network() {
        let net = plain_nn_create(ints(&[2, 8, 2]), plain_int(7));
        assert!(!is_error(net));
        assert_eq!(int_value(plain_nn_parameter_count(net)), 42);
        let settings = options(&[
            ("epochs", plain_int(300)),
            ("learning_rate", plain_float(0.05)),
            ("batch_size", plain_int(4)),
            ("seed", plain_int(1)),
        ]);
        let loss = plain_nn_train(net, rows(&XOR_INPUTS), ints(&[0, 1, 1, 0]), settings);
        assert!(float_value(loss) < 0.05);
        assert_eq!(float_value(plain_nn_accuracy(net, rows(&XOR_INPUTS), ints(&[0, 1, 1, 0]))), 1.0);
        assert_eq!(int_value(plain_nn_predict(net, floats(&[1.0, 0.0]))), 1);
        let probabilities = plain_nn_probabilities(net, floats(&[1.0, 0.0]));
        assert_eq!(list_length(probabilities), Some(2));
        let total = float_value(list_item(probabilities, 0)) + float_value(list_item(probabilities, 1));
        assert!((total - 1.0).abs() < 1e-9);

        let path = std::env::temp_dir().join(format!("plain_nn_ffi_{}.bin", std::process::id()));
        let path_text = path.to_string_lossy().to_string();
        assert!(!is_error(plain_nn_save(net, c_string(&path_text))));
        let restored = plain_nn_load(c_string(&path_text));
        assert!(!is_error(restored));
        assert_eq!(int_value(plain_nn_predict(restored, floats(&[0.0, 1.0]))), 1);
        let _ = std::fs::remove_file(&path);

        assert!(!is_error(plain_nn_free(net)));
        assert!(is_error(plain_nn_free(net))); // freed twice is an error, not a crash
        assert!(is_error(plain_nn_predict(net, floats(&[1.0, 0.0])))); // use after free is an error
        assert!(!is_error(plain_nn_free(restored)));
    }

    #[test]
    fn mistakes_from_plain_are_errors() {
        assert!(is_error(plain_nn_create(plain_int(3), plain_int(1)))); // layers must be a list
        assert!(is_error(plain_nn_create(ints(&[2]), plain_int(1)))); // needs two sizes
        assert!(is_error(plain_nn_create(ints(&[2, -1]), plain_int(1)))); // negative size
        let net = plain_nn_create(ints(&[2, 4, 2]), plain_int(1));
        assert!(is_error(plain_nn_train(net, rows(&XOR_INPUTS), ints(&[0, 1, 1]), plain_int(0)))); // label count
        assert!(is_error(plain_nn_train(net, rows(&XOR_INPUTS), ints(&[0, 1, 1, 5]), plain_int(0)))); // label range
        assert!(is_error(plain_nn_train(net, plain_int(1), ints(&[0]), plain_int(0)))); // inputs not a list
        let bad_epochs = options(&[("epochs", plain_int(-5))]);
        assert!(is_error(plain_nn_train(net, rows(&XOR_INPUTS), ints(&[0, 1, 1, 0]), bad_epochs)));
        assert!(is_error(plain_nn_predict(net, floats(&[1.0])))); // wrong number of features
        assert!(is_error(plain_nn_predict(plain_int(5), floats(&[1.0, 0.0])))); // not a network
        assert!(is_error(plain_nn_save(net, plain_int(1)))); // path must be a string
        assert!(is_error(plain_nn_load(c_string("/definitely/not/here.bin"))));
        assert!(!is_error(plain_nn_free(net)));
    }
}
