//! Optimizers. Call `loss.backward()`, then `step()`.
use crate::{MlError, MlResult, Tensor, Var};

fn check_learning_rate(learning_rate: f64) -> MlResult<()> {
    if learning_rate.is_finite() && learning_rate > 0.0 {
        Ok(())
    } else {
        Err(MlError::Invalid("the learning rate must be a positive number".to_string()))
    }
}

fn zeros_for(params: &[Var]) -> Vec<Tensor> {
    params
        .iter()
        .map(|p| {
            let shape = p.shape();
            let count: usize = shape.iter().product();
            Tensor { data: vec![0.0; count], shape }
        })
        .collect()
}

/// Stochastic gradient descent with optional momentum.
pub struct Sgd {
    params: Vec<Var>,
    learning_rate: f64,
    momentum: f64,
    velocity: Vec<Tensor>,
}

impl Sgd {
    pub fn new(params: Vec<Var>, learning_rate: f64, momentum: f64) -> MlResult<Sgd> {
        check_learning_rate(learning_rate)?;
        if !(0.0..1.0).contains(&momentum) {
            return Err(MlError::Invalid("momentum must be in [0, 1)".to_string()));
        }
        let velocity = zeros_for(&params);
        Ok(Sgd { params, learning_rate, momentum, velocity })
    }

    pub fn step(&mut self) -> MlResult<()> {
        for (param, velocity) in self.params.iter().zip(self.velocity.iter_mut()) {
            let grad = param.grad();
            let mut value = param.value();
            for i in 0..value.data.len() {
                velocity.data[i] = self.momentum * velocity.data[i] + grad.data[i];
                value.data[i] -= self.learning_rate * velocity.data[i];
            }
            param.set_value(value)?;
        }
        Ok(())
    }

    pub fn zero_grad(&self) {
        for param in &self.params {
            param.zero_grad();
        }
    }
}

/// Adam (Kingma and Ba, 2015) with the usual defaults.
pub struct Adam {
    params: Vec<Var>,
    learning_rate: f64,
    beta1: f64,
    beta2: f64,
    epsilon: f64,
    steps: i32,
    first_moment: Vec<Tensor>,
    second_moment: Vec<Tensor>,
}

impl Adam {
    pub fn new(params: Vec<Var>, learning_rate: f64) -> MlResult<Adam> {
        check_learning_rate(learning_rate)?;
        let first_moment = zeros_for(&params);
        let second_moment = zeros_for(&params);
        Ok(Adam { params, learning_rate, beta1: 0.9, beta2: 0.999, epsilon: 1e-8, steps: 0, first_moment, second_moment })
    }

    pub fn step(&mut self) -> MlResult<()> {
        self.steps = self.steps.saturating_add(1);
        let correction1 = 1.0 - self.beta1.powi(self.steps);
        let correction2 = 1.0 - self.beta2.powi(self.steps);
        for (index, param) in self.params.iter().enumerate() {
            let grad = param.grad();
            let mut value = param.value();
            let m = &mut self.first_moment[index];
            let v = &mut self.second_moment[index];
            for i in 0..value.data.len() {
                m.data[i] = self.beta1 * m.data[i] + (1.0 - self.beta1) * grad.data[i];
                v.data[i] = self.beta2 * v.data[i] + (1.0 - self.beta2) * grad.data[i] * grad.data[i];
                let m_hat = m.data[i] / correction1;
                let v_hat = v.data[i] / correction2;
                value.data[i] -= self.learning_rate * m_hat / (v_hat.sqrt() + self.epsilon);
            }
            param.set_value(value)?;
        }
        Ok(())
    }

    pub fn zero_grad(&self) {
        for param in &self.params {
            param.zero_grad();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimise(mut step: impl FnMut(&Var, &Var) -> MlResult<()>, iterations: usize) -> f64 {
        let w = Var::parameter(Tensor::try_new(vec![0.0], vec![1]).unwrap());
        let target = Var::constant(Tensor::try_new(vec![3.0], vec![1]).unwrap());
        for _ in 0..iterations {
            step(&w, &target).unwrap();
        }
        w.value().data[0]
    }

    #[test]
    fn sgd_minimises_a_quadratic() {
        let w = Var::parameter(Tensor::try_new(vec![0.0], vec![1]).unwrap());
        let target = Var::constant(Tensor::try_new(vec![3.0], vec![1]).unwrap());
        let mut sgd = Sgd::new(vec![w.clone()], 0.1, 0.0).unwrap();
        for _ in 0..100 {
            let loss = w.mse(&target).unwrap();
            loss.backward().unwrap();
            sgd.step().unwrap();
        }
        assert!((w.value().data[0] - 3.0).abs() < 1e-6);
    }

    #[test]
    fn adam_minimises_a_quadratic() {
        let mut adam = None;
        let result = minimise(
            |w, target| {
                if adam.is_none() {
                    adam = Some(Adam::new(vec![w.clone()], 0.1)?);
                }
                let loss = w.mse(target)?;
                loss.backward()?;
                adam.as_mut().unwrap().step()
            },
            300,
        );
        assert!((result - 3.0).abs() < 0.05, "ended at {}", result);
    }

    #[test]
    fn invalid_settings_are_rejected() {
        let p = Var::parameter(Tensor::try_new(vec![0.0], vec![1]).unwrap());
        assert!(Sgd::new(vec![p.clone()], 0.0, 0.0).is_err());
        assert!(Sgd::new(vec![p.clone()], -1.0, 0.0).is_err());
        assert!(Sgd::new(vec![p.clone()], f64::NAN, 0.0).is_err());
        assert!(Sgd::new(vec![p.clone()], 0.1, 1.0).is_err());
        assert!(Sgd::new(vec![p.clone()], 0.1, 0.9).is_ok());
        assert!(Adam::new(vec![p], 0.0).is_err());
    }

    #[test]
    fn zero_grad_clears_gradients() {
        let w = Var::parameter(Tensor::try_new(vec![1.0], vec![1]).unwrap());
        w.mul(&w).unwrap().sum().backward().unwrap();
        assert_eq!(w.grad().data, vec![2.0]);
        Sgd::new(vec![w.clone()], 0.1, 0.0).unwrap().zero_grad();
        assert_eq!(w.grad().data, vec![0.0]);
    }
}
