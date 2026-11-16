mod ast;mod codegen;mod lexer;mod parser;mod sema;mod source;
#[cfg(test)] mod tests;
use std::{env,fs,process::{Command,exit},path::{Path,PathBuf}};
use inkwell::context::Context;
use source::Source;

/// Loads a file and every module it USEs.
/// `stack` holds the modules being loaded right now (to detect cycles);
/// `seen` holds every module already merged, so a module imported twice is included once.
fn load_program(path:&Path,seen:&mut Vec<PathBuf>,stack:&mut Vec<PathBuf>,root:Option<&Path>)->Result<ast::Program,String>{
    let canonical=path.canonicalize().map_err(|e|format!("cannot open {}: {}",path.display(),e))?;
    // Imports must stay inside the project directory (symlinks are resolved first).
    if let Some(root)=root{if !canonical.starts_with(root){return Err(format!("cannot import {}: it is outside the project directory {} (set PLAIN_ALLOW_EXTERNAL_IMPORTS=1 to allow this)",path.display(),root.display()))}}
    if stack.contains(&canonical){return Err(format!("module cycle detected at {}",path.display()))}
    if seen.contains(&canonical){return Ok(ast::Program{items:Vec::new()})}
    seen.push(canonical.clone());
    stack.push(canonical.clone());
    let result=load_module(&canonical,seen,stack,root);
    stack.pop();
    result
}

fn load_module(canonical:&Path,seen:&mut Vec<PathBuf>,stack:&mut Vec<PathBuf>,root:Option<&Path>)->Result<ast::Program,String>{
    let text=fs::read_to_string(canonical).map_err(|e|format!("cannot read {}: {}",canonical.display(),e))?;
    let source=||Source::new(canonical.display().to_string(),text.clone());
    let toks=lexer::lex(&text).map_err(|e|source().diagnostic(e.span,&e.message))?;
    let p=parser::Parser::new(toks).parse().map_err(|e|source().diagnostic(e.span,&e.message))?;
    let mut items=Vec::new();
    for item in p.items{
        if let ast::Item::Use(name,sp)=item{
            let child=canonical.parent().unwrap_or(Path::new(".")).join(name);
            match load_program(&child,seen,stack,root){
                Ok(cp)=>items.extend(cp.items),
                Err(msg)=>return Err(source().diagnostic(sp,&msg)),
            }
        }else{items.push(item)}
    }
    Ok(ast::Program{items})
}

/// Runs an external tool; exits with a readable message if it is missing or fails.
fn run_tool(cmd:&mut Command,name:&str){
    match cmd.status(){
        Ok(status) if status.success()=>{},
        Ok(_)=>exit(1),
        Err(e)=>{eprintln!("could not run `{}`: {} (is it installed and on PATH?)",name,e);exit(1)}
    }
}

fn main(){
    let args:Vec<String>=env::args().collect();
    let (input,mode)=if args.len()==2{(Path::new(&args[1]),"run")}else if args.len()==3{
        let m=args[1].as_str();
        if !matches!(m,"check"|"emit-ir"|"build"){eprintln!("usage: plain [check|emit-ir|build] <file.plain>");exit(2)}
        (Path::new(&args[2]),m)
    }else{eprintln!("usage: plain <file.plain> | plain [check|emit-ir|build] <file.plain>");exit(2)};
    // Only the documented literal value opts out; values such as "0" and "false"
    // must not silently disable the import boundary.
    let allow_external=env::var("PLAIN_ALLOW_EXTERNAL_IMPORTS").map(|v|v=="1").unwrap_or(false);
    let root=if allow_external{None}else{input.canonicalize().ok().and_then(|p|p.parent().map(|d|d.to_path_buf()))};
    let program=match load_program(input,&mut Vec::new(),&mut Vec::new(),root.as_deref()){Ok(p)=>p,Err(e)=>{eprintln!("{}",e);exit(1)}};
    if let Err(e)=sema::check(&program){
        let text=fs::read_to_string(input).unwrap_or_default();
        eprintln!("{}",Source::new(input.display().to_string(),text).diagnostic(e.span,&e.message));
        exit(1)
    }
    let ctx=Context::create();
    let cg=codegen::Codegen::new(&ctx,"plain_program");
    let module=match cg.compile(&program){
        Ok(m)=>m,
        Err(e)=>{
            let text=fs::read_to_string(input).unwrap_or_default();
            eprintln!("{}",Source::new(input.display().to_string(),text).diagnostic(e.span,&e.message));
            exit(1)
        }
    };
    // `check` runs the whole compiler (including LLVM verification) but writes nothing.
    if mode=="check"{println!("checked {}",input.display());exit(0)}
    // `prog.plain` builds `prog`. A file with no extension would map onto itself and be overwritten,
    // so give the executable an `.out` suffix in that case.
    let mut out=input.with_extension("");
    if out.as_path()==input{out=PathBuf::from(format!("{}.out",input.display()))}
    let ir=PathBuf::from(format!("{}.ll",out.display()));
    // An input named `program.ll` would otherwise be overwritten by the IR output.
    let ir_parent=ir.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    if let (Ok(source_path), Ok(parent)) = (input.canonicalize(), ir_parent.canonicalize()) {
        if let Some(file_name) = ir.file_name() {
            let target_path = parent.join(file_name);
            if source_path == target_path {
                eprintln!("refusing to overwrite source file {} with generated LLVM IR; use a .plain input filename", input.display());
                exit(2);
            }
        }
    }
    if let Err(e)=module.print_to_file(&ir){eprintln!("could not write LLVM IR: {}",e);exit(1)}
    if mode=="emit-ir"{println!("wrote {}",ir.display());exit(0)}
    let root=Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let runtime=root.join("runtime");
    let ml=root.join("ml");
    for manifest in [runtime.join("Cargo.toml"),ml.join("Cargo.toml")]{
        run_tool(Command::new("cargo").args(["build","--offline","--release","--manifest-path"]).arg(manifest),"cargo");
    }
    // Rust staticlib names differ between Unix/GNU and Windows MSVC targets.
    let (runtime_name, ml_name) = if cfg!(all(target_os = "windows", target_env = "msvc")) {
        ("plain_runtime.lib", "plain_ml.lib")
    } else {
        ("libplain_runtime.a", "libplain_ml.a")
    };
    let lib=root.join("target/release").join(runtime_name);
    let ml_lib=root.join("target/release").join(ml_name);
    if cfg!(windows) && out.extension().is_none() {
        out.set_extension("exe");
    }
    // Optimisation level for the generated program; override with PLAIN_OPT (for example PLAIN_OPT=-O0 for debugging).
    let opt=env::var("PLAIN_OPT").unwrap_or_else(|_|"-O2".to_string());
    if !matches!(opt.as_str(),"-O0"|"-O1"|"-O2"|"-O3"|"-Os"|"-Oz"){eprintln!("PLAIN_OPT must be one of -O0 -O1 -O2 -O3 -Os -Oz");exit(2)}
    let mut clang = Command::new("clang");
    clang.arg(&opt).arg(&ir).arg(&ml_lib).arg(&lib);
    if !cfg!(windows) { clang.arg("-lm"); }
    clang.arg("-o").arg(&out);
    run_tool(&mut clang,"clang");
    if mode=="build"{println!("built {}",out.display());exit(0)}
    // A bare name like `prog` would be looked up on PATH, so run it as `./prog`.
    let run_path=if out.is_absolute()||out.components().count()>1{out.clone()}else{Path::new(".").join(&out)};
    match Command::new(&run_path).status(){
        Ok(status)=>exit(status.code().unwrap_or(1)),
        Err(e)=>{eprintln!("could not run compiled Plain program {}: {}",run_path.display(),e);exit(1)}
    }
}
