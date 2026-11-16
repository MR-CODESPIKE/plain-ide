use crate::source::Span;
#[derive(Clone, Debug, PartialEq)] pub enum Type { Int, Float, Bool, String, List(Box<Type>), Dict, Any, Void }
#[derive(Clone, Debug)] pub struct Program { pub items: Vec<Item> }
#[derive(Clone, Debug)] pub enum Item { Stmt(Stmt), Function(Function), Class(Class), Use(String,Span), Extern(ExternFunction) }
#[derive(Clone, Debug)] pub struct ExternFunction { pub library:String, pub name:String, pub params:Vec<(String,Type)>, pub ret:Type, pub span:Span }
#[derive(Clone, Debug)] pub struct Function { pub name:String, pub params:Vec<(String,Type)>, pub ret:Type, pub body:Vec<Stmt>, pub span:Span }
#[derive(Clone, Debug)] pub struct Class { pub name:String, pub parent:Option<String>, pub fields:Vec<(String,Type)>, pub defaults:Vec<Option<Expr>>, pub methods:Vec<Function>, pub span:Span }
#[derive(Clone, Debug)] pub enum Stmt { Let(Vec<(String,Option<Type>)>,Vec<Expr>,Span), Set(Vec<Expr>,Vec<Expr>,Span), Say(Expr,Span), Ask{prompt:Expr,name:String,ty:Type,span:Span}, Expr(Expr), If{branches:Vec<(Expr,Vec<Stmt>)>,else_body:Vec<Stmt>,span:Span}, Repeat(Repeat,Span), Return(Option<Expr>,Span), Break(Span), Continue(Span), Try{body:Vec<Stmt>,catch:String,catch_body:Vec<Stmt>,span:Span}, Fail(Expr,Span) }
#[derive(Clone, Debug)] pub enum Repeat { Counted{name:String,start:Expr,end:Expr,body:Vec<Stmt>}, While{cond:Expr,body:Vec<Stmt>}, Each{name:String,iter:Expr,body:Vec<Stmt>} }
#[derive(Clone, Debug)] pub enum Expr { Int(i64,Span), Float(f64,Span), Bool(bool,Span), Str(String,Span), Var(String,Span), List(Vec<Expr>,Span), Dict(Vec<(Expr,Expr)>,Span), Unary{op:Unary,expr:Box<Expr>,span:Span}, Binary{left:Box<Expr>,op:Binary,right:Box<Expr>,span:Span}, Call{callee:Box<Expr>,args:Vec<Expr>,span:Span}, Member{object:Box<Expr>,name:String,span:Span}, Index{object:Box<Expr>,index:Box<Expr>,span:Span}, Ternary{yes:Box<Expr>,cond:Box<Expr>,no:Box<Expr>,span:Span} }
#[derive(Clone, Debug)] pub enum Unary { Not, Neg }
#[derive(Clone, Debug)] pub enum Binary { Add,Sub,Mul,Div,Mod,Eq,NotEq,Lt,Lte,Gt,Gte,And,Or }
impl Expr { pub fn span(&self)->Span{match self{Expr::Int(_,s)|Expr::Float(_,s)|Expr::Bool(_,s)|Expr::Str(_,s)|Expr::Var(_,s)|Expr::List(_,s)|Expr::Dict(_,s)=>*s,Expr::Unary{span,..}|Expr::Binary{span,..}|Expr::Call{span,..}|Expr::Member{span,..}|Expr::Index{span,..}|Expr::Ternary{span,..}=>*span}} }
