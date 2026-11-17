//! Regression tests for the compiler front end and code generator.
//! Code generation tests also verify the generated LLVM module (see `Codegen::compile`).
use crate::{ast::*, codegen::Codegen, lexer, parser::Parser, sema};
use inkwell::context::Context;
use std::path::Path;

fn parse(src: &str) -> Result<Program, String> {
    let tokens = lexer::lex(src).map_err(|e| e.message)?;
    Parser::new(tokens).parse().map_err(|e| e.message)
}

fn check(src: &str) -> Result<(), String> {
    let program = parse(src)?;
    sema::check(&program).map_err(|e| e.message)
}

/// Runs lexer, parser, sema and code generation; returns the LLVM IR as text.
fn compile_ir(src: &str) -> Result<String, String> {
    let program = parse(src)?;
    sema::check(&program).map_err(|e| e.message)?;
    let ctx = Context::create();
    Codegen::new(&ctx, "test")
        .compile(&program)
        .map(|module| module.print_to_string().to_string())
        .map_err(|e| e.message)
}

fn expect_error(result: Result<impl std::fmt::Debug, String>, needle: &str) {
    match result {
        Ok(value) => panic!("expected an error containing `{}`, got Ok({:?})", needle, value),
        Err(message) => assert!(message.contains(needle), "error `{}` does not contain `{}`", message, needle),
    }
}

/// True when every `alloca` sits in the entry block of its function (no stack growth inside loops).
fn allocas_only_in_entry(ir: &str) -> bool {
    let mut in_entry = false;
    for line in ir.lines() {
        if line.starts_with("define ") {
            in_entry = true;
            continue;
        }
        let is_label = !line.starts_with(' ') && !line.starts_with('}') && !line.starts_with(';') && line.contains(':');
        if is_label && !line.starts_with("entry:") {
            in_entry = false;
        }
        if line.contains("= alloca ") && !in_entry {
            return false;
        }
    }
    true
}

/// The text of one basic block: from its label line up to the next label.
fn block_text(ir: &str, label: &str) -> String {
    let mut out = Vec::new();
    let mut inside = false;
    for line in ir.lines() {
        let is_label = !line.starts_with(' ') && !line.starts_with('}') && !line.starts_with(';') && line.contains(':');
        if inside && is_label {
            break;
        }
        if line.starts_with(label) {
            inside = true;
        }
        if inside {
            out.push(line);
        }
    }
    out.join("\n")
}

// ---------------------------------------------------------------- lexer / parser

#[test]
fn strings_keep_utf8() {
    let text = "héllo ọ̀rẹ́ ✓";
    let program = parse(&format!("SAY \"{}\"", text)).unwrap();
    match &program.items[0] {
        Item::Stmt(Stmt::Say(Expr::Str(value, _), _)) => assert_eq!(value, text),
        other => panic!("unexpected item {:?}", other),
    }
}

#[test]
fn oversized_integer_is_an_error_not_a_panic() {
    expect_error(check("SAY 99999999999999999999"), "too large");
}

#[test]
fn not_covers_the_whole_comparison() {
    let program = parse("IF NOT 1 > 2 THEN\n    SAY 1\nEND IF").unwrap();
    match &program.items[0] {
        Item::Stmt(Stmt::If { branches, .. }) => match &branches[0].0 {
            Expr::Unary { op: Unary::Not, expr, .. } => assert!(matches!(**expr, Expr::Binary { op: Binary::Gt, .. })),
            other => panic!("NOT should be the outermost operator, got {:?}", other),
        },
        other => panic!("unexpected item {:?}", other),
    }
}

// ---------------------------------------------------------------- semantic checks

#[test]
fn break_and_continue_need_a_loop() {
    expect_error(check("BREAK"), "inside REPEAT");
    expect_error(check("IF TRUE THEN\n    CONTINUE\nEND IF"), "inside REPEAT");
    assert!(check("REPEAT WHILE FALSE\n    BREAK\nEND REPEAT").is_ok());
}

#[test]
fn give_back_needs_a_function() {
    expect_error(check("GIVE BACK 1"), "inside a function");
}

#[test]
fn fail_needs_a_function_or_try() {
    expect_error(check("FAIL \"x\""), "FAIL can only");
    assert!(check("TRY\n    FAIL \"x\"\nCATCH e\n    SAY e\nEND TRY").is_ok());
}

#[test]
fn inheritance_must_be_known_and_acyclic() {
    expect_error(check("CLASS A EXTENDS B\n    LET x AS int\nEND CLASS\nCLASS B EXTENDS A\n    LET y AS int\nEND CLASS"), "inherits from itself");
    expect_error(check("CLASS A EXTENDS Missing\n    LET x AS int\nEND CLASS"), "unknown class");
}

#[test]
fn duplicate_declarations_are_rejected() {
    expect_error(check("TO f a AS int, a AS int GIVES int\n    GIVE BACK a\nEND TO"), "duplicate parameter");
    expect_error(check("CLASS A\n    LET x AS int\n    LET x AS int\nEND CLASS"), "duplicate field");
    expect_error(check("CLASS A\n    TO m\n        SAY 1\n    END TO\n    TO m\n        SAY 2\n    END TO\nEND CLASS"), "duplicate method");
    expect_error(check("TO main\n    SAY 1\nEND TO"), "reserved");
}

#[test]
fn subclass_methods_can_set_inherited_fields() {
    let src = "CLASS A\n    LET n AS int\n    TO NEW\n        SET n = 1\n    END TO\nEND CLASS\nCLASS B EXTENDS A\n    TO bump\n        SET n = n + 1\n    END TO\nEND CLASS";
    assert!(check(src).is_ok());
}

#[test]
fn for_each_variable_does_not_get_a_bogus_type() {
    assert!(check("LET words = [\"a\", \"b\"]\nREPEAT FOR EACH w IN words\n    LET s AS string = w\n    SAY s\nEND REPEAT").is_ok());
}

#[test]
fn try_body_variables_are_not_visible_in_catch() {
    expect_error(compile_ir("TRY\n    LET inner = 1\nCATCH e\n    SAY inner\nEND TRY"), "unknown variable");
}

// ---------------------------------------------------------------- code generation

#[test]
fn block_variables_do_not_leak() {
    expect_error(compile_ir("IF TRUE THEN\n    LET inner = 1\nEND IF\nSAY inner"), "unknown variable");
    expect_error(compile_ir("REPEAT i FROM 1 TO 2\n    LET inner = i\nEND REPEAT\nSAY inner"), "unknown variable");
}

#[test]
fn arity_is_checked_for_functions_builtins_and_constructors() {
    expect_error(compile_ir("TO f a AS int GIVES int\n    GIVE BACK a\nEND TO\nSAY f(1, 2)"), "expects 1 argument");
    expect_error(compile_ir("SAY LENGTH()"), "expects 1 argument");
    expect_error(compile_ir("CLASS A\n    LET n AS int\n    TO NEW v AS int\n        SET n = v\n    END TO\nEND CLASS\nLET a = NEW A()"), "expects 1 argument");
    expect_error(compile_ir("LET x = NEW Missing()"), "unknown class");
}

#[test]
fn continue_in_a_general_counted_loop_reaches_the_increment() {
    // A float start forces the general (non-native) counted loop.
    let ir = compile_ir("REPEAT i FROM 0.5 TO 3.5\n    IF i > 1 THEN\n        CONTINUE\n    END IF\n    SAY i\nEND REPEAT").unwrap();
    assert!(ir.contains("repeat_latch"), "CONTINUE needs a latch block that increments the counter");
}

#[test]
fn counted_loops_check_the_upper_bound_before_incrementing() {
    let native = compile_ir("REPEAT i FROM 9223372036854775807 TO 9223372036854775807\n    SAY i\nEND REPEAT").unwrap();
    assert!(native.contains("native_loop_finished"));
    let dynamic = compile_ir("REPEAT i FROM 0.5 TO 1.5\n    SAY i\nEND REPEAT").unwrap();
    assert!(dynamic.contains("repeat_increment"));
}

#[test]
fn counted_loop_bounds_are_evaluated_once() {
    let ir = compile_ir("LET n = 3\nREPEAT i FROM 1 TO n\n    SET n = n + 1\nEND REPEAT").unwrap();
    // The loop head only reloads the counter; the bound was computed once, before the loop.
    let head = block_text(&ir, "native_repeat_head:");
    assert_eq!(head.matches("= load").count(), 1, "the loop head must not re-read the bound:\n{}", head);
}

#[test]
fn allocas_live_in_the_entry_block() {
    let ir = compile_ir("LET total = 0\nREPEAT i FROM 1 TO 3\n    LET step = i * 2\n    LET label = \"x\"\n    SET total = total + step\n    SAY label\nEND REPEAT\nSAY total").unwrap();
    assert!(allocas_only_in_entry(&ir), "an alloca inside a loop grows the stack on every iteration:\n{}", ir);
}

#[test]
fn errors_inside_nested_blocks_reach_catch() {
    let src = "TO risky n AS int GIVES int\n    IF n < 0 THEN\n        FAIL \"negative\"\n    END IF\n    GIVE BACK n\nEND TO\nTRY\n    IF TRUE THEN\n        LET a = risky(-1)\n        SAY a\n    END IF\nCATCH e\n    SAY e\nEND TRY";
    let ir = compile_ir(src).unwrap();
    assert!(ir.contains("try_continue"), "the LET inside the IF inside the TRY must be guarded");
}

#[test]
fn statements_after_a_terminator_still_produce_valid_ir() {
    assert!(compile_ir("TO f GIVES int\n    GIVE BACK 1\n    SAY 2\nEND TO\nREPEAT WHILE TRUE\n    BREAK\n    SAY 3\nEND REPEAT").is_ok());
}

#[test]
fn methods_dispatch_on_the_runtime_class() {
    let src = "CLASS Animal\n    LET name AS string\n    TO NEW n AS string\n        SET name = n\n    END TO\n    TO speak GIVES string\n        GIVE BACK name + \" makes a sound\"\n    END TO\nEND CLASS\nCLASS Dog EXTENDS Animal\n    TO speak GIVES string\n        GIVE BACK name + \" barks\"\n    END TO\nEND CLASS\nCLASS Cat EXTENDS Animal\n    TO speak GIVES string\n        GIVE BACK name + \" meows\"\n    END TO\nEND CLASS\nLET pet = NEW Dog(\"Rex\")\nSAY pet.speak()";
    let ir = compile_ir(src).unwrap();
    assert!(ir.contains("dispatch_call"), "overridden methods need a runtime class check");
}

#[test]
fn parameters_shadow_fields() {
    let src = "CLASS Box\n    LET size AS int\n    TO NEW size AS int\n        SET self.size = size\n    END TO\nEND CLASS\nLET b = NEW Box(3)\nSAY b.size";
    assert!(compile_ir(src).is_ok());
}

#[test]
fn and_or_short_circuit_and_nested_ternaries_are_valid() {
    let ir = compile_ir("LET x = 3\nIF x > 0 AND x < 5 OR x = 9 THEN\n    SAY 1 IF x > 5 ELSE 2 IF x > 0 ELSE 3\nEND IF").unwrap();
    assert!(ir.contains("sc_rhs"), "the right operand must only run when needed");
}

#[test]
fn multiple_assignment_swaps() {
    assert!(compile_ir("LET a = 1\nLET b = 2\nSET a, b = b, a\nSAY a\nSAY b").is_ok());
}

// ---------------------------------------------------------------- object-oriented features

#[test]
fn field_defaults_must_be_constants_of_the_right_type() {
    assert!(check("CLASS A\n    LET n AS int = 3\n    LET s AS string = \"x\"\n    LET xs AS [int] = [1, 2]\nEND CLASS").is_ok());
    expect_error(check("CLASS A\n    LET n AS int = k\nEND CLASS"), "constant expression");
    expect_error(check("CLASS A\n    LET n AS int = \"x\"\nEND CLASS"), "does not match");
}

#[test]
fn objects_are_initialised_before_the_constructor_runs() {
    let ir = compile_ir("CLASS A\n    LET n AS int\n    LET s AS string = \"hi\"\nEND CLASS\nLET a = NEW A()\nSAY a.n").unwrap();
    assert!(ir.contains("A__#init"), "every class needs a field initialiser");
}

#[test]
fn super_reaches_the_parent_constructor_and_methods() {
    let src = "CLASS A\n    LET n AS int\n    TO NEW v AS int\n        SET n = v\n    END TO\n    TO show GIVES int\n        GIVE BACK n\n    END TO\nEND CLASS\nCLASS B EXTENDS A\n    TO NEW v AS int\n        SUPER.NEW(v)\n    END TO\n    TO show GIVES int\n        GIVE BACK SUPER.show() + 1\n    END TO\nEND CLASS\nLET b = NEW B(1)\nSAY b.show()";
    assert!(compile_ir(src).is_ok());
}

#[test]
fn super_is_only_valid_in_a_method_of_a_derived_class() {
    expect_error(compile_ir("SUPER.m()"), "SUPER can only be used inside a method");
    expect_error(compile_ir("CLASS A\n    TO m\n        SUPER.m()\n    END TO\nEND CLASS"), "no parent class");
}

#[test]
fn is_operator_covers_subclasses() {
    let src = "CLASS A\n    LET n AS int\nEND CLASS\nCLASS B EXTENDS A\n    LET m AS int\nEND CLASS\nLET b = NEW B()\nSAY b IS A";
    let ir = compile_ir(src).unwrap();
    assert!(ir.contains("call ptr @plain_is_instance"));
    expect_error(compile_ir("LET x = 1\nSAY x IS Missing"), "unknown class");
}

#[test]
fn methods_can_call_their_siblings_without_self() {
    let src = "CLASS A\n    TO one GIVES int\n        GIVE BACK 1\n    END TO\n    TO two GIVES int\n        GIVE BACK one() + 1\n    END TO\nEND CLASS\nLET a = NEW A()\nSAY a.two()";
    assert!(compile_ir(src).is_ok());
}

// ---------------------------------------------------------------- the standard library

#[test]
fn every_library_function_is_declared_and_callable() {
    for (name, symbol, arity) in crate::codegen::STDLIB {
        let args = vec!["0"; *arity].join(", ");
        let ir = compile_ir(&format!("SAY {}({})", name, args)).unwrap_or_else(|e| panic!("{} failed to compile: {}", name, e));
        assert!(ir.contains(symbol), "{} should call {}", name, symbol);
        if *arity > 0 {
            let wrong = vec!["0"; *arity + 1].join(", ");
            expect_error(compile_ir(&format!("SAY {}({})", name, wrong)), "expects");
        }
    }
}

#[test]
fn user_functions_override_library_names() {
    let ir = compile_ir("TO SUM a AS int GIVES int\n    GIVE BACK a + 1000\nEND TO\nSAY SUM(1)").unwrap();
    assert!(!ir.contains("call ptr @plain_sum"), "a user function named SUM must win over the library function");
}

// ---------------------------------------------------------------- the example programs

fn compile_file(path: &Path) -> Result<(), String> {
    let program = crate::load_program(path, &mut Vec::new(), &mut Vec::new(), None)?;
    sema::check(&program).map_err(|e| e.message)?;
    let ctx = Context::create();
    Codegen::new(&ctx, "example").compile(&program).map(|_| ()).map_err(|e| e.message)
}

#[test]
fn every_example_compiles_or_is_rejected_as_documented() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples");
    let rejected = ["ffi_bad.plain", "invalid_set.plain"];
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("plain") {
            continue;
        }
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        let result = compile_file(&path);
        if rejected.contains(&name.as_str()) {
            assert!(result.is_err(), "{} should be rejected", name);
        } else {
            assert!(result.is_ok(), "{} failed to compile: {:?}", name, result);
        }
        checked += 1;
    }
    assert!(checked >= 8, "expected at least 8 examples, found {}", checked);
}

#[test]
fn benchmark_programs_compile() {
    let cases = ["../benchmarks/language-speed/sum_loop.plain", "../benchmarks/language-speed/branch_mod.plain", "../benchmarks/real-world/log_summary.plain"];
    let available: Vec<_> = cases.into_iter().filter(|relative| Path::new(env!("CARGO_MANIFEST_DIR")).join(relative).is_file()).collect();
    if available.is_empty() {
        eprintln!("skipping benchmark compilation checks: benchmark fixtures are not included in this checkout");
        return;
    }
    for relative in available {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
        let result = compile_file(&path);
        assert!(result.is_ok(), "{} failed to compile: {:?}", relative, result);
    }
}
