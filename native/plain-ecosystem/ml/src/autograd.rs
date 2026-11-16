//! Reverse-mode automatic differentiation over tensors.
//!
//! Build a graph with the methods on [`Var`], call [`Var::backward`] on a scalar result, then read
//! gradients with [`Var::grad`]. `backward` first resets the gradients of every node in its graph, so
//! calling it twice never double-counts. Use [`Var::zero_grad`] to clear a single node.
use crate::{MlError, MlResult, Tensor};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

#[derive(Clone, Debug)]
enum Op {
    Leaf,
    Add,
    Sub,
    Mul,
    Matmul,
    AddBias,
    Relu,
    Tanh,
    Sigmoid,
    Sum,
    Mean,
    Scale(f64),
    SoftmaxCrossEntropy(Vec<usize>),
}

struct Node {
    value: Tensor,
    grad: Tensor,
    op: Op,
    parents: Vec<Var>,
}

/// A tensor in a computation graph. Cloning a `Var` shares the same node.
#[derive(Clone)]
pub struct Var(Rc<RefCell<Node>>);

fn zeros_like(shape: &[usize]) -> Tensor {
    let count: usize = shape.iter().product();
    Tensor { data: vec![0.0; count], shape: shape.to_vec() }
}

fn id_of(v: &Var) -> usize {
    Rc::as_ptr(&v.0) as usize
}

impl Var {
    fn new(value: Tensor, op: Op, parents: Vec<Var>) -> Var {
        let grad = zeros_like(&value.shape);
        Var(Rc::new(RefCell::new(Node { value, grad, op, parents })))
    }

    /// A trainable leaf.
    pub fn parameter(value: Tensor) -> Var {
        Var::new(value, Op::Leaf, Vec::new())
    }

    /// An input that is not trained (data, labels).
    pub fn constant(value: Tensor) -> Var {
        Var::new(value, Op::Leaf, Vec::new())
    }

    pub fn value(&self) -> Tensor {
        self.0.borrow().value.clone()
    }

    pub fn grad(&self) -> Tensor {
        self.0.borrow().grad.clone()
    }

    pub fn shape(&self) -> Vec<usize> {
        self.0.borrow().value.shape.clone()
    }

    /// Replaces the value (used by optimizers and when loading checkpoints). The shape must not change.
    pub fn set_value(&self, value: Tensor) -> MlResult<()> {
        let mut node = self.0.borrow_mut();
        if node.value.shape != value.shape {
            return Err(MlError::Shape(format!(
                "cannot replace a tensor of shape {:?} with one of shape {:?}",
                node.value.shape, value.shape
            )));
        }
        node.value = value;
        Ok(())
    }

    /// The single value of a one-element tensor.
    pub fn scalar(&self) -> MlResult<f64> {
        let node = self.0.borrow();
        if node.value.data.len() == 1 {
            Ok(node.value.data[0])
        } else {
            Err(MlError::Shape(format!("expected a single value, got shape {:?}", node.value.shape)))
        }
    }

    /// Clears this node's gradient.
    pub fn zero_grad(&self) {
        let mut node = self.0.borrow_mut();
        let shape = node.value.shape.clone();
        node.grad = zeros_like(&shape);
    }

    pub fn add(&self, other: &Var) -> MlResult<Var> {
        let value = self.0.borrow().value.zip_with(&other.0.borrow().value, |a, b| a + b)?;
        Ok(Var::new(value, Op::Add, vec![self.clone(), other.clone()]))
    }

    pub fn sub(&self, other: &Var) -> MlResult<Var> {
        let value = self.0.borrow().value.zip_with(&other.0.borrow().value, |a, b| a - b)?;
        Ok(Var::new(value, Op::Sub, vec![self.clone(), other.clone()]))
    }

    /// Element-wise product of two tensors of the same shape.
    pub fn mul(&self, other: &Var) -> MlResult<Var> {
        let value = self.0.borrow().value.zip_with(&other.0.borrow().value, |a, b| a * b)?;
        Ok(Var::new(value, Op::Mul, vec![self.clone(), other.clone()]))
    }

    pub fn matmul(&self, other: &Var) -> MlResult<Var> {
        let value = self.0.borrow().value.try_matmul(&other.0.borrow().value)?;
        Ok(Var::new(value, Op::Matmul, vec![self.clone(), other.clone()]))
    }

    /// Adds a bias vector (one value per column) to every row of a matrix.
    pub fn add_bias(&self, bias: &Var) -> MlResult<Var> {
        let x = self.0.borrow().value.clone();
        let b = bias.0.borrow().value.clone();
        let (rows, cols) = x.rows_cols()?;
        if b.data.len() != cols {
            return Err(MlError::Shape(format!("bias has {} values but the matrix has {} columns", b.data.len(), cols)));
        }
        let mut out = x.data.clone();
        for r in 0..rows {
            for c in 0..cols {
                out[r * cols + c] += b.data[c];
            }
        }
        Ok(Var::new(Tensor { data: out, shape: x.shape.clone() }, Op::AddBias, vec![self.clone(), bias.clone()]))
    }

    pub fn relu(&self) -> Var {
        let value = self.0.borrow().value.map(|x| x.max(0.0));
        Var::new(value, Op::Relu, vec![self.clone()])
    }

    pub fn tanh(&self) -> Var {
        let value = self.0.borrow().value.map(|x| x.tanh());
        Var::new(value, Op::Tanh, vec![self.clone()])
    }

    pub fn sigmoid(&self) -> Var {
        let value = self.0.borrow().value.map(|x| 1.0 / (1.0 + (-x).exp()));
        Var::new(value, Op::Sigmoid, vec![self.clone()])
    }

    pub fn scale(&self, factor: f64) -> Var {
        let value = self.0.borrow().value.map(|x| x * factor);
        Var::new(value, Op::Scale(factor), vec![self.clone()])
    }

    /// Sum of all elements, as a one-element tensor.
    pub fn sum(&self) -> Var {
        let total = self.0.borrow().value.sum();
        Var::new(Tensor { data: vec![total], shape: vec![1] }, Op::Sum, vec![self.clone()])
    }

    /// Mean of all elements, as a one-element tensor.
    pub fn mean(&self) -> MlResult<Var> {
        let count = self.0.borrow().value.data.len();
        if count == 0 {
            return Err(MlError::Invalid("cannot take the mean of an empty tensor".to_string()));
        }
        let mean = self.0.borrow().value.sum() / count as f64;
        Ok(Var::new(Tensor { data: vec![mean], shape: vec![1] }, Op::Mean, vec![self.clone()]))
    }

    /// Mean squared difference to `target` (same shape), as a one-element tensor.
    pub fn mse(&self, target: &Var) -> MlResult<Var> {
        let difference = self.sub(target)?;
        difference.mul(&difference)?.mean()
    }

    /// Mean cross-entropy between softmax(self) and integer class labels, computed in a numerically
    /// stable way. `self` holds one row of logits per sample.
    pub fn softmax_cross_entropy(&self, labels: &[usize]) -> MlResult<Var> {
        let logits = self.0.borrow().value.clone();
        let (rows, cols) = logits.rows_cols()?;
        if rows == 0 {
            return Err(MlError::Invalid("no samples".to_string()));
        }
        if labels.len() != rows {
            return Err(MlError::Shape(format!("{} labels for {} rows of logits", labels.len(), rows)));
        }
        let mut total = 0.0;
        for r in 0..rows {
            let label = labels[r];
            if label >= cols {
                return Err(MlError::Invalid(format!("label {} is outside 0..{}", label, cols)));
            }
            let row = &logits.data[r * cols..(r + 1) * cols];
            let max = row.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let log_sum = max + row.iter().map(|x| (x - max).exp()).sum::<f64>().ln();
            total += log_sum - row[label];
        }
        let value = Tensor { data: vec![total / rows as f64], shape: vec![1] };
        Ok(Var::new(value, Op::SoftmaxCrossEntropy(labels.to_vec()), vec![self.clone()]))
    }

    /// Computes the gradient of this one-element tensor with respect to every node in its graph.
    pub fn backward(&self) -> MlResult<()> {
        if self.0.borrow().value.data.len() != 1 {
            return Err(MlError::Shape(format!(
                "backward needs a single value, got shape {:?}",
                self.0.borrow().value.shape
            )));
        }
        let order = topological_order(self);
        for node in &order {
            node.zero_grad();
        }
        self.0.borrow_mut().grad.data[0] = 1.0;
        for node in order.iter().rev() {
            let (grad, value, op, parents) = {
                let n = node.0.borrow();
                (n.grad.clone(), n.value.clone(), n.op.clone(), n.parents.clone())
            };
            propagate(&op, &grad, &value, &parents)?;
        }
        Ok(())
    }
}

/// Parents before children. Iterative, so `backward` itself never recurses on deep graphs.
fn topological_order(root: &Var) -> Vec<Var> {
    let mut order = Vec::new();
    let mut seen: HashSet<usize> = HashSet::new();
    let mut stack: Vec<(Var, bool)> = vec![(root.clone(), false)];
    while let Some((node, expanded)) = stack.pop() {
        if expanded {
            order.push(node);
            continue;
        }
        if !seen.insert(id_of(&node)) {
            continue;
        }
        stack.push((node.clone(), true));
        let parents = node.0.borrow().parents.clone();
        for parent in parents {
            if !seen.contains(&id_of(&parent)) {
                stack.push((parent, false));
            }
        }
    }
    order
}

fn accumulate(target: &Var, delta: &Tensor) {
    let mut node = target.0.borrow_mut();
    for (g, d) in node.grad.data.iter_mut().zip(&delta.data) {
        *g += *d;
    }
}

fn propagate(op: &Op, grad: &Tensor, value: &Tensor, parents: &[Var]) -> MlResult<()> {
    match op {
        Op::Leaf => {}
        Op::Add => {
            accumulate(&parents[0], grad);
            accumulate(&parents[1], grad);
        }
        Op::Sub => {
            accumulate(&parents[0], grad);
            accumulate(&parents[1], &grad.map(|g| -g));
        }
        Op::Mul => {
            let a = parents[0].value();
            let b = parents[1].value();
            accumulate(&parents[0], &grad.zip_with(&b, |g, y| g * y)?);
            accumulate(&parents[1], &grad.zip_with(&a, |g, x| g * x)?);
        }
        Op::Matmul => {
            let a = parents[0].value();
            let b = parents[1].value();
            accumulate(&parents[0], &grad.try_matmul(&b.try_transpose()?)?);
            accumulate(&parents[1], &a.try_transpose()?.try_matmul(grad)?);
        }
        Op::AddBias => {
            accumulate(&parents[0], grad);
            let (rows, cols) = grad.rows_cols()?;
            let mut sums = vec![0.0; cols];
            for r in 0..rows {
                for c in 0..cols {
                    sums[c] += grad.data[r * cols + c];
                }
            }
            accumulate(&parents[1], &Tensor { data: sums, shape: parents[1].shape() });
        }
        Op::Relu => {
            let x = parents[0].value();
            accumulate(&parents[0], &grad.zip_with(&x, |g, x| if x > 0.0 { g } else { 0.0 })?);
        }
        Op::Tanh => {
            accumulate(&parents[0], &grad.zip_with(value, |g, y| g * (1.0 - y * y))?);
        }
        Op::Sigmoid => {
            accumulate(&parents[0], &grad.zip_with(value, |g, y| g * y * (1.0 - y))?);
        }
        Op::Sum => {
            let shape = parents[0].shape();
            let count: usize = shape.iter().product();
            accumulate(&parents[0], &Tensor { data: vec![grad.data[0]; count], shape });
        }
        Op::Mean => {
            let shape = parents[0].shape();
            let count: usize = shape.iter().product();
            accumulate(&parents[0], &Tensor { data: vec![grad.data[0] / count as f64; count], shape });
        }
        Op::Scale(factor) => {
            accumulate(&parents[0], &grad.map(|g| g * *factor));
        }
        Op::SoftmaxCrossEntropy(labels) => {
            let logits = parents[0].value();
            let (rows, cols) = logits.rows_cols()?;
            let scale = grad.data[0] / rows as f64;
            let mut out = vec![0.0; rows * cols];
            for r in 0..rows {
                let row = &logits.data[r * cols..(r + 1) * cols];
                let max = row.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let denominator: f64 = row.iter().map(|x| (x - max).exp()).sum();
                for c in 0..cols {
                    let probability = (row[c] - max).exp() / denominator;
                    let target = if c == labels[r] { 1.0 } else { 0.0 };
                    out[r * cols + c] = (probability - target) * scale;
                }
            }
            accumulate(&parents[0], &Tensor { data: out, shape: logits.shape.clone() });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tensor(data: &[f64], shape: &[usize]) -> Tensor {
        Tensor::try_new(data.to_vec(), shape.to_vec()).unwrap()
    }

    fn numeric_gradient(build: &dyn Fn(&[Var]) -> MlResult<Var>, params: &[Var], which: usize) -> Vec<f64> {
        let eps = 1e-6;
        let base = params[which].value();
        let mut grads = Vec::new();
        for i in 0..base.data.len() {
            let mut plus = base.clone();
            plus.data[i] += eps;
            params[which].set_value(plus).unwrap();
            let up = build(params).unwrap().scalar().unwrap();
            let mut minus = base.clone();
            minus.data[i] -= eps;
            params[which].set_value(minus).unwrap();
            let down = build(params).unwrap().scalar().unwrap();
            grads.push((up - down) / (2.0 * eps));
        }
        params[which].set_value(base).unwrap();
        grads
    }

    /// Compares backpropagation with central finite differences for every parameter element.
    fn check_gradients(build: impl Fn(&[Var]) -> MlResult<Var>, params: Vec<Var>) {
        let loss = build(&params).unwrap();
        loss.backward().unwrap();
        for which in 0..params.len() {
            let analytic = params[which].grad().data;
            let numeric = numeric_gradient(&build, &params, which);
            assert_eq!(analytic.len(), numeric.len());
            for (a, n) in analytic.iter().zip(&numeric) {
                assert!((a - n).abs() < 1e-5 * (1.0 + n.abs()), "parameter {}: analytic {} vs numeric {}", which, a, n);
            }
        }
    }

    fn layer_inputs() -> Vec<Var> {
        vec![
            Var::parameter(tensor(&[0.5, -1.2, 2.0, 1.5, 0.3, -0.7], &[2, 3])),
            Var::parameter(tensor(&[0.4, -0.6, 0.9, 0.2, -1.1, 0.8], &[3, 2])),
            Var::parameter(tensor(&[0.1, -0.3], &[1, 2])),
        ]
    }

    #[test]
    fn relu_layer_gradients_match_numeric() {
        check_gradients(|p: &[Var]| -> MlResult<Var> { Ok(p[0].matmul(&p[1])?.add_bias(&p[2])?.relu().sum()) }, layer_inputs());
    }

    #[test]
    fn tanh_sigmoid_scale_mean_gradients_match_numeric() {
        check_gradients(
            |p: &[Var]| -> MlResult<Var> { p[0].matmul(&p[1])?.add_bias(&p[2])?.tanh().scale(2.0).sigmoid().mean() },
            layer_inputs(),
        );
    }

    #[test]
    fn softmax_cross_entropy_gradients_match_numeric() {
        let logits = Var::parameter(tensor(&[0.2, -1.0, 0.7, 1.5, -0.3, 0.9, 0.1, 0.0, 2.0, -0.5, 0.4, 1.1], &[3, 4]));
        check_gradients(|p: &[Var]| -> MlResult<Var> { p[0].softmax_cross_entropy(&[1, 0, 3]) }, vec![logits]);
    }

    #[test]
    fn softmax_cross_entropy_has_the_right_value() {
        let logits = Var::constant(tensor(&[0.0, 0.0], &[1, 2]));
        let loss = logits.softmax_cross_entropy(&[0]).unwrap().scalar().unwrap();
        assert!((loss - std::f64::consts::LN_2).abs() < 1e-12);
        let huge = Var::constant(tensor(&[1000.0, 0.0], &[1, 2])); // would overflow a naive softmax
        assert!(huge.softmax_cross_entropy(&[0]).unwrap().scalar().unwrap().abs() < 1e-9);
    }

    #[test]
    fn mse_sub_and_shared_nodes_have_correct_gradients() {
        let x = Var::parameter(tensor(&[1.0, -2.0, 0.5], &[3]));
        check_gradients(|p: &[Var]| -> MlResult<Var> { p[0].mse(&Var::constant(tensor(&[0.0, 1.0, 2.0], &[3]))) }, vec![x.clone()]);
        check_gradients(|p: &[Var]| -> MlResult<Var> { p[0].mul(&p[0])?.sum().sub(&p[0].sum()) }, vec![x]);
    }

    #[test]
    fn a_node_used_twice_accumulates_both_contributions() {
        let x = Var::parameter(tensor(&[3.0], &[1]));
        let y = x.add(&x).unwrap().sum();
        y.backward().unwrap();
        assert_eq!(x.grad().data, vec![2.0]);
    }

    #[test]
    fn backward_does_not_double_count_and_zero_grad_clears() {
        let x = Var::parameter(tensor(&[1.0, 2.0], &[2]));
        let loss = x.mul(&x).unwrap().sum();
        loss.backward().unwrap();
        loss.backward().unwrap();
        assert_eq!(x.grad().data, vec![2.0, 4.0]); // d(x*x)/dx = 2x, not 4x
        x.zero_grad();
        assert_eq!(x.grad().data, vec![0.0, 0.0]);
    }

    #[test]
    fn mistakes_are_errors_not_panics() {
        let a = Var::constant(tensor(&[1.0, 2.0], &[2]));
        let b = Var::constant(tensor(&[1.0, 2.0, 3.0], &[3]));
        assert!(a.add(&b).is_err());
        assert!(a.matmul(&b).is_err());
        assert!(a.backward().is_err()); // two values: not a scalar
        assert!(a.set_value(tensor(&[1.0], &[1])).is_err());
        assert!(a.scalar().is_err());
        let logits = Var::constant(tensor(&[0.0, 1.0], &[1, 2]));
        assert!(logits.softmax_cross_entropy(&[2]).is_err()); // label out of range
        assert!(logits.softmax_cross_entropy(&[0, 1]).is_err()); // wrong number of labels
        assert!(a.add_bias(&b).is_err()); // not a matrix
    }
}
