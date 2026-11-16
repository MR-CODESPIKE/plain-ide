use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::os::raw::{c_char, c_void};
use std::ptr;

pub mod stdlib;

#[repr(C)]
pub struct PlainValue {
    pub kind: i32,
    pub i: i64,
    pub f: f64,
    pub b: i8,
    pub ptr: *mut c_void,
}

pub const K_INT: i32 = 1;
pub const K_FLOAT: i32 = 2;
pub const K_BOOL: i32 = 3;
pub const K_STRING: i32 = 4;
pub const K_LIST: i32 = 5;
pub const K_DICT: i32 = 6;
pub const K_ERROR: i32 = 7;
pub const K_BORROWED_STRING: i32 = 8;
pub const K_LINE_ITER: i32 = 9;
pub const K_HANDLE: i32 = 10;

/// Largest list that RANGE will build (each item is a heap allocation).
const MAX_RANGE_ITEMS: i128 = 10_000_000;
const MAX_COLLECTION_ITEMS: i64 = 1_000_000;
/// Nesting depth after which formatting and equality stop descending (guards against self-containing lists).
const MAX_NESTING: usize = 16;

struct List { items: Vec<*mut PlainValue> }
struct Dict { items: HashMap<String, *mut PlainValue> }
struct LineReader {
    reader: BufReader<File>,
    current: PlainValue,
    line: String,
    eof: bool,
    error: *mut PlainValue,
}

fn value(kind: i32) -> *mut PlainValue { Box::into_raw(Box::new(PlainValue { kind, i: 0, f: 0.0, b: 0, ptr: ptr::null_mut() })) }
fn string_value(kind: i32, s: &str) -> *mut PlainValue {
    let p = CString::new(s.replace('\0', "\\0")).unwrap_or_default();
    let v = value(kind);
    unsafe { (*v).ptr = Box::into_raw(Box::new(p)) as *mut c_void; }
    v
}
fn is_string_value(v: *mut PlainValue) -> bool {
    if v.is_null() { return false; }
    unsafe { matches!((*v).kind, K_STRING | K_BORROWED_STRING) }
}
unsafe fn string_view<'a>(v: *mut PlainValue) -> Option<&'a str> {
    if v.is_null() { return None; }
    match (*v).kind {
        K_STRING | K_ERROR => {
            let p = (*v).ptr as *mut CString;
            if p.is_null() { None } else { (*p).to_str().ok() }
        }
        K_BORROWED_STRING => {
            let p = (*v).ptr as *mut String;
            if p.is_null() { None } else { Some((&*p).as_str()) }
        }
        _ => None,
    }
}
fn read_string(v: *mut PlainValue) -> String {
    if v.is_null() { return String::new(); }
    unsafe {
        match (*v).kind {
            K_BORROWED_STRING => string_view(v).unwrap_or("").to_owned(),
            K_STRING | K_ERROR => {
                let p = (*v).ptr as *mut CString;
                if p.is_null() { String::new() } else { (*p).to_string_lossy().into_owned() }
            }
            _ => String::new(),
        }
    }
}
fn promote_value(v: *mut PlainValue) -> *mut PlainValue {
    if v.is_null() { return v; }
    unsafe {
        if (*v).kind == K_BORROWED_STRING {
            return string_value(K_STRING, string_view(v).unwrap_or(""));
        }
    }
    v
}
#[no_mangle] pub extern "C" fn plain_promote(v: *mut PlainValue) -> *mut PlainValue { promote_value(v) }
fn type_error(msg: &str) -> *mut PlainValue { string_value(K_ERROR, msg) }

#[no_mangle] pub extern "C" fn plain_int(n: i64) -> *mut PlainValue { let v=value(K_INT); unsafe{(*v).i=n}; v }
#[no_mangle] pub extern "C" fn plain_float(n: f64) -> *mut PlainValue { let v=value(K_FLOAT); unsafe{(*v).f=n}; v }
#[no_mangle] pub extern "C" fn plain_bool(b: i8) -> *mut PlainValue { let v=value(K_BOOL); unsafe{(*v).b=if b!=0{1}else{0}}; v }
#[no_mangle] pub extern "C" fn plain_string(s: *const c_char) -> *mut PlainValue { if s.is_null(){string_value(K_STRING,"")}else{string_value(K_STRING,unsafe{CStr::from_ptr(s)}.to_string_lossy().as_ref())} }
#[no_mangle] pub extern "C" fn plain_error(s: *const c_char) -> *mut PlainValue { if s.is_null(){type_error("Plain error")}else{string_value(K_ERROR,unsafe{CStr::from_ptr(s)}.to_string_lossy().as_ref())} }
/// Only error values count as errors. `null` (the result of a function that returns nothing) is a normal value.
#[no_mangle] pub extern "C" fn plain_is_error(v:*mut PlainValue)->i8 { is_err(v) as i8 }
fn is_err(v:*mut PlainValue)->bool { !v.is_null() && unsafe{(*v).kind==K_ERROR} }
#[no_mangle] pub extern "C" fn plain_fail(v:*mut PlainValue)->*mut PlainValue { if is_err(v) { v } else { string_value(K_ERROR,&value_to_string(v)) } }
#[no_mangle] pub extern "C" fn plain_error_message(v:*mut PlainValue)->*mut PlainValue { if is_err(v){string_value(K_STRING,&read_string(v))}else{string_value(K_STRING,"not an error")} }

/// `value IS Class`: true when `obj` is an instance whose class name is in `names` (the class and all its subclasses).
#[no_mangle] pub extern "C" fn plain_is_instance(obj:*mut PlainValue,names:*mut PlainValue)->*mut PlainValue { if obj.is_null()||names.is_null(){return plain_bool(0)} unsafe{ if (*obj).kind!=K_DICT||(*names).kind!=K_LIST{return plain_bool(0)} let d=&*((*obj).ptr as *mut Dict); let tag=match d.items.get("__class__"){Some(t)=>read_string(*t),None=>return plain_bool(0)}; let l=&*((*names).ptr as *mut List); plain_bool(l.items.iter().any(|n|read_string(*n)==tag) as i8) } }

/// Handles wrap native objects (models, agents). The tag identifies the kind of object, so a handle of one kind is never used as another.
#[no_mangle] pub extern "C" fn plain_handle(ptr:*mut c_void,tag:i64)->*mut PlainValue { let v=value(K_HANDLE); unsafe{(*v).ptr=ptr;(*v).i=tag}; v }
/// Returns the native pointer if `v` is a live handle with this tag, otherwise null.
#[no_mangle] pub extern "C" fn plain_handle_ptr(v:*mut PlainValue,tag:i64)->*mut c_void { if v.is_null(){return ptr::null_mut()} unsafe{if (*v).kind==K_HANDLE&&(*v).i==tag{(*v).ptr}else{ptr::null_mut()}} }
/// Like `plain_handle_ptr`, but also invalidates the handle so it cannot be used (or freed) twice.
#[no_mangle] pub extern "C" fn plain_handle_clear(v:*mut PlainValue,tag:i64)->*mut c_void { let p=plain_handle_ptr(v,tag); if !p.is_null(){unsafe{(*v).ptr=ptr::null_mut()}} p }

#[no_mangle] pub extern "C" fn plain_print(v:*mut PlainValue) {
    let text=if v.is_null(){"null".to_string()}else{unsafe{match (*v).kind{K_ERROR=>format!("Error: {}",read_string(v)),_=>format_value(v,0)}}};
    // A closed stdout (for example `| head`) must not abort the program.
    let _=writeln!(std::io::stdout(),"{}",text);
}
/// The text of a string value, or `None` for any other kind of value (for other crates that read Plain values).
pub fn value_text(v:*mut PlainValue)->Option<String>{ if is_string_value(v){Some(read_string(v))}else{None} }
fn format_value(v:*mut PlainValue,depth:usize)->String{
    if v.is_null(){return "null".into()}
    if depth>MAX_NESTING{return "...".into()}
    unsafe{match (*v).kind{
        K_INT=>(*v).i.to_string(),
        K_FLOAT=>(*v).f.to_string(),
        K_BOOL=>if (*v).b!=0{"TRUE".into()}else{"FALSE".into()},
        K_STRING|K_BORROWED_STRING=>read_string(v),
        K_ERROR=>format!("Error: {}",read_string(v)),
        K_LIST=>{let l=&*((*v).ptr as *mut List);format!("[{}]",l.items.iter().map(|x|format_value(*x,depth+1)).collect::<Vec<_>>().join(", "))}
        K_DICT=>{let d=&*((*v).ptr as *mut Dict);let class_name=d.items.get("__class__").map(|t|read_string(*t)).unwrap_or_default();let mut keys:Vec<&String>=d.items.keys().filter(|k|!k.starts_with("__")).collect();keys.sort();format!("{}{{{}}}",class_name,keys.iter().map(|k|format!("{}: {}",k,format_value(d.items[*k],depth+1))).collect::<Vec<_>>().join(", "))}
        K_HANDLE=>"<handle>".into(),
        K_LINE_ITER=>"<line iterator>".into(),
        _=>"<value>".into(),
    }}
}
/// Text used when a value is joined into a string (`"x = " + x`). Error values contribute just their message.
fn value_to_string(v:*mut PlainValue)->String{if v.is_null(){return "null".into()} unsafe{match (*v).kind{K_ERROR=>read_string(v),_=>format_value(v,0)}}}

fn num(a:*mut PlainValue)->Result<f64,*mut PlainValue>{if a.is_null(){Err(type_error("null is not a number"))}else{unsafe{match (*a).kind{K_INT=>Ok((*a).i as f64),K_FLOAT=>Ok((*a).f),_=>Err(type_error("expected a number"))}}}}
type Result<T,E= *mut PlainValue>=std::result::Result<T,E>;
fn both_int(a:*mut PlainValue,b:*mut PlainValue)->Option<(i64,i64)>{ if a.is_null()||b.is_null(){return None} unsafe{if (*a).kind==K_INT&&(*b).kind==K_INT{Some(((*a).i,(*b).i))}else{None}} }
#[no_mangle] pub extern "C" fn plain_add(a:*mut PlainValue,b:*mut PlainValue)->*mut PlainValue{if is_string_value(a)||is_string_value(b) {return string_value(K_STRING,&format!("{}{}",value_to_string(a),value_to_string(b)))}; if is_err(a){return a} if is_err(b){return b} if let Some((x,y))=both_int(a,b){return plain_int(x.wrapping_add(y))} match (num(a),num(b)){(Ok(x),Ok(y))=>plain_float(x+y), (Err(e),_)|( _,Err(e))=>e}}
#[no_mangle] pub extern "C" fn plain_sub(a:*mut PlainValue,b:*mut PlainValue)->*mut PlainValue{if is_err(a){return a} if is_err(b){return b} if let Some((x,y))=both_int(a,b){return plain_int(x.wrapping_sub(y))} match(num(a),num(b)){(Ok(x),Ok(y))=>plain_float(x-y),(Err(e),_)|(_,Err(e))=>e}}
#[no_mangle] pub extern "C" fn plain_mul(a:*mut PlainValue,b:*mut PlainValue)->*mut PlainValue{if is_err(a){return a} if is_err(b){return b} if let Some((x,y))=both_int(a,b){return plain_int(x.wrapping_mul(y))} match(num(a),num(b)){(Ok(x),Ok(y))=>plain_float(x*y),(Err(e),_)|(_,Err(e))=>e}}
#[no_mangle] pub extern "C" fn plain_div(a:*mut PlainValue,b:*mut PlainValue)->*mut PlainValue{if is_err(a){return a} if is_err(b){return b} if let Some((x,y))=both_int(a,b){if y==0{return type_error("Cannot divide by zero")} return plain_int(x.wrapping_div(y))} match(num(a),num(b)){(Ok(x),Ok(y))=>{if y==0.0{type_error("Cannot divide by zero")}else{plain_float(x/y)}},(Err(e),_)|(_,Err(e))=>e}}
fn values_equal(a:*mut PlainValue,b:*mut PlainValue,depth:usize)->bool{
    if a==b{return true}
    if a.is_null()||b.is_null()||depth>MAX_NESTING{return false}
    unsafe{
        if is_string_value(a)&&is_string_value(b){return string_view(a).unwrap_or("")==string_view(b).unwrap_or("")}
        match ((*a).kind,(*b).kind){
            (K_INT,K_INT)=>(*a).i==(*b).i,
            (K_INT,K_FLOAT)=>((*a).i as f64)==(*b).f,
            (K_FLOAT,K_INT)=>(*a).f==((*b).i as f64),
            (K_FLOAT,K_FLOAT)=>(*a).f==(*b).f,
            (K_BOOL,K_BOOL)=>((*a).b!=0)==((*b).b!=0),
            (K_LIST,K_LIST)=>{let (x,y)=(&*((*a).ptr as *mut List),&*((*b).ptr as *mut List));x.items.len()==y.items.len()&&x.items.iter().zip(&y.items).all(|(p,q)|values_equal(*p,*q,depth+1))}
            (K_DICT,K_DICT)=>{let (x,y)=(&*((*a).ptr as *mut Dict),&*((*b).ptr as *mut Dict));x.items.len()==y.items.len()&&x.items.iter().all(|(k,p)|y.items.get(k).map(|q|values_equal(*p,*q,depth+1)).unwrap_or(false))}
            _=>false,
        }
    }
}
/// Orders two numbers or two strings. `Ok(None)` means "unordered" (NaN); `Err` carries an error value to return.
fn order_of(a:*mut PlainValue,b:*mut PlainValue)->Result<Option<std::cmp::Ordering>>{
    if is_err(a){return Err(a)}
    if is_err(b){return Err(b)}
    if a.is_null()||b.is_null(){return Err(type_error("cannot compare null"))}
    unsafe{
        if is_string_value(a)&&is_string_value(b){return Ok(Some(string_view(a).unwrap_or("").cmp(string_view(b).unwrap_or(""))))}
        if (*a).kind==K_INT&&(*b).kind==K_INT{return Ok(Some((*a).i.cmp(&(*b).i)))}
        match (num(a),num(b)){
            (Ok(x),Ok(y))=>Ok(x.partial_cmp(&y)),
            _=>Err(type_error("cannot compare values")),
        }
    }
}
fn ordered(a:*mut PlainValue,b:*mut PlainValue,accept:fn(std::cmp::Ordering)->bool)->*mut PlainValue{match order_of(a,b){Ok(Some(o))=>plain_bool(accept(o) as i8),Ok(None)=>plain_bool(0),Err(e)=>e}}
#[no_mangle] pub extern "C" fn plain_eq(a:*mut PlainValue,b:*mut PlainValue)->*mut PlainValue{if is_err(a){return a} if is_err(b){return b} plain_bool(values_equal(a,b,0) as i8)}
#[no_mangle] pub extern "C" fn plain_not_eq(a:*mut PlainValue,b:*mut PlainValue)->*mut PlainValue{if is_err(a){return a} if is_err(b){return b} plain_bool((!values_equal(a,b,0)) as i8)}
#[no_mangle] pub extern "C" fn plain_lt(a:*mut PlainValue,b:*mut PlainValue)->*mut PlainValue{ordered(a,b,|o|o==std::cmp::Ordering::Less)}
#[no_mangle] pub extern "C" fn plain_gt(a:*mut PlainValue,b:*mut PlainValue)->*mut PlainValue{ordered(a,b,|o|o==std::cmp::Ordering::Greater)}
#[no_mangle] pub extern "C" fn plain_lte(a:*mut PlainValue,b:*mut PlainValue)->*mut PlainValue{ordered(a,b,|o|o!=std::cmp::Ordering::Greater)}
#[no_mangle] pub extern "C" fn plain_gte(a:*mut PlainValue,b:*mut PlainValue)->*mut PlainValue{ordered(a,b,|o|o!=std::cmp::Ordering::Less)}
/// Falsy: FALSE, 0, 0.0, "", empty list, empty dictionary, null and errors. Everything else is truthy.
#[no_mangle] pub extern "C" fn plain_truth(a:*mut PlainValue)->i8{if a.is_null(){return 0} unsafe{match (*a).kind{K_BOOL=>if (*a).b!=0{1}else{0},K_INT=>((*a).i!=0) as i8,K_FLOAT=>((*a).f!=0.0) as i8,K_STRING|K_BORROWED_STRING=>(!string_view(a).unwrap_or("").is_empty()) as i8,K_ERROR=>0,K_LIST=>(!(*((*a).ptr as *mut List)).items.is_empty()) as i8,K_DICT=>(!(*((*a).ptr as *mut Dict)).items.is_empty()) as i8,_=>1}}}
#[no_mangle] pub extern "C" fn plain_not(a:*mut PlainValue)->*mut PlainValue{if is_err(a){return a} plain_bool((plain_truth(a)==0) as i8)}
#[no_mangle] pub extern "C" fn plain_and(a:*mut PlainValue,b:*mut PlainValue)->*mut PlainValue{if is_err(a){return a} if is_err(b){return b} plain_bool((plain_truth(a)!=0&&plain_truth(b)!=0) as i8)}
#[no_mangle] pub extern "C" fn plain_or(a:*mut PlainValue,b:*mut PlainValue)->*mut PlainValue{if is_err(a){return a} if is_err(b){return b} plain_bool((plain_truth(a)!=0||plain_truth(b)!=0) as i8)}

#[no_mangle] pub extern "C" fn plain_list_new(n:i64,items:*mut *mut PlainValue)->*mut PlainValue{
    if n < 0 || n > MAX_COLLECTION_ITEMS { return type_error("list size is outside the supported limit"); }
    if n > 0 && items.is_null() { return type_error("list items pointer is null"); }
    let mut v=Vec::with_capacity(n as usize);
    unsafe { for i in 0..n { v.push(promote_value(*items.add(i as usize))); } }
    let out=value(K_LIST); unsafe { (*out).ptr=Box::into_raw(Box::new(List{items:v})) as *mut c_void; } out
}
#[no_mangle] pub extern "C" fn plain_list_get(v:*mut PlainValue,idx:*mut PlainValue)->*mut PlainValue{if v.is_null()||idx.is_null(){return type_error("invalid list access")};unsafe{if (*v).kind!=K_LIST||(*idx).kind!=K_INT{return type_error("list index must be an int")};let l=&*((*v).ptr as *mut List);if (*idx).i<0||(*idx).i>=l.items.len() as i64{type_error("list index out of range")}else{l.items[(*idx).i as usize]}}}
#[no_mangle] pub extern "C" fn plain_list_set(v:*mut PlainValue,idx:*mut PlainValue,item:*mut PlainValue)->*mut PlainValue{if v.is_null()||idx.is_null(){return type_error("invalid list assignment")};unsafe{if (*v).kind!=K_LIST||(*idx).kind!=K_INT{return type_error("list index must be an int")};let l=&mut *((*v).ptr as *mut List);if (*idx).i<0||(*idx).i>=l.items.len() as i64{type_error("list index out of range")}else{l.items[(*idx).i as usize]=promote_value(item);plain_bool(1)}}}
#[no_mangle] pub extern "C" fn plain_append(v:*mut PlainValue,item:*mut PlainValue)->*mut PlainValue{if v.is_null(){return type_error("APPEND needs a list")};unsafe{if (*v).kind!=K_LIST{return type_error("APPEND needs a list")};(*( (*v).ptr as *mut List)).items.push(promote_value(item));plain_bool(1)}}
#[no_mangle] pub extern "C" fn plain_remove(v:*mut PlainValue,idx:*mut PlainValue)->*mut PlainValue{if v.is_null()||idx.is_null(){return type_error("REMOVE needs a list and index")};unsafe{if (*v).kind!=K_LIST||(*idx).kind!=K_INT{return type_error("REMOVE expects list and int")};let l=&mut *((*v).ptr as *mut List);if (*idx).i<0||(*idx).i>=l.items.len() as i64{type_error("list index out of range")}else{l.items.remove((*idx).i as usize)}}}
#[no_mangle] pub extern "C" fn plain_length(v:*mut PlainValue)->*mut PlainValue{if v.is_null(){return type_error("LENGTH needs a value")};unsafe{match (*v).kind{K_STRING|K_BORROWED_STRING=>plain_int(string_view(v).unwrap_or("").chars().count() as i64),K_LIST=>plain_int((*( (*v).ptr as *mut List)).items.len() as i64),K_DICT=>plain_int((*( (*v).ptr as *mut Dict)).items.len() as i64),_=>type_error("LENGTH supports strings, lists, and dictionaries")}}}
#[no_mangle] pub extern "C" fn plain_upper(v:*mut PlainValue)->*mut PlainValue{if v.is_null()||unsafe{!is_string_value(v)}{return type_error("UPPER needs a string")};string_value(K_STRING,&read_string(v).to_uppercase())}
#[no_mangle] pub extern "C" fn plain_lower(v:*mut PlainValue)->*mut PlainValue{if v.is_null()||unsafe{!is_string_value(v)}{return type_error("LOWER needs a string")};string_value(K_STRING,&read_string(v).to_lowercase())}
#[no_mangle] pub extern "C" fn plain_split(v:*mut PlainValue,sep:*mut PlainValue)->*mut PlainValue{if v.is_null()||sep.is_null(){return type_error("SPLIT needs two strings")};unsafe{if !is_string_value(v)||!is_string_value(sep){return type_error("SPLIT needs two strings")};let (text,sep_text)=(read_string(v),read_string(sep));let parts:Vec<*mut PlainValue>=if sep_text.is_empty(){text.chars().map(|c|string_value(K_STRING,&c.to_string())).collect()}else{text.split(sep_text.as_str()).map(|p|string_value(K_STRING,p)).collect()};let out=value(K_LIST);(*out).ptr=Box::into_raw(Box::new(List{items:parts})) as *mut c_void;out}}

#[no_mangle] pub extern "C" fn plain_dict_new(n:i64,pairs:*mut *mut PlainValue)->*mut PlainValue{
    if n < 0 || n > MAX_COLLECTION_ITEMS { return type_error("dictionary size is outside the supported limit"); }
    if n > 0 && pairs.is_null() { return type_error("dictionary pairs pointer is null"); }
    let mut d=HashMap::with_capacity(n as usize);
    unsafe { for i in 0..n { let k=*pairs.add((i*2) as usize); let v=*pairs.add((i*2+1) as usize); d.insert(value_to_string(k),promote_value(v)); } }
    let out=value(K_DICT); unsafe { (*out).ptr=Box::into_raw(Box::new(Dict{items:d})) as *mut c_void; } out
}
#[no_mangle] pub extern "C" fn plain_dict_get(v:*mut PlainValue,key:*mut PlainValue)->*mut PlainValue{if v.is_null()||key.is_null(){return type_error("invalid dictionary access")};unsafe{if (*v).kind!=K_DICT{return type_error("expected a dictionary")};let d=&*((*v).ptr as *mut Dict);d.items.get(&value_to_string(key)).copied().unwrap_or_else(||type_error("dictionary key not found"))}}
#[no_mangle] pub extern "C" fn plain_dict_set(v:*mut PlainValue,key:*mut PlainValue,item:*mut PlainValue)->*mut PlainValue{if v.is_null()||key.is_null(){return type_error("invalid dictionary assignment")};unsafe{if (*v).kind!=K_DICT{return type_error("expected a dictionary")};let d=&mut *((*v).ptr as *mut Dict);d.items.insert(value_to_string(key),promote_value(item));plain_bool(1)}}

#[no_mangle] pub extern "C" fn plain_new_instance()->*mut PlainValue{let out=value(K_DICT);unsafe{(*out).ptr=Box::into_raw(Box::new(Dict{items:HashMap::new()})) as *mut c_void};out}
#[no_mangle] pub extern "C" fn plain_read(prompt:*mut PlainValue,kind:i32)->*mut PlainValue{if !prompt.is_null(){let _=write!(std::io::stdout(),"{}: ",value_to_string(prompt));let _=std::io::stdout().flush();}let mut s=String::new();if std::io::stdin().read_line(&mut s).is_err(){return type_error("input failed")};let s=s.trim_end();match kind{K_INT=>s.parse::<i64>().map(|n|plain_int(n)).unwrap_or_else(|_|type_error("expected an integer")),K_FLOAT=>s.parse::<f64>().map(|n|plain_float(n)).unwrap_or_else(|_|type_error("expected a float")),K_BOOL=>match s.to_uppercase().as_str(){"TRUE"=>plain_bool(1),"FALSE"=>plain_bool(0),_=>type_error("expected TRUE or FALSE")},_=>string_value(K_STRING,s)}}

#[no_mangle] pub extern "C" fn plain_mod(a:*mut PlainValue,b:*mut PlainValue)->*mut PlainValue{if is_err(a){return a} if is_err(b){return b} if let Some((x,y))=both_int(a,b){if y==0{return type_error("MOD by zero")} return plain_int(x.wrapping_rem(y))} match(num(a),num(b)){(Ok(x),Ok(y))=>{if y==0.0{type_error("MOD by zero")}else{plain_float(x%y)}},(Err(e),_)|(_,Err(e))=>e}}
#[no_mangle] pub extern "C" fn plain_abs(v:*mut PlainValue)->*mut PlainValue{if is_err(v){return v} if !v.is_null()&&unsafe{(*v).kind==K_INT}{return plain_int(unsafe{(*v).i}.wrapping_abs())} match num(v){Ok(x)=>plain_float(x.abs()),Err(e)=>e}}
#[no_mangle] pub extern "C" fn plain_sqrt(v:*mut PlainValue)->*mut PlainValue{match num(v){Ok(x)=>if x<0.0{type_error("SQRT needs a non-negative number")}else{plain_float(x.sqrt())},Err(e)=>e}}
#[no_mangle] pub extern "C" fn plain_pow(a:*mut PlainValue,b:*mut PlainValue)->*mut PlainValue{match(num(a),num(b)){(Ok(x),Ok(y))=>plain_float(x.powf(y)),(Err(e),_)|(_,Err(e))=>e}}
#[no_mangle] pub extern "C" fn plain_floor(v:*mut PlainValue)->*mut PlainValue{if is_err(v){return v} if !v.is_null()&&unsafe{(*v).kind==K_INT}{return plain_int(unsafe{(*v).i})} match num(v){Ok(x)=>plain_int(x.floor() as i64),Err(e)=>e}}
#[no_mangle] pub extern "C" fn plain_ceil(v:*mut PlainValue)->*mut PlainValue{if is_err(v){return v} if !v.is_null()&&unsafe{(*v).kind==K_INT}{return plain_int(unsafe{(*v).i})} match num(v){Ok(x)=>plain_int(x.ceil() as i64),Err(e)=>e}}
#[no_mangle] pub extern "C" fn plain_sin(v:*mut PlainValue)->*mut PlainValue{match num(v){Ok(x)=>plain_float(x.sin()),Err(e)=>e}}
#[no_mangle] pub extern "C" fn plain_cos(v:*mut PlainValue)->*mut PlainValue{match num(v){Ok(x)=>plain_float(x.cos()),Err(e)=>e}}
#[no_mangle] pub extern "C" fn plain_contains(v:*mut PlainValue,needle:*mut PlainValue)->*mut PlainValue{if !is_string_value(v)||!is_string_value(needle){return type_error("CONTAINS needs two strings")};let (hay,part)=unsafe{(string_view(v).unwrap_or(""),string_view(needle).unwrap_or(""))};plain_bool(hay.contains(part) as i8)}
#[no_mangle] pub extern "C" fn plain_starts_with(v:*mut PlainValue,needle:*mut PlainValue)->*mut PlainValue{if !is_string_value(v)||!is_string_value(needle){return type_error("STARTS_WITH needs two strings")};let (hay,part)=unsafe{(string_view(v).unwrap_or(""),string_view(needle).unwrap_or(""))};plain_bool(hay.starts_with(part) as i8)}
#[no_mangle] pub extern "C" fn plain_ends_with(v:*mut PlainValue,needle:*mut PlainValue)->*mut PlainValue{if !is_string_value(v)||!is_string_value(needle){return type_error("ENDS_WITH needs two strings")};let (hay,part)=unsafe{(string_view(v).unwrap_or(""),string_view(needle).unwrap_or(""))};plain_bool(hay.ends_with(part) as i8)}
#[no_mangle] pub extern "C" fn plain_contains_native(v:*mut PlainValue,needle:*const c_char)->i8{if !is_string_value(v)||needle.is_null(){return 0};unsafe{let hay=string_view(v).unwrap_or("");let part=CStr::from_ptr(needle).to_str().unwrap_or("");hay.contains(part) as i8}}
#[no_mangle] pub extern "C" fn plain_string_empty_native(v:*mut PlainValue)->i8{if !is_string_value(v){return 0};unsafe{string_view(v).unwrap_or("").is_empty() as i8}}
#[no_mangle] pub extern "C" fn plain_join(v:*mut PlainValue,sep:*mut PlainValue)->*mut PlainValue{if v.is_null()||sep.is_null(){return type_error("JOIN needs a list and separator")};unsafe{if (*v).kind!=K_LIST||!is_string_value(sep){return type_error("JOIN needs a list and separator")};let l=&*((*v).ptr as *mut List);string_value(K_STRING,&l.items.iter().map(|x|value_to_string(*x)).collect::<Vec<_>>().join(&read_string(sep)))}}
#[no_mangle] pub extern "C" fn plain_range(start:*mut PlainValue,end:*mut PlainValue)->*mut PlainValue{
    let as_int=|v:*mut PlainValue|->std::result::Result<i64,*mut PlainValue>{if v.is_null(){return Err(type_error("RANGE needs numbers"))}unsafe{match (*v).kind{K_INT=>Ok((*v).i),K_FLOAT=>Ok((*v).f as i64),K_ERROR=>Err(v),_=>Err(type_error("RANGE needs numbers"))}}};
    match (as_int(start),as_int(end)){
        (Ok(a),Ok(b))=>{
            let count=(b as i128)-(a as i128)+1;
            if count>MAX_RANGE_ITEMS{return type_error("RANGE is too large (the limit is 10,000,000 items)")}
            let items:Vec<*mut PlainValue>=if count<=0{Vec::new()}else{(a..=b).map(|i|plain_int(i)).collect()};
            let out=value(K_LIST);unsafe{(*out).ptr=Box::into_raw(Box::new(List{items})) as *mut c_void};out
        },
        (Err(e),_)|(_,Err(e))=>e,
    }
}


#[no_mangle] pub extern "C" fn plain_read_file(path:*mut PlainValue)->*mut PlainValue{if path.is_null(){return type_error("READ_FILE needs a path")};if !is_string_value(path){return type_error("READ_FILE needs a string path")};match std::fs::read_to_string(read_string(path)){Ok(s)=>string_value(K_STRING,&s),Err(e)=>type_error(&format!("READ_FILE failed: {}",e))}}

#[no_mangle] pub extern "C" fn plain_read_lines(path:*mut PlainValue)->*mut PlainValue{
    if !is_string_value(path){return type_error("READ_LINES needs a string path")}
    let file=match File::open(read_string(path)){Ok(f)=>f,Err(e)=>return type_error(&format!("READ_LINES failed: {}",e))};
    let mut state=Box::new(LineReader{
        reader:BufReader::new(file),
        current:PlainValue{kind:K_BORROWED_STRING,i:0,f:0.0,b:0,ptr:ptr::null_mut()},
        line:String::with_capacity(4096),eof:false,error:ptr::null_mut(),
    });
    state.current.ptr=&mut state.line as *mut String as *mut c_void;
    let out=value(K_LINE_ITER);
    unsafe{(*out).ptr=Box::into_raw(state) as *mut c_void;}
    out
}

fn escape_line_nuls(line:&mut String){
    let mut offset=0;
    while let Some(rel)=line[offset..].find('\0'){
        let at=offset+rel;
        line.replace_range(at..at+1,"\\0");
        offset=at+2;
    }
}

#[no_mangle] pub extern "C" fn plain_iter_next(iter:*mut PlainValue,index:i64,out:*mut *mut PlainValue)->i8{
    if iter.is_null()||out.is_null(){return -1}
    unsafe{
        *out=ptr::null_mut();
        match (*iter).kind{
            K_LIST=>{
                let list=(*iter).ptr as *mut List;
                if list.is_null(){return -1}
                let items=&(*list).items;
                if index<0||index>=items.len() as i64{return 0}
                *out=items[index as usize];1
            }
            K_LINE_ITER=>{
                let state_ptr=(*iter).ptr as *mut LineReader;
                if state_ptr.is_null(){return -1}
                let state=&mut *state_ptr;
                if state.eof{return 0}
                state.line.clear();
                match state.reader.read_line(&mut state.line){
                    Ok(0)=>{state.eof=true;0}
                    Ok(_)=>{
                        if state.line.ends_with('\n'){state.line.pop();}
                        escape_line_nuls(&mut state.line);
                        state.current.ptr=&mut state.line as *mut String as *mut c_void;
                        *out=&mut state.current as *mut PlainValue;1
                    }
                    Err(e)=>{
                        state.eof=true;
                        state.error=type_error(&format!("READ_LINES failed: {}",e));
                        *out=state.error;-1
                    }
                }
            }
            _=>{*out=iter;-1}
        }
    }
}

#[no_mangle] pub extern "C" fn plain_iterator_fail(iter:*mut PlainValue){
    let message=if iter.is_null(){"invalid iterator".to_string()}else{unsafe{match (*iter).kind{
        K_ERROR=>read_string(iter),
        K_LINE_ITER=>{let p=(*iter).ptr as *mut LineReader;if p.is_null(){"invalid line iterator".to_string()}else if !(*p).error.is_null(){read_string((*p).error)}else{"line iterator failed".to_string()}},
        _=>"FOR EACH expects a list or READ_LINES iterator".to_string(),
    }}};
    eprintln!("Plain runtime error: {}",message);
    std::process::exit(1)
}
#[no_mangle] pub extern "C" fn plain_write_file(path:*mut PlainValue,contents:*mut PlainValue)->*mut PlainValue{if path.is_null()||contents.is_null(){return type_error("WRITE_FILE needs path and contents")};unsafe{if !is_string_value(path)||!is_string_value(contents){return type_error("WRITE_FILE needs two strings")}};match std::fs::write(read_string(path),read_string(contents)){Ok(())=>plain_bool(1),Err(e)=>type_error(&format!("WRITE_FILE failed: {}",e))}}
#[no_mangle] pub extern "C" fn plain_env(name:*mut PlainValue)->*mut PlainValue{if name.is_null(){return type_error("ENV needs a name")};unsafe{if !is_string_value(name){return type_error("ENV needs a string name")}};match std::env::var(read_string(name)){Ok(s)=>string_value(K_STRING,&s),Err(_)=>type_error("environment variable not found")}}
fn unix_millis()->std::result::Result<i64,()>{std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d|d.as_millis() as i64).map_err(|_|())}
#[no_mangle] pub extern "C" fn plain_now_ms_i64()->i64{unix_millis().unwrap_or(0)}
#[no_mangle] pub extern "C" fn plain_now_ms()->*mut PlainValue{match unix_millis(){Ok(d)=>plain_int(d),Err(_)=>type_error("system clock before Unix epoch")}}


#[cfg(test)]
mod streaming_tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;

    fn temp_file(tag:&str,bytes:&[u8])->PathBuf{
        let p=std::env::temp_dir().join(format!("plain-stream-{}-{}",std::process::id(),tag));
        let mut f=std::fs::File::create(&p).unwrap();f.write_all(bytes).unwrap();p
    }
    fn read_next(iter:*mut PlainValue,index:i64)->(i8,*mut PlainValue){let mut out=ptr::null_mut();let status=plain_iter_next(iter,index,&mut out);(status,out)}
    fn owned_text(v:*mut PlainValue)->String{unsafe{string_view(v).unwrap().to_owned()}}

    #[test]
    fn collection_ffi_rejects_invalid_lengths_and_null_buffers() {
        assert!(is_err(plain_list_new(-1, ptr::null_mut())));
        assert!(is_err(plain_list_new(1, ptr::null_mut())));
        assert!(is_err(plain_dict_new(-1, ptr::null_mut())));
        assert!(is_err(plain_dict_new(1, ptr::null_mut())));
    }

    #[test]
    fn stream_preserves_line_rules_reuses_value_and_promotes_escapes(){
        let path=temp_file("lines",b"GET /x\n\nrow\r\nlast");
        let path_v=string_value(K_STRING,path.to_str().unwrap());
        let iter=plain_read_lines(path_v);
        let (s1,v1)=read_next(iter,0);assert_eq!(s1,1);assert_eq!(owned_text(v1),"GET /x");
        let reusable=v1;
        let get=CString::new("GET").unwrap();assert_eq!(plain_contains_native(v1,get.as_ptr()),1);
        assert_eq!(plain_string_empty_native(v1),0);
        let saved=plain_promote(v1);
        let mut values=[v1];let list=plain_list_new(1,values.as_mut_ptr());
        let (s2,v2)=read_next(iter,1);assert_eq!(s2,1);assert_eq!(v2,reusable);assert_eq!(owned_text(v2),"");
        assert_eq!(owned_text(saved),"GET /x");
        let idx=plain_int(0);let stored=plain_list_get(list,idx);assert_eq!(owned_text(stored),"GET /x");
        let (s3,v3)=read_next(iter,2);assert_eq!(s3,1);assert_eq!(owned_text(v3),"row\r");
        let (s4,v4)=read_next(iter,3);assert_eq!(s4,1);assert_eq!(owned_text(v4),"last");
        assert_eq!(read_next(iter,4).0,0);
        let _=std::fs::remove_file(path);
    }

    #[test]
    fn stream_escapes_nul_and_accepts_utf8_and_unterminated_final_line(){
        let path=temp_file("nul",b"caf\xc3\xa9\0x");
        let iter=plain_read_lines(string_value(K_STRING,path.to_str().unwrap()));
        let (status,line)=read_next(iter,0);assert_eq!(status,1);assert_eq!(owned_text(line),"caf\u{e9}\\0x");
        assert_eq!(read_next(iter,1).0,0);
        let _=std::fs::remove_file(path);
    }

    #[test]
    fn stream_empty_input_and_bad_utf8_are_reported(){
        let empty=temp_file("empty",b"");
        let iter=plain_read_lines(string_value(K_STRING,empty.to_str().unwrap()));
        assert_eq!(read_next(iter,0).0,0);
        let bad=temp_file("bad",b"ok\n\xff");
        let bad_iter=plain_read_lines(string_value(K_STRING,bad.to_str().unwrap()));
        assert_eq!(read_next(bad_iter,0).0,1);
        let (status,error)=read_next(bad_iter,1);assert_eq!(status,-1);assert_eq!(unsafe{(*error).kind},K_ERROR);
        let missing=std::env::temp_dir().join(format!("plain-stream-missing-{}",std::process::id()));
        let error=plain_read_lines(string_value(K_STRING,missing.to_str().unwrap()));assert_eq!(unsafe{(*error).kind},K_ERROR);
        let _=std::fs::remove_file(empty);let _=std::fs::remove_file(bad);
    }

    #[test]
    fn borrowed_contains_matches_empty_needle_and_string_semantics(){
        let v=string_value(K_STRING,"abc");
        let empty=CString::new("").unwrap();let b=CString::new("b").unwrap();let z=CString::new("z").unwrap();
        assert_eq!(plain_contains_native(v,empty.as_ptr()),1);
        assert_eq!(plain_contains_native(v,b.as_ptr()),1);
        assert_eq!(plain_contains_native(v,z.as_ptr()),0);
        assert_eq!(plain_string_empty_native(string_value(K_STRING,"")),1);
        assert_eq!(plain_string_empty_native(plain_int(0)),0);
    }

    #[test]
    fn list_iterator_uses_native_index_and_observes_current_length(){
        let mut items=[plain_int(1),plain_int(2)];let list=plain_list_new(2,items.as_mut_ptr());
        let (a,x)=read_next(list,0);assert_eq!(a,1);assert_eq!(unsafe{(*x).i},1);
        let appended=plain_int(3);let _=plain_append(list,appended);
        let (b,y)=read_next(list,2);assert_eq!(b,1);assert_eq!(unsafe{(*y).i},3);
        assert_eq!(read_next(list,3).0,0);
    }
}

#[cfg(test)]
mod semantics_tests {
    use super::*;

    fn int(n: i64) -> *mut PlainValue { plain_int(n) }
    fn text(s: &str) -> *mut PlainValue { string_value(K_STRING, s) }
    fn truthy(v: *mut PlainValue) -> bool { plain_truth(v) != 0 }
    fn is_true(v: *mut PlainValue) -> bool { unsafe { (*v).kind == K_BOOL && (*v).b != 0 } }
    fn as_int(v: *mut PlainValue) -> i64 { unsafe { assert_eq!((*v).kind, K_INT); (*v).i } }
    fn list_len(v: *mut PlainValue) -> usize { unsafe { (&*((*v).ptr as *mut List)).items.len() } }

    #[test]
    fn integer_arithmetic_is_exact_above_2_pow_53() {
        let big = int(9_007_199_254_740_993); // 2^53 + 1 cannot be represented as an f64
        assert_eq!(as_int(plain_add(big, int(0))), 9_007_199_254_740_993);
        assert_eq!(as_int(plain_sub(big, int(1))), 9_007_199_254_740_992);
        assert_eq!(as_int(plain_mul(int(3_037_000_499), int(3_037_000_499))), 9_223_372_030_926_249_001);
    }

    #[test]
    fn integer_overflow_wraps_like_the_native_fast_path() {
        assert_eq!(as_int(plain_add(int(i64::MAX), int(1))), i64::MIN);
        assert_eq!(as_int(plain_div(int(i64::MIN), int(-1))), i64::MIN);
        assert_eq!(as_int(plain_mod(int(i64::MIN), int(-1))), 0);
    }

    #[test]
    fn division_and_modulo_by_zero_are_errors_and_division_truncates() {
        assert!(is_err(plain_div(int(1), int(0))));
        assert!(is_err(plain_mod(int(1), int(0))));
        assert!(is_err(plain_div(plain_float(1.0), plain_float(0.0))));
        assert_eq!(as_int(plain_div(int(7), int(2))), 3);
        assert_eq!(as_int(plain_div(int(-7), int(2))), -3);
        assert_eq!(as_int(plain_mod(int(-7), int(3))), -1);
    }

    #[test]
    fn errors_propagate_through_operators() {
        let e = type_error("boom");
        assert_eq!(plain_add(e, int(1)), e);
        assert_eq!(plain_lt(int(1), e), e);
        assert_eq!(plain_eq(e, int(1)), e);
        assert_eq!(plain_and(e, plain_bool(1)), e);
        assert_eq!(plain_not(e), e);
        assert!(!truthy(e));
    }

    #[test]
    fn comparisons_do_not_turn_errors_into_true() {
        let e = type_error("boom");
        assert_eq!(plain_lte(e, int(1)), e);
        assert_eq!(plain_gte(int(1), e), e);
        assert!(!truthy(plain_lte(e, int(1))));
    }

    #[test]
    fn equality_is_type_aware_and_deep() {
        assert!(is_true(plain_eq(int(1), int(1))));
        assert!(is_true(plain_eq(int(1), plain_float(1.0))));
        assert!(!is_true(plain_eq(int(1), text("1"))));
        assert!(!is_true(plain_eq(text("a"), text("b"))));
        let (mut a, mut b, mut c) = ([int(1), int(2)], [int(1), int(2)], [int(1), int(3)]);
        let (la, lb, lc) = (plain_list_new(2, a.as_mut_ptr()), plain_list_new(2, b.as_mut_ptr()), plain_list_new(2, c.as_mut_ptr()));
        assert!(is_true(plain_eq(la, lb)));
        assert!(!is_true(plain_eq(la, lc)));
        assert!(is_true(plain_not_eq(la, lc)));
    }

    #[test]
    fn ordering_requires_matching_types() {
        assert!(is_true(plain_lt(int(1), plain_float(1.5))));
        assert!(is_true(plain_lt(text("apple"), text("banana"))));
        assert!(is_err(plain_lt(int(1), text("2"))));
        assert!(!is_true(plain_lt(plain_float(f64::NAN), plain_float(1.0))));
    }

    #[test]
    fn truthiness_is_defined() {
        assert!(!truthy(int(0)));
        assert!(truthy(int(2)));
        assert!(!truthy(text("")));
        assert!(truthy(text("x")));
        assert!(!truthy(plain_bool(0)));
        assert!(!truthy(ptr::null_mut()));
        let mut none: [*mut PlainValue; 0] = [];
        assert!(!truthy(plain_list_new(0, none.as_mut_ptr())));
        let mut one = [int(1)];
        assert!(truthy(plain_list_new(1, one.as_mut_ptr())));
    }

    #[test]
    fn boolean_operators_use_truthiness() {
        assert!(is_true(plain_and(int(1), text("x"))));
        assert!(!is_true(plain_and(int(1), int(0))));
        assert!(is_true(plain_or(int(0), int(5))));
        assert!(!is_true(plain_or(plain_bool(0), text(""))));
    }

    #[test]
    fn range_is_bounded_and_overflow_safe() {
        assert!(is_err(plain_range(int(1), int(100_000_000))));
        assert!(is_err(plain_range(int(i64::MIN), int(i64::MAX))));
        assert_eq!(list_len(plain_range(int(i64::MAX - 1), int(i64::MAX))), 2);
        assert_eq!(list_len(plain_range(int(5), int(1))), 0);
        assert_eq!(list_len(plain_range(int(1), int(3))), 3);
    }

    #[test]
    fn collections_print_recursively_and_hide_internal_keys() {
        let mut inner = [int(2), int(3)];
        let nested = plain_list_new(2, inner.as_mut_ptr());
        let mut outer = [int(1), nested];
        let list = plain_list_new(2, outer.as_mut_ptr());
        assert_eq!(value_to_string(list), "[1, [2, 3]]");
        let mut pairs = [text("b"), int(2), text("a"), int(1)];
        assert_eq!(value_to_string(plain_dict_new(2, pairs.as_mut_ptr())), "{a: 1, b: 2}");
        let instance = plain_new_instance();
        plain_dict_set(instance, text("__class__"), text("Dog"));
        plain_dict_set(instance, text("name"), text("Rex"));
        assert_eq!(value_to_string(instance), "Dog{name: Rex}");
    }

    #[test]
    fn self_containing_lists_do_not_overflow_the_stack() {
        let mut cell = [int(0)];
        let looped = plain_list_new(1, cell.as_mut_ptr());
        plain_append(looped, looped);
        assert!(value_to_string(looped).contains("..."));
        assert!(is_true(plain_eq(looped, looped)));
    }

    #[test]
    fn string_access_is_safe_for_every_kind_of_value() {
        let e = type_error("boom");
        assert_eq!(read_string(e), "boom");
        assert_eq!(read_string(int(5)), ""); // not a string: no undefined behaviour
        assert_eq!(read_string(ptr::null_mut()), "");
        assert_eq!(read_string(plain_error_message(e)), "boom");
    }

    #[test]
    fn only_error_values_are_errors() {
        assert_eq!(plain_is_error(ptr::null_mut()), 0); // null is a value (a function that returns nothing)
        assert_eq!(plain_is_error(int(1)), 0);
        assert_eq!(plain_is_error(type_error("x")), 1);
    }

    #[test]
    fn fail_builds_a_real_error() {
        let e = plain_fail(text("bad input"));
        assert_eq!(plain_is_error(e), 1);
        assert_eq!(read_string(e), "bad input");
        assert_eq!(plain_fail(e), e);
    }

    #[test]
    fn handles_are_tag_checked_and_single_use() {
        let boxed = Box::into_raw(Box::new(7u32)) as *mut c_void;
        let handle = plain_handle(boxed, 42);
        assert_eq!(plain_handle_ptr(handle, 42), boxed);
        assert!(plain_handle_ptr(handle, 43).is_null()); // wrong kind of object
        assert!(plain_handle_ptr(int(1), 42).is_null()); // not a handle at all
        assert_eq!(plain_handle_clear(handle, 42), boxed);
        assert!(plain_handle_clear(handle, 42).is_null()); // already released
        unsafe { drop(Box::from_raw(boxed as *mut u32)) };
    }

    #[test]
    fn split_with_an_empty_separator_yields_characters() {
        assert_eq!(list_len(plain_split(text("abc"), text(""))), 3);
        assert_eq!(list_len(plain_split(text("a,b"), text(","))), 2);
    }

    #[test]
    fn abs_floor_and_ceil_keep_integers_exact() {
        assert_eq!(as_int(plain_abs(int(-9_007_199_254_740_993))), 9_007_199_254_740_993);
        assert_eq!(as_int(plain_floor(int(9_007_199_254_740_993))), 9_007_199_254_740_993);
        assert_eq!(as_int(plain_ceil(plain_float(3.1))), 4);
    }

    #[test]
    fn is_instance_checks_the_class_tag() {
        let dog = plain_new_instance();
        plain_dict_set(dog, text("__class__"), text("Dog"));
        let mut names = [text("Animal"), text("Dog")];
        let list = plain_list_new(2, names.as_mut_ptr());
        assert!(is_true(plain_is_instance(dog, list)));
        let mut other = [text("Cat")];
        let cats = plain_list_new(1, other.as_mut_ptr());
        assert!(!is_true(plain_is_instance(dog, cats)));
        assert!(!is_true(plain_is_instance(int(5), list)));
        assert!(!is_true(plain_is_instance(plain_new_instance(), list))); // a plain dictionary is not an object
    }

    #[test]
    fn native_contains_rejects_non_strings() {
        let needle = CString::new("a").unwrap();
        assert_eq!(plain_contains_native(int(5), needle.as_ptr()), 0);
    }
}
