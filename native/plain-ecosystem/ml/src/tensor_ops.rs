//! Checked tensor operations: every failure is an `MlError`, never a panic.
use crate::{MlError, MlResult, Tensor};

/// Largest tensor the library will allocate (10 million values, 80 MB of f64).
pub const MAX_TENSOR_ELEMENTS: usize = 10_000_000;

/// Number of elements in a shape, rejecting overflow and oversized tensors.
pub fn element_count(shape: &[usize]) -> MlResult<usize> {
    let mut count: usize = 1;
    for &dim in shape {
        count = count
            .checked_mul(dim)
            .ok_or_else(|| MlError::Shape(format!("shape {:?} is too large", shape)))?;
    }
    if count > MAX_TENSOR_ELEMENTS {
        return Err(MlError::Shape(format!(
            "shape {:?} has {} elements; the limit is {}",
            shape, count, MAX_TENSOR_ELEMENTS
        )));
    }
    Ok(count)
}

impl Tensor {
    /// Like `Tensor::new`, but returns an error instead of panicking when the data does not fit the shape.
    pub fn try_new(data: Vec<f64>, shape: Vec<usize>) -> MlResult<Tensor> {
        let count = element_count(&shape)?;
        if data.len() != count {
            return Err(MlError::Shape(format!("{} values do not fit shape {:?}", data.len(), shape)));
        }
        Ok(Tensor { data, shape })
    }

    pub fn try_zeros(shape: Vec<usize>) -> MlResult<Tensor> {
        let count = element_count(&shape)?;
        Ok(Tensor { data: vec![0.0; count], shape })
    }

    pub fn map(&self, f: impl Fn(f64) -> f64) -> Tensor {
        Tensor { data: self.data.iter().map(|x| f(*x)).collect(), shape: self.shape.clone() }
    }

    pub fn zip_with(&self, other: &Tensor, f: impl Fn(f64, f64) -> f64) -> MlResult<Tensor> {
        if self.shape != other.shape {
            return Err(MlError::Shape(format!("shapes {:?} and {:?} differ", self.shape, other.shape)));
        }
        Ok(Tensor {
            data: self.data.iter().zip(&other.data).map(|(a, b)| f(*a, *b)).collect(),
            shape: self.shape.clone(),
        })
    }

    pub fn sum(&self) -> f64 {
        self.data.iter().sum()
    }

    pub fn rows_cols(&self) -> MlResult<(usize, usize)> {
        if self.shape.len() == 2 {
            Ok((self.shape[0], self.shape[1]))
        } else {
            Err(MlError::Shape(format!("expected a matrix, got shape {:?}", self.shape)))
        }
    }

    pub fn try_transpose(&self) -> MlResult<Tensor> {
        let (rows, cols) = self.rows_cols()?;
        let mut out = vec![0.0; self.data.len()];
        for r in 0..rows {
            for c in 0..cols {
                out[c * rows + r] = self.data[r * cols + c];
            }
        }
        Ok(Tensor { data: out, shape: vec![cols, rows] })
    }

    pub fn try_matmul(&self, other: &Tensor) -> MlResult<Tensor> {
        let (m, k) = self.rows_cols()?;
        let (k2, n) = other.rows_cols()?;
        if k != k2 {
            return Err(MlError::Shape(format!("cannot multiply shape {:?} by {:?}", self.shape, other.shape)));
        }
        element_count(&[m, n])?;
        let mut out = vec![0.0; m * n];
        for i in 0..m {
            for q in 0..k {
                let a = self.data[i * k + q];
                for j in 0..n {
                    out[i * n + j] += a * other.data[q * n + j];
                }
            }
        }
        Ok(Tensor { data: out, shape: vec![m, n] })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_are_checked_without_panicking() {
        assert!(Tensor::try_new(vec![1.0, 2.0, 3.0], vec![2, 2]).is_err());
        assert!(Tensor::try_new(vec![1.0, 2.0, 3.0, 4.0], vec![2, 2]).is_ok());
        assert!(Tensor::try_zeros(vec![usize::MAX, 2]).is_err()); // overflow
        assert!(Tensor::try_zeros(vec![100_000, 100_000]).is_err()); // over the size limit, nothing allocated
        assert!(element_count(&[1000, 1000]).is_ok());
    }

    #[test]
    fn matrix_operations_report_shape_errors() {
        let a = Tensor::try_new(vec![1.0, 2.0, 3.0, 4.0], vec![2, 2]).unwrap();
        let b = Tensor::try_new(vec![5.0, 6.0, 7.0], vec![1, 3]).unwrap();
        assert!(a.try_matmul(&b).is_err());
        assert!(a.zip_with(&b, |x, y| x + y).is_err());
        let flat = Tensor::try_new(vec![1.0, 2.0], vec![2]).unwrap();
        assert!(flat.try_transpose().is_err());
        assert_eq!(a.try_matmul(&a).unwrap().data, vec![7.0, 10.0, 15.0, 22.0]);
        assert_eq!(a.try_transpose().unwrap().data, vec![1.0, 3.0, 2.0, 4.0]);
        assert_eq!(a.sum(), 10.0);
        assert_eq!(a.map(|x| x * 2.0).data, vec![2.0, 4.0, 6.0, 8.0]);
    }
}
