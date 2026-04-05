//! JSONata expression parser.
//!
//! Parses a JSONata expression string into an AST using a Pratt parser.

pub mod ast;
mod pratt;
mod process;
mod tokenizer;

use crate::{Error, Result};

use ast::*;
use pratt::Symbol;
use tokenizer::*;

#[derive(Debug)]
pub struct Parser<'a> {
    pub tokenizer: Tokenizer<'a>,
    pub token: Token,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Result<Self> {
        let mut tokenizer = Tokenizer::new(source);
        Ok(Self {
            token: tokenizer.next_token()?,
            tokenizer,
        })
    }

    pub fn token(&self) -> &Token {
        &self.token
    }

    pub fn next_token(&mut self) -> Result<()> {
        self.token = self.tokenizer.next_token()?;
        Ok(())
    }

    pub fn expect(&mut self, expected: TokenKind) -> Result<()> {
        if self.token.kind == TokenKind::End {
            return Err(Error::S0203ExpectedTokenBeforeEnd(
                self.token.span,
                expected.to_string(),
            ));
        }

        if self.token.kind != expected {
            return Err(Error::S0202UnexpectedToken(
                self.token.span,
                expected.to_string(),
                self.token.kind.to_string(),
            ));
        }

        self.next_token()?;
        Ok(())
    }

    pub fn expression(&mut self, bp: u32) -> Result<Ast> {
        let mut last = self.token.clone();
        self.next_token()?;

        let mut left = last.null_denotation(self)?;

        while bp < self.token.left_binding_power() {
            last = self.token.clone();
            self.next_token()?;
            left = last.left_denotation(self, left)?;
        }

        Ok(left)
    }
}

/// Parse a JSONata expression string into an AST.
pub fn parse(source: &str) -> Result<Ast> {
    let mut parser = Parser::new(source)?;
    let ast = parser.expression(0)?;
    if !matches!(parser.token().kind, TokenKind::End) {
        return Err(Error::S0201SyntaxError(
            parser.token().span,
            parser.tokenizer.string_from_token(parser.token()),
        ));
    }
    ast.process()
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! parse_ok {
        ($name:ident, $source:expr) => {
            #[test]
            fn $name() {
                parse($source).expect("failed to parse");
            }
        };
    }

    parse_ok!(basic_path, "Address1.City");
    parse_ok!(backtick_path, "Other.`Over 18 ?`");
    parse_ok!(array_index, "Phone1[0]");
    parse_ok!(negative_array_index, "Phone2[-1]");
    parse_ok!(index_and_path, "Phone3[0].Number");
    parse_ok!(range, "Phone4[[0..1]]");
    parse_ok!(context_index, "$[0]");
    parse_ok!(context_path, "$.ref");
    parse_ok!(predicate, "Phone5[type='mobile']");
    parse_ok!(predicate_and_path, "Phone6[type='mobile'].number");
    parse_ok!(suffix_wildcard, "Address2.*");
    parse_ok!(prefix_wildcard, "*.Postcode1");
    parse_ok!(prefix_descendent, "**.Postcode2");
    parse_ok!(string_concatenation, "FirstName & ' ' & Surname");
    parse_ok!(grouped_concat, "Address3.(Street & ', ' & City)");
    parse_ok!(mixed_type_concat, "5&0&true");
    parse_ok!(addition, "Numbers1[0] + Numbers[1]");
    parse_ok!(subtraction, "Numbers2[0] - Numbers[1]");
    parse_ok!(multiplication, "Numbers3[0] * Numbers[1]");
    parse_ok!(division, "Numbers4[0] / Numbers[1]");
    parse_ok!(modulus, "Numbers5[0] % Numbers[1]");
    parse_ok!(equal, "Numbers6[0] = Numbers[5]");
    parse_ok!(not_equal, "Numbers7[0] != Numbers[5]");
    parse_ok!(less_than, "Numbers8[0] < Numbers[5]");
    parse_ok!(less_than_equal, "Numbers9[0] <= Numbers[5]");
    parse_ok!(greater_than, "Numbers10[0] > Numbers[5]");
    parse_ok!(greater_than_equal, "Numbers11[0] >= Numbers[5]");
    parse_ok!(string_in, r#""01962 001234" in Phone.number"#);
    parse_ok!(
        boolean_and,
        "(Numbers12[2] != 0) and (Numbers[5] != Numbers[1])"
    );
    parse_ok!(
        boolean_or,
        "(Numbers13[2] != 0) or (Numbers[5] = Numbers[1])"
    );
    parse_ok!(array_constructor, "Email1.[address]");
    parse_ok!(
        array_constructor_2,
        "[Address4, Other.`Alternative.Address`].City"
    );
    parse_ok!(object_constructor, "Phone7.{type: number}");
    parse_ok!(math, "(5 + 3) * 4");
    parse_ok!(block_expression, "(expr1; expr2; expr3)");
    parse_ok!(sort_1, "Account.Order.Product^(Price)");
    parse_ok!(sort_2, "Account.Order.Product^(>Price)");
    parse_ok!(sort_3, "Account.Order.Product^(>Price, <Quantity)");
    parse_ok!(regex_literal, "/[0-9]+/");
    parse_ok!(
        function_application,
        r#"Customer.Email ~> $substringAfter("@") ~> $substringBefore(".") ~> $uppercase()"#
    );
    parse_ok!(
        object_transform_1,
        "payload ~> |Account.Order.Product|{'Price': Price * 1.2}|"
    );
    parse_ok!(
        object_transform_2,
        r#"$ ~> |Account.Order.Product|{'Total': Price * Quantity}, ['Price', 'Quantity']|"#
    );

    // Parent operator — was unimplemented!() in Stedi, now parses cleanly
    parse_ok!(parent_operator, "Account.Order.Product.%.`Account Name`");

    parse_ok!(
        variable_assignment,
        r#"
        Invoice.(
          $p := Product.Price;
          $q := Product.Quantity;
          $p * $q
        )
    "#
    );

    parse_ok!(
        function_definition,
        r#"
        (
          $volume := function($l, $w, $h){ $l * $w * $h };
          $volume(10, 10, 5);
        )
    "#
    );

    parse_ok!(
        recursive_function,
        r#"
        (
          $factorial:= function($x){ $x <= 1 ? 1 : $x * $factorial($x-1) };
          $factorial(4)
        )
    "#
    );

    parse_ok!(
        higher_order_functions,
        r#"
        (
          $twice := function($f) { function($x){ $f($f($x)) } };
          $add3 := function($y){ $y + 3 };
          $add6 := $twice($add3);
          $add6(7)
        )
    "#
    );

    parse_ok!(
        partial_application,
        r#"
        (
          $firstN := $substring(?, 0, ?);
          $first5 := $firstN(?, 5);
          $first5("Hello, World")
        )
    "#
    );

    parse_ok!(
        complex_expression,
        r#"
        (
          $pi := 3.1415926535897932384626;
          $plot := function($x) {(
            $floor := $string ~> $substringBefore(?, '.') ~> $number;
            $index := $floor(($x + 1) * 20 + 0.5);
            $join([0..$index].('.')) & 'O' & $join([$index..40].('.'))
          )};
          $product := function($a, $b) { $a * $b };
          $factorial := function($n) { $n = 0 ? 1 : $reduce([1..$n], $product) };
          $sin := function($x){ $cos($x - $pi/2) };
          $cos := function($x){ $x > $pi ? $cos($x - 2 * $pi) : $x < -$pi ? $cos($x + 2 * $pi) :
            $sum([0..12].($power(-1, $) * $power($x, 2*$) / $factorial(2*$)))
          };
          [0..24].$sin($*$pi/12).$plot($)
        )
    "#
    );

    // Verify parent operator doesn't panic (Stedi bug)
    #[test]
    fn parent_operator_does_not_panic() {
        // These caused unimplemented!() panics in Stedi's jsonata-rs
        let result = parse("%");
        assert!(result.is_ok() || result.is_err()); // doesn't panic

        let result = parse("$.%");
        assert!(result.is_ok() || result.is_err()); // doesn't panic
    }

    #[test]
    fn invalid_expressions_return_errors() {
        assert!(parse("[").is_err());
        assert!(parse("(").is_err());
        assert!(parse("{").is_err());
        assert!(parse("|").is_err());
        assert!(parse("~").is_err());
        assert!(parse("@").is_err());
        assert!(parse("#").is_err());
    }
}
