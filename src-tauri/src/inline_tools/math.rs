/// Recursive-descent arithmetic expression evaluator.
///
/// Supports `+`, `-`, `*`, `/`, `%`, `^` (right-associative), unary `+`/`-`,
/// parentheses, constants (`pi`, `e`, `tau`), and functions
/// (`sqrt`, `abs`, `sin`, `cos`, `tan`, `ln`, `log`, `round`, `floor`, `ceil`).

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    LParen,
    RParen,
}

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }

        match c {
            '+' => {
                tokens.push(Token::Plus);
                i += 1;
            }
            '-' => {
                tokens.push(Token::Minus);
                i += 1;
            }
            '*' => {
                tokens.push(Token::Star);
                i += 1;
            }
            '/' => {
                tokens.push(Token::Slash);
                i += 1;
            }
            '%' => {
                tokens.push(Token::Percent);
                i += 1;
            }
            '^' => {
                tokens.push(Token::Caret);
                i += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            '0'..='9' | '.' => {
                let start = i;
                let mut has_dot = c == '.';
                i += 1;
                while i < chars.len() {
                    let ch = chars[i];
                    if ch.is_ascii_digit() || ch == '_' {
                        i += 1;
                    } else if ch == '.' && !has_dot {
                        has_dot = true;
                        i += 1;
                    } else if (ch == 'e' || ch == 'E')
                        && i + 1 < chars.len()
                        && (chars[i + 1].is_ascii_digit()
                            || chars[i + 1] == '+'
                            || chars[i + 1] == '-')
                    {
                        i += 1;
                        if chars[i] == '+' || chars[i] == '-' {
                            i += 1;
                        }
                    } else {
                        break;
                    }
                }
                let raw: String = chars[start..i].iter().filter(|&&ch| ch != '_').collect();
                let val: f64 = raw
                    .parse()
                    .map_err(|_| format!("Invalid number: '{raw}'"))?;
                tokens.push(Token::Number(val));
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let start = i;
                i += 1;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let ident: String = chars[start..i].iter().collect();
                tokens.push(Token::Ident(ident.to_ascii_lowercase()));
            }
            _ => return Err(format!("Unexpected character: '{c}'")),
        }
    }

    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<Token> {
        let tok = self.tokens.get(self.pos).cloned();
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn parse_expr(&mut self) -> Result<f64, String> {
        self.parse_add_sub()
    }

    fn parse_add_sub(&mut self) -> Result<f64, String> {
        let mut left = self.parse_mul_div()?;
        while let Some(tok) = self.peek() {
            match tok {
                Token::Plus => {
                    self.next();
                    left += self.parse_mul_div()?;
                }
                Token::Minus => {
                    self.next();
                    left -= self.parse_mul_div()?;
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_mul_div(&mut self) -> Result<f64, String> {
        let mut left = self.parse_power()?;
        while let Some(tok) = self.peek() {
            match tok {
                Token::Star => {
                    self.next();
                    left *= self.parse_power()?;
                }
                Token::Slash => {
                    self.next();
                    let right = self.parse_power()?;
                    if right == 0.0 {
                        return Err("Division by zero".to_string());
                    }
                    left /= right;
                }
                Token::Percent => {
                    self.next();
                    let right = self.parse_power()?;
                    if right == 0.0 {
                        return Err("Modulo by zero".to_string());
                    }
                    left %= right;
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_power(&mut self) -> Result<f64, String> {
        let base = self.parse_unary()?;
        if matches!(self.peek(), Some(Token::Caret)) {
            self.next();
            let exp = self.parse_power()?;
            Ok(base.powf(exp))
        } else {
            Ok(base)
        }
    }

    fn parse_unary(&mut self) -> Result<f64, String> {
        match self.peek() {
            Some(Token::Plus) => {
                self.next();
                self.parse_unary()
            }
            Some(Token::Minus) => {
                self.next();
                Ok(-self.parse_unary()?)
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<f64, String> {
        match self.next() {
            Some(Token::Number(n)) => Ok(n),
            Some(Token::LParen) => {
                let val = self.parse_expr()?;
                match self.next() {
                    Some(Token::RParen) => Ok(val),
                    _ => Err("Expected closing ')'".to_string()),
                }
            }
            Some(Token::Ident(name)) => {
                if matches!(self.peek(), Some(Token::LParen)) {
                    self.next(); // consume '('
                    let arg = self.parse_expr()?;
                    match self.next() {
                        Some(Token::RParen) => eval_function(&name, arg),
                        _ => Err(format!("Expected ')' after function '{name}'")),
                    }
                } else {
                    match name.as_str() {
                        "pi" => Ok(std::f64::consts::PI),
                        "e" => Ok(std::f64::consts::E),
                        "tau" => Ok(std::f64::consts::TAU),
                        _ => Err(format!("Unknown identifier: '{name}'")),
                    }
                }
            }
            Some(other) => Err(format!("Unexpected token: {other:?}")),
            None => Err("Unexpected end of expression".to_string()),
        }
    }
}

fn eval_function(name: &str, arg: f64) -> Result<f64, String> {
    match name {
        "sqrt" => {
            if arg < 0.0 {
                Err("sqrt of negative number".to_string())
            } else {
                Ok(arg.sqrt())
            }
        }
        "abs" => Ok(arg.abs()),
        "sin" => Ok(arg.sin()),
        "cos" => Ok(arg.cos()),
        "tan" => Ok(arg.tan()),
        "ln" => {
            if arg <= 0.0 {
                Err("ln of non-positive number".to_string())
            } else {
                Ok(arg.ln())
            }
        }
        "log" => {
            if arg <= 0.0 {
                Err("log of non-positive number".to_string())
            } else {
                Ok(arg.log10())
            }
        }
        "round" => Ok(arg.round()),
        "floor" => Ok(arg.floor()),
        "ceil" => Ok(arg.ceil()),
        _ => Err(format!("Unknown function: '{name}'")),
    }
}

/// Evaluates an arithmetic expression and returns the numeric result.
pub fn eval_math(input: &str) -> Result<f64, String> {
    let tokens = tokenize(input)?;
    if tokens.is_empty() {
        return Err("Empty expression".to_string());
    }
    let mut parser = Parser::new(tokens);
    let val = parser.parse_expr()?;
    if parser.pos < parser.tokens.len() {
        return Err("Unexpected trailing tokens".to_string());
    }
    if !val.is_finite() {
        return Err("Result is not finite".to_string());
    }
    Ok(val)
}

/// Formats a floating-point number cleanly, stripping floating-point noise and trailing zeros.
pub fn format_number(val: f64) -> String {
    if val == 0.0 {
        return "0".to_string();
    }
    // Round to 10 significant decimal places to suppress 0.1 + 0.2 = 0.30000000000000004
    let rounded = format!("{val:.10}");
    let trimmed = rounded.trim_end_matches('0').trim_end_matches('.');
    if trimmed == "-0" {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Returns `true` if `input` looks like an arithmetic expression suitable for auto-detection
/// without an `=` prefix (i.e. contains an operator, parens, or math function).
pub fn looks_like_math_expr(input: &str) -> bool {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return false;
    }
    let has_operator = trimmed.contains('+')
        || trimmed.contains('-')
        || trimmed.contains('*')
        || trimmed.contains('/')
        || trimmed.contains('%')
        || trimmed.contains('^');
    let has_paren = trimmed.contains('(') && trimmed.contains(')');
    if !has_operator && !has_paren {
        return false;
    }
    // Must contain at least one digit to avoid matching plain words with hyphens like "foo-bar".
    trimmed.chars().any(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precedence_and_parentheses() {
        assert_eq!(eval_math("1920 * 0.75").unwrap(), 1440.0);
        assert_eq!(eval_math("2 + 3 * 4").unwrap(), 14.0);
        assert_eq!(eval_math("(2 + 3) * 4").unwrap(), 20.0);
        assert_eq!(eval_math("2 ^ 3 ^ 2").unwrap(), 512.0);
        assert_eq!(eval_math("-5 + 12 / 3").unwrap(), -1.0);
    }

    #[test]
    fn functions_and_constants() {
        assert_eq!(eval_math("sqrt(144)").unwrap(), 12.0);
        assert_eq!(format_number(eval_math("0.1 + 0.2").unwrap()), "0.3");
        assert!((eval_math("pi * 2").unwrap() - std::f64::consts::TAU).abs() < 1e-12);
    }

    #[test]
    fn error_cases() {
        assert!(eval_math("10 / 0").is_err());
        assert!(eval_math("(2 + 3").is_err());
        assert!(eval_math("sqrt(-1)").is_err());
        assert!(eval_math("unknown_var + 1").is_err());
    }

    #[test]
    fn auto_detect_heuristics() {
        assert!(looks_like_math_expr("1920 * 0.75"));
        assert!(looks_like_math_expr("(2+3)*4"));
        assert!(!looks_like_math_expr("spotify"));
        assert!(!looks_like_math_expr("foo-bar"));
        assert!(!looks_like_math_expr("2024"));
    }
}
