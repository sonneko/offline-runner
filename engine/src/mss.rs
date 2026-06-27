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
}

#[derive(Debug, Clone)]
pub enum Statement {
    Assignment(String, Expr),
    If(Expr, Vec<Statement>, Option<Vec<Statement>>),
    CommandCall(String, Vec<Expr>),
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

fn parse_expr(input: &str) -> IResult<&str, Expr> {
    alt((parse_literal, parse_variable)).parse(input)
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
    alt((parse_if_stmt, parse_assignment, parse_command_call_stmt)).parse(input)
}

fn parse_program(input: &str) -> IResult<&str, Vec<Statement>> {
    many0(terminated(parse_statement, multispace0)).parse(input)
}

pub struct Interpreter {
    variables: HashMap<String, String>,
}

impl Interpreter {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    pub fn run(&mut self, code: &str) -> String {
        match parse_program(code) {
            Ok((_, statements)) => {
                let mut output = Vec::new();
                for stmt in statements {
                    match self.execute_statement(stmt) {
                        Ok(res) => if !res.is_empty() { output.push(res); },
                        Err(e) => return format!("Runtime Error: {}", e),
                    }
                }
                output.join("\n")
            }
            Err(e) => format!("Parse Error: {:?}", e),
        }
    }

    fn execute_statement(&mut self, stmt: Statement) -> Result<String, String> {
        match stmt {
            Statement::Assignment(name, expr) => {
                let val = self.evaluate_expr(expr)?;
                self.variables.insert(name, val);
                Ok(String::new())
            }
            Statement::CommandCall(name, args) => {
                let mut _evaluated_args = Vec::new();
                for arg in args {
                    _evaluated_args.push(self.evaluate_expr(arg)?);
                }
                Ok(format!("[Executed @{}]", name))
            }
            Statement::If(condition, then_block, else_block) => {
                let val = self.evaluate_expr(condition)?;
                if !val.is_empty() && val != "false" && val != "0" {
                    let mut out = Vec::new();
                    for s in then_block {
                        out.push(self.execute_statement(s)?);
                    }
                    Ok(out.join("\n"))
                } else if let Some(eb) = else_block {
                    let mut out = Vec::new();
                    for s in eb {
                        out.push(self.execute_statement(s)?);
                    }
                    Ok(out.join("\n"))
                } else {
                    Ok(String::new())
                }
            }
        }
    }

    fn evaluate_expr(&self, expr: Expr) -> Result<String, String> {
        match expr {
            Expr::Literal(s) => Ok(s),
            Expr::Variable(name) => self.variables.get(&name).cloned().ok_or_else(|| format!("Undefined variable: ${}", name)),
        }
    }
}
