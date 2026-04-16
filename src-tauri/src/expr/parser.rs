/// Tokenizer and recursive-descent parser for wtch condition expressions.
///
/// Produces an `Expr` AST consumed by the evaluator in `eval.rs`.
use std::fmt;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// All errors produced by the tokenizer, parser, and evaluator.
#[derive(Debug, PartialEq, Clone)]
pub enum ExprError {
    /// The input string contains a character or token sequence that is not valid.
    UnexpectedToken(String),
    /// The parser encountered end-of-input earlier than expected.
    UnexpectedEof,
    /// A field referenced in an expression is absent from the JSON data.
    MissingField(String),
    /// The expression attempts an operation that is not defined for the given types
    /// (e.g. adding a string and a number).
    TypeError(String),
    /// Division by zero was attempted.
    DivisionByZero,
    /// An array index is out of bounds.
    IndexOutOfBounds(usize),
}

impl fmt::Display for ExprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedToken(t) => write!(f, "unexpected token: {t}"),
            Self::UnexpectedEof => write!(f, "unexpected end of expression"),
            Self::MissingField(name) => write!(f, "missing field: .{name}"),
            Self::TypeError(msg) => write!(f, "type error: {msg}"),
            Self::DivisionByZero => write!(f, "division by zero"),
            Self::IndexOutOfBounds(i) => write!(f, "index out of bounds: {i}"),
        }
    }
}

impl std::error::Error for ExprError {}

// ---------------------------------------------------------------------------
// Tokens
// ---------------------------------------------------------------------------

/// All token variants produced by the tokenizer.
#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    /// `.`
    Dot,
    /// An identifier (field name, keyword resolved to `And`/`Or`/`Not`/`Bool`)
    Ident(String),
    /// Numeric literal (stored as f64 for uniform handling)
    Number(f64),
    /// String literal (content without surrounding quotes)
    StringLit(String),
    /// Operator: `+`, `-`, `*`, `/`, `>`, `<`, `>=`, `<=`, `==`, `!=`
    Op(String),
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `and`
    And,
    /// `or`
    Or,
    /// `not`
    Not,
    /// `true` or `false`
    Bool(bool),
}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

/// Converts a raw expression string into a flat list of tokens.
pub fn tokenize(input: &str) -> Result<Vec<Token>, ExprError> {
    let chars: Vec<char> = input.chars().collect();
    let mut pos = 0;
    let mut tokens = Vec::new();

    while pos < chars.len() {
        match chars[pos] {
            ' ' | '\t' | '\r' | '\n' => {
                pos += 1;
            }
            '.' => {
                tokens.push(Token::Dot);
                pos += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                pos += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                pos += 1;
            }
            '[' => {
                tokens.push(Token::LBracket);
                pos += 1;
            }
            ']' => {
                tokens.push(Token::RBracket);
                pos += 1;
            }
            '"' => {
                pos += 1;
                let start = pos;
                while pos < chars.len() && chars[pos] != '"' {
                    // Basic escape: skip the escaped character.
                    if chars[pos] == '\\' {
                        pos += 1;
                    }
                    pos += 1;
                }
                if pos >= chars.len() {
                    return Err(ExprError::UnexpectedEof);
                }
                let content: String = chars[start..pos].iter().collect();
                tokens.push(Token::StringLit(content));
                pos += 1; // consume closing quote
            }
            '>' => {
                if pos + 1 < chars.len() && chars[pos + 1] == '=' {
                    tokens.push(Token::Op(">=".into()));
                    pos += 2;
                } else {
                    tokens.push(Token::Op(">".into()));
                    pos += 1;
                }
            }
            '<' => {
                if pos + 1 < chars.len() && chars[pos + 1] == '=' {
                    tokens.push(Token::Op("<=".into()));
                    pos += 2;
                } else {
                    tokens.push(Token::Op("<".into()));
                    pos += 1;
                }
            }
            '=' => {
                if pos + 1 < chars.len() && chars[pos + 1] == '=' {
                    tokens.push(Token::Op("==".into()));
                    pos += 2;
                } else {
                    return Err(ExprError::UnexpectedToken("=".into()));
                }
            }
            '!' => {
                if pos + 1 < chars.len() && chars[pos + 1] == '=' {
                    tokens.push(Token::Op("!=".into()));
                    pos += 2;
                } else {
                    return Err(ExprError::UnexpectedToken("!".into()));
                }
            }
            '+' => {
                tokens.push(Token::Op("+".into()));
                pos += 1;
            }
            '-' => {
                // A minus sign that is immediately followed by a digit is treated as
                // a negative number literal only when the previous token is not a
                // value-producing token (number, identifier, `)`, `]`).
                let prev_is_value = matches!(
                    tokens.last(),
                    Some(Token::Number(_))
                        | Some(Token::StringLit(_))
                        | Some(Token::Ident(_))
                        | Some(Token::Bool(_))
                        | Some(Token::RParen)
                        | Some(Token::RBracket)
                );
                if !prev_is_value
                    && pos + 1 < chars.len()
                    && chars[pos + 1].is_ascii_digit()
                {
                    // Parse as negative number.
                    pos += 1;
                    let (num, new_pos) = parse_number(&chars, pos);
                    tokens.push(Token::Number(-num));
                    pos = new_pos;
                } else {
                    tokens.push(Token::Op("-".into()));
                    pos += 1;
                }
            }
            '*' => {
                tokens.push(Token::Op("*".into()));
                pos += 1;
            }
            '/' => {
                tokens.push(Token::Op("/".into()));
                pos += 1;
            }
            c if c.is_ascii_digit() => {
                let (num, new_pos) = parse_number(&chars, pos);
                tokens.push(Token::Number(num));
                pos = new_pos;
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = pos;
                while pos < chars.len()
                    && (chars[pos].is_alphanumeric() || chars[pos] == '_')
                {
                    pos += 1;
                }
                let word: String = chars[start..pos].iter().collect();
                let token = match word.as_str() {
                    "and" => Token::And,
                    "or" => Token::Or,
                    "not" => Token::Not,
                    "true" => Token::Bool(true),
                    "false" => Token::Bool(false),
                    _ => Token::Ident(word),
                };
                tokens.push(token);
            }
            c => {
                return Err(ExprError::UnexpectedToken(c.to_string()));
            }
        }
    }

    Ok(tokens)
}

/// Parses a non-negative decimal number (integer or float) starting at `pos`.
/// Returns `(value, new_pos)`.
fn parse_number(chars: &[char], mut pos: usize) -> (f64, usize) {
    let start = pos;
    while pos < chars.len() && chars[pos].is_ascii_digit() {
        pos += 1;
    }
    if pos < chars.len() && chars[pos] == '.' {
        pos += 1;
        while pos < chars.len() && chars[pos].is_ascii_digit() {
            pos += 1;
        }
    }
    let s: String = chars[start..pos].iter().collect();
    let num: f64 = s.parse().unwrap_or(0.0);
    (num, pos)
}

// ---------------------------------------------------------------------------
// AST
// ---------------------------------------------------------------------------

/// Abstract syntax tree for a condition expression.
#[derive(Debug, PartialEq, Clone)]
pub enum Expr {
    /// A path into the JSON data, e.g. `.cost` or `.nested.field`.
    /// Each element is either a field name (`PathStep::Key`) or an array index
    /// (`PathStep::Index`).
    Field(Vec<PathStep>),
    /// A literal value.
    Literal(Literal),
    /// A binary operation.
    BinaryOp {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    /// A unary operation (`not`).
    UnaryOp { op: UnaryOp, operand: Box<Expr> },
}

/// One step in a field path.
#[derive(Debug, PartialEq, Clone)]
pub enum PathStep {
    Key(String),
    Index(usize),
}

/// Literal value kinds.
#[derive(Debug, PartialEq, Clone)]
pub enum Literal {
    Number(f64),
    Str(String),
    Bool(bool),
    Null,
}

/// Binary operator kinds.
#[derive(Debug, PartialEq, Clone)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Gt,
    Lt,
    Gte,
    Lte,
    Eq,
    Neq,
    And,
    Or,
}

/// Unary operator kinds.
#[derive(Debug, PartialEq, Clone)]
pub enum UnaryOp {
    Not,
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Recursive-descent parser that turns a token slice into an `Expr` AST.
///
/// Precedence (lowest to highest):
/// 1. `or`
/// 2. `and`
/// 3. `not` (unary prefix)
/// 4. Comparisons: `== != > < >= <=`
/// 5. Additive: `+ -`
/// 6. Multiplicative: `* /`
/// 7. Unary minus (not yet needed; literal negative numbers are handled in tokenizer)
/// 8. Primary: literal, field, grouped `(expr)`
struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        let t = self.tokens.get(self.pos);
        self.pos += 1;
        t
    }

    fn expect_token(&mut self, expected: &Token) -> Result<(), ExprError> {
        match self.advance() {
            Some(t) if t == expected => Ok(()),
            Some(t) => Err(ExprError::UnexpectedToken(format!("{t:?}"))),
            None => Err(ExprError::UnexpectedEof),
        }
    }

    // Level 1: or
    fn parse_or(&mut self) -> Result<Expr, ExprError> {
        let mut left = self.parse_and()?;
        while matches!(self.peek(), Some(Token::Or)) {
            self.advance();
            let right = self.parse_and()?;
            left = Expr::BinaryOp {
                op: BinOp::Or,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    // Level 2: and
    fn parse_and(&mut self) -> Result<Expr, ExprError> {
        let mut left = self.parse_not()?;
        while matches!(self.peek(), Some(Token::And)) {
            self.advance();
            let right = self.parse_not()?;
            left = Expr::BinaryOp {
                op: BinOp::And,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    // Level 3: not (unary prefix)
    fn parse_not(&mut self) -> Result<Expr, ExprError> {
        if matches!(self.peek(), Some(Token::Not)) {
            self.advance();
            let operand = self.parse_not()?;
            return Ok(Expr::UnaryOp {
                op: UnaryOp::Not,
                operand: Box::new(operand),
            });
        }
        self.parse_comparison()
    }

    // Level 4: comparisons
    fn parse_comparison(&mut self) -> Result<Expr, ExprError> {
        let left = self.parse_additive()?;
        let op = match self.peek() {
            Some(Token::Op(s)) => match s.as_str() {
                "==" => BinOp::Eq,
                "!=" => BinOp::Neq,
                ">" => BinOp::Gt,
                "<" => BinOp::Lt,
                ">=" => BinOp::Gte,
                "<=" => BinOp::Lte,
                _ => return Ok(left),
            },
            _ => return Ok(left),
        };
        self.advance();
        let right = self.parse_additive()?;
        Ok(Expr::BinaryOp {
            op,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    // Level 5: + -
    fn parse_additive(&mut self) -> Result<Expr, ExprError> {
        let mut left = self.parse_multiplicative()?;
        loop {
            let op = match self.peek() {
                Some(Token::Op(s)) if s == "+" => BinOp::Add,
                Some(Token::Op(s)) if s == "-" => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplicative()?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    // Level 6: * /
    fn parse_multiplicative(&mut self) -> Result<Expr, ExprError> {
        let mut left = self.parse_primary()?;
        loop {
            let op = match self.peek() {
                Some(Token::Op(s)) if s == "*" => BinOp::Mul,
                Some(Token::Op(s)) if s == "/" => BinOp::Div,
                _ => break,
            };
            self.advance();
            let right = self.parse_primary()?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    // Level 7: primary expressions
    fn parse_primary(&mut self) -> Result<Expr, ExprError> {
        match self.peek().cloned() {
            Some(Token::LParen) => {
                self.advance();
                let expr = self.parse_or()?;
                self.expect_token(&Token::RParen)?;
                Ok(expr)
            }
            Some(Token::Dot) => self.parse_field(),
            Some(Token::Number(n)) => {
                self.advance();
                Ok(Expr::Literal(Literal::Number(n)))
            }
            Some(Token::StringLit(s)) => {
                self.advance();
                Ok(Expr::Literal(Literal::Str(s)))
            }
            Some(Token::Bool(b)) => {
                self.advance();
                Ok(Expr::Literal(Literal::Bool(b)))
            }
            Some(t) => Err(ExprError::UnexpectedToken(format!("{t:?}"))),
            None => Err(ExprError::UnexpectedEof),
        }
    }

    /// Parse a field path starting with `.`.
    /// Handles: `.key`, `.key.sub`, `.key[0]`, `.key[0].sub`, etc.
    fn parse_field(&mut self) -> Result<Expr, ExprError> {
        let mut steps: Vec<PathStep> = Vec::new();

        // Consume the leading dot and first identifier.
        self.expect_token(&Token::Dot)?;
        match self.peek().cloned() {
            Some(Token::Ident(name)) => {
                self.advance();
                steps.push(PathStep::Key(name));
            }
            Some(t) => return Err(ExprError::UnexpectedToken(format!("{t:?}"))),
            None => return Err(ExprError::UnexpectedEof),
        }

        // Continue consuming `.key` or `[index]` chains.
        loop {
            match self.peek().cloned() {
                Some(Token::Dot) => {
                    self.advance();
                    match self.peek().cloned() {
                        Some(Token::Ident(name)) => {
                            self.advance();
                            steps.push(PathStep::Key(name));
                        }
                        Some(t) => {
                            return Err(ExprError::UnexpectedToken(format!("{t:?}")))
                        }
                        None => return Err(ExprError::UnexpectedEof),
                    }
                }
                Some(Token::LBracket) => {
                    self.advance();
                    match self.peek().cloned() {
                        Some(Token::Number(n)) => {
                            self.advance();
                            let idx = n as usize;
                            self.expect_token(&Token::RBracket)?;
                            steps.push(PathStep::Index(idx));
                        }
                        Some(t) => {
                            return Err(ExprError::UnexpectedToken(format!("{t:?}")))
                        }
                        None => return Err(ExprError::UnexpectedEof),
                    }
                }
                _ => break,
            }
        }

        Ok(Expr::Field(steps))
    }
}

// ---------------------------------------------------------------------------
// Public parse entry point
// ---------------------------------------------------------------------------

/// Parses an expression string into an AST.
pub fn parse(input: &str) -> Result<Expr, ExprError> {
    let tokens = tokenize(input)?;
    if tokens.is_empty() {
        return Err(ExprError::UnexpectedEof);
    }
    let mut parser = Parser::new(&tokens);
    let expr = parser.parse_or()?;
    if parser.pos < parser.tokens.len() {
        let leftover = &parser.tokens[parser.pos];
        return Err(ExprError::UnexpectedToken(format!("{leftover:?}")));
    }
    Ok(expr)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Tokenizer tests -----------------------------------------------------

    #[test]
    fn tokenize_simple_field() {
        let tokens = tokenize(".cost").unwrap();
        assert_eq!(tokens, vec![Token::Dot, Token::Ident("cost".into())]);
    }

    #[test]
    fn tokenize_comparison() {
        let tokens = tokenize(".cost > 50").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Dot,
                Token::Ident("cost".into()),
                Token::Op(">".into()),
                Token::Number(50.0),
            ]
        );
    }

    #[test]
    fn tokenize_string_literal() {
        let tokens = tokenize(r#".status == "ok""#).unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Dot,
                Token::Ident("status".into()),
                Token::Op("==".into()),
                Token::StringLit("ok".into()),
            ]
        );
    }

    #[test]
    fn tokenize_boolean_keywords() {
        let tokens = tokenize("true and false").unwrap();
        assert_eq!(
            tokens,
            vec![Token::Bool(true), Token::And, Token::Bool(false)]
        );
    }

    #[test]
    fn tokenize_not_keyword() {
        let tokens = tokenize("not .enabled").unwrap();
        assert_eq!(
            tokens,
            vec![Token::Not, Token::Dot, Token::Ident("enabled".into())]
        );
    }

    #[test]
    fn tokenize_array_index() {
        let tokens = tokenize(".items[0]").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Dot,
                Token::Ident("items".into()),
                Token::LBracket,
                Token::Number(0.0),
                Token::RBracket,
            ]
        );
    }

    #[test]
    fn tokenize_two_char_ops() {
        let ops = [">=", "<=", "==", "!="];
        for op in ops {
            let tokens = tokenize(&format!("1 {op} 2")).unwrap();
            assert_eq!(tokens[1], Token::Op(op.into()), "operator {op}");
        }
    }

    #[test]
    fn tokenize_float() {
        let tokens = tokenize("0.8").unwrap();
        assert_eq!(tokens, vec![Token::Number(0.8)]);
    }

    #[test]
    fn tokenize_negative_literal() {
        let tokens = tokenize("-5").unwrap();
        assert_eq!(tokens, vec![Token::Number(-5.0)]);
    }

    #[test]
    fn tokenize_error_bare_equals() {
        assert_eq!(
            tokenize("= foo"),
            Err(ExprError::UnexpectedToken("=".into()))
        );
    }

    #[test]
    fn tokenize_unterminated_string() {
        assert_eq!(tokenize(r#""hello"#), Err(ExprError::UnexpectedEof));
    }

    // -- Parser tests --------------------------------------------------------

    #[test]
    fn parse_simple_field() {
        let expr = parse(".cost").unwrap();
        assert_eq!(expr, Expr::Field(vec![PathStep::Key("cost".into())]));
    }

    #[test]
    fn parse_nested_field() {
        let expr = parse(".a.b").unwrap();
        assert_eq!(
            expr,
            Expr::Field(vec![PathStep::Key("a".into()), PathStep::Key("b".into())])
        );
    }

    #[test]
    fn parse_field_with_index() {
        let expr = parse(".items[0]").unwrap();
        assert_eq!(
            expr,
            Expr::Field(vec![PathStep::Key("items".into()), PathStep::Index(0)])
        );
    }

    #[test]
    fn parse_field_index_then_subfield() {
        let expr = parse(".items[0].name").unwrap();
        assert_eq!(
            expr,
            Expr::Field(vec![
                PathStep::Key("items".into()),
                PathStep::Index(0),
                PathStep::Key("name".into()),
            ])
        );
    }

    #[test]
    fn parse_comparison_gt() {
        let expr = parse(".cost > 50").unwrap();
        assert_eq!(
            expr,
            Expr::BinaryOp {
                op: BinOp::Gt,
                left: Box::new(Expr::Field(vec![PathStep::Key("cost".into())])),
                right: Box::new(Expr::Literal(Literal::Number(50.0))),
            }
        );
    }

    #[test]
    fn parse_arithmetic_precedence() {
        // `.limit * 0.8` must parse as `(* .limit 0.8)`, not mixed with outer `>`
        let expr = parse(".cost > .limit * 0.8").unwrap();
        match expr {
            Expr::BinaryOp { op: BinOp::Gt, left, right } => {
                assert_eq!(*left, Expr::Field(vec![PathStep::Key("cost".into())]));
                assert_eq!(
                    *right,
                    Expr::BinaryOp {
                        op: BinOp::Mul,
                        left: Box::new(Expr::Field(vec![PathStep::Key("limit".into())])),
                        right: Box::new(Expr::Literal(Literal::Number(0.8))),
                    }
                );
            }
            other => panic!("unexpected AST: {other:?}"),
        }
    }

    #[test]
    fn parse_and_or_precedence() {
        // `a and b or c` => `(or (and a b) c)`
        let expr = parse(".a and .b or .c").unwrap();
        match expr {
            Expr::BinaryOp { op: BinOp::Or, left, .. } => match *left {
                Expr::BinaryOp { op: BinOp::And, .. } => {}
                other => panic!("expected And on left of Or, got {other:?}"),
            },
            other => panic!("expected Or at root, got {other:?}"),
        }
    }

    #[test]
    fn parse_not_unary() {
        let expr = parse("not .enabled").unwrap();
        assert_eq!(
            expr,
            Expr::UnaryOp {
                op: UnaryOp::Not,
                operand: Box::new(Expr::Field(vec![PathStep::Key("enabled".into())])),
            }
        );
    }

    #[test]
    fn parse_grouped_expression() {
        let expr = parse("(.x > 1) and (.y < 10)").unwrap();
        match expr {
            Expr::BinaryOp { op: BinOp::And, left, right } => {
                assert!(matches!(*left, Expr::BinaryOp { op: BinOp::Gt, .. }));
                assert!(matches!(*right, Expr::BinaryOp { op: BinOp::Lt, .. }));
            }
            other => panic!("unexpected AST: {other:?}"),
        }
    }

    #[test]
    fn parse_string_equality() {
        let expr = parse(r#".status == "ok""#).unwrap();
        assert_eq!(
            expr,
            Expr::BinaryOp {
                op: BinOp::Eq,
                left: Box::new(Expr::Field(vec![PathStep::Key("status".into())])),
                right: Box::new(Expr::Literal(Literal::Str("ok".into()))),
            }
        );
    }

    #[test]
    fn parse_error_empty_input() {
        assert_eq!(parse(""), Err(ExprError::UnexpectedEof));
    }

    #[test]
    fn parse_error_unclosed_paren() {
        assert!(parse("(.cost > 1").is_err());
    }

    #[test]
    fn parse_error_trailing_token() {
        assert!(parse(".cost > 1 )").is_err());
    }
}
