//! A ready-to-use neural-network classifier: mini-batch training with Adam and softmax cross-entropy.
use crate::{Activation, Adam, MlError, MlResult, Mlp, Rng, Tensor, Var};

/// Most epochs one `train` call will run.
pub const MAX_EPOCHS: usize = 100_000;
/// Caps total sample passes, even when both the data set and epoch count are individually valid.
const MAX_TRAINING_SAMPLE_EPOCHS: usize = 100_000_000;

#[derive(Clone, Debug)]
pub struct TrainConfig {
    pub epochs: usize,
    pub learning_rate: f64,
    pub batch_size: usize,
    /// Controls weight initialisation order and batch shuffling, so a run is reproducible.
    pub seed: u64,
}

impl Default for TrainConfig {
    fn default() -> TrainConfig {
        TrainConfig { epochs: 100, learning_rate: 0.01, batch_size: 32, seed: 1 }
    }
}

pub struct Classifier {
    pub mlp: Mlp,
}

/// Fraction of predictions that equal the labels.
pub fn accuracy(predictions: &[usize], labels: &[usize]) -> MlResult<f64> {
    if predictions.len() != labels.len() {
        return Err(MlError::Shape(format!("{} predictions for {} labels", predictions.len(), labels.len())));
    }
    if labels.is_empty() {
        return Err(MlError::Invalid("no samples".to_string()));
    }
    let correct = predictions.iter().zip(labels).filter(|(p, l)| p == l).count();
    Ok(correct as f64 / labels.len() as f64)
}

impl Classifier {
    /// `sizes` is the width of every layer, for example `[2, 8, 2]`. Hidden layers use ReLU.
    pub fn new(sizes: &[usize], seed: u64) -> MlResult<Classifier> {
        Ok(Classifier { mlp: Mlp::new(sizes, Activation::Relu, seed)? })
    }

    pub fn input_size(&self) -> usize {
        self.mlp.sizes[0]
    }

    pub fn classes(&self) -> usize {
        self.mlp.sizes[self.mlp.sizes.len() - 1]
    }

    fn check_samples(&self, inputs: &[Vec<f64>], labels: &[usize]) -> MlResult<()> {
        if inputs.is_empty() {
            return Err(MlError::Invalid("there are no samples".to_string()));
        }
        if inputs.len() != labels.len() {
            return Err(MlError::Shape(format!("{} samples but {} labels", inputs.len(), labels.len())));
        }
        let width = self.input_size();
        for (index, row) in inputs.iter().enumerate() {
            if row.len() != width {
                return Err(MlError::Shape(format!("sample {} has {} features but the network expects {}", index, row.len(), width)));
            }
            if row.iter().any(|x| !x.is_finite()) {
                return Err(MlError::Invalid(format!("sample {} contains a value that is not a finite number", index)));
            }
        }
        let classes = self.classes();
        if let Some(bad) = labels.iter().find(|l| **l >= classes) {
            return Err(MlError::Invalid(format!("label {} is outside 0..{}", bad, classes)));
        }
        Ok(())
    }

    /// Trains on the samples and returns the mean loss of the last epoch.
    pub fn train(&self, inputs: &[Vec<f64>], labels: &[usize], config: &TrainConfig) -> MlResult<f64> {
        self.check_samples(inputs, labels)?;
        if config.epochs == 0 || config.epochs > MAX_EPOCHS {
            return Err(MlError::Invalid(format!("epochs must be between 1 and {}", MAX_EPOCHS)));
        }
        if config.batch_size == 0 {
            return Err(MlError::Invalid("batch_size must be at least 1".to_string()));
        }
        let work = inputs.len().checked_mul(config.epochs)
            .ok_or_else(|| MlError::Invalid("training workload is too large".to_string()))?;
        if work > MAX_TRAINING_SAMPLE_EPOCHS {
            return Err(MlError::Invalid(format!("training workload exceeds the {} sample-epoch limit", MAX_TRAINING_SAMPLE_EPOCHS)));
        }
        let width = self.input_size();
        let mut optimizer = Adam::new(self.mlp.parameters(), config.learning_rate)?;
        let mut rng = Rng::new(config.seed);
        let mut order: Vec<usize> = (0..inputs.len()).collect();
        let mut last_epoch_loss = 0.0;
        for _ in 0..config.epochs {
            rng.shuffle(&mut order);
            let mut epoch_loss = 0.0;
            for chunk in order.chunks(config.batch_size) {
                let mut data = Vec::with_capacity(chunk.len() * width);
                let mut batch_labels = Vec::with_capacity(chunk.len());
                for &index in chunk {
                    data.extend_from_slice(&inputs[index]);
                    batch_labels.push(labels[index]);
                }
                let batch = Var::constant(Tensor::try_new(data, vec![chunk.len(), width])?);
                let logits = self.mlp.forward(&batch)?;
                let loss = logits.softmax_cross_entropy(&batch_labels)?;
                loss.backward()?;
                optimizer.step()?;
                epoch_loss += loss.scalar()? * chunk.len() as f64;
            }
            last_epoch_loss = epoch_loss / inputs.len() as f64;
        }
        Ok(last_epoch_loss)
    }

    /// Class probabilities for one sample.
    pub fn probabilities(&self, input: &[f64]) -> MlResult<Vec<f64>> {
        if input.len() != self.input_size() {
            return Err(MlError::Shape(format!("the input has {} features but the network expects {}", input.len(), self.input_size())));
        }
        if input.iter().any(|x| !x.is_finite()) {
            return Err(MlError::Invalid("the input contains a value that is not a finite number".to_string()));
        }
        let batch = Var::constant(Tensor::try_new(input.to_vec(), vec![1, input.len()])?);
        let logits = self.mlp.forward(&batch)?.value().data;
        let max = logits.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let exps: Vec<f64> = logits.iter().map(|v| (v - max).exp()).collect();
        let total: f64 = exps.iter().sum();
        Ok(exps.iter().map(|e| e / total).collect())
    }

    /// The most likely class for one sample.
    pub fn predict(&self, input: &[f64]) -> MlResult<usize> {
        let probabilities = self.probabilities(input)?;
        let mut best = 0;
        for (index, p) in probabilities.iter().enumerate() {
            if *p > probabilities[best] {
                best = index;
            }
        }
        Ok(best)
    }

    pub fn accuracy(&self, inputs: &[Vec<f64>], labels: &[usize]) -> MlResult<f64> {
        self.check_samples(inputs, labels)?;
        let mut predictions = Vec::with_capacity(inputs.len());
        for row in inputs {
            predictions.push(self.predict(row)?);
        }
        accuracy(&predictions, labels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xor() -> (Vec<Vec<f64>>, Vec<usize>) {
        (vec![vec![0.0, 0.0], vec![0.0, 1.0], vec![1.0, 0.0], vec![1.0, 1.0]], vec![0, 1, 1, 0])
    }

    fn blobs() -> (Vec<Vec<f64>>, Vec<usize>) {
        let mut rng = Rng::new(5);
        let (mut inputs, mut labels) = (Vec::new(), Vec::new());
        for (class, (cx, cy)) in [(0.0, 0.0), (4.0, 0.0), (0.0, 4.0)].iter().enumerate() {
            for _ in 0..30 {
                inputs.push(vec![cx + rng.uniform(-0.5, 0.5), cy + rng.uniform(-0.5, 0.5)]);
                labels.push(class);
            }
        }
        (inputs, labels)
    }

    #[test]
    fn learns_xor() {
        let (inputs, labels) = xor();
        let net = Classifier::new(&[2, 8, 2], 7).unwrap();
        let config = TrainConfig { epochs: 300, learning_rate: 0.05, batch_size: 4, seed: 1 };
        let loss = net.train(&inputs, &labels, &config).unwrap();
        assert!(loss < 0.05, "loss was {}", loss);
        assert_eq!(net.accuracy(&inputs, &labels).unwrap(), 1.0);
        assert_eq!(net.predict(&[1.0, 0.0]).unwrap(), 1);
        assert_eq!(net.predict(&[1.0, 1.0]).unwrap(), 0);
    }

    #[test]
    fn learns_three_classes_with_mini_batches() {
        let (inputs, labels) = blobs();
        let net = Classifier::new(&[2, 8, 3], 3).unwrap();
        let config = TrainConfig { epochs: 100, learning_rate: 0.05, batch_size: 16, seed: 2 };
        net.train(&inputs, &labels, &config).unwrap();
        assert!(net.accuracy(&inputs, &labels).unwrap() >= 0.98);
        let probabilities = net.probabilities(&[4.0, 0.0]).unwrap();
        assert_eq!(probabilities.len(), 3);
        assert!((probabilities.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        assert_eq!(net.predict(&[4.0, 0.0]).unwrap(), 1);
    }

    #[test]
    fn training_is_reproducible() {
        let (inputs, labels) = xor();
        let config = TrainConfig { epochs: 50, learning_rate: 0.05, batch_size: 2, seed: 4 };
        let a = Classifier::new(&[2, 6, 2], 9).unwrap();
        let b = Classifier::new(&[2, 6, 2], 9).unwrap();
        let loss_a = a.train(&inputs, &labels, &config).unwrap();
        let loss_b = b.train(&inputs, &labels, &config).unwrap();
        assert_eq!(loss_a, loss_b);
        assert_eq!(a.probabilities(&[0.3, 0.9]).unwrap(), b.probabilities(&[0.3, 0.9]).unwrap());
    }

    #[test]
    fn bad_input_is_an_error() {
        let (inputs, labels) = xor();
        let net = Classifier::new(&[2, 4, 2], 1).unwrap();
        let config = TrainConfig::default();
        assert!(net.train(&[], &[], &config).is_err());
        assert!(net.train(&inputs, &labels[..3], &config).is_err());
        assert!(net.train(&[vec![1.0], vec![2.0]], &[0, 1], &config).is_err()); // wrong number of features
        assert!(net.train(&inputs, &[0, 1, 2, 0], &config).is_err()); // label 2 with only 2 classes
        assert!(net.train(&[vec![f64::NAN, 0.0]], &[0], &config).is_err());
        assert!(net.train(&inputs, &labels, &TrainConfig { epochs: 0, ..TrainConfig::default() }).is_err());
        assert!(net.train(&inputs, &labels, &TrainConfig { batch_size: 0, ..TrainConfig::default() }).is_err());
        assert!(net.train(&inputs, &labels, &TrainConfig { learning_rate: -1.0, ..TrainConfig::default() }).is_err());
        assert!(net.predict(&[1.0]).is_err());
        assert!(net.predict(&[f64::INFINITY, 0.0]).is_err());
        assert!(accuracy(&[0, 1], &[0]).is_err());
        assert!(accuracy(&[], &[]).is_err());
        assert_eq!(accuracy(&[0, 1, 1, 0], &[0, 1, 0, 0]).unwrap(), 0.75);
    }
}
