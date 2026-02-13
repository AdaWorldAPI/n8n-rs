//! Expression parser for n8n expression syntax.
//!
//! Parses expressions like:
//! - `{{ $json.field }}`
//! - `{{ $node["Name"].json.field }}`
//! - `{{ $input.first().json }}`
//! - `{{ $json.name.toUpperCase() }}`

use super::ExpressionError;

/// Parsed expression AST node.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Literal value.
    Literal(Literal),
    /// Variable reference ($json, $input, etc.).
    Variable(String),
    /// Property access (a.b).
    PropertyAccess {
        object: Box<Expr>,
        property: String,
    },
    /// Index access (a[0] or a["key"]).
    IndexAccess {
        object: Box<Expr>,
        index: Box<Expr>,
    },
    /// Method call (a.method(args)).
    MethodCall {
        object: Box<Expr>,
        method: String,
        args: Vec<Expr>,
    },
    /// Function call (func(args)).
    FunctionCall {
        name: String,
        args: Vec<Expr>,
    },
    /// Binary operation (a + b, a == b, etc.).
    BinaryOp {
        left: Box<Expr>,
        op: BinaryOperator,
        right: Box<Expr>,
    },
    /// Unary operation (!a, -a).
    UnaryOp {
        op: UnaryOperator,
        operand: Box<Expr>,
    },
    /// Conditional (a ? b : c).
    Conditional {
        condition: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
    },
    /// Array literal [a, b, c].
    Array(Vec<Expr>),
    /// Object literal {a: b, c: d}.
    Object(Vec<(String, Expr)>),
    /// Template literal with embedded expressions.
    Template(Vec<TemplatePart>),
}

/// Literal values.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    // Comparison
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    // Logical
    And,
    Or,
    // Nullish coalescing
    NullishCoalesce,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    Not,
    Neg,
}

/// Part of a template literal.
#[derive(Debug, Clone, PartialEq)]
pub enum TemplatePart {
    String(String),
    Expression(Box<Expr>),
}

/// Token type for lexing.
#[derive(Debug, Clone, PartialEq)]
enum Token {
    // Literals
    Null,
    True,
    False,
    Number(f64),
    String(String),
    // Identifiers
    Ident(String),
    Variable(String), // $json, $input, etc.
    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Not,
    NullishCoalesce, // ??
    // Punctuation
    Dot,
    Comma,
    Colon,
    Question,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    // Special
    Eof,
}

/// Lexer for tokenizing expression strings.
struct Lexer<'a> {
    input: &'a str,
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    current_pos: usize,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            chars: input.char_indices().peekable(),
            current_pos: 0,
        }
    }

    fn next_token(&mut self) -> Result<Token, ExpressionError> {
        self.skip_whitespace();

        let Some(&(pos, ch)) = self.chars.peek() else {
            return Ok(Token::Eof);
        };
        self.current_pos = pos;

        match ch {
            // Single character tokens
            '+' => {
                self.chars.next();
                Ok(Token::Plus)
            }
            '-' => {
                self.chars.next();
                Ok(Token::Minus)
            }
            '*' => {
                self.chars.next();
                Ok(Token::Star)
            }
            '/' => {
                self.chars.next();
                Ok(Token::Slash)
            }
            '%' => {
                self.chars.next();
                Ok(Token::Percent)
            }
            '.' => {
                self.chars.next();
                Ok(Token::Dot)
            }
            ',' => {
                self.chars.next();
                Ok(Token::Comma)
            }
            ':' => {
                self.chars.next();
                Ok(Token::Colon)
            }
            '(' => {
                self.chars.next();
                Ok(Token::LParen)
            }
            ')' => {
                self.chars.next();
                Ok(Token::RParen)
            }
            '[' => {
                self.chars.next();
                Ok(Token::LBracket)
            }
            ']' => {
                self.chars.next();
                Ok(Token::RBracket)
            }
            '{' => {
                self.chars.next();
                Ok(Token::LBrace)
            }
            '}' => {
                self.chars.next();
                Ok(Token::RBrace)
            }

            // Multi-character operators
            '?' => {
                self.chars.next();
                if self.chars.peek().map(|&(_, c)| c) == Some('?') {
                    self.chars.next();
                    Ok(Token::NullishCoalesce)
                } else {
                    Ok(Token::Question)
                }
            }
            '=' => {
                self.chars.next();
                if self.chars.peek().map(|&(_, c)| c) == Some('=') {
                    self.chars.next();
                    // Check for ===
                    if self.chars.peek().map(|&(_, c)| c) == Some('=') {
                        self.chars.next();
                    }
                    Ok(Token::Eq)
                } else {
                    Err(ExpressionError::ParseError(
                        "Unexpected '=' - did you mean '=='?".to_string(),
                    ))
                }
            }
            '!' => {
                self.chars.next();
                if self.chars.peek().map(|&(_, c)| c) == Some('=') {
                    self.chars.next();
                    // Check for !==
                    if self.chars.peek().map(|&(_, c)| c) == Some('=') {
                        self.chars.next();
                    }
                    Ok(Token::Ne)
                } else {
                    Ok(Token::Not)
                }
            }
            '<' => {
                self.chars.next();
                if self.chars.peek().map(|&(_, c)| c) == Some('=') {
                    self.chars.next();
                    Ok(Token::Le)
                } else {
                    Ok(Token::Lt)
                }
            }
            '>' => {
                self.chars.next();
                if self.chars.peek().map(|&(_, c)| c) == Some('=') {
                    self.chars.next();
                    Ok(Token::Ge)
                } else {
                    Ok(Token::Gt)
                }
            }
            '&' => {
                self.chars.next();
                if self.chars.peek().map(|&(_, c)| c) == Some('&') {
                    self.chars.next();
                    Ok(Token::And)
                } else {
                    Err(ExpressionError::ParseError(
                        "Unexpected '&' - did you mean '&&'?".to_string(),
                    ))
                }
            }
            '|' => {
                self.chars.next();
                if self.chars.peek().map(|&(_, c)| c) == Some('|') {
                    self.chars.next();
                    Ok(Token::Or)
                } else {
                    Err(ExpressionError::ParseError(
                        "Unexpected '|' - did you mean '||'?".to_string(),
                    ))
                }
            }

            // String literals
            '"' | '\'' => self.read_string(),

            // Numbers
            '0'..='9' => self.read_number(),

            // Variables ($json, $input, etc.)
            '$' => self.read_variable(),

            // Identifiers and keywords
            'a'..='z' | 'A'..='Z' | '_' => self.read_identifier(),

            _ => Err(ExpressionError::ParseError(format!(
                "Unexpected character: '{}'",
                ch
            ))),
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(&(_, ch)) = self.chars.peek() {
            if ch.is_whitespace() {
                self.chars.next();
            } else {
                break;
            }
        }
    }

    fn read_string(&mut self) -> Result<Token, ExpressionError> {
        let quote = self.chars.next().unwrap().1;
        let mut s = String::new();

        loop {
            match self.chars.next() {
                Some((_, ch)) if ch == quote => break,
                Some((_, '\\')) => {
                    // Escape sequence
                    match self.chars.next() {
                        Some((_, 'n')) => s.push('\n'),
                        Some((_, 'r')) => s.push('\r'),
                        Some((_, 't')) => s.push('\t'),
                        Some((_, '\\')) => s.push('\\'),
                        Some((_, c)) if c == quote => s.push(c),
                        Some((_, c)) => {
                            s.push('\\');
                            s.push(c);
                        }
                        None => {
                            return Err(ExpressionError::ParseError(
                                "Unterminated string".to_string(),
                            ))
                        }
                    }
                }
                Some((_, ch)) => s.push(ch),
                None => {
                    return Err(ExpressionError::ParseError(
                        "Unterminated string".to_string(),
                    ))
                }
            }
        }

        Ok(Token::String(s))
    }

    fn read_number(&mut self) -> Result<Token, ExpressionError> {
        let start = self.current_pos;
        let mut end = start;

        while let Some(&(pos, ch)) = self.chars.peek() {
            if ch.is_ascii_digit() || ch == '.' {
                end = pos + ch.len_utf8();
                self.chars.next();
            } else {
                break;
            }
        }

        let num_str = &self.input[start..end];
        let num: f64 = num_str.parse().map_err(|_| {
            ExpressionError::ParseError(format!("Invalid number: {}", num_str))
        })?;

        Ok(Token::Number(num))
    }

    fn read_variable(&mut self) -> Result<Token, ExpressionError> {
        self.chars.next(); // consume '$'
        let start = self
            .chars
            .peek()
            .map(|&(pos, _)| pos)
            .unwrap_or(self.input.len());
        let mut end = start;

        while let Some(&(pos, ch)) = self.chars.peek() {
            if ch.is_alphanumeric() || ch == '_' {
                end = pos + ch.len_utf8();
                self.chars.next();
            } else {
                break;
            }
        }

        let var_name = &self.input[start..end];
        if var_name.is_empty() {
            return Err(ExpressionError::ParseError(
                "Expected variable name after '$'".to_string(),
            ));
        }

        Ok(Token::Variable(var_name.to_string()))
    }

    fn read_identifier(&mut self) -> Result<Token, ExpressionError> {
        let start = self.current_pos;
        let mut end = start;

        while let Some(&(pos, ch)) = self.chars.peek() {
            if ch.is_alphanumeric() || ch == '_' {
                end = pos + ch.len_utf8();
                self.chars.next();
            } else {
                break;
            }
        }

        let ident = &self.input[start..end];

        // Check for keywords
        match ident {
            "null" => Ok(Token::Null),
            "true" => Ok(Token::True),
            "false" => Ok(Token::False),
            _ => Ok(Token::Ident(ident.to_string())),
        }
    }
}

/// Parser for building AST from tokens.
pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current: Token,
}

impl<'a> Parser<'a> {
    /// Create a new parser for the given expression.
    pub fn new(input: &'a str) -> Result<Self, ExpressionError> {
        let mut lexer = Lexer::new(input);
        let current = lexer.next_token()?;
        Ok(Self { lexer, current })
    }

    /// Parse the expression into an AST.
    pub fn parse(&mut self) -> Result<Expr, ExpressionError> {
        self.parse_expression()
    }

    fn advance(&mut self) -> Result<Token, ExpressionError> {
        let prev = std::mem::replace(&mut self.current, self.lexer.next_token()?);
        Ok(prev)
    }

    fn expect(&mut self, expected: Token) -> Result<(), ExpressionError> {
        if self.current == expected {
            self.advance()?;
            Ok(())
        } else {
            Err(ExpressionError::ParseError(format!(
                "Expected {:?}, got {:?}",
                expected, self.current
            )))
        }
    }

    fn parse_expression(&mut self) -> Result<Expr, ExpressionError> {
        self.parse_conditional()
    }

    fn parse_conditional(&mut self) -> Result<Expr, ExpressionError> {
        let mut expr = self.parse_or()?;

        if self.current == Token::Question {
            self.advance()?;
            let then_expr = self.parse_expression()?;
            self.expect(Token::Colon)?;
            let else_expr = self.parse_expression()?;
            expr = Expr::Conditional {
                condition: Box::new(expr),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
            };
        }

        Ok(expr)
    }

    fn parse_or(&mut self) -> Result<Expr, ExpressionError> {
        let mut left = self.parse_and()?;

        while self.current == Token::Or {
            self.advance()?;
            let right = self.parse_and()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Or,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, ExpressionError> {
        let mut left = self.parse_nullish_coalesce()?;

        while self.current == Token::And {
            self.advance()?;
            let right = self.parse_nullish_coalesce()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::And,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_nullish_coalesce(&mut self) -> Result<Expr, ExpressionError> {
        let mut left = self.parse_equality()?;

        while self.current == Token::NullishCoalesce {
            self.advance()?;
            let right = self.parse_equality()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::NullishCoalesce,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr, ExpressionError> {
        let mut left = self.parse_comparison()?;

        loop {
            let op = match &self.current {
                Token::Eq => BinaryOperator::Eq,
                Token::Ne => BinaryOperator::Ne,
                _ => break,
            };
            self.advance()?;
            let right = self.parse_comparison()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr, ExpressionError> {
        let mut left = self.parse_additive()?;

        loop {
            let op = match &self.current {
                Token::Lt => BinaryOperator::Lt,
                Token::Le => BinaryOperator::Le,
                Token::Gt => BinaryOperator::Gt,
                Token::Ge => BinaryOperator::Ge,
                _ => break,
            };
            self.advance()?;
            let right = self.parse_additive()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expr, ExpressionError> {
        let mut left = self.parse_multiplicative()?;

        loop {
            let op = match &self.current {
                Token::Plus => BinaryOperator::Add,
                Token::Minus => BinaryOperator::Sub,
                _ => break,
            };
            self.advance()?;
            let right = self.parse_multiplicative()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, ExpressionError> {
        let mut left = self.parse_unary()?;

        loop {
            let op = match &self.current {
                Token::Star => BinaryOperator::Mul,
                Token::Slash => BinaryOperator::Div,
                Token::Percent => BinaryOperator::Mod,
                _ => break,
            };
            self.advance()?;
            let right = self.parse_unary()?;
            left = Expr::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, ExpressionError> {
        match &self.current {
            Token::Not => {
                self.advance()?;
                let operand = self.parse_unary()?;
                Ok(Expr::UnaryOp {
                    op: UnaryOperator::Not,
                    operand: Box::new(operand),
                })
            }
            Token::Minus => {
                self.advance()?;
                let operand = self.parse_unary()?;
                Ok(Expr::UnaryOp {
                    op: UnaryOperator::Neg,
                    operand: Box::new(operand),
                })
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, ExpressionError> {
        let mut expr = self.parse_primary()?;

        loop {
            match &self.current {
                Token::Dot => {
                    self.advance()?;
                    match self.advance()? {
                        Token::Ident(name) => {
                            // Check if it's a method call
                            if self.current == Token::LParen {
                                self.advance()?;
                                let args = self.parse_argument_list()?;
                                self.expect(Token::RParen)?;
                                expr = Expr::MethodCall {
                                    object: Box::new(expr),
                                    method: name,
                                    args,
                                };
                            } else {
                                expr = Expr::PropertyAccess {
                                    object: Box::new(expr),
                                    property: name,
                                };
                            }
                        }
                        other => {
                            return Err(ExpressionError::ParseError(format!(
                                "Expected property name after '.', got {:?}",
                                other
                            )))
                        }
                    }
                }
                Token::LBracket => {
                    self.advance()?;
                    let index = self.parse_expression()?;
                    self.expect(Token::RBracket)?;
                    expr = Expr::IndexAccess {
                        object: Box::new(expr),
                        index: Box::new(index),
                    };
                }
                Token::LParen => {
                    // Function call on expression
                    if let Expr::Variable(name) = &expr {
                        self.advance()?;
                        let args = self.parse_argument_list()?;
                        self.expect(Token::RParen)?;
                        expr = Expr::FunctionCall {
                            name: name.clone(),
                            args,
                        };
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, ExpressionError> {
        match self.advance()? {
            Token::Null => Ok(Expr::Literal(Literal::Null)),
            Token::True => Ok(Expr::Literal(Literal::Boolean(true))),
            Token::False => Ok(Expr::Literal(Literal::Boolean(false))),
            Token::Number(n) => Ok(Expr::Literal(Literal::Number(n))),
            Token::String(s) => Ok(Expr::Literal(Literal::String(s))),
            Token::Variable(name) => Ok(Expr::Variable(name)),
            Token::Ident(name) => {
                // Check if it's a function call
                if self.current == Token::LParen {
                    self.advance()?;
                    let args = self.parse_argument_list()?;
                    self.expect(Token::RParen)?;
                    Ok(Expr::FunctionCall { name, args })
                } else {
                    // Treat as variable-like identifier
                    Ok(Expr::Variable(name))
                }
            }
            Token::LParen => {
                let expr = self.parse_expression()?;
                self.expect(Token::RParen)?;
                Ok(expr)
            }
            Token::LBracket => {
                let mut elements = Vec::new();
                if self.current != Token::RBracket {
                    elements.push(self.parse_expression()?);
                    while self.current == Token::Comma {
                        self.advance()?;
                        if self.current == Token::RBracket {
                            break; // trailing comma
                        }
                        elements.push(self.parse_expression()?);
                    }
                }
                self.expect(Token::RBracket)?;
                Ok(Expr::Array(elements))
            }
            Token::LBrace => {
                let mut pairs = Vec::new();
                if self.current != Token::RBrace {
                    pairs.push(self.parse_object_pair()?);
                    while self.current == Token::Comma {
                        self.advance()?;
                        if self.current == Token::RBrace {
                            break; // trailing comma
                        }
                        pairs.push(self.parse_object_pair()?);
                    }
                }
                self.expect(Token::RBrace)?;
                Ok(Expr::Object(pairs))
            }
            other => Err(ExpressionError::ParseError(format!(
                "Unexpected token: {:?}",
                other
            ))),
        }
    }

    fn parse_object_pair(&mut self) -> Result<(String, Expr), ExpressionError> {
        let key = match self.advance()? {
            Token::Ident(name) | Token::String(name) => name,
            other => {
                return Err(ExpressionError::ParseError(format!(
                    "Expected object key, got {:?}",
                    other
                )))
            }
        };
        self.expect(Token::Colon)?;
        let value = self.parse_expression()?;
        Ok((key, value))
    }

    fn parse_argument_list(&mut self) -> Result<Vec<Expr>, ExpressionError> {
        let mut args = Vec::new();
        if self.current != Token::RParen {
            args.push(self.parse_expression()?);
            while self.current == Token::Comma {
                self.advance()?;
                args.push(self.parse_expression()?);
            }
        }
        Ok(args)
    }
}

/// Parse an expression string into an AST.
pub fn parse(input: &str) -> Result<Expr, ExpressionError> {
    let mut parser = Parser::new(input)?;
    let expr = parser.parse()?;

    // Ensure we consumed all input
    if parser.current != Token::Eof {
        return Err(ExpressionError::ParseError(format!(
            "Unexpected token after expression: {:?}",
            parser.current
        )));
    }

    Ok(expr)
}

/// Parse a template string with embedded expressions.
/// Template format: "Hello {{ $json.name }}!"
pub fn parse_template(input: &str) -> Result<Expr, ExpressionError> {
    let mut parts = Vec::new();
    let mut current_pos = 0;

    while current_pos < input.len() {
        if let Some(start) = input[current_pos..].find("{{") {
            // Add text before the expression
            if start > 0 {
                parts.push(TemplatePart::String(
                    input[current_pos..current_pos + start].to_string(),
                ));
            }
            current_pos += start + 2;

            // Find closing }}
            let end = input[current_pos..]
                .find("}}")
                .ok_or_else(|| ExpressionError::ParseError("Unclosed {{ in template".to_string()))?;

            // Parse the expression
            let expr_str = input[current_pos..current_pos + end].trim();
            let expr = parse(expr_str)?;
            parts.push(TemplatePart::Expression(Box::new(expr)));

            current_pos += end + 2;
        } else {
            // Rest is plain text
            parts.push(TemplatePart::String(input[current_pos..].to_string()));
            break;
        }
    }

    // If there's only one expression part and no strings, return the expression directly
    if parts.len() == 1 {
        match parts.into_iter().next() {
            Some(TemplatePart::Expression(expr)) => return Ok(*expr),
            Some(part) => return Ok(Expr::Template(vec![part])),
            None => unreachable!(),
        }
    }

    Ok(Expr::Template(parts))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_variable() {
        let expr = parse("$json").unwrap();
        assert_eq!(expr, Expr::Variable("json".to_string()));
    }

    #[test]
    fn test_parse_property_access() {
        let expr = parse("$json.name").unwrap();
        assert_eq!(
            expr,
            Expr::PropertyAccess {
                object: Box::new(Expr::Variable("json".to_string())),
                property: "name".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_index_access() {
        let expr = parse("$json[0]").unwrap();
        assert_eq!(
            expr,
            Expr::IndexAccess {
                object: Box::new(Expr::Variable("json".to_string())),
                index: Box::new(Expr::Literal(Literal::Number(0.0))),
            }
        );
    }

    #[test]
    fn test_parse_method_call() {
        let expr = parse("$json.name.toUpperCase()").unwrap();
        assert_eq!(
            expr,
            Expr::MethodCall {
                object: Box::new(Expr::PropertyAccess {
                    object: Box::new(Expr::Variable("json".to_string())),
                    property: "name".to_string(),
                }),
                method: "toUpperCase".to_string(),
                args: vec![],
            }
        );
    }

    #[test]
    fn test_parse_binary_op() {
        let expr = parse("1 + 2").unwrap();
        assert_eq!(
            expr,
            Expr::BinaryOp {
                left: Box::new(Expr::Literal(Literal::Number(1.0))),
                op: BinaryOperator::Add,
                right: Box::new(Expr::Literal(Literal::Number(2.0))),
            }
        );
    }

    #[test]
    fn test_parse_template() {
        let expr = parse_template("Hello {{ $json.name }}!").unwrap();
        assert!(matches!(expr, Expr::Template(_)));
    }
}
