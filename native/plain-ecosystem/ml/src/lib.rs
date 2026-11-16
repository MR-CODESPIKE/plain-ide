use plain_runtime::{plain_bool, plain_error, plain_float, plain_handle, plain_handle_clear, plain_handle_ptr, plain_int, plain_string, PlainValue};
use std::cell::RefCell;
use std::ffi::CString;
use std::os::raw::{c_char, c_void};
use std::rc::Rc;

pub mod autograd;
pub mod checkpoint;
pub mod classifier;
pub mod classifier_ffi;
pub mod error;
pub mod nn;
pub mod optim;
pub mod tensor_ops;

pub use autograd::Var;
pub use checkpoint::{load_params, save_params};
pub use classifier::{accuracy, Classifier, TrainConfig, MAX_EPOCHS};
pub use error::{MlError, MlResult};
pub use nn::{Activation, Linear, Mlp, Rng};
pub use optim::{Adam, Sgd};
pub use tensor_ops::{element_count, MAX_TENSOR_ELEMENTS};

#[derive(Clone, Debug, PartialEq)]
pub struct Tensor { pub data: Vec<f64>, pub shape: Vec<usize> }
impl Tensor {
    pub fn new(data: Vec<f64>, shape: Vec<usize>) -> Self {
        assert_eq!(data.len(), shape.iter().product());
        Self { data, shape }
    }
    pub fn zeros(shape: Vec<usize>) -> Self { Self::new(vec![0.0; shape.iter().product()], shape) }
    pub fn ones(shape: Vec<usize>) -> Self { Self::new(vec![1.0; shape.iter().product()], shape) }
    pub fn add(&self, other: &Self) -> Self {
        assert_eq!(self.shape, other.shape);
        Self::new(self.data.iter().zip(&other.data).map(|(a,b)| a+b).collect(), self.shape.clone())
    }
    pub fn mul(&self, other: &Self) -> Self {
        assert_eq!(self.shape, other.shape);
        Self::new(self.data.iter().zip(&other.data).map(|(a,b)| a*b).collect(), self.shape.clone())
    }
    pub fn transpose(&self) -> Self {
        assert_eq!(self.shape.len(), 2);
        let (rows, cols) = (self.shape[0], self.shape[1]);
        let mut out = vec![0.0; self.data.len()];
        for r in 0..rows { for c in 0..cols { out[c*rows+r] = self.data[r*cols+c]; } }
        Self::new(out, vec![cols, rows])
    }
    pub fn matmul(&self, other: &Self) -> Self {
        assert_eq!(self.shape.len(), 2); assert_eq!(other.shape.len(), 2);
        let (m, k, n) = (self.shape[0], self.shape[1], other.shape[1]);
        assert_eq!(k, other.shape[0]);
        let mut out = vec![0.0; m*n];
        for i in 0..m { for j in 0..n { for q in 0..k { out[i*n+j] += self.data[i*k+q] * other.data[q*n+j]; } } }
        Self::new(out, vec![m, n])
    }
}

#[derive(Clone)]
struct Scalar(Rc<RefCell<ScalarNode>>);
struct ScalarNode { data: f64, grad: f64, op: ScalarOp, parents: Vec<Scalar> }
#[derive(Clone, Copy)] enum ScalarOp { Leaf, Add, Mul, Relu }
fn scalar(data: f64) -> Scalar { Scalar(Rc::new(RefCell::new(ScalarNode { data, grad: 0.0, op: ScalarOp::Leaf, parents: vec![] }))) }
fn scalar_op(a: &Scalar, b: Option<&Scalar>, op: ScalarOp, data: f64) -> Scalar {
    let mut parents = vec![a.clone()]; if let Some(x) = b { parents.push(x.clone()); }
    Scalar(Rc::new(RefCell::new(ScalarNode { data, grad: 0.0, op, parents })))
}
fn sadd(a:&Scalar,b:&Scalar)->Scalar { scalar_op(a,Some(b),ScalarOp::Add,a.0.borrow().data+b.0.borrow().data) }
fn smul(a:&Scalar,b:&Scalar)->Scalar { scalar_op(a,Some(b),ScalarOp::Mul,a.0.borrow().data*b.0.borrow().data) }
fn ssub(a:&Scalar,b:&Scalar)->Scalar { sadd(a,&smul(&scalar(-1.0),b)) }
fn srelu(a:&Scalar)->Scalar { scalar_op(a,None,ScalarOp::Relu,a.0.borrow().data.max(0.0)) }
fn topo(x:&Scalar, seen:&mut std::collections::HashSet<usize>, out:&mut Vec<Scalar>) {
    let id=Rc::as_ptr(&x.0) as usize; if !seen.insert(id){return}
    for p in &x.0.borrow().parents { topo(p,seen,out); } out.push(x.clone());
}
fn backward(root:&Scalar) {
    let mut order=Vec::new(); topo(root,&mut std::collections::HashSet::new(),&mut order); root.0.borrow_mut().grad=1.0;
    for node in order.into_iter().rev() {
        let (g,op,parents)={let n=node.0.borrow();(n.grad,n.op,n.parents.clone())};
        match op {
            ScalarOp::Add => { parents[0].0.borrow_mut().grad += g; parents[1].0.borrow_mut().grad += g; }
            ScalarOp::Mul => { let a=parents[0].0.borrow().data; let b=parents[1].0.borrow().data; parents[0].0.borrow_mut().grad += g*b; parents[1].0.borrow_mut().grad += g*a; }
            ScalarOp::Relu => { if parents[0].0.borrow().data > 0.0 { parents[0].0.borrow_mut().grad += g; } }
            ScalarOp::Leaf => {}
        }
    }
}

#[derive(Clone)]
struct TensorNode(Rc<RefCell<TensorNodeInner>>);
struct TensorNodeInner { value: Tensor, grad: Tensor, op: TensorOp, parents: Vec<TensorNode> }
#[derive(Clone, Copy)] enum TensorOp { Leaf, Add, Mul, Matmul, Relu }
fn tleaf(value: Tensor) -> TensorNode { let grad=Tensor::zeros(value.shape.clone()); TensorNode(Rc::new(RefCell::new(TensorNodeInner{value,grad,op:TensorOp::Leaf,parents:vec![]}))) }
fn tbin(a:&TensorNode,b:&TensorNode,op:TensorOp,value:Tensor)->TensorNode { let grad=Tensor::zeros(value.shape.clone()); TensorNode(Rc::new(RefCell::new(TensorNodeInner{value,grad,op,parents:vec![a.clone(),b.clone()]}))) }
fn tadd(a:&TensorNode,b:&TensorNode)->TensorNode { tbin(a,b,TensorOp::Add,a.0.borrow().value.add(&b.0.borrow().value)) }
fn tmul(a:&TensorNode,b:&TensorNode)->TensorNode { tbin(a,b,TensorOp::Mul,a.0.borrow().value.mul(&b.0.borrow().value)) }
fn tmatmul(a:&TensorNode,b:&TensorNode)->TensorNode { tbin(a,b,TensorOp::Matmul,a.0.borrow().value.matmul(&b.0.borrow().value)) }
fn trelu(a:&TensorNode)->TensorNode { let v=a.0.borrow().value.clone();let value=Tensor::new(v.data.iter().map(|x|x.max(0.0)).collect(),v.shape);let grad=Tensor::zeros(value.shape.clone());TensorNode(Rc::new(RefCell::new(TensorNodeInner{value,grad,op:TensorOp::Relu,parents:vec![a.clone()]}))) }
fn tensor_topo(x:&TensorNode,seen:&mut std::collections::HashSet<usize>,out:&mut Vec<TensorNode>){let id=Rc::as_ptr(&x.0) as usize;if !seen.insert(id){return}for p in &x.0.borrow().parents{tensor_topo(p,seen,out)}out.push(x.clone())}
fn add_grad(dst:&mut Tensor, src:&Tensor){for (a,b) in dst.data.iter_mut().zip(&src.data){*a+=b;}}
fn tensor_backward(root:&TensorNode){let mut order=Vec::new();tensor_topo(root,&mut std::collections::HashSet::new(),&mut order);let root_shape=root.0.borrow().value.shape.clone();root.0.borrow_mut().grad=Tensor::ones(root_shape);for node in order.into_iter().rev(){let (g,op,parents)={let n=node.0.borrow();(n.grad.clone(),n.op,n.parents.clone())};match op{TensorOp::Add=>{add_grad(&mut parents[0].0.borrow_mut().grad,&g);add_grad(&mut parents[1].0.borrow_mut().grad,&g)},TensorOp::Mul=>{let a=parents[0].0.borrow().value.clone();let b=parents[1].0.borrow().value.clone();add_grad(&mut parents[0].0.borrow_mut().grad,&g.mul(&b));add_grad(&mut parents[1].0.borrow_mut().grad,&g.mul(&a))},TensorOp::Matmul=>{let a=parents[0].0.borrow().value.clone();let b=parents[1].0.borrow().value.clone();let bt=b.transpose();let at=a.transpose();add_grad(&mut parents[0].0.borrow_mut().grad,&g.matmul(&bt));add_grad(&mut parents[1].0.borrow_mut().grad,&at.matmul(&g))},TensorOp::Relu=>{let a=parents[0].0.borrow().value.clone();let mask=Tensor::new(a.data.iter().map(|x|if *x>0.0{1.0}else{0.0}).collect(),a.shape);add_grad(&mut parents[0].0.borrow_mut().grad,&g.mul(&mask))},TensorOp::Leaf=>{}}}}

pub struct Agent { weights: [f64; 12], last_loss: f64 }
fn predict_raw(w:&[f64;12],x:f64,y:f64)->f64 { let h0=(w[0]*x+w[1]*y+w[2]).max(0.0);let h1=(w[3]*x+w[4]*y+w[5]).max(0.0);let h2=(w[6]*x+w[7]*y+w[8]).max(0.0);w[9]*h0+w[10]*h1+w[11]*h2 }
fn train_agent()->Agent { let data=[(0.0,0.0,0.0),(0.0,1.0,1.0),(1.0,0.0,1.0),(1.0,1.0,0.0)];let mut w=[0.15,-0.20,0.05,0.30,0.10,-0.10,-0.25,0.20,0.10,0.20,0.20,-0.20];let mut last=0.0;for epoch in 0..4000{let mut loss_sum=0.0;for &(x,y,target) in &data{let p:Vec<Scalar>=w.iter().map(|v|scalar(*v)).collect();let sx=scalar(x);let sy=scalar(y);let h0=srelu(&sadd(&sadd(&smul(&p[0],&sx),&smul(&p[1],&sy)),&p[2]));let h1=srelu(&sadd(&sadd(&smul(&p[3],&sx),&smul(&p[4],&sy)),&p[5]));let h2=srelu(&sadd(&sadd(&smul(&p[6],&sx),&smul(&p[7],&sy)),&p[8]));let out=sadd(&sadd(&smul(&p[9],&h0),&smul(&p[10],&h1)),&smul(&p[11],&h2));let err=ssub(&out,&scalar(target));let loss=smul(&err,&err);loss_sum+=loss.0.borrow().data;backward(&loss);for(i,v)in p.iter().enumerate(){w[i]-=0.03*v.0.borrow().grad;}}last=loss_sum/4.0;if epoch%1000==0&&last<0.01{break}}Agent{weights:w,last_loss:last}}

fn c_string(text:&str)->*mut PlainValue { let c=CString::new(text).unwrap_or_default(); plain_string(c.as_ptr()) }
/// Returns a real Plain error value (not an ordinary string), so TRY/CATCH sees it.
fn ffi_error(message:&str)->*mut PlainValue { let c=CString::new(message).unwrap_or_default(); plain_error(c.as_ptr()) }
const TAG_AGENT: i64 = 0x504C_4147;
const TAG_BIG_MODEL: i64 = 0x504C_424D;
/// Upper bound for `plain_big_model_train` so a bad argument cannot hang the program.
const MAX_BIG_MODEL_STEPS: i64 = 1_000_000;
#[no_mangle] pub extern "C" fn plain_ffi_add_numbers(a:*mut PlainValue,b:*mut PlainValue)->*mut PlainValue { if a.is_null()||b.is_null(){return ffi_error("ffi error: null argument")};unsafe{if (*a).kind!=1||(*b).kind!=1{return ffi_error("ffi error: expected int,int")};plain_int((*a).i.wrapping_add((*b).i))} }
#[no_mangle] pub extern "C" fn plain_ml_train()->*mut PlainValue { plain_handle(Box::into_raw(Box::new(train_agent())) as *mut c_void,TAG_AGENT) }
#[no_mangle] pub extern "C" fn plain_ml_last_loss(agent:*mut PlainValue)->*mut PlainValue { let p=plain_handle_ptr(agent,TAG_AGENT) as *const Agent; if p.is_null(){return ffi_error("ffi error: invalid or released agent")};unsafe{plain_float((*p).last_loss)} }
#[no_mangle] pub extern "C" fn plain_ml_classify(agent:*mut PlainValue,x:*mut PlainValue,y:*mut PlainValue)->*mut PlainValue { let p=plain_handle_ptr(agent,TAG_AGENT) as *const Agent; if p.is_null()||x.is_null()||y.is_null(){return ffi_error("ffi error: invalid agent or inputs")};unsafe{if (*x).kind!=1||(*y).kind!=1{return ffi_error("ffi error: classifier expects int,int")};let r=predict_raw(&(*p).weights,(*x).i as f64,(*y).i as f64);c_string(if r>=0.5{"CLASS_A"}else{"CLASS_B"})} }
/// Releases an agent. The handle is invalidated, so freeing twice is reported instead of crashing.
#[no_mangle] pub extern "C" fn plain_ml_free(agent:*mut PlainValue)->*mut PlainValue { let p=plain_handle_clear(agent,TAG_AGENT) as *mut Agent; if p.is_null(){return ffi_error("ffi error: agent already released or invalid")};unsafe{drop(Box::from_raw(p))};plain_bool(1) }

#[cfg(test)]
mod tests { use super::*;
 #[test] fn tensor_ops_and_transpose(){let a=Tensor::new(vec![1.0,2.0,3.0,4.0],vec![2,2]);let b=Tensor::new(vec![5.0,6.0,7.0,8.0],vec![2,2]);assert_eq!(a.add(&b).data,vec![6.0,8.0,10.0,12.0]);assert_eq!(a.mul(&b).data,vec![5.0,12.0,21.0,32.0]);assert_eq!(a.matmul(&b).data,vec![19.0,22.0,43.0,50.0]);assert_eq!(a.transpose().data,vec![1.0,3.0,2.0,4.0]);}
 #[test] fn reverse_mode_gradient_matches_manual(){let x=scalar(2.0);let y=scalar(3.0);let z=sadd(&smul(&x,&y),&x);backward(&z);assert!((x.0.borrow().grad-4.0).abs()<1e-9);assert!((y.0.borrow().grad-2.0).abs()<1e-9);}
 #[test] fn tensor_matmul_gradient_matches_numeric(){let a=tleaf(Tensor::new(vec![1.2,2.0],vec![1,2]));let b=tleaf(Tensor::new(vec![3.0,4.0,5.0,6.0],vec![2,2]));let y=tmatmul(&a,&b);let loss=tadd(&y,&y);tensor_backward(&loss);let eps=1e-6;let base=a.0.borrow().value.data[0];let mut ap=a.0.borrow().value.clone();ap.data[0]+=eps;let plus=ap.matmul(&b.0.borrow().value).data.iter().sum::<f64>()*2.0;ap.data[0]-=2.0*eps;let minus=ap.matmul(&b.0.borrow().value).data.iter().sum::<f64>()*2.0;let numeric=(plus-minus)/(2.0*eps);assert!((a.0.borrow().grad.data[0]-numeric).abs()<1e-5);assert_eq!(a.0.borrow().value.data[0],base);}
 #[test] fn big_model_reports_honest_metrics(){let model=BigModel::train_with(3_000,7);assert_eq!(model.parameter_count(),BIG_PARAM_COUNT);assert!(model.loss.is_finite());assert!(model.trained_tokens>0&&model.trained_tokens<=BIG_TRAIN_TOKENS);assert!(model.coverage()<0.25,"only part of the vocabulary is ever trained");assert!(model.heldout_accuracy()<0.5,"tokens that were never trained cannot be learned, but held-out accuracy was {}",model.heldout_accuracy());}
 #[test] fn big_model_training_is_reproducible_and_checkpoints_round_trip(){let a=BigModel::train_with(500,3);let b=BigModel::train_with(500,3);assert_eq!(a.loss,b.loss);let restored=BigModel::from_bytes(&a.to_bytes()).unwrap();for token in [0usize,1,2,3,10,4095]{assert_eq!(a.predict(token),restored.predict(token));}assert_eq!(restored.steps,500);assert!(BigModel::from_bytes(&a.to_bytes()[..100]).is_err());}
 #[test] fn big_model_parameter_budget_is_exact(){assert_eq!(BIG_PARAM_COUNT,10_000_000);assert_eq!(BIG_HEAD_BIAS+BIG_CLASSES,9_999_876);assert_eq!(BIG_PARAM_COUNT-(BIG_HEAD_BIAS+BIG_CLASSES),124);}
 #[test] fn xor_training_reduces_loss_and_classifies(){let a=train_agent();assert!(a.last_loss<0.08,"loss was {}",a.last_loss);assert!(predict_raw(&a.weights,0.0,0.0)<0.5);assert!(predict_raw(&a.weights,0.0,1.0)>=0.5);assert!(predict_raw(&a.weights,1.0,0.0)>=0.5);assert!(predict_raw(&a.weights,1.0,1.0)<0.5);}
}

const BIG_PARAM_COUNT: usize = 10_000_000;
const BIG_VOCAB: usize = 19_527;
const BIG_DIM: usize = 512;
const BIG_CLASSES: usize = 4;
const BIG_HEAD_START: usize = BIG_VOCAB * BIG_DIM;
const BIG_HEAD_BIAS: usize = BIG_HEAD_START + BIG_DIM * BIG_CLASSES;
/// Tokens 0..4096 are used for training and evaluation; the rest of the vocabulary is never touched.
const BIG_TRAIN_TOKENS: usize = 4096;

/// A deliberately compact, sparse-training 10M-parameter model. The trainable core is
/// a 19,527 x 512 token embedding plus a 512 x 4 classifier head; the final 124
/// reserved parameters keep the model capacity exactly 10,000,000 parameters.
pub struct BigModel { params: Vec<f32>, steps: u64, loss: f64, accuracy: f64, heldout_accuracy: f64, trained_tokens: usize }
impl BigModel {
    pub fn parameter_count(&self) -> usize { self.params.len() }
    fn embedding_offset(token: usize) -> usize { (token % BIG_VOCAB) * BIG_DIM }
    fn logits(&self, token: usize) -> [f32; BIG_CLASSES] { let e=Self::embedding_offset(token);let mut out=[0.0;BIG_CLASSES];for c in 0..BIG_CLASSES{let h=BIG_HEAD_START+c*BIG_DIM;let mut s=self.params[BIG_HEAD_BIAS+c];for d in 0..BIG_DIM{s+=self.params[e+d]*self.params[h+d]}out[c]=s}out }
    fn train_step(&mut self, token: usize, label: usize, lr: f32) -> f64 { let e=Self::embedding_offset(token);let mut logits=self.logits(token);let max=logits.iter().copied().fold(f32::NEG_INFINITY,f32::max);let mut exps=[0.0;BIG_CLASSES];let mut sum=0.0;for c in 0..BIG_CLASSES{exps[c]=(logits[c]-max).exp();sum+=exps[c]}let mut err=[0.0;BIG_CLASSES];for c in 0..BIG_CLASSES{let p=exps[c]/sum;err[c]=p-if c==label{1.0}else{0.0}}let loss=-(exps[label]/sum).max(1e-12).ln() as f64;let mut grad_e=[0.0f32;BIG_DIM];for c in 0..BIG_CLASSES{let h=BIG_HEAD_START+c*BIG_DIM;for d in 0..BIG_DIM{grad_e[d]+=err[c]*self.params[h+d];self.params[h+d]-=lr*err[c]*self.params[e+d]}self.params[BIG_HEAD_BIAS+c]-=lr*err[c]}for d in 0..BIG_DIM{self.params[e+d]-=lr*grad_e[d]}loss }
    /// Tokens with `token % 7 == 6` are never trained on, so `heldout_accuracy` shows whether the model generalises.
    fn is_held_out(token: usize) -> bool { token % 7 == 6 }
    pub fn train(steps: usize) -> Self { Self::train_with(steps, 0xC0FFEE) }
    /// Trains on the synthetic task "label = token % 4". A given seed always gives the same model.
    pub fn train_with(steps: usize, seed: u64) -> Self {
        let mut params = vec![0.0f32; BIG_PARAM_COUNT];
        let mut state = seed | 1; // xorshift needs a non-zero state
        for x in &mut params[..BIG_HEAD_BIAS + BIG_CLASSES] {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            *x = ((state as i64 % 10_000) as f32) / 500_000.0;
        }
        let mut model = Self { params, steps: 0, loss: 0.0, accuracy: 0.0, heldout_accuracy: 0.0, trained_tokens: 0 };
        let mut seen = vec![false; BIG_TRAIN_TOKENS];
        let mut loss_sum = 0.0;
        for step in 0..steps {
            let mut token = (step * 37 + 11) % BIG_TRAIN_TOKENS;
            if Self::is_held_out(token) { token = (token + 1) % BIG_TRAIN_TOKENS; }
            seen[token] = true;
            loss_sum += model.train_step(token, token % BIG_CLASSES, 0.08);
            model.steps += 1;
        }
        model.trained_tokens = seen.iter().filter(|s| **s).count();
        model.loss = if steps == 0 { 0.0 } else { loss_sum / steps as f64 };
        if steps > 0 {
            model.accuracy = model.accuracy_over(|t| !Self::is_held_out(t));
            model.heldout_accuracy = model.accuracy_over(Self::is_held_out);
        }
        model
    }
    fn accuracy_over(&self, include: impl Fn(usize) -> bool) -> f64 {
        let (mut correct, mut total) = (0usize, 0usize);
        for token in 0..BIG_TRAIN_TOKENS {
            if include(token) {
                total += 1;
                if self.predict(token) == token % BIG_CLASSES { correct += 1; }
            }
        }
        if total == 0 { 0.0 } else { correct as f64 / total as f64 }
    }
    /// Accuracy on tokens the model was trained on.
    pub fn train_accuracy(&self) -> f64 { self.accuracy }
    /// Accuracy on tokens that were never trained on (about 25 % is chance level for this task).
    pub fn heldout_accuracy(&self) -> f64 { self.heldout_accuracy }
    /// Share of the 19,527-token vocabulary that received at least one training step.
    pub fn coverage(&self) -> f64 { self.trained_tokens as f64 / BIG_VOCAB as f64 }
    const CHECKPOINT_HEADER: usize = 56;
    /// All parameters plus the training statistics (about 40 MB).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::CHECKPOINT_HEADER + self.params.len() * 4);
        out.extend_from_slice(b"PLBM");
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&self.steps.to_le_bytes());
        out.extend_from_slice(&self.loss.to_bits().to_le_bytes());
        out.extend_from_slice(&self.accuracy.to_bits().to_le_bytes());
        out.extend_from_slice(&self.heldout_accuracy.to_bits().to_le_bytes());
        out.extend_from_slice(&(self.trained_tokens as u64).to_le_bytes());
        out.extend_from_slice(&(self.params.len() as u64).to_le_bytes());
        for p in &self.params { out.extend_from_slice(&p.to_bits().to_le_bytes()); }
        out
    }
    pub fn from_bytes(bytes: &[u8]) -> MlResult<BigModel> {
        if bytes.len() != Self::CHECKPOINT_HEADER + BIG_PARAM_COUNT * 4 || &bytes[0..4] != &b"PLBM"[..] {
            return Err(MlError::Format("this is not a Plain big-model checkpoint".to_string()));
        }
        let word = |start: usize| -> u64 { let mut a = [0u8; 8]; a.copy_from_slice(&bytes[start..start + 8]); u64::from_le_bytes(a) };
        let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        if version != 1 { return Err(MlError::Format(format!("unsupported checkpoint version {}", version))); }
        if word(48) as usize != BIG_PARAM_COUNT { return Err(MlError::Format("the parameter count does not match".to_string())); }
        let params: Vec<f32> = bytes[Self::CHECKPOINT_HEADER..].chunks_exact(4).map(|c| f32::from_bits(u32::from_le_bytes([c[0], c[1], c[2], c[3]]))).collect();
        Ok(BigModel { params, steps: word(8), loss: f64::from_bits(word(16)), accuracy: f64::from_bits(word(24)), heldout_accuracy: f64::from_bits(word(32)), trained_tokens: word(40) as usize })
    }
    pub fn predict(&self, token: usize)->usize{let l=self.logits(token);l.iter().enumerate().max_by(|a,b|a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal)).map(|x|x.0).unwrap_or(0)}
}
#[no_mangle] pub extern "C" fn plain_big_model_train(steps:*mut PlainValue)->*mut PlainValue{let n:i64=if steps.is_null(){20_000}else{unsafe{if (*steps).kind==1{(*steps).i.max(0)}else{20_000}}};if n>MAX_BIG_MODEL_STEPS{return ffi_error("ffi error: steps exceeds the supported limit of 1,000,000")};plain_handle(Box::into_raw(Box::new(BigModel::train(n as usize))) as *mut c_void,TAG_BIG_MODEL)}
fn big_model<'a>(v:*mut PlainValue)->Option<&'a BigModel>{let p=plain_handle_ptr(v,TAG_BIG_MODEL) as *const BigModel;if p.is_null(){None}else{Some(unsafe{&*p})}}
#[no_mangle] pub extern "C" fn plain_big_model_parameter_count(model:*mut PlainValue)->*mut PlainValue{match big_model(model){Some(m)=>plain_int(m.parameter_count() as i64),None=>ffi_error("ffi error: invalid or released model")}}
#[no_mangle] pub extern "C" fn plain_big_model_loss(model:*mut PlainValue)->*mut PlainValue{match big_model(model){Some(m)=>plain_float(m.loss),None=>ffi_error("ffi error: invalid or released model")}}
#[no_mangle] pub extern "C" fn plain_big_model_accuracy(model:*mut PlainValue)->*mut PlainValue{match big_model(model){Some(m)=>plain_float(m.accuracy),None=>ffi_error("ffi error: invalid or released model")}}
#[no_mangle] pub extern "C" fn plain_big_model_predict(model:*mut PlainValue,token:*mut PlainValue)->*mut PlainValue{match big_model(model){Some(m)=>{if token.is_null()||unsafe{(*token).kind}!=1{return ffi_error("ffi error: token must be an int")};plain_int(m.predict(unsafe{(*token).i}.max(0) as usize) as i64)},None=>ffi_error("ffi error: invalid or released model")}}
#[no_mangle] pub extern "C" fn plain_big_model_train_seeded(steps:*mut PlainValue,seed:*mut PlainValue)->*mut PlainValue{let n:i64=if steps.is_null(){20_000}else{unsafe{if (*steps).kind==1{(*steps).i.max(0)}else{20_000}}};if n>MAX_BIG_MODEL_STEPS{return ffi_error("ffi error: steps exceeds the supported limit of 1,000,000")};let s:u64=if seed.is_null(){0xC0FFEE}else{unsafe{if (*seed).kind==1{(*seed).i as u64}else{0xC0FFEE}}};plain_handle(Box::into_raw(Box::new(BigModel::train_with(n as usize,s))) as *mut c_void,TAG_BIG_MODEL)}
#[no_mangle] pub extern "C" fn plain_big_model_heldout_accuracy(model:*mut PlainValue)->*mut PlainValue{match big_model(model){Some(m)=>plain_float(m.heldout_accuracy()),None=>ffi_error("ffi error: invalid or released model")}}
#[no_mangle] pub extern "C" fn plain_big_model_coverage(model:*mut PlainValue)->*mut PlainValue{match big_model(model){Some(m)=>plain_float(m.coverage()),None=>ffi_error("ffi error: invalid or released model")}}
#[no_mangle] pub extern "C" fn plain_big_model_save(model:*mut PlainValue,path:*mut PlainValue)->*mut PlainValue{let m=match big_model(model){Some(m)=>m,None=>return ffi_error("ffi error: invalid or released model")};let path=match plain_runtime::value_text(path){Some(p)=>p,None=>return ffi_error("ffi error: the path must be a string")};match std::fs::write(&path,m.to_bytes()){Ok(())=>plain_bool(1),Err(e)=>ffi_error(&format!("ffi error: {}: {}",path,e))}}
#[no_mangle] pub extern "C" fn plain_big_model_load(path:*mut PlainValue)->*mut PlainValue{let path=match plain_runtime::value_text(path){Some(p)=>p,None=>return ffi_error("ffi error: the path must be a string")};let bytes=match std::fs::read(&path){Ok(b)=>b,Err(e)=>return ffi_error(&format!("ffi error: {}: {}",path,e))};match BigModel::from_bytes(&bytes){Ok(m)=>plain_handle(Box::into_raw(Box::new(m)) as *mut c_void,TAG_BIG_MODEL),Err(e)=>ffi_error(&format!("ffi error: {}",e))}}
#[no_mangle] pub extern "C" fn plain_big_model_free(model:*mut PlainValue)->*mut PlainValue{let p=plain_handle_clear(model,TAG_BIG_MODEL) as *mut BigModel;if p.is_null(){return ffi_error("ffi error: model already released or invalid")};unsafe{drop(Box::from_raw(p))};plain_bool(1)}
