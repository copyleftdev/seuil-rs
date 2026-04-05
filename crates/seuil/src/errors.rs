//! Error types for seuil-rs.
//!
//! All errors carry a [`Span`] indicating the byte range in the source expression
//! that caused the error. Error codes match the JSONata reference implementation
//! for compatibility.

use std::{error, fmt};

/// A byte range in the source expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Start byte offset (inclusive).
    pub start: usize,
    /// End byte offset (exclusive).
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn at(pos: usize) -> Self {
        Self {
            start: pos,
            end: pos,
        }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.start == self.end {
            write!(f, "{}", self.start)
        } else {
            write!(f, "{}..{}", self.start, self.end)
        }
    }
}

/// All possible errors from parsing and evaluating JSONata expressions.
///
/// Error codes follow the JSONata reference implementation:
/// - `Sxxxx` — Static errors (compile time)
/// - `Txxxx` — Type errors
/// - `Dxxxx` — Dynamic errors (evaluation time)
/// - `Uxxxx` — Resource/limit errors
#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub enum Error {
    // -----------------------------------------------------------------------
    // Compile-time errors (S01xx: tokenizer, S02xx: parser, S03xx: regex)
    // -----------------------------------------------------------------------
    S0101UnterminatedStringLiteral(Span),
    S0102LexedNumberOutOfRange(Span, String),
    S0103UnsupportedEscape(Span, char),
    S0104InvalidUnicodeEscape(Span),
    S0105UnterminatedQuoteProp(Span),
    S0106UnterminatedComment(Span),
    S0201SyntaxError(Span, String),
    S0202UnexpectedToken(Span, String, String),
    S0203ExpectedTokenBeforeEnd(Span, String),
    S0204UnknownOperator(Span, String),
    S0208InvalidFunctionParam(Span, String),
    S0209InvalidPredicate(Span),
    S0210MultipleGroupBy(Span),
    S0211InvalidUnary(Span, String),
    S0212ExpectedVarLeft(Span),
    S0213InvalidStep(Span, String),
    S0214ExpectedVarRight(Span, String),
    S0215BindingAfterPredicates(Span),
    S0216BindingAfterSort(Span),
    S0301EmptyRegex(Span),
    S0302UnterminatedRegex(Span),
    S0303InvalidRegex(Span, String),

    // -----------------------------------------------------------------------
    // Runtime errors (D1xxx: evaluator, D2xxx: operators, D3xxx: functions)
    // -----------------------------------------------------------------------
    D1001NumberOutOfRange(f64),
    D1002NegatingNonNumeric(Span, String),
    D1004ZeroLengthMatch(Span),
    D1009MultipleKeys(Span, String),
    D2014RangeOutOfBounds(Span, isize),
    D3001StringNotFinite(Span),
    D3010EmptyPattern(Span),
    D3011NegativeLimit(Span),
    D3012InvalidReplacementType(Span),
    D3020NegativeLimit(Span),
    D3030NonNumericCast(Span, String),
    D3050SecondArgument(String),
    D3060SqrtNegative(Span, String),
    D3061PowUnrepresentable(Span, String, String),
    D3070InvalidDefaultSort(Span),
    D3110InvalidDateTimeString(String),
    D3132UnknownComponent(String),
    D3133PictureStringNameModifier(String),
    D3134TooManyTzDigits(String),
    D3135PictureStringNoClosingBracket(String),
    D3136DatetimeComponentsMissing(String),
    D3137Error(String),
    D3138SingleTooMany(String),
    D3139SingleTooFew(String),
    D3141Assert(String),

    // -----------------------------------------------------------------------
    // Type errors (T04xx: signatures, T1xxx: evaluation, T2xxx: operators)
    // -----------------------------------------------------------------------
    T0410ArgumentNotValid(Span, usize, String),
    T0411ContextNotValid(Span, usize, String),
    T0412ArgumentMustBeArrayOfType(Span, usize, String, String),
    T1003NonStringKey(Span, String),
    T1005InvokedNonFunctionSuggest(Span, String),
    T1006InvokedNonFunction(Span),
    T2001LeftSideNotNumber(Span, String),
    T2002RightSideNotNumber(Span, String),
    T2003LeftSideNotInteger(Span),
    T2004RightSideNotInteger(Span),
    T2006RightSideNotFunction(Span),
    T2007CompareTypeMismatch(Span, String, String),
    T2008InvalidOrderBy(Span),
    T2009BinaryOpMismatch(Span, String, String, String),
    T2010BinaryOpTypes(Span, String),
    T2011UpdateNotObject(Span, String),
    T2012DeleteNotStrings(Span, String),
    T2013BadClone(Span),

    // -----------------------------------------------------------------------
    // Resource limit errors
    // -----------------------------------------------------------------------
    DepthLimitExceeded { limit: usize, span: Option<Span> },
    TimeLimitExceeded { limit_ms: u64 },
    MemoryLimitExceeded { limit_bytes: usize },

    // -----------------------------------------------------------------------
    // Internal errors (should never surface to users)
    // -----------------------------------------------------------------------
    UnsupportedNode(Span, String),
    InvalidJsonInput(String),
}

impl error::Error for Error {}

impl Error {
    /// Returns the JSONata-compatible error code string.
    pub fn code(&self) -> &str {
        match self {
            Error::S0101UnterminatedStringLiteral(..) => "S0101",
            Error::S0102LexedNumberOutOfRange(..) => "S0102",
            Error::S0103UnsupportedEscape(..) => "S0103",
            Error::S0104InvalidUnicodeEscape(..) => "S0104",
            Error::S0105UnterminatedQuoteProp(..) => "S0105",
            Error::S0106UnterminatedComment(..) => "S0106",
            Error::S0201SyntaxError(..) => "S0201",
            Error::S0202UnexpectedToken(..) => "S0202",
            Error::S0203ExpectedTokenBeforeEnd(..) => "S0203",
            Error::S0204UnknownOperator(..) => "S0204",
            Error::S0208InvalidFunctionParam(..) => "S0208",
            Error::S0209InvalidPredicate(..) => "S0209",
            Error::S0210MultipleGroupBy(..) => "S0210",
            Error::S0211InvalidUnary(..) => "S0211",
            Error::S0212ExpectedVarLeft(..) => "S0212",
            Error::S0213InvalidStep(..) => "S0213",
            Error::S0214ExpectedVarRight(..) => "S0214",
            Error::S0215BindingAfterPredicates(..) => "S0215",
            Error::S0216BindingAfterSort(..) => "S0216",
            Error::S0301EmptyRegex(..) => "S0301",
            Error::S0302UnterminatedRegex(..) => "S0302",
            Error::S0303InvalidRegex(..) => "S0303",
            Error::D1001NumberOutOfRange(..) => "D1001",
            Error::D1002NegatingNonNumeric(..) => "D1002",
            Error::D1004ZeroLengthMatch(..) => "D1004",
            Error::D1009MultipleKeys(..) => "D1009",
            Error::D2014RangeOutOfBounds(..) => "D2014",
            Error::D3001StringNotFinite(..) => "D3001",
            Error::D3010EmptyPattern(..) => "D3010",
            Error::D3011NegativeLimit(..) => "D3011",
            Error::D3012InvalidReplacementType(..) => "D3012",
            Error::D3020NegativeLimit(..) => "D3020",
            Error::D3030NonNumericCast(..) => "D3030",
            Error::D3050SecondArgument(..) => "D3050",
            Error::D3060SqrtNegative(..) => "D3060",
            Error::D3061PowUnrepresentable(..) => "D3061",
            Error::D3070InvalidDefaultSort(..) => "D3070",
            Error::D3110InvalidDateTimeString(..) => "D3110",
            Error::D3132UnknownComponent(..) => "D3132",
            Error::D3133PictureStringNameModifier(..) => "D3133",
            Error::D3134TooManyTzDigits(..) => "D3134",
            Error::D3135PictureStringNoClosingBracket(..) => "D3135",
            Error::D3136DatetimeComponentsMissing(..) => "D3136",
            Error::D3137Error(..) => "D3137",
            Error::D3138SingleTooMany(..) => "D3138",
            Error::D3139SingleTooFew(..) => "D3139",
            Error::D3141Assert(..) => "D3141",
            Error::T0410ArgumentNotValid(..) => "T0410",
            Error::T0411ContextNotValid(..) => "T0411",
            Error::T0412ArgumentMustBeArrayOfType(..) => "T0412",
            Error::T1003NonStringKey(..) => "T1003",
            Error::T1005InvokedNonFunctionSuggest(..) => "T1005",
            Error::T1006InvokedNonFunction(..) => "T1006",
            Error::T2001LeftSideNotNumber(..) => "T2001",
            Error::T2002RightSideNotNumber(..) => "T2002",
            Error::T2003LeftSideNotInteger(..) => "T2003",
            Error::T2004RightSideNotInteger(..) => "T2004",
            Error::T2006RightSideNotFunction(..) => "T2006",
            Error::T2007CompareTypeMismatch(..) => "T2007",
            Error::T2008InvalidOrderBy(..) => "T2008",
            Error::T2009BinaryOpMismatch(..) => "T2009",
            Error::T2010BinaryOpTypes(..) => "T2010",
            Error::T2011UpdateNotObject(..) => "T2011",
            Error::T2012DeleteNotStrings(..) => "T2012",
            Error::T2013BadClone(..) => "T2013",
            Error::DepthLimitExceeded { .. } => "U1001",
            Error::TimeLimitExceeded { .. } => "U1001",
            Error::MemoryLimitExceeded { .. } => "U1002",
            Error::UnsupportedNode(..) => "U9999",
            Error::InvalidJsonInput(..) => "U9998",
        }
    }

    /// Returns the span where this error occurred, if available.
    pub fn span(&self) -> Option<Span> {
        match self {
            Error::S0101UnterminatedStringLiteral(s)
            | Error::S0105UnterminatedQuoteProp(s)
            | Error::S0106UnterminatedComment(s)
            | Error::S0209InvalidPredicate(s)
            | Error::S0210MultipleGroupBy(s)
            | Error::S0212ExpectedVarLeft(s)
            | Error::S0215BindingAfterPredicates(s)
            | Error::S0216BindingAfterSort(s)
            | Error::S0301EmptyRegex(s)
            | Error::S0302UnterminatedRegex(s)
            | Error::T2003LeftSideNotInteger(s)
            | Error::T2004RightSideNotInteger(s)
            | Error::T1006InvokedNonFunction(s)
            | Error::T2006RightSideNotFunction(s) => Some(*s),

            Error::S0102LexedNumberOutOfRange(s, _)
            | Error::S0103UnsupportedEscape(s, _)
            | Error::S0104InvalidUnicodeEscape(s)
            | Error::S0201SyntaxError(s, _)
            | Error::S0202UnexpectedToken(s, _, _)
            | Error::S0203ExpectedTokenBeforeEnd(s, _)
            | Error::S0204UnknownOperator(s, _)
            | Error::S0208InvalidFunctionParam(s, _)
            | Error::S0211InvalidUnary(s, _)
            | Error::S0213InvalidStep(s, _)
            | Error::S0214ExpectedVarRight(s, _)
            | Error::S0303InvalidRegex(s, _)
            | Error::D1002NegatingNonNumeric(s, _)
            | Error::D1004ZeroLengthMatch(s)
            | Error::D1009MultipleKeys(s, _)
            | Error::D2014RangeOutOfBounds(s, _)
            | Error::D3001StringNotFinite(s)
            | Error::D3010EmptyPattern(s)
            | Error::D3011NegativeLimit(s)
            | Error::D3012InvalidReplacementType(s)
            | Error::D3020NegativeLimit(s)
            | Error::D3030NonNumericCast(s, _)
            | Error::D3060SqrtNegative(s, _)
            | Error::D3061PowUnrepresentable(s, _, _)
            | Error::D3070InvalidDefaultSort(s)
            | Error::T0410ArgumentNotValid(s, _, _)
            | Error::T0411ContextNotValid(s, _, _)
            | Error::T0412ArgumentMustBeArrayOfType(s, _, _, _)
            | Error::T1003NonStringKey(s, _)
            | Error::T1005InvokedNonFunctionSuggest(s, _)
            | Error::T2001LeftSideNotNumber(s, _)
            | Error::T2002RightSideNotNumber(s, _)
            | Error::T2007CompareTypeMismatch(s, _, _)
            | Error::T2008InvalidOrderBy(s)
            | Error::T2009BinaryOpMismatch(s, _, _, _)
            | Error::T2010BinaryOpTypes(s, _)
            | Error::T2011UpdateNotObject(s, _)
            | Error::T2012DeleteNotStrings(s, _)
            | Error::T2013BadClone(s)
            | Error::UnsupportedNode(s, _) => Some(*s),

            Error::DepthLimitExceeded { span, .. } => *span,

            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code())?;
        if let Some(span) = self.span() {
            write!(f, " @ {}", span)?;
        }
        write!(f, ": ")?;

        match self {
            Error::S0101UnterminatedStringLiteral(_) => {
                write!(f, "String literal must be terminated by a matching quote")
            }
            Error::S0102LexedNumberOutOfRange(_, n) => write!(f, "Number out of range: {n}"),
            Error::S0103UnsupportedEscape(_, c) => {
                write!(f, "Unsupported escape sequence: \\{c}")
            }
            Error::S0104InvalidUnicodeEscape(_) => {
                write!(
                    f,
                    "The escape sequence \\u must be followed by 4 hex digits"
                )
            }
            Error::S0105UnterminatedQuoteProp(_) => {
                write!(
                    f,
                    "Quoted property name must be terminated with a backquote (`)"
                )
            }
            Error::S0106UnterminatedComment(_) => write!(f, "Comment has no closing tag"),
            Error::S0201SyntaxError(_, t) => write!(f, "Syntax error `{t}`"),
            Error::S0202UnexpectedToken(_, e, a) => write!(f, "Expected `{e}`, got `{a}`"),
            Error::S0203ExpectedTokenBeforeEnd(_, t) => {
                write!(f, "Expected `{t}` before end of expression")
            }
            Error::S0204UnknownOperator(_, t) => write!(f, "Unknown operator: `{t}`"),
            Error::S0208InvalidFunctionParam(_, k) => write!(
                f,
                "Parameter `{k}` of function definition must be a variable name (start with $)"
            ),
            Error::S0209InvalidPredicate(_) => write!(
                f,
                "A predicate cannot follow a grouping expression in a step"
            ),
            Error::S0210MultipleGroupBy(_) => {
                write!(f, "Each step can only have one grouping expression")
            }
            Error::S0211InvalidUnary(_, k) => {
                write!(f, "The symbol `{k}` cannot be used as a unary operator")
            }
            Error::S0212ExpectedVarLeft(_) => write!(
                f,
                "The left side of `:=` must be a variable name (start with $)"
            ),
            Error::S0213InvalidStep(_, k) => write!(
                f,
                "The literal value `{k}` cannot be used as a step within a path expression"
            ),
            Error::S0214ExpectedVarRight(_, k) => write!(
                f,
                "The right side of `{k}` must be a variable name (start with $)"
            ),
            Error::S0215BindingAfterPredicates(_) => write!(
                f,
                "A context variable binding must precede any predicates on a step"
            ),
            Error::S0216BindingAfterSort(_) => write!(
                f,
                "A context variable binding must precede the 'order-by' clause on a step"
            ),
            Error::S0301EmptyRegex(_) => {
                write!(f, "Empty regular expressions are not allowed")
            }
            Error::S0302UnterminatedRegex(_) => {
                write!(f, "No terminating / in regular expression")
            }
            Error::S0303InvalidRegex(_, m) => write!(f, "{m}"),
            Error::D1001NumberOutOfRange(n) => write!(f, "Number out of range: {n}"),
            Error::D1002NegatingNonNumeric(_, v) => {
                write!(f, "Cannot negate a non-numeric value `{v}`")
            }
            Error::D1004ZeroLengthMatch(_) => {
                write!(f, "Regular expression matches zero length string")
            }
            Error::D1009MultipleKeys(_, k) => {
                write!(f, "Multiple key definitions evaluate to same key: {k}")
            }
            Error::D2014RangeOutOfBounds(_, s) => write!(
                f,
                "The size of the sequence allocated by the range operator (..) must not exceed 1e7. Attempted to allocate {s}"
            ),
            Error::D3001StringNotFinite(_) => write!(
                f,
                "Attempting to invoke string function on Infinity or NaN"
            ),
            Error::D3010EmptyPattern(_) => write!(
                f,
                "Second argument of replace function cannot be an empty string"
            ),
            Error::D3011NegativeLimit(_) => write!(
                f,
                "Fourth argument of replace function must evaluate to a positive number"
            ),
            Error::D3012InvalidReplacementType(_) => write!(
                f,
                "Attempted to replace a matched string with a non-string value"
            ),
            Error::D3020NegativeLimit(_) => write!(
                f,
                "Third argument of split function must evaluate to a positive number"
            ),
            Error::D3030NonNumericCast(_, n) => {
                write!(f, "Unable to cast value to a number: {n}")
            }
            Error::D3050SecondArgument(p) => write!(
                f,
                "{p}: The second argument of reduce function must be a function with at least two arguments"
            ),
            Error::D3060SqrtNegative(_, n) => write!(
                f,
                "The sqrt function cannot be applied to a negative number: {n}"
            ),
            Error::D3061PowUnrepresentable(_, b, e) => write!(
                f,
                "The power function has resulted in a value that cannot be represented as a JSON number: base={b}, exponent={e}"
            ),
            Error::D3070InvalidDefaultSort(_) => write!(
                f,
                "The single argument form of the sort function can only be applied to an array of strings or an array of numbers"
            ),
            Error::D3110InvalidDateTimeString(m) => write!(
                f,
                "{m}: Timestamp could not be parsed"
            ),
            Error::D3132UnknownComponent(m) => write!(
                f,
                "{m}: Unknown component in date/time picture string"
            ),
            Error::D3133PictureStringNameModifier(m) => write!(
                f,
                "{m}: The 'name' modifier can only be applied to months and days"
            ),
            Error::D3134TooManyTzDigits(m) => write!(
                f,
                "{m}: The timezone integer format specifier cannot have more than four digits"
            ),
            Error::D3135PictureStringNoClosingBracket(m) => {
                write!(f, "{m}: No matching closing bracket ']' in date/time picture string")
            }
            Error::D3136DatetimeComponentsMissing(m) => write!(
                f,
                "{m}: The date/time components are underspecified"
            ),
            Error::D3137Error(m) => write!(f, "{m}"),
            Error::D3138SingleTooMany(m) => write!(
                f,
                "{m}: The $single() function expected exactly 1 matching result. Instead it matched more."
            ),
            Error::D3139SingleTooFew(m) => write!(
                f,
                "{m}: The $single() function expected exactly 1 matching result. Instead it matched 0."
            ),
            Error::D3141Assert(m) => write!(f, "{m}"),
            Error::T0410ArgumentNotValid(_, i, t) => write!(
                f,
                "Argument {i} of function {t} does not match function signature"
            ),
            Error::T0411ContextNotValid(_, i, t) => write!(
                f,
                "Context value is not a type that is supported by the function signature of {t} (argument {i})"
            ),
            Error::T0412ArgumentMustBeArrayOfType(_, i, t, ty) => {
                write!(f, "Argument {i} of function {t} must be an array of {ty}")
            }
            Error::T1003NonStringKey(_, v) => write!(
                f,
                "Key in object structure must evaluate to a string; got: {v}"
            ),
            Error::T1005InvokedNonFunctionSuggest(_, t) => {
                write!(f, "Attempted to invoke a non-function. Did you mean ${t}?")
            }
            Error::T1006InvokedNonFunction(_) => {
                write!(f, "Attempted to invoke a non-function")
            }
            Error::T2001LeftSideNotNumber(_, o) => write!(
                f,
                "The left side of the `{o}` operator must evaluate to a number"
            ),
            Error::T2002RightSideNotNumber(_, o) => write!(
                f,
                "The right side of the `{o}` operator must evaluate to a number"
            ),
            Error::T2003LeftSideNotInteger(_) => write!(
                f,
                "The left side of the range operator (..) must evaluate to an integer"
            ),
            Error::T2004RightSideNotInteger(_) => write!(
                f,
                "The right side of the range operator (..) must evaluate to an integer"
            ),
            Error::T2006RightSideNotFunction(_) => write!(
                f,
                "The right side of the function application operator ~> must be a function"
            ),
            Error::T2007CompareTypeMismatch(_, a, b) => write!(
                f,
                "Type mismatch when comparing values {a} and {b} in order-by clause"
            ),
            Error::T2008InvalidOrderBy(_) => write!(
                f,
                "The expressions within an order-by clause must evaluate to numeric or string values"
            ),
            Error::T2009BinaryOpMismatch(_, l, r, o) => write!(
                f,
                "The values {l} and {r} either side of operator {o} must be of the same data type"
            ),
            Error::T2010BinaryOpTypes(_, o) => write!(
                f,
                "The expressions either side of operator `{o}` must evaluate to numeric or string values"
            ),
            Error::T2011UpdateNotObject(_, v) => write!(
                f,
                "The insert/update clause of the transform expression must evaluate to an object: {v}"
            ),
            Error::T2012DeleteNotStrings(_, v) => write!(
                f,
                "The delete clause of the transform expression must evaluate to a string or array of strings: {v}"
            ),
            Error::T2013BadClone(_) => write!(
                f,
                "The transform expression clones the input object using the $clone() function. This has been overridden in the current scope by a non-function."
            ),
            Error::DepthLimitExceeded { limit, .. } => write!(
                f,
                "Stack overflow error: recursion depth exceeded limit of {limit}"
            ),
            Error::TimeLimitExceeded { limit_ms } => write!(
                f,
                "Expression evaluation timeout: exceeded {limit_ms}ms limit"
            ),
            Error::MemoryLimitExceeded { limit_bytes } => write!(
                f,
                "Memory limit exceeded: exceeded {} byte limit",
                limit_bytes
            ),
            Error::UnsupportedNode(_, desc) => write!(f, "Unsupported node: {desc}"),
            Error::InvalidJsonInput(msg) => write!(f, "Invalid JSON input: {msg}"),
        }
    }
}
