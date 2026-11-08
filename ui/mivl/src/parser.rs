use std::collections::BTreeMap;
use crate::{Error, Result, ast::{Node, Property, Expr, Operator, Statement}, lexer::{lex, Kind, Token}};

pub fn parse(source: &str) -> Result<Node> {
    let tokens = lex(source)?;
    let mut parser = Parser { tokens, index: 0 };
    parser.skip_separators();
    let result = parser.object(0)?;
    parser.skip_separators();
    if parser.current().kind != Kind::End { return Err(parser.error("expected end of document")); }
    Ok(result)
}

struct Parser { tokens: Vec<Token>, index: usize }
impl Parser {
    fn current(&self) -> &Token { &self.tokens[self.index] }
    fn kind(&self) -> &Kind { &self.current().kind }
    fn advance(&mut self) { if self.index < self.tokens.len() - 1 { self.index += 1; } }
    fn error(&self, message: impl Into<String>) -> Error {
        Error::new(message, self.current().line, self.current().column)
    }
    fn sym(&self, s: &str) -> bool { matches!(self.kind(), Kind::Symbol(x) if *x == s) }
    fn eat(&mut self, s: &str) -> bool { if self.sym(s) { self.advance(); true } else { false } }
    fn expect(&mut self, s: &str) -> Result<()> {
        if self.eat(s) { Ok(()) } else { Err(self.error(format!("expected '{s}'"))) }
    }
    fn ident(&mut self) -> Result<String> {
        match self.kind().clone() {
            Kind::Ident(x) => { self.advance(); Ok(x) }
            _ => Err(self.error("expected identifier")),
        }
    }
    fn skip_separators(&mut self) { while self.eat(";") {} }
    fn object(&mut self, depth: usize) -> Result<Node> {
        if depth > 64 { return Err(self.error("maximum object nesting (64) exceeded")); }
        let kind = self.ident()?;
        self.expect("{")?;
        let mut node = Node {
            kind, properties: Vec::new(), handlers: BTreeMap::new(), children: Vec::new(), delegate: None,
        };
        loop {
            self.skip_separators();
            if self.eat("}") { break; }
            if self.kind() == &Kind::End { return Err(self.error("unterminated object")); }
            let first = self.ident()?;
            if first == "property" {
                let ty = self.ident()?;
                if !matches!(ty.as_str(), "int" | "real" | "double" | "bool" | "string" | "var" | "color") {
                    return Err(self.error(format!("unsupported property type '{ty}'")));
                }
                let name = self.ident()?;
                self.expect(":")?;
                let expression = self.expression(0, 0)?;
                self.add_prop(&mut node, name, Some(ty), expression)?;
            } else if self.sym("{") {
                self.index -= 1; // Put the component name back.
                node.children.push(self.object(depth + 1)?);
            } else {
                let mut name = first;
                loop {
                    if self.eat(".") { name.push('.'); name.push_str(&self.ident()?); }
                    else if self.eat("-") {
                        let next = self.ident()?;
                        let mut chars = next.chars();
                        if let Some(first) = chars.next() { name.extend(first.to_uppercase()); name.extend(chars); }
                    } else { break; }
                }
                self.expect(":")?;
                if name == "delegate" && matches!(self.kind(), Kind::Ident(_))
                    && matches!(self.tokens.get(self.index + 1).map(|x| &x.kind), Some(Kind::Symbol("{"))) {
                    if node.delegate.is_some() { return Err(self.error("duplicate delegate")); }
                    node.delegate = Some(Box::new(self.object(depth + 1)?));
                } else if name.starts_with("on") && name.chars().nth(2).is_some_and(|c| c.is_ascii_uppercase()) && self.eat("{") {
                    let mut instructions = Vec::new();
                    loop {
                        self.skip_separators();
                        if self.eat("}") { break; }
                        if self.kind() == &Kind::End { return Err(self.error("unterminated event handler")); }
                        // Event blocks support both commands and property assignments.
                        // Reactive property expressions are intentionally kept side-effect-free.
                        let is_call = matches!(self.kind(), Kind::Ident(_))
                            && matches!(self.tokens.get(self.index + 1).map(|x| &x.kind), Some(Kind::Symbol("(")));
                        if is_call {
                            let expr = self.expression(0, 0)?;
                            if !matches!(&expr, Expr::Call(name, _) if name == "invoke" || name == "invoke_async") {
                                return Err(self.error("standalone handler calls must use invoke(...) or invoke_async(...)"));
                            }
                            instructions.push(Statement::Call(expr));
                        } else {
                            let target = self.path()?;
                            let operator = if self.eat("=") { Operator::Eq }
                                else if self.eat("+=") { Operator::Add }
                                else if self.eat("-=") { Operator::Sub }
                                else if self.eat("*=") { Operator::Mul }
                                else if self.eat("/=") { Operator::Div }
                                else { return Err(self.error("expected assignment or invoke(...) in handler")); };
                            let expression = self.expression(0, 0)?;
                            instructions.push(Statement::Assign { target, operator, expression });
                        }
                        if instructions.len() > 1024 { return Err(self.error("too many handler statements")); }
                        if !self.sym("}") && !self.sym(";") {
                            return Err(self.error("separate statements using a newline or ';'"));
                        }
                    }
                    if node.handlers.insert(name.clone(), instructions).is_some() {
                        return Err(self.error(format!("duplicate handler {name}")));
                    }
                } else {
                    let expression = self.expression(0, 0)?;
                    self.add_prop(&mut node, name, None, expression)?;
                }
            }
            if !self.sym("}") && !self.sym(";") {
                return Err(self.error("expected newline or ';' between object members"));
            }
        }
        Ok(node)
    }
    fn add_prop(&self, node: &mut Node, name: String, declared_type: Option<String>, expression: Expr) -> Result<()> {
        if node.properties.iter().any(|p| p.name == name) {
            return Err(self.error(format!("duplicate property '{name}'")));
        }
        node.properties.push(Property { name, declared_type, expression });
        Ok(())
    }
    fn path(&mut self) -> Result<Vec<String>> {
        let mut parts = vec![self.ident()?];
        while self.eat(".") { parts.push(self.ident()?); }
        Ok(parts)
    }
    fn expression(&mut self, min_power: u8, depth: usize) -> Result<Expr> {
        if depth > 64 { return Err(self.error("expression nesting limit exceeded")); }
        let mut lhs = if self.eat("-") {
            Expr::Unary(Operator::Sub, Box::new(self.expression(14, depth + 1)?))
        } else if self.eat("!") {
            Expr::Unary(Operator::Not, Box::new(self.expression(14, depth + 1)?))
        } else if self.eat("[") {
            let mut items = Vec::new();
            self.skip_separators();
            if !self.sym("]") {
                loop {
                    items.push(self.expression(0, depth + 1)?);
                    if items.len() > 4096 { return Err(self.error("array exceeds 4096 entries")); }
                    if !self.eat(",") { break; }
                    self.skip_separators();
                    if self.sym("]") { break; }
                }
            }
            self.expect("]")?;
            Expr::List(items)
        } else if self.eat("(") {
            let x = self.expression(0, depth + 1)?;
            self.expect(")")?; x
        } else {
            match self.kind().clone() {
                Kind::Number(n) => { self.advance(); Expr::Number(n) }
                Kind::Dimension(n, unit) => { self.advance(); Expr::Length(n, unit) }
                Kind::Ident(ref s) if s == "auto" => { self.advance(); Expr::String("auto".into()) }
                Kind::String(s) => { self.advance(); Expr::String(s) }
                Kind::Ident(ref s) if s == "true" => { self.advance(); Expr::Bool(true) }
                Kind::Ident(ref s) if s == "false" => { self.advance(); Expr::Bool(false) }
                Kind::Ident(ref s) if s == "null" => { self.advance(); Expr::Null }
                Kind::Ident(_) => {
                    let path = self.path()?;
                    if self.eat("(") {
                        if path.len() != 1 { return Err(self.error("function names must be unqualified (use invoke(\"command\", ...))")); }
                        let mut args = Vec::new();
                        if !self.sym(")") {
                            loop {
                                args.push(self.expression(0, depth + 1)?);
                                if args.len() > 32 { return Err(self.error("too many function arguments")); }
                                if !self.eat(",") { break; }
                            }
                        }
                        self.expect(")")?;
                        Expr::Call(path[0].clone(), args)
                    } else { Expr::Path(path) }
                }
                _ => return Err(self.error("expected expression")),
            }
        };
        loop {
            if min_power == 0 && self.eat("?") {
                let yes = self.expression(0, depth + 1)?;
                self.expect(":")?;
                let no = self.expression(0, depth + 1)?;
                lhs = Expr::Conditional(Box::new(lhs), Box::new(yes), Box::new(no));
                continue;
            }
            let (power, op) = match self.kind() {
                Kind::Symbol("||") => (1, Operator::Or),
                Kind::Symbol("&&") => (3, Operator::And),
                Kind::Symbol("==") => (5, Operator::Eq),
                Kind::Symbol("!=") => (5, Operator::Ne),
                Kind::Symbol("<") => (7, Operator::Lt),
                Kind::Symbol("<=") => (7, Operator::Le),
                Kind::Symbol(">") => (7, Operator::Gt),
                Kind::Symbol(">=") => (7, Operator::Ge),
                Kind::Symbol("+") => (9, Operator::Add),
                Kind::Symbol("-") => (9, Operator::Sub),
                Kind::Symbol("*") => (11, Operator::Mul),
                Kind::Symbol("/") => (11, Operator::Div),
                Kind::Symbol("%") => (11, Operator::Rem),
                _ => break,
            };
            if power < min_power { break; }
            self.advance();
            let rhs = self.expression(power + 1, depth + 1)?;
            lhs = Expr::Binary(Box::new(lhs), op, Box::new(rhs));
        }
        Ok(lhs)
    }
}
