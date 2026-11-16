use std::collections::{HashMap, HashSet};
use crate::{ast::*, source::{CompileError, Span}};

/// Where a statement appears; decides whether BREAK / CONTINUE / GIVE BACK / FAIL are legal.
#[derive(Clone, Copy)]
struct Ctx { in_loop: bool, in_fn: bool, in_try: bool }

pub fn check(program: &Program) -> Result<(), CompileError> {
    let mut names = HashSet::new();
    for item in &program.items {
        match item {
            Item::Function(f) => {
                if f.name == "main" { return Err(CompileError::new(f.span, "`main` is reserved for the program entry point; choose another function name")); }
                if !names.insert(f.name.clone()) { return Err(CompileError::new(f.span, format!("duplicate function `{}`", f.name))); }
                check_params(&f.params, f.span)?;
            }
            Item::Class(c) => { if !names.insert(c.name.clone()) { return Err(CompileError::new(c.span, format!("duplicate class `{}`", c.name))); } }
            Item::Extern(x) => {
                if !names.insert(x.name.clone()) { return Err(CompileError::new(x.span, format!("duplicate external function `{}`", x.name))); }
                check_params(&x.params, x.span)?;
            }
            _ => {}
        }
    }
    let top_ctx = Ctx { in_loop: false, in_fn: false, in_try: false };
    let fn_ctx = Ctx { in_loop: false, in_fn: true, in_try: false };
    let mut top = HashMap::new();
    for item in &program.items {
        match item {
            Item::Stmt(s) => check_block(std::slice::from_ref(s), &mut top, top_ctx)?,
            Item::Function(f) => {
                let mut vars = f.params.iter().cloned().collect::<HashMap<_, _>>();
                check_block(&f.body, &mut vars, fn_ctx)?;
            }
            Item::Class(c) => {
                if let Some(parent) = &c.parent {
                    if !program.items.iter().any(|i| matches!(i, Item::Class(k) if &k.name == parent)) {
                        return Err(CompileError::new(c.span, format!("class `{}` extends unknown class `{}`", c.name, parent)));
                    }
                }
                let mut method_names = HashSet::new();
                for method in &c.methods {
                    if !method_names.insert(method.name.as_str()) { return Err(CompileError::new(method.span, format!("duplicate method `{}` in class `{}`", method.name, c.name))); }
                    check_params(&method.params, method.span)?;
                }
                let mut field_names = HashSet::new();
                for (field, _) in &c.fields {
                    if !field_names.insert(field.as_str()) { return Err(CompileError::new(c.span, format!("duplicate field `{}` in class `{}`", field, c.name))); }
                }
                for (index, default) in c.defaults.iter().enumerate() {
                    if let Some(expr) = default {
                        let (field, declared) = &c.fields[index];
                        if !is_constant(expr) { return Err(CompileError::new(expr.span(), format!("the default value of field `{}` must be a constant expression", field))); }
                        let inferred = infer(expr, &HashMap::new());
                        if *declared != Type::Any && inferred != Type::Any && *declared != inferred { return Err(CompileError::new(expr.span(), format!("the default value of field `{}` does not match its type", field))); }
                    }
                }
                let chain = class_chain(program, c);
                if chain.len() > program.items.len() {
                    return Err(CompileError::new(c.span, format!("class `{}` inherits from itself", c.name)));
                }
                for method in &c.methods {
                    let mut vars = method.params.iter().cloned().collect::<HashMap<_, _>>();
                    vars.insert("self".to_string(), Type::Dict);
                    // Fields of parent classes are visible too; the child's own fields win.
                    for class in chain.iter().rev() {
                        for (field, ty) in &class.fields { vars.insert(field.clone(), ty.clone()); }
                    }
                    check_block(&method.body, &mut vars, fn_ctx)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// The class followed by its ancestors (child first). Stops at an unknown parent.
/// A chain longer than the number of items means the inheritance is circular.
fn class_chain<'a>(program: &'a Program, class: &'a Class) -> Vec<&'a Class> {
    let mut chain = vec![class];
    let mut current = class;
    while let Some(parent) = &current.parent {
        if chain.len() > program.items.len() { break; }
        let next = program.items.iter().find_map(|item| match item {
            Item::Class(k) if &k.name == parent => Some(k),
            _ => None,
        });
        match next { Some(k) => { chain.push(k); current = k; } None => break }
    }
    chain
}

/// Field defaults are evaluated when an object is created, so they may not depend on variables.
fn is_constant(expr: &Expr) -> bool {
    match expr {
        Expr::Int(..) | Expr::Float(..) | Expr::Bool(..) | Expr::Str(..) => true,
        Expr::List(items, _) => items.iter().all(is_constant),
        Expr::Dict(pairs, _) => pairs.iter().all(|(k, v)| is_constant(k) && is_constant(v)),
        Expr::Unary { expr, .. } => is_constant(expr),
        Expr::Binary { left, right, .. } => is_constant(left) && is_constant(right),
        Expr::Ternary { yes, cond, no, .. } => is_constant(yes) && is_constant(cond) && is_constant(no),
        _ => false,
    }
}

fn check_params(params: &[(String, Type)], span: Span) -> Result<(), CompileError> {
    let mut seen = HashSet::new();
    for (name, _) in params {
        if !seen.insert(name.as_str()) { return Err(CompileError::new(span, format!("duplicate parameter `{}`", name))); }
    }
    Ok(())
}

fn check_block(body: &[Stmt], vars: &mut HashMap<String, Type>, ctx: Ctx) -> Result<(), CompileError> {
    for stmt in body {
        match stmt {
            Stmt::Let(names, values, span) => {
                if names.len() != values.len() { return Err(CompileError::new(*span, "LET names and values must have the same count")); }
                for ((name, explicit), value) in names.iter().zip(values) {
                    if vars.contains_key(name) { return Err(CompileError::new(*span, format!("LET cannot redeclare `{}`; use SET", name))); }
                    let inferred = infer(value, vars);
                    if let Some(expected) = explicit { if *expected != Type::Any && inferred != Type::Any && *expected != inferred { return Err(CompileError::new(*span, format!("type mismatch for `{}`", name))); } }
                    vars.insert(name.clone(), explicit.clone().unwrap_or(inferred));
                }
            }
            Stmt::Set(lhs, rhs, span) => {
                if lhs.len() != rhs.len() { return Err(CompileError::new(*span, "SET names and values must have the same count")); }
                for target in lhs { if let Expr::Var(name, _) = target { if !vars.contains_key(name) { return Err(CompileError::new(target.span(), format!("SET requires an existing variable `{}`", name))); } } }
            }
            Stmt::Ask { name, span, .. } => { if vars.contains_key(name) { return Err(CompileError::new(*span, format!("ASK cannot redeclare `{}`", name))); } vars.insert(name.clone(), Type::Any); }
            Stmt::If { branches, else_body, .. } => {
                for (_, branch) in branches { let mut inner = vars.clone(); check_block(branch, &mut inner, ctx)?; }
                let mut inner = vars.clone();
                check_block(else_body, &mut inner, ctx)?;
            }
            Stmt::Repeat(repeat, _) => {
                let mut inner = vars.clone();
                let body_ctx = Ctx { in_loop: true, ..ctx };
                match repeat {
                    Repeat::Counted { name, body, .. } => { inner.insert(name.clone(), Type::Int); check_block(body, &mut inner, body_ctx)?; }
                    // The element type of a list is not tracked, so the loop variable is `any`.
                    Repeat::Each { name, body, .. } => { inner.insert(name.clone(), Type::Any); check_block(body, &mut inner, body_ctx)?; }
                    Repeat::While { body, .. } => check_block(body, &mut inner, body_ctx)?,
                }
            }
            Stmt::Try { body, catch, catch_body, .. } => {
                // The TRY body and the CATCH body are separate scopes: variables made in the body are not visible in CATCH.
                let mut body_scope = vars.clone();
                check_block(body, &mut body_scope, Ctx { in_try: true, ..ctx })?;
                let mut catch_scope = vars.clone();
                catch_scope.insert(catch.clone(), Type::String);
                check_block(catch_body, &mut catch_scope, ctx)?;
            }
            Stmt::Say(e, _) | Stmt::Expr(e) => { let _ = infer(e, vars); }
            Stmt::Fail(e, span) => {
                if !(ctx.in_fn || ctx.in_try) { return Err(CompileError::new(*span, "FAIL can only be used inside a function or a TRY block")); }
                let _ = infer(e, vars);
            }
            Stmt::Return(value, span) => {
                if !ctx.in_fn { return Err(CompileError::new(*span, "GIVE BACK can only be used inside a function")); }
                if let Some(e) = value { let _ = infer(e, vars); }
            }
            Stmt::Break(span) | Stmt::Continue(span) => {
                if !ctx.in_loop { return Err(CompileError::new(*span, "BREAK and CONTINUE can only be used inside REPEAT")); }
            }
        }
    }
    Ok(())
}

fn infer(expr: &Expr, vars: &HashMap<String, Type>) -> Type { match expr { Expr::Int(..) => Type::Int, Expr::Float(..) => Type::Float, Expr::Bool(..) => Type::Bool, Expr::Str(..) => Type::String, Expr::List(items, _) => Type::List(Box::new(items.first().map(|x| infer(x, vars)).unwrap_or(Type::Any))), Expr::Dict(..) => Type::Dict, Expr::Var(name, _) => vars.get(name).cloned().unwrap_or(Type::Any), Expr::Ternary { yes, no, .. } => { let a = infer(yes, vars); let b = infer(no, vars); if a == b { a } else { Type::Any } }, _ => Type::Any } }
#[allow(dead_code)] fn _span(stmt: &Stmt) -> Span { match stmt { Stmt::Let(_,_,s)|Stmt::Set(_,_,s)|Stmt::Say(_,s)|Stmt::Break(s)|Stmt::Continue(s)|Stmt::Return(_,s)|Stmt::Fail(_,s) => *s, Stmt::Ask{span,..}|Stmt::If{span,..}|Stmt::Repeat(_,span)|Stmt::Try{span,..} => *span, Stmt::Expr(e) => e.span() } }
