use logos::Logos;

#[derive(Logos, Debug, PartialEq)]
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

    #[regex("[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),

    #[regex("\"[^\"]*\"", |lex| lex.slice()[1..lex.slice().len()-1].to_string())]
    String(String),

    #[regex("`[^`]*`", |lex| lex.slice()[1..lex.slice().len()-1].to_string())]
    Backtick(String),
}

pub struct Interpreter;

impl Interpreter {
    pub fn run(code: &str) -> String {
        let mut lex = Token::lexer(code);
        let mut tokens = Vec::new();
        while let Some(token) = lex.next() {
            match token {
                Ok(t) => tokens.push(format!("{:?}", t)),
                Err(_) => tokens.push("Error".to_string()),
            }
        }
        format!("Parsed tokens: {:?}", tokens)
    }
}
