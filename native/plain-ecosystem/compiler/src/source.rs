#[derive(Clone, Copy, Debug, Default)]
pub struct Span { pub start: usize, pub end: usize, pub line: usize, pub column: usize }

pub struct Source { pub name: String, pub text: String, pub lines: Vec<String> }
impl Source {
    pub fn new(name: impl Into<String>, text: impl Into<String>) -> Self { let text=text.into(); let lines=text.lines().map(ToOwned::to_owned).collect(); Self{name:name.into(),text,lines} }
    pub fn diagnostic(&self, span: Span, message: &str) -> String { let line=self.lines.get(span.line.saturating_sub(1)).map(String::as_str).unwrap_or(""); format!("error: {}\n  --> {}:{}:{}\n   |\n{:>3} | {}\n   | {}^\n",message,self.name,span.line,span.column,span.line,line," ".repeat(span.column.saturating_sub(1))) }
}
#[derive(Debug)]
pub struct CompileError { pub span: Span, pub message: String }
impl CompileError { pub fn new(span: Span, message: impl Into<String>)->Self{Self{span,message:message.into()}} }
