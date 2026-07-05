use logos::Logos;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_until},
    character::complete::{alpha1, alphanumeric1, multispace0, char},
    combinator::{map, recognize, opt},
    multi::many0,
    sequence::{delimited, pair, preceded, terminated},
    IResult,
    Parser,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use futures::future::LocalBoxFuture;

#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip r"[ \t\n\f]+")]
pub enum Token {
    #[token("$")]
    Dollar,
    #[token("=")]
    Assign,
    #[token("@")]
    At,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[regex("[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),
    #[regex("\"([^\"\\\\]|\\\\.)*\"", |lex| {
        let s = lex.slice();
        let mut result = String::with_capacity(s.len());
        let mut chars = s[1..s.len()-1].chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => result.push('\n'),
                    Some('r') => result.push('\r'),
                    Some('t') => result.push('\t'),
                    Some('\\') => result.push('\\'),
                    Some('"') => result.push('"'),
                    Some(other) => {
                        result.push('\\');
                        result.push(other);
                    }
                    None => result.push('\\'),
                }
            } else {
                result.push(c);
            }
        }
        result
    })]
    String(String),
    #[regex("`[^`]*`", |lex| lex.slice()[1..lex.slice().len()-1].to_string())]
    Backtick(String),
}

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(String),
    Variable(String),
    BinaryOp(Box<Expr>, String, Box<Expr>),
    CommandSub(String),
    Call(String, Vec<Expr>),
}

#[derive(Debug, Clone)]
pub enum Statement {
    Assignment(String, Expr),
    If(Expr, Vec<Statement>, Option<Vec<Statement>>),
    While(Expr, Vec<Statement>),
    For(String, Expr, Vec<Statement>),
    CommandCall(String, Vec<Expr>),
    FunctionDef(String, Vec<String>, Vec<Statement>),
    Return(Expr),
    Expression(Expr),
}

fn parse_identifier(input: &str) -> IResult<&str, String> {
    map(
        recognize(pair(
            alt((alpha1, tag("_"))),
            many0(alt((alphanumeric1, tag("_")))),
        )),
        |s: &str| s.to_string(),
    )
    .parse(input)
}

fn parse_var_identifier(input: &str) -> IResult<&str, String> {
    preceded(char('$'), parse_identifier).parse(input)
}

fn parse_literal(input: &str) -> IResult<&str, Expr> {
    if !input.starts_with('"') {
        return Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)));
    }
    let chars = input[1..].chars();
    let mut parsed_len = 1;
    let mut escaped = false;
    let mut content = String::new();

    for c in chars {
        parsed_len += c.len_utf8();
        if escaped {
            match c {
                'n' => content.push('\n'),
                'r' => content.push('\r'),
                't' => content.push('\t'),
                '\\' => content.push('\\'),
                '"' => content.push('"'),
                _ => {
                    content.push('\\');
                    content.push(c);
                }
            }
            escaped = false;
        } else {
            if c == '\\' {
                escaped = true;
            } else if c == '"' {
                return Ok((&input[parsed_len..], Expr::Literal(content)));
            } else {
                content.push(c);
            }
        }
    }
    Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
}

fn parse_variable(input: &str) -> IResult<&str, Expr> {
    map(parse_var_identifier, Expr::Variable).parse(input)
}

fn parse_command_sub(input: &str) -> IResult<&str, Expr> {
    map(
        delimited(char('`'), take_until("`"), char('`')),
        |s: &str| Expr::CommandSub(s.to_string()),
    )
    .parse(input)
}

fn parse_call_expr(input: &str) -> IResult<&str, Expr> {
    let (input, name) = parse_identifier(input)?;
    let (input, _) = char('(').parse(input)?;
    let (input, args) = opt(pair(
        parse_expr,
        many0(preceded(delimited(multispace0, char(','), multispace0), parse_expr))
    )).parse(input)?;
    let (input, _) = char(')').parse(input)?;

    let mut all_args = Vec::new();
    if let Some((first, rest)) = args {
        all_args.push(first);
        all_args.extend(rest);
    }

    Ok((input, Expr::Call(name, all_args)))
}

fn parse_primary_expr(input: &str) -> IResult<&str, Expr> {
    alt((parse_literal, parse_variable, parse_command_sub, parse_call_expr)).parse(input)
}

fn parse_expr(input: &str) -> IResult<&str, Expr> {
    let (input, _) = multispace0(input)?;
    let (input, left) = parse_primary_expr(input)?;
    let (input, _) = multispace0(input)?;
    let (input, op) = opt(alt((
        tag("=="), tag("!="), tag("<="), tag(">="), tag("<"), tag(">"),
        tag("+"), tag("-"), tag("*"), tag("/")
    ))).parse(input)?;

    if let Some(op_str) = op {
        let (input, _) = multispace0(input)?;
        let (input, right) = parse_expr(input)?;
        Ok((input, Expr::BinaryOp(Box::new(left), op_str.to_string(), Box::new(right))))
    } else {
        Ok((input, left))
    }
}

fn parse_block(input: &str) -> IResult<&str, Vec<Statement>> {
    delimited(
        terminated(char('{'), multispace0),
        parse_program,
        terminated(char('}'), multispace0)
    ).parse(input)
}

fn parse_if_stmt(input: &str) -> IResult<&str, Statement> {
    let (input, _) = tag("if").parse(input)?;
    let (input, _) = multispace0(input)?;
    let (input, condition) = parse_expr(input)?;
    let (input, _) = multispace0(input)?;
    let (input, then_block) = parse_block(input)?;
    let (input, else_block) = opt(preceded(terminated(tag("else"), multispace0), parse_block)).parse(input)?;
    Ok((input, Statement::If(condition, then_block, else_block)))
}

fn parse_while_stmt(input: &str) -> IResult<&str, Statement> {
    let (input, _) = tag("while").parse(input)?;
    let (input, _) = multispace0(input)?;
    let (input, condition) = parse_expr(input)?;
    let (input, _) = multispace0(input)?;
    let (input, body) = parse_block(input)?;
    Ok((input, Statement::While(condition, body)))
}

fn parse_for_stmt(input: &str) -> IResult<&str, Statement> {
    let (input, _) = tag("for").parse(input)?;
    let (input, _) = multispace0(input)?;
    let (input, var_name) = parse_var_identifier(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = tag("in").parse(input)?;
    let (input, _) = multispace0(input)?;
    let (input, list_expr) = parse_expr(input)?;
    let (input, _) = multispace0(input)?;
    let (input, body) = parse_block(input)?;
    Ok((input, Statement::For(var_name, list_expr, body)))
}

fn parse_command_call_stmt(input: &str) -> IResult<&str, Statement> {
    let (input, _) = char('@')(input)?;
    let (input, name) = parse_identifier(input)?;
    let (input, args) = many0(preceded(multispace0, parse_expr)).parse(input)?;
    Ok((input, Statement::CommandCall(name, args)))
}

fn parse_assignment(input: &str) -> IResult<&str, Statement> {
    let (input, name) = parse_var_identifier(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = char('=')(input)?;
    let (input, _) = multispace0(input)?;
    let (input, expr) = parse_expr(input)?;
    Ok((input, Statement::Assignment(name, expr)))
}

fn parse_function_def(input: &str) -> IResult<&str, Statement> {
    let (input, _) = tag("func").parse(input)?;
    let (input, _) = multispace0(input)?;
    let (input, name) = parse_identifier(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = char('(').parse(input)?;
    let (input, args) = opt(pair(
        parse_var_identifier,
        many0(preceded(
            delimited(multispace0, char(','), multispace0),
            parse_var_identifier,
        )),
    ))
    .parse(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = char(')').parse(input)?;
    let (input, _) = multispace0(input)?;
    let (input, body) = parse_block(input)?;

    let mut all_args = Vec::new();
    if let Some((first, rest)) = args {
        all_args.push(first);
        all_args.extend(rest);
    }

    Ok((input, Statement::FunctionDef(name, all_args, body)))
}

fn parse_return_stmt(input: &str) -> IResult<&str, Statement> {
    let (input, _) = tag("return").parse(input)?;
    let (input, _) = multispace0(input)?;
    let (input, expr) = parse_expr(input)?;
    Ok((input, Statement::Return(expr)))
}

fn parse_expression_stmt(input: &str) -> IResult<&str, Statement> {
    map(parse_expr, Statement::Expression).parse(input)
}

fn parse_statement(input: &str) -> IResult<&str, Statement> {
    let (input, _) = multispace0(input)?;
    alt((
        parse_if_stmt,
        parse_while_stmt,
        parse_for_stmt,
        parse_function_def,
        parse_return_stmt,
        parse_assignment,
        parse_command_call_stmt,
        parse_expression_stmt,
    ))
    .parse(input)
}

pub fn parse_program(input: &str) -> IResult<&str, Vec<Statement>> {
    many0(terminated(parse_statement, multispace0)).parse(input)
}

#[derive(Clone)]
pub struct Function {
    params: Vec<String>,
    body: Vec<Statement>,
}

pub struct Interpreter<F, G, S>
where
    F: Fn(String) -> LocalBoxFuture<'static, Result<String, String>> + 'static,
    G: Fn(String) -> LocalBoxFuture<'static, Result<String, String>> + 'static,
    S: Fn(u64) -> LocalBoxFuture<'static, ()> + 'static
{
    pub variables: Vec<HashMap<String, String>>, // Stack of scopes
    pub functions: HashMap<String, Function>,
    pub call_stack: Vec<String>,
    pub command_executor: F,
    pub http_get_fn: G,
    pub sleep_fn: S,
    pub cancel_flag: Arc<AtomicBool>,
}

impl<F, G, S> Interpreter<F, G, S>
where
    F: Fn(String) -> LocalBoxFuture<'static, Result<String, String>> + 'static,
    G: Fn(String) -> LocalBoxFuture<'static, Result<String, String>> + 'static,
    S: Fn(u64) -> LocalBoxFuture<'static, ()> + 'static
{
    pub fn new(executor: F, http_get_fn: G, sleep_fn: S) -> Self {
        Self {
            variables: vec![HashMap::new()],
            functions: HashMap::new(),
            call_stack: vec!["main".to_string()],
            command_executor: executor,
            http_get_fn,
            sleep_fn,
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    fn get_var(&self, name: &str) -> Option<String> {
        for scope in self.variables.iter().rev() {
            if let Some(val) = scope.get(name) {
                return Some(val.clone());
            }
        }
        None
    }

    fn set_var(&mut self, name: String, val: String) {
        if let Some(scope) = self.variables.last_mut() {
            scope.insert(name, val);
        }
    }

    pub fn set_env(&mut self, name: &str, val: &str) {
        if let Some(scope) = self.variables.first_mut() {
            scope.insert(name.to_string(), val.to_string());
        }
    }

    pub async fn run(&mut self, code: &str) -> String {
        self.cancel_flag.store(false, Ordering::SeqCst);
        match parse_program(code) {
            Ok((_, statements)) => {
                let mut output = Vec::new();
                for stmt in statements {
                    if self.cancel_flag.load(Ordering::SeqCst) {
                        return "Interrupted".to_string();
                    }
                    match self.execute_statement(stmt).await {
                        Ok(res) => {
                            if res != "__NO_STDOUT__" && !res.is_empty() {
                                output.push(res);
                            }
                        }
                        Err(e) => {
                            let mut trace = String::new();
                            trace.push_str(&format!("Runtime Error: {}\nStack trace:\n", e));
                            for (i, frame) in self.call_stack.iter().rev().enumerate() {
                                trace.push_str(&format!("  {}: {}\n", i, frame));
                            }
                            return trace;
                        }
                    }
                }
                output.join("\n")
            }
            Err(e) => format!("Parse Error: {:?}", e),
        }
    }

    pub fn execute_statement(&mut self, stmt: Statement) -> LocalBoxFuture<'_, Result<String, String>> {
        Box::pin(async move {
            match stmt {
                Statement::Assignment(name, expr) => {
                    let val = self.evaluate_expr(expr).await?;
                    self.set_var(name, val);
                    Ok(String::new())
                }
                Statement::CommandCall(name, args) => {
                    let mut evaluated_args = Vec::new();
                    for arg in args {
                        evaluated_args.push(self.evaluate_expr(arg).await?);
                    }
                    let cmd_line = format!("{} {}", name, evaluated_args.join(" "));
                    (self.command_executor)(cmd_line).await
                }
                Statement::If(condition, then_block, else_block) => {
                    let val = self.evaluate_expr(condition).await?;
                    if !val.is_empty() && val != "false" && val != "0" {
                        let mut out = Vec::new();
                        for s in then_block {
                            let res = self.execute_statement(s).await?;
                            if res.starts_with("__RETURN__:") {
                                return Ok(res);
                            }
                            if !res.is_empty() {
                                out.push(res);
                            }
                        }
                        Ok(out.join("\n"))
                    } else if let Some(eb) = else_block {
                        let mut out = Vec::new();
                        for s in eb {
                            let res = self.execute_statement(s).await?;
                            if res.starts_with("__RETURN__:") {
                                return Ok(res);
                            }
                            if !res.is_empty() {
                                out.push(res);
                            }
                        }
                        Ok(out.join("\n"))
                    } else {
                        Ok(String::new())
                    }
                }
                Statement::While(condition, body) => {
                    let mut output = Vec::new();
                    while {
                        let val = self.evaluate_expr(condition.clone()).await?;
                        !val.is_empty() && val != "false" && val != "0"
                    } {
                        if self.cancel_flag.load(Ordering::SeqCst) {
                            return Err("Interrupted".to_string());
                        }
                        for s in &body {
                            let res = self.execute_statement(s.clone()).await?;
                            if res.starts_with("__RETURN__:") {
                                return Ok(res);
                            }
                            if !res.is_empty() {
                                output.push(res);
                            }
                        }
                    }
                    Ok(output.join("\n"))
                }
                Statement::For(var, list_expr, body) => {
                    let list_val = self.evaluate_expr(list_expr).await?;
                    let items: Vec<&str> = list_val.split_whitespace().collect();
                    let mut output = Vec::new();
                    for item in items {
                        if self.cancel_flag.load(Ordering::SeqCst) {
                            return Err("Interrupted".to_string());
                        }
                        self.set_var(var.clone(), item.to_string());
                        for s in &body {
                            let res = self.execute_statement(s.clone()).await?;
                            if res.starts_with("__RETURN__:") {
                                return Ok(res);
                            }
                            if !res.is_empty() {
                                output.push(res);
                            }
                        }
                    }
                    Ok(output.join("\n"))
                }
                Statement::FunctionDef(name, params, body) => {
                    self.functions.insert(name, Function { params, body });
                    Ok(String::new())
                }
                Statement::Return(expr) => {
                    let val = self.evaluate_expr(expr).await?;
                    Ok(format!("__RETURN__:{}", val))
                }
                Statement::Expression(expr) => {
                    self.evaluate_expr(expr).await
                }
            }
        })
    }

    pub fn evaluate_expr(&mut self, expr: Expr) -> LocalBoxFuture<'_, Result<String, String>> {
        Box::pin(async move {
            match expr {
                Expr::Literal(s) => Ok(s),
                Expr::Variable(name) => self.get_var(&name).ok_or_else(|| format!("Undefined variable: ${}", name)),
                Expr::CommandSub(cmd) => {
                    (self.command_executor)(cmd).await
                }
                Expr::Call(name, args) => {
                    let mut evaluated_args = Vec::new();
                    for arg in args {
                        evaluated_args.push(self.evaluate_expr(arg).await?);
                    }

                    // Check if it's a user-defined function call
                    if let Some(func) = self.functions.get(&name).cloned() {
                        self.call_stack.push(name.clone());
                        let mut new_scope = HashMap::new();
                        for (i, param) in func.params.iter().enumerate() {
                            let val = evaluated_args.get(i).cloned().unwrap_or_default();
                            new_scope.insert(param.clone(), val);
                        }
                        self.variables.push(new_scope);

                        let mut output: Vec<String> = Vec::new();
                        let mut return_val = String::new();
                        for s in func.body {
                            if self.cancel_flag.load(Ordering::SeqCst) {
                                return Err("Interrupted".to_string());
                            }
                            let res = self.execute_statement(s).await?;
                            if res.starts_with("__RETURN__:") {
                                return_val = res["__RETURN__:".len()..].to_string();
                                break;
                            }
                            if !res.is_empty() {
                                output.push(res);
                            }
                        }
                        self.variables.pop();
                        self.call_stack.pop();
                        return if !return_val.is_empty() { Ok(return_val) } else { Ok(output.join("\n")) };
                    }

                    match name.as_str() {
                        "print" => Ok(evaluated_args.join(" ")),
                        "len" => Ok(evaluated_args.get(0).map(|s| s.len().to_string()).unwrap_or("0".to_string())),
                        "sleep" => {
                            if let Some(ms_str) = evaluated_args.get(0) {
                                if let Ok(ms) = ms_str.parse::<u64>() {
                                    (self.sleep_fn)(ms).await;
                                }
                            }
                            Ok(String::new())
                        }
                        "http_get" => {
                            if let Some(url) = evaluated_args.get(0) {
                                (self.http_get_fn)(url.clone()).await
                            } else {
                                Err("http_get requires a URL".to_string())
                            }
                        }
                        "json_parse" => {
                            if let Some(json_str) = evaluated_args.get(0) {
                                let v: serde_json::Value = serde_json::from_str(json_str).map_err(|e| format!("JSON Parse Error: {}", e))?;
                                Ok(v.to_string())
                            } else {
                                Err("json_parse requires a string".to_string())
                            }
                        }
                        _ => Err(format!("Unknown call: {}", name)),
                    }
                }
                Expr::BinaryOp(left, op, right) => {
                    let l_val = self.evaluate_expr(*left).await?;
                    let r_val = self.evaluate_expr(*right).await?;

                    match op.as_str() {
                        "==" => Ok((l_val == r_val).to_string()),
                        "!=" => Ok((l_val != r_val).to_string()),
                        "+" => {
                            if let (Ok(l_num), Ok(r_num)) = (l_val.parse::<f64>(), r_val.parse::<f64>()) {
                                Ok((l_num + r_num).to_string())
                            } else {
                                Ok(format!("{}{}", l_val, r_val))
                            }
                        }
                        "-" => {
                            let l_num: f64 = l_val.parse().map_err(|_| "Invalid number")?;
                            let r_num: f64 = r_val.parse().map_err(|_| "Invalid number")?;
                            Ok((l_num - r_num).to_string())
                        }
                        "*" => {
                            let l_num: f64 = l_val.parse().map_err(|_| "Invalid number")?;
                            let r_num: f64 = r_val.parse().map_err(|_| "Invalid number")?;
                            Ok((l_num * r_num).to_string())
                        }
                        "/" => {
                            let l_num: f64 = l_val.parse().map_err(|_| "Invalid number")?;
                            let r_num: f64 = r_val.parse().map_err(|_| "Invalid number")?;
                            if r_num == 0.0 {
                                return Err("Division by zero".to_string());
                            }
                            Ok((l_num / r_num).to_string())
                        }
                        _ => Err(format!("Unsupported operator: {}", op)),
                    }
                }
            }
        })
    }
}
