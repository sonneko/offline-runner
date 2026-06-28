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

#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip r"[ \t\n\f]+")]
enum Token {
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
    Backtick(String),
}

#[derive(Debug, Clone)]
pub enum Statement {
    Assignment(String, Expr),
    If(Expr, Vec<Statement>, Option<Vec<Statement>>),
    CommandCall(String, Vec<Expr>),
    For(String, Expr, Vec<Statement>),
    While(Expr, Vec<Statement>),
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

fn parse_backtick(input: &str) -> IResult<&str, Expr> {
    map(delimited(char('`'), take_until("`"), char('`')), |s: &str| Expr::Backtick(s.to_string())).parse(input)
}

fn parse_primary_expr(input: &str) -> IResult<&str, Expr> {
    alt((parse_literal, parse_variable, parse_backtick)).parse(input)
}

fn parse_expr(input: &str) -> IResult<&str, Expr> {
    let (input, left) = parse_primary_expr(input)?;
    let (input, _) = multispace0(input)?;
    let (input, op) = opt(alt((
        tag("=="), tag("!="), tag("<="), tag(">="), tag("<"), tag(">"),
        tag("+"), tag("-")
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

fn parse_for_stmt(input: &str) -> IResult<&str, Statement> {
    let (input, _) = tag("for").parse(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = char('$')(input)?;
    let (input, var_name) = parse_identifier(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = tag("in")(input)?;
    let (input, _) = multispace0(input)?;
    let (input, list_expr) = parse_expr(input)?;
    let (input, _) = multispace0(input)?;
    let (input, block) = parse_block(input)?;
    Ok((input, Statement::For(var_name, list_expr, block)))
}

fn parse_while_stmt(input: &str) -> IResult<&str, Statement> {
    let (input, _) = tag("while").parse(input)?;
    let (input, _) = multispace0(input)?;
    let (input, condition) = parse_expr(input)?;
    let (input, _) = multispace0(input)?;
    let (input, block) = parse_block(input)?;
    Ok((input, Statement::While(condition, block)))
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

fn parse_statement(input: &str) -> IResult<&str, Statement> {
    let (input, _) = multispace0(input)?;
    alt((parse_if_stmt, parse_for_stmt, parse_while_stmt, parse_assignment, parse_command_call_stmt)).parse(input)
}

fn parse_program(input: &str) -> IResult<&str, Vec<Statement>> {
    many0(terminated(parse_statement, multispace0)).parse(input)
}

pub struct Interpreter {
    variables: HashMap<String, String>,
    pub cmd_executor: Option<fn(String) -> futures::future::LocalBoxFuture<'static, Result<String, String>>>,
}

impl Interpreter {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            cmd_executor: None,
        }
    }

    pub async fn run(&mut self, code: &str) -> String {
        match parse_program(code) {
            Ok((_, statements)) => {
                let mut output = Vec::new();
                for stmt in statements {
                    match self.execute_statement(stmt).await {
                        Ok(res) => {
                            let trimmed = res.trim();
                            if !trimmed.is_empty() { output.push(trimmed.to_string()); }
                        },
                        Err(e) => return format!("Runtime Error: {}", e),
                    }
                }
                output.join("\n")
            }
            Err(e) => format!("Parse Error: {:?}", e),
        }
    }

    fn execute_statement<'a>(&'a mut self, stmt: Statement) -> futures::future::LocalBoxFuture<'a, Result<String, String>> {
        use futures::future::FutureExt;
        async move {
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

                    match name.as_str() {
                        "print" => {
                            Ok(evaluated_args.join(" "))
                        }
                        "len" => {
                            Ok(evaluated_args.get(0).map(|s| s.len().to_string()).unwrap_or("0".to_string()))
                        }
                        "sleep" => {
                            if let Some(ms_str) = evaluated_args.get(0) {
                                if let Ok(ms) = ms_str.parse::<f64>() {
                                    #[cfg(target_arch = "wasm32")]
                                    crate::js_sleep(ms).await;
                                }
                            }
                            Ok(String::new())
                        }
                        "http_get" => {
                            if let Some(url) = evaluated_args.get(0) {
                                #[cfg(target_arch = "wasm32")]
                                {
                                    let val = crate::js_http_get(url).await;
                                    Ok(val.as_string().unwrap_or_default())
                                }
                                #[cfg(not(target_arch = "wasm32"))]
                                { Ok(format!("[HTTP GET {}]", url)) }
                            } else {
                                Err("http_get requires a URL".to_string())
                            }
                        }
                        _ => {
                            if let Some(executor) = self.cmd_executor {
                                let cmd_line = format!("{} {}", name, evaluated_args.join(" "));
                                executor(cmd_line).await
                            } else {
                                Ok(format!("[Executed @{}]", name))
                            }
                        }
                    }
                }
                Statement::If(condition, then_block, else_block) => {
                    let val = self.evaluate_expr(condition).await?;
                    if !val.is_empty() && val != "false" && val != "0" {
                        let mut out = Vec::new();
                        for s in then_block {
                            out.push(self.execute_statement(s).await?);
                        }
                        Ok(out.join("\n"))
                    } else if let Some(eb) = else_block {
                        let mut out = Vec::new();
                        for s in eb {
                            out.push(self.execute_statement(s).await?);
                        }
                        Ok(out.join("\n"))
                    } else {
                        Ok(String::new())
                    }
                }
                Statement::For(var_name, list_expr, block) => {
                    let val = self.evaluate_expr(list_expr).await?;
                    let items: Vec<&str> = val.split_whitespace().collect();
                    let mut out = Vec::new();
                    for item in items {
                        self.variables.insert(var_name.clone(), item.to_string());
                        for s in &block {
                            out.push(self.execute_statement(s.clone()).await?);
                        }
                    }
                    Ok(out.join("\n"))
                }
                Statement::While(condition, block) => {
                    let mut out = Vec::new();
                    let mut iterations = 0;
                    loop {
                        let val = self.evaluate_expr(condition.clone()).await?;
                        if val.is_empty() || val == "false" || val == "0" {
                            break;
                        }
                        for s in &block {
                            out.push(self.execute_statement(s.clone()).await?);
                        }
                        iterations += 1;
                        if iterations > 1000 { return Err("Infinite loop detected".to_string()); }
                    }
                    Ok(out.join("\n"))
                }
            }
        }.boxed_local()
    }

    fn evaluate_expr<'a>(&'a self, expr: Expr) -> futures::future::LocalBoxFuture<'a, Result<String, String>> {
        use futures::future::FutureExt;
        async move {
            match expr {
                Expr::Literal(s) => Ok(s),
                Expr::Variable(name) => {
                    if let Some(val) = self.variables.get(&name) {
                        Ok(val.clone())
                    } else {
                        let vfs = crate::vfs::get_vfs().lock().unwrap();
                        if let Some(val) = vfs.env_vars.get(&name) {
                            Ok(val.clone())
                        } else {
                            Err(format!("Undefined variable: ${}", name))
                        }
                    }
                },
                Expr::Backtick(cmd) => {
                    if let Some(executor) = self.cmd_executor {
                        (executor)(cmd).await
                    } else {
                        Err("No command executor provided".to_string())
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
                        _ => Err(format!("Unsupported operator: {}", op)),
                    }
                }
            }
        }.boxed_local()
    }
}
