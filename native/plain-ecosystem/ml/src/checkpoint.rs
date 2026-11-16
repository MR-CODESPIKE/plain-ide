//! Saving and loading model parameters in a small, versioned binary format (little endian).
//!
//! Loading is all-or-nothing: the data is fully validated before any parameter is changed.
use crate::tensor_ops::element_count;
use crate::{Activation, MlError, MlResult, Mlp, Tensor, Var};

const PARAMS_MAGIC: &[u8] = b"PLNP";
const MODEL_MAGIC: &[u8] = b"PLNM";
const VERSION: u32 = 1;
const MAX_DIMS: usize = 8;

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Reader<'a> {
        Reader { bytes, position: 0 }
    }

    fn take(&mut self, count: usize) -> MlResult<&'a [u8]> {
        let bytes: &'a [u8] = self.bytes;
        let end = match self.position.checked_add(count) {
            Some(end) if end <= bytes.len() => end,
            _ => return Err(MlError::Format("unexpected end of data".to_string())),
        };
        let slice = &bytes[self.position..end];
        self.position = end;
        Ok(slice)
    }

    fn u32(&mut self) -> MlResult<u32> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn u64(&mut self) -> MlResult<u64> {
        let b = self.take(8)?;
        Ok(u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
    }

    fn f64(&mut self) -> MlResult<f64> {
        Ok(f64::from_bits(self.u64()?))
    }

    fn finished(&self) -> bool {
        self.position == self.bytes.len()
    }
}

/// Serialises the values of `params` (shapes included).
pub fn save_params(params: &[Var]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(PARAMS_MAGIC);
    put_u32(&mut out, VERSION);
    put_u32(&mut out, params.len() as u32);
    for param in params {
        let tensor = param.value();
        put_u32(&mut out, tensor.shape.len() as u32);
        for &dim in &tensor.shape {
            put_u64(&mut out, dim as u64);
        }
        for &x in &tensor.data {
            put_u64(&mut out, x.to_bits());
        }
    }
    out
}

fn read_params(reader: &mut Reader, params: &[Var]) -> MlResult<Vec<Tensor>> {
    if reader.take(4)? != PARAMS_MAGIC {
        return Err(MlError::Format("this is not a Plain parameter checkpoint".to_string()));
    }
    let version = reader.u32()?;
    if version != VERSION {
        return Err(MlError::Format(format!("unsupported checkpoint version {}", version)));
    }
    let count = reader.u32()? as usize;
    if count != params.len() {
        return Err(MlError::Format(format!("the checkpoint has {} tensors but the model has {}", count, params.len())));
    }
    let mut loaded = Vec::with_capacity(count);
    for param in params {
        let dims = reader.u32()? as usize;
        if dims > MAX_DIMS {
            return Err(MlError::Format("a tensor has too many dimensions".to_string()));
        }
        let mut shape = Vec::with_capacity(dims);
        for _ in 0..dims {
            shape.push(reader.u64()? as usize);
        }
        if shape != param.shape() {
            return Err(MlError::Format(format!("a tensor has shape {:?} but the model expects {:?}", shape, param.shape())));
        }
        let elements = element_count(&shape)?;
        let mut data = Vec::with_capacity(elements);
        for _ in 0..elements {
            data.push(reader.f64()?);
        }
        loaded.push(Tensor { data, shape });
    }
    Ok(loaded)
}

/// Restores the values of `params` from `save_params` output. The shapes must match.
pub fn load_params(params: &[Var], bytes: &[u8]) -> MlResult<()> {
    let mut reader = Reader::new(bytes);
    let loaded = read_params(&mut reader, params)?;
    if !reader.finished() {
        return Err(MlError::Format("unexpected trailing data".to_string()));
    }
    // Commit only after the entire file has been validated.
    for (param, tensor) in params.iter().zip(loaded) {
        param.set_value(tensor)?;
    }
    Ok(())
}

impl Mlp {
    /// The architecture (layer sizes and activation) followed by all parameters.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MODEL_MAGIC);
        put_u32(&mut out, VERSION);
        put_u32(&mut out, self.activation.code());
        put_u32(&mut out, self.sizes.len() as u32);
        for &size in &self.sizes {
            put_u64(&mut out, size as u64);
        }
        out.extend_from_slice(&save_params(&self.parameters()));
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> MlResult<Mlp> {
        let mut reader = Reader::new(bytes);
        if reader.take(4)? != MODEL_MAGIC {
            return Err(MlError::Format("this is not a Plain network file".to_string()));
        }
        let version = reader.u32()?;
        if version != VERSION {
            return Err(MlError::Format(format!("unsupported network file version {}", version)));
        }
        let activation = Activation::from_code(reader.u32()?)
            .ok_or_else(|| MlError::Format("unknown activation function".to_string()))?;
        let layer_count = reader.u32()? as usize;
        if !(2..=64).contains(&layer_count) {
            return Err(MlError::Format("a network needs between 2 and 64 layer sizes".to_string()));
        }
        let mut sizes = Vec::with_capacity(layer_count);
        for _ in 0..layer_count {
            sizes.push(reader.u64()? as usize);
        }
        let mlp = Mlp::new(&sizes, activation, 0).map_err(|e| MlError::Format(e.to_string()))?;
        let params = mlp.parameters();
        let loaded = read_params(&mut reader, &params)?;
        if !reader.finished() {
            return Err(MlError::Format("unexpected trailing data".to_string()));
        }
        for (param, tensor) in params.iter().zip(loaded) {
            param.set_value(tensor)?;
        }
        Ok(mlp)
    }

    pub fn save(&self, path: &str) -> MlResult<()> {
        std::fs::write(path, self.to_bytes()).map_err(|e| MlError::Io(format!("{}: {}", path, e)))
    }

    pub fn load(path: &str) -> MlResult<Mlp> {
        let bytes = std::fs::read(path).map_err(|e| MlError::Io(format!("{}: {}", path, e)))?;
        Mlp::from_bytes(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Activation;

    fn params() -> Vec<Var> {
        vec![
            Var::parameter(Tensor::try_new(vec![1.0, 2.0, 3.0, 4.0], vec![2, 2]).unwrap()),
            Var::parameter(Tensor::try_new(vec![0.5, -0.5], vec![1, 2]).unwrap()),
        ]
    }

    #[test]
    fn parameters_round_trip_exactly() {
        let source = params();
        let bytes = save_params(&source);
        let target = vec![
            Var::parameter(Tensor::try_zeros(vec![2, 2]).unwrap()),
            Var::parameter(Tensor::try_zeros(vec![1, 2]).unwrap()),
        ];
        load_params(&target, &bytes).unwrap();
        assert_eq!(target[0].value().data, vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(target[1].value().data, vec![0.5, -0.5]);
    }

    #[test]
    fn bad_checkpoints_are_rejected_and_change_nothing() {
        let bytes = save_params(&params());
        let target = vec![
            Var::parameter(Tensor::try_zeros(vec![2, 2]).unwrap()),
            Var::parameter(Tensor::try_zeros(vec![1, 2]).unwrap()),
        ];
        assert!(load_params(&target, &bytes[..bytes.len() - 1]).is_err()); // truncated
        let mut extended = bytes.clone();
        extended.push(0);
        assert!(load_params(&target, &extended).is_err()); // trailing data
        let mut wrong_magic = bytes.clone();
        wrong_magic[0] = b'X';
        assert!(load_params(&target, &wrong_magic).is_err());
        let different_shape = vec![
            Var::parameter(Tensor::try_zeros(vec![4, 1]).unwrap()),
            Var::parameter(Tensor::try_zeros(vec![1, 2]).unwrap()),
        ];
        assert!(load_params(&different_shape, &bytes).is_err());
        assert!(load_params(&target[..1], &bytes).is_err()); // wrong number of tensors
        assert!(load_params(&target, &[]).is_err());
        assert_eq!(target[0].value().data, vec![0.0; 4]); // nothing was modified by the failures
    }

    #[test]
    fn a_network_round_trips_through_bytes_and_files() {
        let mlp = Mlp::new(&[2, 4, 3], Activation::Tanh, 3).unwrap();
        let x = Var::constant(Tensor::try_new(vec![0.3, -0.8], vec![1, 2]).unwrap());
        let expected = mlp.forward(&x).unwrap().value();
        let restored = Mlp::from_bytes(&mlp.to_bytes()).unwrap();
        assert_eq!(restored.sizes, vec![2, 4, 3]);
        assert_eq!(restored.activation, Activation::Tanh);
        assert_eq!(restored.forward(&x).unwrap().value(), expected);
        let path = std::env::temp_dir().join(format!("plain_ml_net_{}.bin", std::process::id()));
        let path_text = path.to_string_lossy().to_string();
        mlp.save(&path_text).unwrap();
        assert_eq!(Mlp::load(&path_text).unwrap().forward(&x).unwrap().value(), expected);
        let _ = std::fs::remove_file(&path);
        assert!(Mlp::load("/definitely/not/here.bin").is_err());
    }

    #[test]
    fn corrupt_network_files_are_rejected() {
        let bytes = Mlp::new(&[2, 3, 2], Activation::Relu, 1).unwrap().to_bytes();
        assert!(Mlp::from_bytes(&bytes[..10]).is_err());
        assert!(Mlp::from_bytes(b"not a network").is_err());
        let mut bad_activation = bytes.clone();
        bad_activation[8] = 99;
        assert!(Mlp::from_bytes(&bad_activation).is_err());
        let mut huge = bytes.clone();
        // claim an enormous first layer: must fail cleanly instead of allocating
        huge[16..24].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(Mlp::from_bytes(&huge).is_err());
    }
}
