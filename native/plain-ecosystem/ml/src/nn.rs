//! Layers, a small deterministic random generator and a multi-layer perceptron.
use crate::{MlError, MlResult, Tensor, Var};

/// splitmix64: a small, fast generator. The same seed always gives the same sequence.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_add(0x9E37_79B9_7F4A_7C15))
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A float in [0, 1).
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn uniform(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * self.next_f64()
    }

    /// An index in 0..n (0 when n is 0).
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }

    /// Fisher-Yates shuffle.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below(i + 1);
            items.swap(i, j);
        }
    }
}

/// A fully connected layer: `x * weight + bias`.
pub struct Linear {
    pub weight: Var,
    pub bias: Var,
}

impl Linear {
    /// Xavier/Glorot uniform initialisation.
    pub fn new(inputs: usize, outputs: usize, rng: &mut Rng) -> MlResult<Linear> {
        if inputs == 0 || outputs == 0 {
            return Err(MlError::Invalid("layer sizes must be at least 1".to_string()));
        }
        let count = crate::tensor_ops::element_count(&[inputs, outputs])?;
        let limit = (6.0 / (inputs + outputs) as f64).sqrt();
        let data: Vec<f64> = (0..count).map(|_| rng.uniform(-limit, limit)).collect();
        Ok(Linear {
            weight: Var::parameter(Tensor::try_new(data, vec![inputs, outputs])?),
            bias: Var::parameter(Tensor::try_zeros(vec![1, outputs])?),
        })
    }

    pub fn forward(&self, x: &Var) -> MlResult<Var> {
        x.matmul(&self.weight)?.add_bias(&self.bias)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Activation {
    Relu,
    Tanh,
    Sigmoid,
}

impl Activation {
    pub fn apply(self, x: &Var) -> Var {
        match self {
            Activation::Relu => x.relu(),
            Activation::Tanh => x.tanh(),
            Activation::Sigmoid => x.sigmoid(),
        }
    }

    pub fn code(self) -> u32 {
        match self {
            Activation::Relu => 0,
            Activation::Tanh => 1,
            Activation::Sigmoid => 2,
        }
    }

    pub fn from_code(code: u32) -> Option<Activation> {
        match code {
            0 => Some(Activation::Relu),
            1 => Some(Activation::Tanh),
            2 => Some(Activation::Sigmoid),
            _ => None,
        }
    }
}

/// Fully connected layers with an activation between them. The last layer returns raw logits.
pub struct Mlp {
    pub sizes: Vec<usize>,
    pub activation: Activation,
    pub layers: Vec<Linear>,
}

impl Mlp {
    /// `sizes` lists the width of every layer, for example `[2, 8, 2]` is 2 inputs, 8 hidden units, 2 outputs.
    pub fn new(sizes: &[usize], activation: Activation, seed: u64) -> MlResult<Mlp> {
        if sizes.len() < 2 {
            return Err(MlError::Invalid("a network needs at least an input and an output size".to_string()));
        }
        if sizes.len() > 64 {
            return Err(MlError::Invalid("a network can have at most 64 layers".to_string()));
        }
        let mut total: usize = 0;
        for pair in sizes.windows(2) {
            let weights = crate::tensor_ops::element_count(&[pair[0], pair[1]])?;
            total = total.saturating_add(weights).saturating_add(pair[1]);
        }
        if total > crate::tensor_ops::MAX_TENSOR_ELEMENTS {
            return Err(MlError::Invalid(format!("a network with {} parameters is too large (the limit is {})", total, crate::tensor_ops::MAX_TENSOR_ELEMENTS)));
        }
        let mut rng = Rng::new(seed);
        let mut layers = Vec::new();
        for pair in sizes.windows(2) {
            layers.push(Linear::new(pair[0], pair[1], &mut rng)?);
        }
        Ok(Mlp { sizes: sizes.to_vec(), activation, layers })
    }

    pub fn forward(&self, x: &Var) -> MlResult<Var> {
        let mut hidden = x.clone();
        for (index, layer) in self.layers.iter().enumerate() {
            hidden = layer.forward(&hidden)?;
            if index + 1 < self.layers.len() {
                hidden = self.activation.apply(&hidden);
            }
        }
        Ok(hidden)
    }

    pub fn parameters(&self) -> Vec<Var> {
        let mut params = Vec::new();
        for layer in &self.layers {
            params.push(layer.weight.clone());
            params.push(layer.bias.clone());
        }
        params
    }

    pub fn parameter_count(&self) -> usize {
        self.parameters().iter().map(|p| p.value().data.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_is_deterministic_and_in_range() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        let mut r = Rng::new(1);
        for _ in 0..1000 {
            let f = r.next_f64();
            assert!((0.0..1.0).contains(&f));
            assert!(r.below(7) < 7);
        }
        assert_eq!(r.below(0), 0);
    }

    #[test]
    fn shuffle_is_a_permutation_and_reproducible() {
        let mut items: Vec<usize> = (0..50).collect();
        Rng::new(9).shuffle(&mut items);
        let mut again: Vec<usize> = (0..50).collect();
        Rng::new(9).shuffle(&mut again);
        assert_eq!(items, again);
        assert_ne!(items, (0..50).collect::<Vec<usize>>());
        items.sort();
        assert_eq!(items, (0..50).collect::<Vec<usize>>());
    }

    #[test]
    fn layers_have_the_right_shapes_and_initial_range() {
        let mlp = Mlp::new(&[2, 8, 2], Activation::Relu, 7).unwrap();
        assert_eq!(mlp.parameter_count(), 2 * 8 + 8 + 8 * 2 + 2);
        assert_eq!(Mlp::new(&[3, 5, 4, 2], Activation::Tanh, 1).unwrap().parameter_count(), 54);
        let limit = (6.0f64 / 10.0).sqrt();
        assert!(mlp.layers[0].weight.value().data.iter().all(|w| w.abs() <= limit));
        assert!(mlp.layers[0].bias.value().data.iter().all(|b| *b == 0.0));
        let x = Var::constant(Tensor::try_new(vec![0.5, -0.5], vec![1, 2]).unwrap());
        assert_eq!(mlp.forward(&x).unwrap().shape(), vec![1, 2]);
    }

    #[test]
    fn the_same_seed_gives_the_same_network() {
        let a = Mlp::new(&[2, 4, 2], Activation::Relu, 5).unwrap();
        let b = Mlp::new(&[2, 4, 2], Activation::Relu, 5).unwrap();
        let c = Mlp::new(&[2, 4, 2], Activation::Relu, 6).unwrap();
        assert_eq!(a.layers[0].weight.value(), b.layers[0].weight.value());
        assert_ne!(a.layers[0].weight.value(), c.layers[0].weight.value());
    }

    #[test]
    fn invalid_architectures_are_rejected() {
        assert!(Mlp::new(&[3], Activation::Relu, 1).is_err());
        assert!(Mlp::new(&[3, 0, 2], Activation::Relu, 1).is_err());
        assert!(Mlp::new(&[100_000, 100_000], Activation::Relu, 1).is_err()); // far too large
        assert!(Mlp::new(&vec![2; 100], Activation::Relu, 1).is_err()); // too many layers
        assert!(Mlp::new(&[usize::MAX, usize::MAX], Activation::Relu, 1).is_err()); // overflow
    }
}
