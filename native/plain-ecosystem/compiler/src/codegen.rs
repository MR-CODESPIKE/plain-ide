use std::collections::{HashMap,HashSet};
use std::path::Path;
use inkwell::{context::Context,module::Module,builder::Builder,values::{FunctionValue,PointerValue,BasicValueEnum,BasicMetadataValueEnum},types::BasicTypeEnum,IntPredicate};
use crate::{ast::*,source::CompileError};

type V<'ctx>=inkwell::values::BasicValueEnum<'ctx>;
/// Library functions callable from Plain: (Plain name, runtime symbol, number of arguments).
/// `docs/stdlib.json` documents the same list and `scripts/check_stdlib.py` keeps the two in sync.
pub const STDLIB: &[(&str, &str, usize)] = &[
    // strings
    ("TRIM", "plain_trim", 1), ("REPLACE", "plain_replace", 3), ("SUBSTRING", "plain_substring", 3),
    ("INDEX_OF", "plain_index_of", 2), ("REVERSE", "plain_reverse", 1), ("REPEAT_STRING", "plain_repeat_string", 2),
    // conversion and types
    ("TO_STRING", "plain_to_string", 1), ("TO_INT", "plain_to_int", 1), ("TO_FLOAT", "plain_to_float", 1),
    ("TYPE_OF", "plain_type_of", 1), ("IS_ERROR", "plain_is_error_value", 1), ("ERROR_MESSAGE", "plain_error_message", 1),
    // math
    ("MIN", "plain_min", 2), ("MAX", "plain_max", 2), ("ROUND", "plain_round", 1), ("EXP", "plain_exp", 1),
    ("LOG", "plain_log", 1), ("TAN", "plain_tan", 1), ("PI", "plain_pi", 0), ("SIGN", "plain_sign", 1), ("CLAMP", "plain_clamp", 3),
    // random numbers
    ("RANDOM", "plain_random", 0), ("RANDOM_INT", "plain_random_int", 2), ("SEED", "plain_seed", 1),
    // collections
    ("SORT", "plain_sort", 1), ("SLICE", "plain_slice", 3), ("KEYS", "plain_keys", 1), ("VALUES", "plain_values", 1),
    ("HAS_KEY", "plain_has_key", 2), ("DELETE_KEY", "plain_delete_key", 2), ("SUM", "plain_sum", 1),
    ("LIST_MIN", "plain_list_min", 1), ("LIST_MAX", "plain_list_max", 1), ("UNIQUE", "plain_unique", 1),
    ("COPY", "plain_copy", 1), ("CONCAT", "plain_concat", 2),
    // files, time and the process
    ("SLEEP", "plain_sleep", 1), ("FILE_EXISTS", "plain_file_exists", 1), ("APPEND_FILE", "plain_append_file", 2),
    ("ARGS", "plain_args", 0), ("EXIT", "plain_exit", 1),
    // JSON
    ("JSON_ENCODE", "plain_json_encode", 1), ("JSON_DECODE", "plain_json_decode", 1),
];

pub struct Codegen<'ctx>{pub ctx:&'ctx Context,pub module:Module<'ctx>,pub builder:Builder<'ctx>,ptr:inkwell::types::PointerType<'ctx>,runtime:HashMap<String,FunctionValue<'ctx>>,functions:HashMap<String,FunctionValue<'ctx>>,externs:HashMap<String,ExternFunction>,classes:HashMap<String,Class>,vars:Vec<HashMap<String,PointerValue<'ctx>>>,int_vars:Vec<HashMap<String,PointerValue<'ctx>>>,native_int_candidates:HashSet<String>,field_names:Vec<std::collections::HashSet<String>>,loops:Vec<(inkwell::basic_block::BasicBlock<'ctx>,inkwell::basic_block::BasicBlock<'ctx>)>,error_slot:Option<PointerValue<'ctx>>,catch_stack:Vec<inkwell::basic_block::BasicBlock<'ctx>>,current_class:Option<String>,pub diagnostics:Vec<CompileError>}
impl<'ctx> Codegen<'ctx>{
 pub fn new(ctx:&'ctx Context,name:&str)->Self{let m=ctx.create_module(name);let b=ctx.create_builder();let ptr=ctx.ptr_type(inkwell::AddressSpace::default());let mut g=Self{ctx,module:m,builder:b,ptr,runtime:HashMap::new(),functions:HashMap::new(),externs:HashMap::new(),classes:HashMap::new(),vars:Vec::new(),int_vars:Vec::new(),native_int_candidates:HashSet::new(),field_names:Vec::new(),loops:Vec::new(),error_slot:None,catch_stack:Vec::new(),current_class:None,diagnostics:Vec::new()};g.declare_runtime();g.declare_stdlib();g}
 fn declare_runtime(&mut self){let p=self.ptr.fn_type(&[self.ptr.into()],false);let pp=self.ptr.fn_type(&[self.ptr.into(),self.ptr.into()],false);let p0=self.ptr.fn_type(&[],false);let i8=self.ctx.i8_type();let i64=self.ctx.i64_type();let i32=self.ctx.i32_type();let f64=self.ctx.f64_type();let cchar=self.ctx.ptr_type(inkwell::AddressSpace::default());let mut d=|n:&str,t:inkwell::types::FunctionType<'ctx>|{let f=self.module.add_function(n,t,None);self.runtime.insert(n.into(),f);};d("plain_int",self.ptr.fn_type(&[i64.into()],false));d("plain_float",self.ptr.fn_type(&[f64.into()],false));d("plain_bool",self.ptr.fn_type(&[i8.into()],false));d("plain_string",self.ptr.fn_type(&[cchar.into()],false));d("plain_error",self.ptr.fn_type(&[cchar.into()],false));d("plain_is_error",self.ctx.i8_type().fn_type(&[self.ptr.into()],false));d("plain_error_message",p);d("plain_fail",p);d("plain_truth",self.ctx.i8_type().fn_type(&[self.ptr.into()],false));d("plain_print",self.ctx.void_type().fn_type(&[self.ptr.into()],false));for n in ["plain_add","plain_sub","plain_mul","plain_div","plain_mod","plain_eq","plain_not_eq","plain_lt","plain_lte","plain_gt","plain_gte","plain_and","plain_or"]{d(n,pp);}d("plain_not",p);d("plain_list_new",self.ptr.fn_type(&[i64.into(),self.ptr.into()],false));d("plain_list_get",pp);d("plain_promote",p);d("plain_read_lines",p);d("plain_iter_next",i8.fn_type(&[self.ptr.into(),i64.into(),self.ptr.into()],false));d("plain_contains_native",i8.fn_type(&[self.ptr.into(),cchar.into()],false));d("plain_string_empty_native",i8.fn_type(&[self.ptr.into()],false));d("plain_iterator_fail",self.ctx.void_type().fn_type(&[self.ptr.into()],false));d("plain_list_set",self.ptr.fn_type(&[self.ptr.into(),self.ptr.into(),self.ptr.into()],false));d("plain_append",pp);d("plain_remove",pp);d("plain_length",p);d("plain_upper",p);d("plain_lower",p);d("plain_split",pp);d("plain_dict_new",self.ptr.fn_type(&[i64.into(),self.ptr.into()],false));d("plain_dict_get",pp);d("plain_dict_set",self.ptr.fn_type(&[self.ptr.into(),self.ptr.into(),self.ptr.into()],false));d("plain_new_instance",p0);d("plain_read",self.ptr.fn_type(&[self.ptr.into(),i32.into()],false));for n in ["plain_abs","plain_sqrt","plain_floor","plain_ceil","plain_sin","plain_cos"]{d(n,p);}for n in ["plain_pow","plain_contains","plain_starts_with","plain_ends_with","plain_join","plain_range","plain_write_file","plain_is_instance"]{d(n,pp);}for n in ["plain_read_file","plain_env"]{d(n,p);}d("plain_now_ms",self.ptr.fn_type(&[],false));d("plain_now_ms_i64",i64.fn_type(&[],false));}
 fn ptr_type_cchar(&self)->inkwell::types::PointerType<'ctx>{self.ctx.i8_type().ptr_type(inkwell::AddressSpace::default())}
 pub fn compile(mut self,p:&Program)->Result<Module<'ctx>,CompileError>{self.native_int_candidates=find_native_int_candidates(p);for i in &p.items{if let Item::Class(c)=i{self.classes.insert(c.name.clone(),c.clone());}}for i in &p.items{if let Item::Class(c)=i{let key=format!("{}__#init",c.name);let ty=self.ptr.fn_type(&[self.ptr.into()],false);let f=self.module.add_function(&key,ty,None);self.functions.insert(key,f);}}for i in &p.items{if let Item::Extern(x)=i{self.externs.insert(x.name.clone(),x.clone());let ty=self.ptr.fn_type(&vec![self.ptr.into();x.params.len()],false);self.functions.insert(x.name.clone(),self.module.add_function(&x.name,ty,None));}}for i in &p.items{if let Item::Function(f)=i{self.declare_user(f,"fn");}if let Item::Class(c)=i{for m in &c.methods{self.declare_user(m,&format!("{}__",c.name));}}}let main_t=self.ctx.i32_type().fn_type(&[],false);let main=self.module.add_function("main",main_t,None);let entry=self.ctx.append_basic_block(main,"entry");self.builder.position_at_end(entry);self.vars.push(HashMap::new());self.int_vars.push(HashMap::new());for i in &p.items{if let Item::Stmt(s)=i{self.emit_stmt(s)?;}}if self.builder.get_insert_block().unwrap().get_terminator().is_none(){self.builder.build_return(Some(&self.ctx.i32_type().const_int(0,false))).unwrap();}self.vars.pop();self.int_vars.pop();for i in &p.items{match i{Item::Function(f)=>self.emit_function(f,"fn")?,Item::Class(c)=>{self.emit_class_init(c)?;for m in &c.methods{self.emit_method(c,m)?}},Item::Use(_,sp)=>return Err(CompileError::new(*sp,"module imports must be resolved before code generation")),_=>{}}}if !self.diagnostics.is_empty(){return Err(self.diagnostics.remove(0))}if let Err(problem)=self.module.verify(){return Err(CompileError::new(crate::source::Span::default(),format!("internal compiler error: the generated LLVM IR is invalid: {}",problem.to_string())))}Ok(self.module)}
 fn declare_user(&mut self,f:&Function,prefix:&str){let n=if prefix=="fn"{f.name.clone()}else{format!("{}{}",prefix,f.name)};let ty=self.ptr.fn_type(&vec![self.ptr.into();f.params.len()+if prefix=="fn"{0}else{1}],false);self.functions.insert(n.clone(),self.module.add_function(&n,ty,None));}
 fn emit_function(&mut self,f:&Function,prefix:&str)->Result<(),CompileError>{let n=if prefix=="fn"{f.name.clone()}else{format!("{}{}",prefix,f.name)};let fun=self.functions[&n];let entry=self.ctx.append_basic_block(fun,"entry");self.builder.position_at_end(entry);self.vars.push(HashMap::new());self.int_vars.push(HashMap::new());for(i,(name,_))in f.params.iter().enumerate(){let slot=self.entry_slot(name);self.builder.build_store(slot,fun.get_nth_param(i as u32).unwrap()).unwrap();self.vars.last_mut().unwrap().insert(name.clone(),slot);}for s in &f.body{self.emit_stmt(s)?;}if self.builder.get_insert_block().unwrap().get_terminator().is_none(){self.builder.build_return(Some(&self.null_value())).unwrap();}self.vars.pop();self.int_vars.pop();Ok(())}
 fn emit_method(&mut self,c:&Class,f:&Function)->Result<(),CompileError>{let n=format!("{}__{}",c.name,f.name);let fun=self.functions[&n];let entry=self.ctx.append_basic_block(fun,"entry");self.builder.position_at_end(entry);self.vars.push(HashMap::new());self.int_vars.push(HashMap::new());let mut fields=std::collections::HashSet::new();self.collect_fields(&c.name,&mut fields);self.field_names.push(fields);self.current_class=Some(c.name.clone());let self_slot=self.entry_slot("self");self.builder.build_store(self_slot,fun.get_nth_param(0).unwrap()).unwrap();self.vars.last_mut().unwrap().insert("self".into(),self_slot);for(i,(name,_))in f.params.iter().enumerate(){let slot=self.entry_slot(name);self.builder.build_store(slot,fun.get_nth_param((i+1) as u32).unwrap()).unwrap();self.vars.last_mut().unwrap().insert(name.clone(),slot);}for s in &f.body{self.emit_stmt(s)?;}if self.builder.get_insert_block().unwrap().get_terminator().is_none(){self.builder.build_return(Some(&self.null_value())).unwrap();}self.field_names.pop();self.current_class=None;self.vars.pop();self.int_vars.pop();Ok(())}
 fn collect_fields(&self,name:&str,out:&mut std::collections::HashSet<String>){if let Some(c)=self.classes.get(name){if let Some(parent)=&c.parent{self.collect_fields(parent,out)}for(f,_)in &c.fields{out.insert(f.clone());}}}
 fn method_key(&self,class_name:&str,method:&str)->Option<String>{let direct=format!("{}__{}",class_name,method);if self.functions.contains_key(&direct){return Some(direct)}self.classes.get(class_name).and_then(|c|c.parent.as_ref()).and_then(|p|self.method_key(p,method))}
 fn start_dead_block(&self){let fun=self.builder.get_insert_block().unwrap().get_parent().unwrap();let dead=self.ctx.append_basic_block(fun,"after_terminator");self.builder.position_at_end(dead);}
 fn null_value(&self)->V<'ctx>{self.runtime["plain_bool"].get_type().get_return_type().unwrap().into_pointer_type().const_null().into()}
 fn entry_builder(&self)->Builder<'ctx>{
  let fun=self.builder.get_insert_block().unwrap().get_parent().unwrap();
  let entry=fun.get_first_basic_block().unwrap();
  let b=self.ctx.create_builder();
  match entry.get_first_instruction(){Some(first)=>b.position_before(&first),None=>b.position_at_end(entry)}
  b
 }
 fn entry_alloca<T:inkwell::types::BasicType<'ctx>>(&self,ty:T,name:&str)->PointerValue<'ctx>{
  self.entry_builder().build_alloca(ty,name).unwrap()
 }
 fn entry_slot(&self,name:&str)->PointerValue<'ctx>{
  let b=self.entry_builder();
  let slot=b.build_alloca(self.ptr,name).unwrap();
  b.build_store(slot,self.ptr.const_null()).unwrap();
  slot
 }
 fn push_scope(&mut self){self.vars.push(HashMap::new());self.int_vars.push(HashMap::new());}
 fn pop_scope(&mut self){self.vars.pop();self.int_vars.pop();}
 fn truth_of(&self,v:V<'ctx>)->inkwell::values::IntValue<'ctx>{
  let t=self.builder.build_call(self.runtime["plain_truth"],&[v.into()],"truth").unwrap().try_as_basic_value().left().unwrap().into_int_value();
  self.builder.build_int_compare(IntPredicate::NE,t,self.ctx.i8_type().const_zero(),"truthy").unwrap()
 }
 fn guard_value(&mut self,v:PointerValue<'ctx>)->Result<(),CompileError>{
  match (self.error_slot,self.catch_stack.last().copied()){
   (Some(_),Some(catchb))=>self.guard(v,catchb),
   _=>Ok(()),
  }
 }
 fn check_arity(&self,key:&str,given:usize,sp:crate::source::Span,implicit:usize)->Result<(),CompileError>{
  let expected=self.functions[key].count_params() as usize;
  if expected!=given{
   return Err(CompileError::new(sp,format!("`{}` expects {} argument(s), got {}",key.rsplit("__").next().unwrap_or(key),expected.saturating_sub(implicit),given.saturating_sub(implicit))))
  }
  Ok(())
 }
 fn emit_method_call(&mut self,sp:crate::source::Span,object:V<'ctx>,name:&str,args2:Vec<BasicMetadataValueEnum<'ctx>>)->Result<V<'ctx>,CompileError>{
  let mut class_names:Vec<String>=self.classes.keys().cloned().collect();
  class_names.sort();
  let mut candidates:Vec<(String,String)>=Vec::new();
  for c in &class_names{if let Some(key)=self.method_key(c,name){candidates.push((c.clone(),key));}}
  if candidates.is_empty(){return Err(CompileError::new(sp,format!("unknown method `{}`",name)))}
  let first_key=candidates[0].1.clone();
  if candidates.iter().all(|(_,k)|*k==first_key){
   self.check_arity(&first_key,args2.len(),sp,1)?;
   return Ok(self.call_user(&first_key,&args2));
  }
  let usable:Vec<(String,String)>=candidates.iter().filter(|(_,k)|self.functions[k.as_str()].count_params() as usize==args2.len()).cloned().collect();
  if usable.is_empty(){
   self.check_arity(&candidates[0].1,args2.len(),sp,1)?;
   return Err(CompileError::new(sp,format!("no `{}` method accepts {} argument(s)",name,args2.len()-1)));
  }
  let fun=self.builder.get_insert_block().unwrap().get_parent().unwrap();
  let tag_key=self.const_string("__class__");
  let tag=self.runtime("plain_dict_get",&[object.into(),tag_key.into()]);
  let end=self.ctx.append_basic_block(fun,"dispatch_end");
  let mut incoming:Vec<(V<'ctx>,inkwell::basic_block::BasicBlock<'ctx>)>=Vec::new();
  for(class,key)in &usable{
   let wanted=self.const_string(class);
   let same=self.runtime("plain_eq",&[tag.into(),wanted.into()]);
   let cond=self.truth_of(same);
   let yes=self.ctx.append_basic_block(fun,"dispatch_call");
   let no=self.ctx.append_basic_block(fun,"dispatch_next");
   self.builder.build_conditional_branch(cond,yes,no).unwrap();
   self.builder.position_at_end(yes);
   let result=self.call_user(key,&args2);
   self.builder.build_unconditional_branch(end).unwrap();
   incoming.push((result,self.builder.get_insert_block().unwrap()));
   self.builder.position_at_end(no);
  }
  let message=self.builder.build_global_string_ptr("method called on a value that is not an object with this method","method_error").unwrap();
  let failure=self.runtime("plain_error",&[message.as_pointer_value().into()]);
  self.builder.build_unconditional_branch(end).unwrap();
  incoming.push((failure,self.builder.get_insert_block().unwrap()));
  self.builder.position_at_end(end);
  let phi=self.builder.build_phi(self.ptr,"dispatch_result").unwrap();
  let refs:Vec<(&dyn inkwell::values::BasicValue<'ctx>,inkwell::basic_block::BasicBlock<'ctx>)>=incoming.iter().map(|(v,b)|(v as &dyn inkwell::values::BasicValue<'ctx>,*b)).collect();
  phi.add_incoming(&refs);
  Ok(phi.as_basic_value())
 }
 fn declare_stdlib(&mut self){
  for (_,symbol,arity) in STDLIB{
   if self.runtime.contains_key(*symbol){continue}
   let ty=self.ptr.fn_type(&vec![self.ptr.into();*arity],false);
   let f=self.module.add_function(symbol,ty,None);
   self.runtime.insert(symbol.to_string(),f);
  }
 }
 fn is_subclass(&self,class:&str,ancestor:&str)->bool{
  let mut current=class.to_string();
  for _ in 0..=self.classes.len(){
   if current==ancestor{return true}
   match self.classes.get(&current).and_then(|c|c.parent.clone()){Some(parent)=>current=parent,None=>return false}
  }
  false
 }
 fn default_for_type(&mut self,ty:&Type)->V<'ctx>{
  match ty{
   Type::Int=>self.runtime("plain_int",&[self.ctx.i64_type().const_zero().into()]),
   Type::Float=>self.runtime("plain_float",&[self.ctx.f64_type().const_zero().into()]),
   Type::Bool=>self.runtime("plain_bool",&[self.ctx.i8_type().const_zero().into()]),
   Type::String=>self.const_string("").into(),
   Type::List(_)=>self.runtime("plain_list_new",&[self.ctx.i64_type().const_zero().into(),self.ptr.const_null().into()]),
   Type::Dict=>self.runtime("plain_dict_new",&[self.ctx.i64_type().const_zero().into(),self.ptr.const_null().into()]),
   Type::Any|Type::Void=>self.null_value(),
  }
 }
 /// `Class__#init(object)` sets every field (inherited ones first) to its declared default or a default for its type.
 fn emit_class_init(&mut self,c:&Class)->Result<(),CompileError>{
  let key=format!("{}__#init",c.name);
  let fun=self.functions[key.as_str()];
  let entry=self.ctx.append_basic_block(fun,"entry");
  self.builder.position_at_end(entry);
  self.vars.push(HashMap::new());
  self.int_vars.push(HashMap::new());
  let object=fun.get_nth_param(0).unwrap();
  if let Some(parent)=&c.parent{
   let parent_key=format!("{}__#init",parent);
   if self.functions.contains_key(parent_key.as_str()){self.call_user(&parent_key,&[object.into()]);}
  }
  for((field,ty),default)in c.fields.iter().zip(c.defaults.iter()){
   let value=match default{
    Some(expr)=>{let v=self.emit_expr(expr)?;self.runtime("plain_promote",&[v.into()])},
    None=>self.default_for_type(ty),
   };
   let field_key=self.const_string(field);
   self.runtime("plain_dict_set",&[object.into(),field_key.into(),value.into()]);
  }
  self.builder.build_return(Some(&self.null_value())).unwrap();
  self.vars.pop();
  self.int_vars.pop();
  Ok(())
 }
 /// `SUPER.name(args)`: calls the parent class's implementation directly (no dynamic dispatch).
 fn emit_super_call(&mut self,sp:crate::source::Span,name:&str,av:Vec<BasicMetadataValueEnum<'ctx>>)->Result<V<'ctx>,CompileError>{
  let class=self.current_class.clone().ok_or_else(||CompileError::new(sp,"SUPER can only be used inside a method"))?;
  let parent=self.classes.get(&class).and_then(|c|c.parent.clone()).ok_or_else(||CompileError::new(sp,format!("class `{}` has no parent class, so SUPER is not available",class)))?;
  let self_slot=self.vars.iter().rev().find_map(|m|m.get("self")).copied().ok_or_else(||CompileError::new(sp,"SUPER needs `self`"))?;
  let object=self.builder.build_load(self.ptr,self_slot,"self").unwrap();
  match self.method_key(&parent,name){
   Some(key)=>{
    let mut args2=vec![object.into()];
    args2.extend(av);
    self.check_arity(&key,args2.len(),sp,1)?;
    Ok(self.call_user(&key,&args2))
   },
   None=>{
    if name=="NEW"&&av.is_empty(){Ok(self.null_value())}
    else{Err(CompileError::new(sp,format!("parent class `{}` has no method `{}`",parent,name)))}
   }
  }
 }
 fn runtime(&self,n:&str,args:&[BasicMetadataValueEnum<'ctx>])->V<'ctx>{self.builder.build_call(self.runtime[n],args,&format!("{}_tmp",n)).unwrap().try_as_basic_value().left().unwrap()}
 fn call_user(&self,n:&str,args:&[BasicMetadataValueEnum<'ctx>])->V<'ctx>{self.builder.build_call(self.functions[n],args,&format!("{}_call",n)).unwrap().try_as_basic_value().left().unwrap()}
 fn alloc_values(&self,vals:&[V<'ctx>],name:&str)->PointerValue<'ctx>{
  let arr=self.entry_builder().build_array_alloca(self.ptr,self.ctx.i64_type().const_int(vals.len() as u64,false),name).unwrap();
  for(i,v)in vals.iter().enumerate(){
   let p=unsafe{self.builder.build_gep(self.ptr,arr,&[self.ctx.i64_type().const_int(i as u64,false)],&format!("{}_{}",name,i)).unwrap()};
   self.builder.build_store(p,*v).unwrap();
  }
  arr
 }
 fn find_native_int_slot(&self,name:&str)->Option<PointerValue<'ctx>>{for index in (0..self.int_vars.len()).rev(){if let Some(slot)=self.int_vars[index].get(name){return Some(*slot)}if self.vars.get(index).map(|scope|scope.contains_key(name)).unwrap_or(false){return None}}None}
 fn can_emit_native_int(&self,e:&Expr)->bool{match e{Expr::Int(..)=>true,Expr::Var(name,_)=>self.find_native_int_slot(name).is_some(),Expr::Unary{op:Unary::Neg,expr,..}=>self.can_emit_native_int(expr),Expr::Binary{left,op,right,..}=>{let arithmetic=matches!(op,Binary::Add|Binary::Sub|Binary::Mul);let modulo=matches!(op,Binary::Mod)&&native_nonzero_int_literal(right);(arithmetic||modulo)&&self.can_emit_native_int(left)&&self.can_emit_native_int(right)},Expr::Call{callee,args,..}=>args.is_empty()&&matches!(callee.as_ref(),Expr::Var(name,_) if name=="NOW_MS"),_=>false}}
 fn emit_native_int(&mut self,e:&Expr)->Result<Option<inkwell::values::IntValue<'ctx>>,CompileError>{if !self.can_emit_native_int(e){return Ok(None)}let value=match e{Expr::Int(n,_)=>self.ctx.i64_type().const_int(*n as u64,true),Expr::Var(name,_)=>{let slot=self.find_native_int_slot(name).unwrap();self.builder.build_load(self.ctx.i64_type(),slot,name).unwrap().into_int_value()},Expr::Unary{op:Unary::Neg,expr,..}=>{let v=self.emit_native_int(expr)?.unwrap();self.builder.build_int_neg(v,"native_neg").unwrap()},Expr::Binary{left,op,right,..}=>{let a=self.emit_native_int(left)?.unwrap();let b=self.emit_native_int(right)?.unwrap();match op{Binary::Add=>self.builder.build_int_add(a,b,"native_add").unwrap(),Binary::Sub=>self.builder.build_int_sub(a,b,"native_sub").unwrap(),Binary::Mul=>self.builder.build_int_mul(a,b,"native_mul").unwrap(),Binary::Mod=>self.builder.build_int_signed_rem(a,b,"native_rem").unwrap(),_=>return Ok(None)}},Expr::Call{callee,..} if matches!(callee.as_ref(),Expr::Var(name,_) if name=="NOW_MS")=>self.builder.build_call(self.runtime["plain_now_ms_i64"],&[],"now_ms_i64").unwrap().try_as_basic_value().left().unwrap().into_int_value(),_=>return Ok(None)};Ok(Some(value))}
 fn emit_native_bool(&mut self,e:&Expr)->Result<Option<inkwell::values::IntValue<'ctx>>,CompileError>{
  match e {
   Expr::Call{callee,args,..} if matches!(callee.as_ref(),Expr::Var(name,_) if name=="CONTAINS") && args.len()==2 => {
    if let Expr::Str(needle,_)=&args[1]{
     let hay=self.emit_expr(&args[0])?;
     let global=self.builder.build_global_string_ptr(needle,"contains_literal").unwrap().as_pointer_value();
     let raw=self.builder.build_call(self.runtime["plain_contains_native"],&[hay.into(),global.into()],"contains_native").unwrap().try_as_basic_value().left().unwrap().into_int_value();
     return Ok(Some(self.builder.build_int_compare(IntPredicate::NE,raw,self.ctx.i8_type().const_zero(),"contains_cond").unwrap()));
    }
    Ok(None)
   }
   Expr::Binary{left,op,right,..} if matches!(op,Binary::Eq|Binary::NotEq) => {
    let (value,lit)=match (&**left,&**right){(v,Expr::Str(s,_)) if s.is_empty()=>(v,Some(s)),(Expr::Str(s,_),v) if s.is_empty()=>(v,Some(s)),_ => (left.as_ref(),None)};
    if lit.is_some(){
     let v=self.emit_expr(value)?;
     let raw=self.builder.build_call(self.runtime["plain_string_empty_native"],&[v.into()],"string_empty").unwrap().try_as_basic_value().left().unwrap().into_int_value();
     let pred=if matches!(op,Binary::Eq){IntPredicate::NE}else{IntPredicate::EQ};
     return Ok(Some(self.builder.build_int_compare(pred,raw,self.ctx.i8_type().const_zero(),"string_empty_cond").unwrap()));
    }
    let predicate=match op{Binary::Eq=>IntPredicate::EQ,Binary::NotEq=>IntPredicate::NE,_=>unreachable!()};
    if !self.can_emit_native_int(left)||!self.can_emit_native_int(right){return Ok(None)}
    let a=self.emit_native_int(left)?.unwrap();let b=self.emit_native_int(right)?.unwrap();
    Ok(Some(self.builder.build_int_compare(predicate,a,b,"native_cmp").unwrap()))
   }
   Expr::Binary{left,op,right,..}=>{
    let predicate=match op{Binary::Lt=>IntPredicate::SLT,Binary::Lte=>IntPredicate::SLE,Binary::Gt=>IntPredicate::SGT,Binary::Gte=>IntPredicate::SGE,_=>return Ok(None)};
    if !self.can_emit_native_int(left)||!self.can_emit_native_int(right){return Ok(None)}
    let a=self.emit_native_int(left)?.unwrap();let b=self.emit_native_int(right)?.unwrap();
    Ok(Some(self.builder.build_int_compare(predicate,a,b,"native_cmp").unwrap()))
   }
   _=>Ok(None)
  }
 }
 fn emit_expr(&mut self,e:&Expr)->Result<V<'ctx>,CompileError>{if self.can_emit_native_int(e){let value=self.emit_native_int(e)?.ok_or_else(||CompileError::new(e.span(),"native integer lowering failed"))?;Ok(self.runtime("plain_int",&[value.into()]))}else{self.emit_expr_dynamic(e)}}
 fn emit_native_counted_loop(&mut self,name:&str,start:&Expr,end_expr:&Expr,body:&[Stmt])->Result<(),CompileError>{
  let start_value=self.emit_native_int(start)?.ok_or_else(||CompileError::new(start.span(),"integer loop start lowering failed"))?;
  // The upper bound is evaluated once, before the loop starts.
  let limit=self.emit_native_int(end_expr)?.ok_or_else(||CompileError::new(end_expr.span(),"integer loop end lowering failed"))?;
  let slot=self.entry_alloca(self.ctx.i64_type(),name);
  self.builder.build_store(slot,start_value).unwrap();
  self.push_scope();
  self.int_vars.last_mut().unwrap().insert(name.to_string(),slot);
  let fun=self.builder.get_insert_block().unwrap().get_parent().unwrap();
  let head=self.ctx.append_basic_block(fun,"native_repeat_head");
  let block_body=self.ctx.append_basic_block(fun,"native_repeat_body");
  let latch=self.ctx.append_basic_block(fun,"native_repeat_latch");
  let end=self.ctx.append_basic_block(fun,"native_repeat_end");
  self.builder.build_unconditional_branch(head).unwrap();
  self.builder.position_at_end(head);
  let current=self.builder.build_load(self.ctx.i64_type(),slot,"native_loop_value").unwrap().into_int_value();
  let condition=self.builder.build_int_compare(IntPredicate::SLE,current,limit,"native_loop_cond").unwrap();
  self.builder.build_conditional_branch(condition,block_body,end).unwrap();
  self.builder.position_at_end(block_body);
  self.loops.push((end,latch));
  for statement in body{self.emit_stmt(statement)?;}
  self.loops.pop();
  if self.builder.get_insert_block().unwrap().get_terminator().is_none(){self.builder.build_unconditional_branch(latch).unwrap();}
  self.builder.position_at_end(latch);
  // Do not increment the inclusive upper bound: i64::MAX + 1 would wrap
  // and make the signed `current <= limit` test true forever.
  let current=self.builder.build_load(self.ctx.i64_type(),slot,"native_loop_latch_value").unwrap().into_int_value();
  let finished=self.builder.build_int_compare(IntPredicate::SGE,current,limit,"native_loop_finished").unwrap();
  let increment=self.ctx.append_basic_block(fun,"native_repeat_increment");
  self.builder.build_conditional_branch(finished,end,increment).unwrap();
  self.builder.position_at_end(increment);
  let next=self.builder.build_int_add(current,self.ctx.i64_type().const_int(1,false),"native_loop_next").unwrap();
  self.builder.build_store(slot,next).unwrap();
  self.builder.build_unconditional_branch(head).unwrap();
  self.builder.position_at_end(end);
  self.pop_scope();
  Ok(())
 }
 fn emit_stmt(&mut self,s:&Stmt)->Result<Option<PointerValue<'ctx>>,CompileError>{
  match s{
   Stmt::Let(ns,es,sp)=>{
    if ns.len()!=es.len(){return Err(CompileError::new(*sp,"LET names and values must match"))}
    for((n,explicit),e)in ns.iter().zip(es){
     let annotation_allows_int=explicit.as_ref().map(|t|matches!(t,Type::Int)).unwrap_or(true);
     let use_native=self.native_int_candidates.contains(n)&&annotation_allows_int&&self.can_emit_native_int(e);
     if use_native{
      let value=self.emit_native_int(e)?.ok_or_else(||CompileError::new(e.span(),"native integer analysis mismatch"))?;
      let slot=self.entry_alloca(self.ctx.i64_type(),n);
      self.builder.build_store(slot,value).unwrap();
      self.int_vars.last_mut().unwrap().insert(n.clone(),slot);
     }else{
      let v=self.emit_expr(e)?;
      let v=self.runtime("plain_promote",&[v.into()]).into_pointer_value();
      self.guard_value(v)?;
      let slot=self.entry_slot(n);
      self.builder.build_store(slot,v).unwrap();
      self.vars.last_mut().unwrap().insert(n.clone(),slot);
     }
    }
    Ok(None)
   },
   Stmt::Set(ls,es,sp)=>{
    // All right-hand sides are evaluated first, then assigned, so `SET a, b = b, a` swaps.
    let mut staged:Vec<(Option<PointerValue<'ctx>>,Option<inkwell::values::IntValue<'ctx>>,Option<V<'ctx>>)>=Vec::new();
    for(l,e)in ls.iter().zip(es){
     let native_slot=match l{Expr::Var(name,_)=>self.find_native_int_slot(name),_=>None};
     if let Some(slot)=native_slot{
      let value=self.emit_native_int(e)?.ok_or_else(||CompileError::new(e.span(),"cannot assign a non-integer expression to an inferred integer"))?;
      staged.push((Some(slot),Some(value),None));
     }else{
      let v=self.emit_expr(e)?;
      let v=self.runtime("plain_promote",&[v.into()]);
      self.guard_value(v.into_pointer_value())?;
      staged.push((None,None,Some(v)));
     }
    }
    for(l,(slot,int_value,dynamic))in ls.iter().zip(staged){
     if let(Some(slot),Some(value))=(slot,int_value){
      self.builder.build_store(slot,value).unwrap();
     }else if let Some(v)=dynamic{
      self.assign(l,v,*sp)?;
     }
    }
    Ok(None)
   },
   Stmt::Say(e,_)=>{
    let v=self.emit_expr(e)?.into_pointer_value();
    self.guard_value(v)?;
    self.builder.build_call(self.runtime["plain_print"],&[v.into()],"").unwrap();
    Ok(None)
   },
   Stmt::Ask{prompt,name,ty,..}=>{
    let p=self.emit_expr(prompt)?;
    let kind=match ty{Type::Int=>1u64,Type::Float=>2,Type::Bool=>3,_=>4};
    let prompt_ptr=self.to_cstr(p);
    let read_fn=self.runtime["plain_read"];
    let kind_value=self.ctx.i32_type().const_int(kind,false);
    let v=self.builder.build_call(read_fn,&[prompt_ptr.into(),kind_value.into()],"read").unwrap().try_as_basic_value().left().unwrap().into_pointer_value();
    self.guard_value(v)?;
    let slot=self.entry_slot(name);
    self.builder.build_store(slot,v).unwrap();
    self.vars.last_mut().unwrap().insert(name.clone(),slot);
    Ok(None)
   },
   Stmt::Expr(e)=>{
    let v=self.emit_expr(e)?.into_pointer_value();
    self.guard_value(v)?;
    Ok(None)
   },
   Stmt::Return(e,_)=>{
    let v=e.as_ref().map(|x|self.emit_expr(x)).transpose()?.unwrap_or_else(||self.null_value().into());
    let v=self.runtime("plain_promote",&[v.into()]);
    self.guard_value(v.into_pointer_value())?;
    self.builder.build_return(Some(&v)).unwrap();
    self.start_dead_block();
    Ok(None)
   },
   Stmt::Fail(e,_)=>{
    let v=self.emit_expr(e)?;
    let ev=self.runtime("plain_fail",&[v.into()]);
    let target=self.catch_stack.last().copied();
    match (self.error_slot,target){
     (Some(slot),Some(catchb))=>{
      self.builder.build_store(slot,ev).unwrap();
      self.builder.build_unconditional_branch(catchb).unwrap();
      self.start_dead_block();
      Ok(None)
     },
     _=>{
      self.builder.build_return(Some(&ev)).unwrap();
      self.start_dead_block();
      Ok(None)
     }
    }
   },
   Stmt::Break(_)=>{
    if let Some((br,_))=self.loops.last().copied(){self.builder.build_unconditional_branch(br).unwrap();self.start_dead_block();}
    Ok(None)
   },
   Stmt::Continue(_)=>{
    if let Some((_,cont))=self.loops.last().copied(){self.builder.build_unconditional_branch(cont).unwrap();self.start_dead_block();}
    Ok(None)
   },
   Stmt::If{branches,else_body,..}=>{self.emit_if(branches,else_body)?;Ok(None)},
   Stmt::Repeat(r,_)=>{self.emit_repeat(r)?;Ok(None)},
   Stmt::Try{body,catch,catch_body,..}=>{self.emit_try(body,catch,catch_body)?;Ok(None)},
  }
 }
 fn to_cstr(&mut self,v:V<'ctx>)->PointerValue<'ctx>{v.into_pointer_value()}
 fn assign(&mut self,l:&Expr,v:V<'ctx>,sp:crate::source::Span)->Result<(),CompileError>{
  match l{
   Expr::Var(n,_)=>{
    // Locals and parameters shadow fields of the current object.
    let local=self.vars.iter().rev().find_map(|m|m.get(n)).copied();
    if let Some(slot)=local{
     self.builder.build_store(slot,v).unwrap();
     Ok(())
    }else if self.field_names.last().map(|f|f.contains(n)).unwrap_or(false){
     let self_slot=self.vars.iter().rev().find_map(|m|m.get("self")).copied().unwrap();
     let obj=self.builder.build_load(self.ptr,self_slot,"self").unwrap();
     let key=self.const_string(n);
     let r=self.runtime("plain_dict_set",&[obj.into(),key.into(),v.into()]).into_pointer_value();
     self.guard_value(r)?;
     Ok(())
    }else{
     Err(CompileError::new(sp,format!("SET requires existing variable `{}`",n)))
    }
   },
   Expr::Index{object,index,..}=>{
    let o=self.emit_expr(object)?;
    let i=self.emit_expr(index)?;
    let r=self.runtime("plain_list_set",&[o.into(),i.into(),v.into()]).into_pointer_value();
    self.guard_value(r)?;
    Ok(())
   },
   Expr::Member{object,name,..}=>{
    let o=self.emit_expr(object)?;
    let k=self.const_string(name);
    let r=self.runtime("plain_dict_set",&[o.into(),k.into(),v.into()]).into_pointer_value();
    self.guard_value(r)?;
    Ok(())
   },
   _=>Err(CompileError::new(sp,"SET target must be a variable, index, or field")),
  }
 }
 fn emit_if(&mut self,branches:&[(Expr,Vec<Stmt>)],else_body:&[Stmt])->Result<(),CompileError>{
  let fun=self.builder.get_insert_block().unwrap().get_parent().unwrap();
  let end=self.ctx.append_basic_block(fun,"if_end");
  let mut next=None;
  for(c,b)in branches{
   let test=next.unwrap_or_else(||self.builder.get_insert_block().unwrap());
   self.builder.position_at_end(test);
   let cond=if let Some(native)=self.emit_native_bool(c)?{
    native
   }else{
    let v=self.emit_expr(c)?.into_pointer_value();
    self.guard_value(v)?;
    self.truth_of(v.into())
   };
   let thenb=self.ctx.append_basic_block(fun,"if_then");
   let elseb=self.ctx.append_basic_block(fun,"if_else");
   self.builder.build_conditional_branch(cond,thenb,elseb).unwrap();
   self.builder.position_at_end(thenb);
   self.push_scope();
   for s in b{self.emit_stmt(s)?;}
   self.pop_scope();
   if self.builder.get_insert_block().unwrap().get_terminator().is_none(){self.builder.build_unconditional_branch(end).unwrap();}
   next=Some(elseb);
  }
  let e=next.unwrap_or_else(||self.builder.get_insert_block().unwrap());
  self.builder.position_at_end(e);
  self.push_scope();
  for s in else_body{self.emit_stmt(s)?;}
  self.pop_scope();
  if self.builder.get_insert_block().unwrap().get_terminator().is_none(){self.builder.build_unconditional_branch(end).unwrap();}
  self.builder.position_at_end(end);
  Ok(())
 }
 fn emit_repeat(&mut self,r:&Repeat)->Result<(),CompileError>{if let Repeat::Counted{name,start,end,body}=r{if self.native_int_candidates.contains(name)&&self.can_emit_native_int(start)&&self.can_emit_native_int(end){return self.emit_native_counted_loop(name,start,end,body)}}if let Repeat::Each{name,iter,body}=r{return self.emit_repeat_each_native(name,iter,body)}self.emit_repeat_dynamic(r)}
 fn emit_repeat_each_native(&mut self,name:&str,iter:&Expr,body:&[Stmt])->Result<(),CompileError>{
  let iterator=self.emit_expr(iter)?;
  self.guard_value(iterator.into_pointer_value())?;
  let fun=self.builder.get_insert_block().unwrap().get_parent().unwrap();
  self.push_scope();
  let index=self.entry_alloca(self.ctx.i64_type(),"each_index_native");
  self.builder.build_store(index,self.ctx.i64_type().const_zero()).unwrap();
  let item_slot=self.entry_slot(name);
  self.vars.last_mut().unwrap().insert(name.to_string(),item_slot);
  let out_slot=self.entry_alloca(self.ptr,"each_out");
  let head=self.ctx.append_basic_block(fun,"each_native_head");
  let block_body=self.ctx.append_basic_block(fun,"each_native_body");
  let not_item=self.ctx.append_basic_block(fun,"each_native_not_item");
  let fail=self.ctx.append_basic_block(fun,"each_native_fail");
  let latch=self.ctx.append_basic_block(fun,"each_native_latch");
  let end=self.ctx.append_basic_block(fun,"each_native_end");
  self.builder.build_unconditional_branch(head).unwrap();
  self.builder.position_at_end(head);
  let idx=self.builder.build_load(self.ctx.i64_type(),index,"each_native_index").unwrap().into_int_value();
  let status=self.builder.build_call(self.runtime["plain_iter_next"],&[iterator.into(),idx.into(),out_slot.into()],"each_status").unwrap().try_as_basic_value().left().unwrap().into_int_value();
  let has_item=self.builder.build_int_compare(IntPredicate::EQ,status,self.ctx.i8_type().const_int(1,false),"each_has_item").unwrap();
  self.builder.build_conditional_branch(has_item,block_body,not_item).unwrap();
  self.builder.position_at_end(not_item);
  let is_end=self.builder.build_int_compare(IntPredicate::EQ,status,self.ctx.i8_type().const_zero(),"each_is_end").unwrap();
  self.builder.build_conditional_branch(is_end,end,fail).unwrap();
  self.builder.position_at_end(fail);
  self.builder.build_call(self.runtime["plain_iterator_fail"],&[iterator.into()],"").unwrap();
  self.builder.build_unreachable().unwrap();
  self.builder.position_at_end(block_body);
  let item=self.builder.build_load(self.ptr,out_slot,"each_item").unwrap();
  self.builder.build_store(item_slot,item).unwrap();
  self.loops.push((end,latch));
  for statement in body{self.emit_stmt(statement)?;}
  self.loops.pop();
  if self.builder.get_insert_block().unwrap().get_terminator().is_none(){self.builder.build_unconditional_branch(latch).unwrap();}
  self.builder.position_at_end(latch);
  let current=self.builder.build_load(self.ctx.i64_type(),index,"each_latch_index").unwrap().into_int_value();
  let next=self.builder.build_int_add(current,self.ctx.i64_type().const_int(1,false),"each_next_index").unwrap();
  self.builder.build_store(index,next).unwrap();
  self.builder.build_unconditional_branch(head).unwrap();
  self.builder.position_at_end(end);
  self.pop_scope();
  Ok(())
 }
 fn emit_repeat_dynamic(&mut self,r:&Repeat)->Result<(),CompileError>{
  let fun=self.builder.get_insert_block().unwrap().get_parent().unwrap();
  match r{
   Repeat::While{cond,body}=>{
    let head=self.ctx.append_basic_block(fun,"repeat_head");
    let block_body=self.ctx.append_basic_block(fun,"repeat_body");
    let end=self.ctx.append_basic_block(fun,"repeat_end");
    self.builder.build_unconditional_branch(head).unwrap();
    self.builder.position_at_end(head);
    let value=self.emit_expr(cond)?.into_pointer_value();
    self.guard_value(value)?;
    let condition=self.truth_of(value.into());
    self.builder.build_conditional_branch(condition,block_body,end).unwrap();
    self.builder.position_at_end(block_body);
    // CONTINUE re-checks the condition, which is exactly what a WHILE loop needs.
    self.loops.push((end,head));
    self.push_scope();
    for statement in body{self.emit_stmt(statement)?;}
    self.pop_scope();
    self.loops.pop();
    if self.builder.get_insert_block().unwrap().get_terminator().is_none(){self.builder.build_unconditional_branch(head).unwrap();}
    self.builder.position_at_end(end);
    Ok(())
   },
   Repeat::Counted{name,start,end:end_expr,body}=>{
    let start_value=self.emit_expr(start)?;
    // The upper bound is evaluated once, before the loop starts.
    let limit=self.emit_expr(end_expr)?;
    let slot=self.entry_slot(name);
    self.builder.build_store(slot,start_value).unwrap();
    self.push_scope();
    self.vars.last_mut().unwrap().insert(name.clone(),slot);
    let head=self.ctx.append_basic_block(fun,"repeat_head");
    let block_body=self.ctx.append_basic_block(fun,"repeat_body");
    let latch=self.ctx.append_basic_block(fun,"repeat_latch");
    let end=self.ctx.append_basic_block(fun,"repeat_end");
    self.builder.build_unconditional_branch(head).unwrap();
    self.builder.position_at_end(head);
    let current=self.builder.build_load(self.ptr,slot,"loop_value").unwrap();
    let allowed=self.runtime("plain_lte",&[current.into(),limit.into()]).into_pointer_value();
    self.guard_value(allowed)?;
    let condition=self.truth_of(allowed.into());
    self.builder.build_conditional_branch(condition,block_body,end).unwrap();
    self.builder.position_at_end(block_body);
    // CONTINUE must run the increment in the latch block, not jump back to the head.
    self.loops.push((end,latch));
    for statement in body{self.emit_stmt(statement)?;}
    self.loops.pop();
    if self.builder.get_insert_block().unwrap().get_terminator().is_none(){self.builder.build_unconditional_branch(latch).unwrap();}
    self.builder.position_at_end(latch);
    // Stop before incrementing the inclusive upper bound, avoiding wraparound.
    let current=self.builder.build_load(self.ptr,slot,"loop_value").unwrap();
    let finished=self.runtime("plain_gte",&[current.into(),limit.into()]);
    self.guard_value(finished)?;
    let done=self.truth_of(finished.into());
    let increment=self.ctx.append_basic_block(fun,"repeat_increment");
    self.builder.build_conditional_branch(done,end,increment).unwrap();
    self.builder.position_at_end(increment);
    let one=self.runtime("plain_int",&[self.ctx.i64_type().const_int(1,false).into()]);
    let next=self.runtime("plain_add",&[current.into(),one.into()]);
    self.builder.build_store(slot,next).unwrap();
    self.builder.build_unconditional_branch(head).unwrap();
    self.builder.position_at_end(end);
    self.pop_scope();
    Ok(())
   },
   Repeat::Each{name,iter,body}=>self.emit_repeat_each_native(name,iter,body),
  }
 }
 fn emit_try(&mut self,body:&[Stmt],catch:&str,cb:&[Stmt])->Result<(),CompileError>{
  let fun=self.builder.get_insert_block().unwrap().get_parent().unwrap();
  let catchb=self.ctx.append_basic_block(fun,"catch");
  let end=self.ctx.append_basic_block(fun,"try_end");
  let slot=self.entry_slot("error");
  let old=self.error_slot.replace(slot);
  self.catch_stack.push(catchb);
  self.push_scope();
  // Every statement guards its own error results (see guard_value), including nested blocks.
  for s in body{self.emit_stmt(s)?;}
  self.pop_scope();
  self.catch_stack.pop();
  self.error_slot=old;
  if self.builder.get_insert_block().unwrap().get_terminator().is_none(){self.builder.build_unconditional_branch(end).unwrap();}
  self.builder.position_at_end(catchb);
  self.push_scope();
  let es=self.entry_slot(catch);
  let caught=self.builder.build_load(self.ptr,slot,"error").unwrap();
  self.builder.build_store(es,caught).unwrap();
  self.vars.last_mut().unwrap().insert(catch.to_string(),es);
  for s in cb{self.emit_stmt(s)?;}
  self.pop_scope();
  if self.builder.get_insert_block().unwrap().get_terminator().is_none(){self.builder.build_unconditional_branch(end).unwrap();}
  self.builder.position_at_end(end);
  Ok(())
 }
 fn guard(&mut self,v:PointerValue<'ctx>,catch:inkwell::basic_block::BasicBlock<'ctx>)->Result<(),CompileError>{let t=self.builder.build_call(self.runtime["plain_is_error"],&[v.into()],"is_error").unwrap().try_as_basic_value().left().unwrap().into_int_value();let c=self.builder.build_int_compare(IntPredicate::NE,t,self.ctx.i8_type().const_zero(),"error_cond").unwrap();let cont=self.ctx.append_basic_block(self.builder.get_insert_block().unwrap().get_parent().unwrap(),"try_continue");if let Some(slot)=self.error_slot{self.builder.build_store(slot,v).unwrap();}self.builder.build_conditional_branch(c,catch,cont).unwrap();self.builder.position_at_end(cont);Ok(())}
 fn emit_expr_dynamic(&mut self,e:&Expr)->Result<V<'ctx>,CompileError>{
  match e{
   Expr::Int(n,_)=>Ok(self.runtime("plain_int",&[self.ctx.i64_type().const_int(*n as u64,true).into()])),
   Expr::Float(n,_)=>Ok(self.runtime("plain_float",&[self.ctx.f64_type().const_float(*n).into()])),
   Expr::Bool(b,_)=>Ok(self.runtime("plain_bool",&[self.ctx.i8_type().const_int(*b as u64,false).into()])),
   Expr::Str(s,_)=>Ok(self.const_string(s).into()),
   Expr::Var(n,sp)=>{
    // Locals and parameters shadow fields of the current object.
    let local=self.vars.iter().rev().find_map(|m|m.get(n)).copied();
    if let Some(slot)=local{
     Ok(self.builder.build_load(self.ptr,slot,n).unwrap())
    }else if self.field_names.last().map(|f|f.contains(n)).unwrap_or(false){
     let self_slot=self.vars.iter().rev().find_map(|m|m.get("self")).copied().unwrap();
     let obj=self.builder.build_load(self.ptr,self_slot,"self").unwrap();
     let key=self.const_string(n);
     Ok(self.runtime("plain_dict_get",&[obj.into(),key.into()]))
    }else{
     Err(CompileError::new(*sp,format!("unknown variable `{}`",n)))
    }
   },
   Expr::List(xs,_)=>{
    let vs=xs.iter().map(|x|self.emit_expr(x)).collect::<Result<Vec<_>,_>>()?;
    let a=self.alloc_values(&vs,"list");
    Ok(self.runtime("plain_list_new",&[self.ctx.i64_type().const_int(vs.len() as u64,false).into(),a.into()]))
   },
   Expr::Dict(ps,_)=>{
    let mut vs=Vec::new();
    for(k,v)in ps{vs.push(self.emit_expr(k)?);vs.push(self.emit_expr(v)?);}
    let a=self.alloc_values(&vs,"dict");
    Ok(self.runtime("plain_dict_new",&[self.ctx.i64_type().const_int((vs.len()/2) as u64,false).into(),a.into()]))
   },
   Expr::Unary{op,expr,..}=>{
    let v=self.emit_expr(expr)?;
    if matches!(op,Unary::Not){
     let args=[v.into()];
     Ok(self.runtime("plain_not",&args))
    }else{
     let zero=self.runtime("plain_int",&[self.ctx.i64_type().const_zero().into()]);
     let args=[zero.into(),v.into()];
     Ok(self.runtime("plain_sub",&args))
    }
   },
   Expr::Binary{left,op,right,..} if matches!(op,Binary::And|Binary::Or)=>{
    // Short-circuit evaluation: the right operand only runs when the left one does not decide the result.
    let is_and=matches!(op,Binary::And);
    let fun=self.builder.get_insert_block().unwrap().get_parent().unwrap();
    let a=self.emit_expr(left)?.into_pointer_value();
    let is_err=self.builder.build_call(self.runtime["plain_is_error"],&[a.into()],"left_is_error").unwrap().try_as_basic_value().left().unwrap().into_int_value();
    let err_flag=self.builder.build_int_compare(IntPredicate::NE,is_err,self.ctx.i8_type().const_zero(),"left_error").unwrap();
    let truth=self.builder.build_call(self.runtime["plain_truth"],&[a.into()],"left_truth").unwrap().try_as_basic_value().left().unwrap().into_int_value();
    let decided=self.builder.build_int_compare(if is_and{IntPredicate::EQ}else{IntPredicate::NE},truth,self.ctx.i8_type().const_zero(),"left_decides").unwrap();
    let short=self.builder.build_or(err_flag,decided,"short_circuit").unwrap();
    let short_b=self.ctx.append_basic_block(fun,"sc_short");
    let rhs_b=self.ctx.append_basic_block(fun,"sc_rhs");
    let end=self.ctx.append_basic_block(fun,"sc_end");
    self.builder.build_conditional_branch(short,short_b,rhs_b).unwrap();
    let helper=if is_and{"plain_and"}else{"plain_or"};
    self.builder.position_at_end(short_b);
    let short_v=self.runtime(helper,&[a.into(),a.into()]);
    let short_end=self.builder.get_insert_block().unwrap();
    self.builder.build_unconditional_branch(end).unwrap();
    self.builder.position_at_end(rhs_b);
    let b=self.emit_expr(right)?;
    let full=self.runtime(helper,&[a.into(),b.into()]);
    let rhs_end=self.builder.get_insert_block().unwrap();
    self.builder.build_unconditional_branch(end).unwrap();
    self.builder.position_at_end(end);
    let phi=self.builder.build_phi(self.ptr,"sc_result").unwrap();
    phi.add_incoming(&[(&short_v,short_end),(&full,rhs_end)]);
    Ok(phi.as_basic_value())
   },
   Expr::Binary{left,op,right,..}=>{
    let a=self.emit_expr(left)?;
    let b=self.emit_expr(right)?;
    Ok(self.runtime(match op{Binary::Add=>"plain_add",Binary::Sub=>"plain_sub",Binary::Mul=>"plain_mul",Binary::Div=>"plain_div",Binary::Mod=>"plain_mod",Binary::Eq=>"plain_eq",Binary::NotEq=>"plain_not_eq",Binary::Lt=>"plain_lt",Binary::Lte=>"plain_lte",Binary::Gt=>"plain_gt",Binary::Gte=>"plain_gte",Binary::And=>"plain_and",Binary::Or=>"plain_or"},&[a.into(),b.into()]))
   },
   Expr::Index{object,index,..}=>{
    let o=self.emit_expr(object)?;
    let i=self.emit_expr(index)?;
    Ok(self.runtime("plain_list_get",&[o.into(),i.into()]))
   },
   Expr::Member{object,name,..}=>{
    let o=self.emit_expr(object)?;
    let k=self.const_string(name);
    let args=[o.into(),k.into()];
    Ok(self.runtime("plain_dict_get",&args))
   },
   Expr::Ternary{yes,cond,no,..}=>{
    let fun=self.builder.get_insert_block().unwrap().get_parent().unwrap();
    let tb=self.ctx.append_basic_block(fun,"ternary_yes");
    let nb=self.ctx.append_basic_block(fun,"ternary_no");
    let end=self.ctx.append_basic_block(fun,"ternary_end");
    let c=self.emit_expr(cond)?.into_pointer_value();
    self.guard_value(c)?;
    let t=self.truth_of(c.into());
    self.builder.build_conditional_branch(t,tb,nb).unwrap();
    self.builder.position_at_end(tb);
    let y=self.emit_expr(yes)?;
    // The branch may have created new blocks, so the phi must use the block we ended in.
    let y_end=self.builder.get_insert_block().unwrap();
    self.builder.build_unconditional_branch(end).unwrap();
    self.builder.position_at_end(nb);
    let n=self.emit_expr(no)?;
    let n_end=self.builder.get_insert_block().unwrap();
    self.builder.build_unconditional_branch(end).unwrap();
    self.builder.position_at_end(end);
    let phi=self.builder.build_phi(self.ptr,"ternary").unwrap();
    phi.add_incoming(&[(&y,y_end),(&n,n_end)]);
    Ok(phi.as_basic_value())
   },
   Expr::Call{callee,args,..}=>{
    let av=args.iter().map(|a|{let v=self.emit_expr(a)?;let v=self.runtime("plain_promote",&[v.into()]);Ok::<BasicMetadataValueEnum,CompileError>(v.into())}).collect::<Result<Vec<BasicMetadataValueEnum>,_>>()?;
    match &**callee{
     Expr::Var(n,sp)=>{
      if let Some(class_name)=n.strip_prefix("IS::"){
       if !self.classes.contains_key(class_name){return Err(CompileError::new(*sp,format!("unknown class `{}`",class_name)))}
       let mut names:Vec<String>=self.classes.keys().filter(|c|self.is_subclass(c.as_str(),class_name)).cloned().collect();
       names.sort();
       let values:Vec<V<'ctx>>=names.iter().map(|nm|self.const_string(nm).into()).collect();
       let array=self.alloc_values(&values,"class_names");
       let list=self.runtime("plain_list_new",&[self.ctx.i64_type().const_int(values.len() as u64,false).into(),array.into()]);
       Ok(self.runtime("plain_is_instance",&[av[0],list.into()]))
      }else if let Some(class_name)=n.strip_prefix("NEW::"){
       if !self.classes.contains_key(class_name){return Err(CompileError::new(*sp,format!("unknown class `{}`",class_name)))}
       let instance=self.runtime("plain_new_instance",&[]);
       let tag_key=self.const_string("__class__");
       let tag_value=self.const_string(class_name);
       self.runtime("plain_dict_set",&[instance.into(),tag_key.into(),tag_value.into()]);
       let init_key=format!("{}__#init",class_name);
       self.call_user(&init_key,&[instance.into()]);
       if let Some(key)=self.method_key(class_name,"NEW"){
        self.check_arity(&key,av.len()+1,*sp,1)?;
        let mut ctor_args=vec![instance.into()];
        ctor_args.extend(av);
        let r=self.call_user(&key,&ctor_args).into_pointer_value();
        self.guard_value(r)?;
       }else if !av.is_empty(){
        return Err(CompileError::new(*sp,format!("class `{}` has no constructor (TO NEW) but {} argument(s) were given",class_name,av.len())))
       }
       Ok(instance)
      }else{
       let builtin=match n.as_str(){"APPEND"=>"plain_append","REMOVE"=>"plain_remove","LENGTH"=>"plain_length","UPPER"=>"plain_upper","LOWER"=>"plain_lower","SPLIT"=>"plain_split","ABS"=>"plain_abs","SQRT"=>"plain_sqrt","POW"=>"plain_pow","FLOOR"=>"plain_floor","CEIL"=>"plain_ceil","SIN"=>"plain_sin","COS"=>"plain_cos","CONTAINS"=>"plain_contains","STARTS_WITH"=>"plain_starts_with","ENDS_WITH"=>"plain_ends_with","JOIN"=>"plain_join","RANGE"=>"plain_range","READ_FILE"=>"plain_read_file","WRITE_FILE"=>"plain_write_file","ENV"=>"plain_env","READ_LINES"=>"plain_read_lines","NOW_MS"=>"plain_now_ms","MOD"=>"plain_mod",_=>STDLIB.iter().find(|(name,_,_)|*name==n.as_str()&&!self.functions.contains_key(n.as_str())).map(|(_,symbol,_)|*symbol).unwrap_or(n.as_str())};
       if let Some(f)=self.runtime.get(builtin).copied(){
        if f.count_params() as usize!=av.len(){return Err(CompileError::new(*sp,format!("`{}` expects {} argument(s), got {}",n,f.count_params(),av.len())))}
        Ok(self.builder.build_call(f,&av,"builtin").unwrap().try_as_basic_value().left().unwrap())
       }else if let Some(ext)=self.externs.get(n){
        if av.len()!=ext.params.len(){return Err(CompileError::new(*sp,format!("external function `{}` expects {} arguments, got {}",n,ext.params.len(),av.len())))}
        for (arg,(_,ty)) in args.iter().zip(&ext.params){if !Self::ffi_compatible(arg,ty){return Err(CompileError::new(arg.span(),format!("argument type does not match external function `{}`",n)))}}
        Ok(self.call_user(n,&av))
       }else if self.functions.contains_key(n){
        self.check_arity(n,av.len(),*sp,0)?;
        Ok(self.call_user(n,&av))
       }else if self.current_class.clone().filter(|c|self.method_key(c,n).is_some()).is_some(){
        let self_slot=self.vars.iter().rev().find_map(|m|m.get("self")).copied().ok_or_else(||CompileError::new(*sp,"methods can only be called without `self.` inside a class"))?;
        let object=self.builder.build_load(self.ptr,self_slot,"self").unwrap();
        let mut args2=vec![object.into()];
        args2.extend(av);
        self.emit_method_call(*sp,object,n,args2)
       }else{
        Err(CompileError::new(*sp,format!("unknown function `{}`",n)))
       }
      }
     },
     Expr::Member{object,name,..}=>{
      if let Expr::Var(object_name,_)=&**object{
       if object_name.eq_ignore_ascii_case("super"){return self.emit_super_call(callee.span(),name,av)}
      }
      let o=self.emit_expr(object)?;
      let o=self.runtime("plain_promote",&[o.into()]);
      let mut args2=vec![o.into()];
      args2.extend(av);
      self.emit_method_call(callee.span(),o,name,args2)
     },
     _=>Err(CompileError::new(callee.span(),"invalid call target")),
    }
   },
  }
 }
 fn ffi_compatible(expr:&Expr,ty:&Type)->bool{match (expr,ty){(Expr::Int(..),Type::Int)|(Expr::Float(..),Type::Float)|(Expr::Bool(..),Type::Bool)|(Expr::Str(..),Type::String)=>true,(_,Type::Any)=>true,(Expr::Int(..),_)|(Expr::Float(..),_)|(Expr::Bool(..),_)|(Expr::Str(..),_)=>false,(_,_)=>true}}
 fn const_string(&mut self,s:&str)->PointerValue<'ctx>{let g=self.builder.build_global_string_ptr(s,"str").unwrap();self.runtime("plain_string",&[g.as_pointer_value().into()]).into_pointer_value()}
}

fn native_integer_literal(e:&Expr)->Option<i64>{match e{Expr::Int(n,_)=>Some(*n),Expr::Unary{op:Unary::Neg,expr,..}=>native_integer_literal(expr).map(i64::wrapping_neg),_=>None}}
fn native_nonzero_int_literal(e:&Expr)->bool{matches!(native_integer_literal(e),Some(n) if n!=0&&n!=-1)}
fn native_int_syntax(e:&Expr,names:&HashSet<String>)->bool{match e{Expr::Int(..)=>true,Expr::Var(name,_)=>names.contains(name),Expr::Unary{op:Unary::Neg,expr,..}=>native_int_syntax(expr,names),Expr::Binary{left,op,right,..}=>{let arithmetic=matches!(op,Binary::Add|Binary::Sub|Binary::Mul);let modulo=matches!(op,Binary::Mod)&&native_nonzero_int_literal(right);(arithmetic||modulo)&&native_int_syntax(left,names)&&native_int_syntax(right,names)},Expr::Call{callee,args,..}=>args.is_empty()&&matches!(callee.as_ref(),Expr::Var(name,_) if name=="NOW_MS"),_=>false}}
fn gather_native_facts(stmts:&[Stmt],lets:&mut Vec<(String,Option<Type>,Expr)>,loops:&mut Vec<(String,Expr,Expr)>,sets:&mut Vec<(String,Expr)>){for stmt in stmts{match stmt{Stmt::Let(names,values,_)=>for((name,ty),value)in names.iter().zip(values){lets.push((name.clone(),ty.clone(),value.clone()));},Stmt::Set(lhs,rhs,_)=>for(target,value)in lhs.iter().zip(rhs){if let Expr::Var(name,_)=target{sets.push((name.clone(),value.clone()));}},Stmt::If{branches,else_body,..}=>{for(_,block)in branches{gather_native_facts(block,lets,loops,sets)}gather_native_facts(else_body,lets,loops,sets)},Stmt::Repeat(Repeat::Counted{name,start,end,body},_)=>{loops.push((name.clone(),start.clone(),end.clone()));gather_native_facts(body,lets,loops,sets)},Stmt::Repeat(Repeat::While{body,..}|Repeat::Each{body,..},_)=>gather_native_facts(body,lets,loops,sets),Stmt::Try{body,catch_body,..}=>{gather_native_facts(body,lets,loops,sets);gather_native_facts(catch_body,lets,loops,sets)},_=>{}}}}
fn find_native_int_candidates(program:&Program)->HashSet<String>{let mut lets=Vec::new();let mut loops=Vec::new();let mut sets=Vec::new();for item in &program.items{match item{Item::Stmt(stmt)=>gather_native_facts(std::slice::from_ref(stmt),&mut lets,&mut loops,&mut sets),Item::Function(f)=>gather_native_facts(&f.body,&mut lets,&mut loops,&mut sets),Item::Class(c)=>for m in &c.methods{gather_native_facts(&m.body,&mut lets,&mut loops,&mut sets)},_=>{}}}let mut names=HashSet::new();loop{let before=names.len();for(name,ty,value)in &lets{let annotation_allows=ty.as_ref().map(|t|matches!(t,Type::Int)).unwrap_or(true);if annotation_allows&&native_int_syntax(value,&names){names.insert(name.clone());}}for(name,start,end)in &loops{if native_int_syntax(start,&names)&&native_int_syntax(end,&names){names.insert(name.clone());}}if names.len()==before{break}}loop{let invalid:HashSet<String>=sets.iter().filter(|(name,value)|names.contains(name)&&!native_int_syntax(value,&names)).map(|(name,_)|name.clone()).collect();if invalid.is_empty(){break}let before=names.len();names.retain(|name|!invalid.contains(name));if names.len()==before{break}}names}
