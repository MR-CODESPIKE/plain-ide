# Plain ML library (`plain_ml`)

A small, dependency-free machine-learning toolkit written in Rust, usable from Rust and from Plain.
It is meant for learning, demos and small models. It is not a replacement for PyTorch or JAX.

## What it contains

| Module | What it does |
|---|---|
| `tensor_ops` | Checked tensor maths (`try_new`, `try_matmul`, `try_transpose`, ...). Shape mistakes are `MlError`s, never panics. Tensors are limited to 10 million values (80 MB per `f64` tensor). |
| `autograd` | Reverse-mode automatic differentiation: `Var` with `add`, `sub`, `mul`, `matmul`, `add_bias`, `relu`, `tanh`, `sigmoid`, `scale`, `sum`, `mean`, `mse`, `softmax_cross_entropy`, `backward`, `zero_grad`. `backward` resets the gradients of its graph first, so calling it twice never double-counts. Gradients are verified against finite differences in the tests. |
| `nn` | `Linear` (Xavier init), `Mlp` (ReLU/tanh/sigmoid hidden layers), a deterministic `Rng` (splitmix64) with shuffling. |
| `optim` | `Sgd` (with momentum) and `Adam`. |
| `classifier` | `Classifier`: mini-batch training with Adam and softmax cross-entropy, `predict`, `probabilities`, `accuracy`. A seed makes a run reproducible. |
| `checkpoint` | Versioned binary save/load for parameters and whole networks. Loading is all-or-nothing and validates every field. |
| `classifier_ffi` | The functions below, for Plain programs. |
| (root) | The original demos: a tiny XOR agent (`plain_ml_*`) and a 10-million-parameter model (`plain_big_model_*`). |

## Using a network from Plain

```
EXTERN "plain_ml" FUNCTION plain_nn_create(layers AS any, seed AS int) GIVES any
EXTERN "plain_ml" FUNCTION plain_nn_train(net AS any, inputs AS any, labels AS any, options AS any) GIVES any
EXTERN "plain_ml" FUNCTION plain_nn_predict(net AS any, input AS any) GIVES any

LET net = plain_nn_create([2, 8, 2], 7)          # 2 inputs, 8 hidden units, 2 classes
LET loss = plain_nn_train(net, [[0, 0], [0, 1], [1, 0], [1, 1]], [0, 1, 1, 0],
                          {epochs: 300, learning_rate: 0.05, batch_size: 4, seed: 1})
SAY plain_nn_predict(net, [1, 0])
```

| Function | Returns |
|---|---|
| `plain_nn_create(layers, seed)` | a network handle. `layers` is a list of widths (at least two). |
| `plain_nn_train(net, inputs, labels, options)` | mean loss of the last epoch. `options` (all optional): `epochs` (default 100), `learning_rate` (0.01), `batch_size` (32), `seed` (1). `labels` are integers from 0 to classes-1. |
| `plain_nn_predict(net, input)` | the most likely class (integer) |
| `plain_nn_probabilities(net, input)` | a list with one probability per class |
| `plain_nn_accuracy(net, inputs, labels)` | fraction classified correctly (float) |
| `plain_nn_parameter_count(net)` | number of weights and biases |
| `plain_nn_save(net, path)` / `plain_nn_load(path)` | write / read the architecture and weights |
| `plain_nn_free(net)` | releases the network. A freed or invalid handle is an error value, never a crash. |

Every function reports problems (wrong sizes, labels out of range, non-numeric input, bad paths) as error values that `TRY`/`CATCH` and `IS_ERROR` can handle. See `examples/nn_classifier.plain`.

## The 10-million-parameter demo, honestly

`plain_big_model_*` trains a model with exactly 10,000,000 parameters (a 19,527 x 512 embedding table plus a classification head) on a **synthetic task: the label of a token is `token % 4`**. That makes it a demo of the training, FFI and checkpoint plumbing, not a language model. Measured behaviour (see `ml/src/lib.rs` tests and `plain_big_model_heldout_accuracy`):

- It only ever sees tokens below 4,096, so at most about 21% of the vocabulary is trained (`plain_big_model_coverage`).
- Tokens with `token % 7 == 6` are held out and never trained. After 20,000 steps the accuracy is about 100% on trained tokens and about 25% (chance level) on held-out tokens: the model memorises tokens, it does not generalise.
- `plain_big_model_train_seeded(steps, seed)` makes a run reproducible; `plain_big_model_save` / `plain_big_model_load` write and read a checkpoint (about 40 MB).

## Limits and known gaps

- CPU only, `f64` tensors, no batching of the 10M model, no convolution, pooling, dropout, attention or learning-rate schedules.
- Training on large data sets is single-threaded and not tuned for speed.
- Tensors above 10 million values, networks above that many parameters, more than 64 layers, more than 100,000 epochs, or more than 100 million sample-epochs per training call are rejected. Classifier input matrices are also capped at 10 million total features.
- There is no language-model training pipeline for the Plain dataset yet.
