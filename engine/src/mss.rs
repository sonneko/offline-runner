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
    #[regex("\"[^\"]*\"", |lex| lex.slice()[1..lex.slice().len()-1].to_string())]
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
}

#[derive(Debug, Clone)]
pub enum Statement {
    Assignment(String, Expr),
    If(Expr, Vec<Statement>, Option<Vec<Statement>>),
    While(Expr, Vec<Statement>),
    For(String, Expr, Vec<Statement>),
    CommandCall(String, Vec<Expr>),
    BuiltinCall(String, Vec<Expr>),
}

fn parse_identifier(input: &str) -> IResult<&str, String> {
    map(recognize(pair(alt((alpha1, tag("_"))), many0(alt((alphanumeric1, tag("_")))))), |s: &str| s.to_string()).parse(input)
}

fn parse_literal(input: &str) -> IResult<&str, Expr> {
    map(delimited(char('"'), take_until("\""), char('"')), |s: &str| Expr::Literal(s.to_string())).parse(input)
}

fn parse_variable(input: &str) -> IResult<&str, Expr> {
    map(preceded(char('$'), parse_identifier), Expr::Variable).parse(input)
}

fn parse_command_sub(input: &str) -> IResult<&str, Expr> {
    map(
        delimited(char('`'), take_until("`"), char('`')),
        |s: &str| Expr::CommandSub(s.to_string()),
    )
    .parse(input)
}

fn parse_primary_expr(input: &str) -> IResult<&str, Expr> {
    alt((parse_literal, parse_variable, parse_command_sub)).parse(input)
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
    let (input, _) = char('$')(input)?;
    let (input, var_name) = parse_identifier(input)?;
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
    let (input, _) = char('$')(input)?;
    let (input, name) = parse_identifier(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = char('=')(input)?;
    let (input, _) = multispace0(input)?;
    let (input, expr) = parse_expr(input)?;
    Ok((input, Statement::Assignment(name, expr)))
}

fn parse_builtin_call(input: &str) -> IResult<&str, Statement> {
    let (input, name) = parse_identifier(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = char('(').parse(input)?;
    let (input, args) = opt(pair(
        parse_expr,
        many0(preceded(delimited(multispace0, char(','), multispace0), parse_expr))
    )).parse(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = char(')').parse(input)?;

    let mut all_args = Vec::new();
    if let Some((first, rest)) = args {
        all_args.push(first);
        all_args.extend(rest);
    }

    Ok((input, Statement::BuiltinCall(name, all_args)))
}

fn parse_statement(input: &str) -> IResult<&str, Statement> {
    let (input, _) = multispace0(input)?;
    alt((
        parse_if_stmt,
        parse_while_stmt,
        parse_for_stmt,
        parse_assignment,
        parse_command_call_stmt,
        parse_builtin_call,
    ))
    .parse(input)
}

fn parse_program(input: &str) -> IResult<&str, Vec<Statement>> {
    many0(terminated(parse_statement, multispace0)).parse(input)
}

pub struct Interpreter<F>
where F: Fn(String) -> LocalBoxFuture<'static, Result<String, String>> + 'static
{
    pub variables: HashMap<String, String>,
    pub command_executor: F,
}

impl<F> Interpreter<F>
where F: Fn(String) -> LocalBoxFuture<'static, Result<String, String>> + 'static
{
    pub fn new(executor: F) -> Self {
        Self {
            variables: HashMap::new(),
            command_executor: executor,
        }
    }

    pub async fn run(&mut self, code: &str) -> String {
        match parse_program(code) {
            Ok((_, statements)) => {
                let mut output = Vec::new();
                for stmt in statements {
                    match self.execute_statement(stmt).await {
                        Ok(res) => {
                            if !res.is_empty() {
                                output.push(res);
                            }
                        }
                        Err(e) => return format!("Runtime Error: {}", e),
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
                    self.variables.insert(name, val);
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
                            if !res.is_empty() {
                                out.push(res);
                            }
                        }
                        Ok(out.join("\n"))
                    } else if let Some(eb) = else_block {
                        let mut out = Vec::new();
                        for s in eb {
                            let res = self.execute_statement(s).await?;
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
                        for s in &body {
                            let res = self.execute_statement(s.clone()).await?;
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
                        self.variables.insert(var.clone(), item.to_string());
                        for s in &body {
                            let res = self.execute_statement(s.clone()).await?;
                            if !res.is_empty() {
                                output.push(res);
                            }
                        }
                    }
                    Ok(output.join("\n"))
                }
                Statement::BuiltinCall(name, args) => {
                    let mut evaluated_args = Vec::new();
                    for arg in args {
                        evaluated_args.push(self.evaluate_expr(arg).await?);
                    }
                    match name.as_str() {
                        "print" => Ok(evaluated_args.join(" ")),
                        "len" => Ok(evaluated_args.get(0).map(|s| s.len().to_string()).unwrap_or("0".to_string())),
                        "sleep" => {
                            if let Some(ms_str) = evaluated_args.get(0) {
                                if let Ok(ms) = ms_str.parse::<u64>() {
                                    // Async sleep on Wasm is usually handled by JS promises.
                                    // For now, we will rely on a JS bridge or use wasm-bindgen-futures::JsFuture to wrap a setTimeout promise.
                                    // Since we don't have a direct bridge here, we'll try to implement it if needed or leave as placeholder.
                                    #[cfg(target_arch = "wasm32")]
                                    {
                                         // Placeholder for async sleep on Wasm
                                    }
                                    #[cfg(not(target_arch = "wasm32"))]
                                    {
                                        // Still use tokio for native tests if available, but we removed it.
                                        // Let's just use std::thread::sleep for native tests (blocking is OK for tests).
                                        std::thread::sleep(std::time::Duration::from_millis(ms));
                                    }
                                }
                            }
                            Ok(String::new())
                        }
                        "http_get" => {
                            Ok(format!("[Fetch content of {}]", evaluated_args.get(0).unwrap_or(&"".to_string())))
                        }
                        _ => Err(format!("Unknown builtin: {}", name)),
                    }
                }
            }
        })
    }

    pub fn evaluate_expr(&self, expr: Expr) -> LocalBoxFuture<'_, Result<String, String>> {
        Box::pin(async move {
            match expr {
                Expr::Literal(s) => Ok(s),
                Expr::Variable(name) => self.variables.get(&name).cloned().ok_or_else(|| format!("Undefined variable: ${}", name)),
                Expr::CommandSub(cmd) => {
                    (self.command_executor)(cmd).await
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
