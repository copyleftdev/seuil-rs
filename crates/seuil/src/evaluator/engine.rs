//! Core evaluator engine for JSONata expressions.
//!
//! Walks the AST produced by the parser and produces a `Value` result.
//! All values are arena-allocated; the scope lives inside the `Evaluator`
//! behind a `RefCell` so that nested native-fn calls never conflict.

use bumpalo::collections::String as BumpString;
use bumpalo::collections::Vec as BumpVec;
use bumpalo::Bump;

use crate::clock::Environment;
use crate::evaluator::scope::ScopeStack;
use crate::evaluator::value::{ArrayFlags, EvalScratch, FnContext, Value};
use crate::parser::ast::{Ast, AstKind, BinaryOp, SortTerms, UnaryOp};
use crate::{Error, Result, Span};

/// Entry in the transform updates table: (matched node pointer, optional update, optional delete keys).
type TransformUpdate<'a> = (
    *const Value<'a>,
    Option<&'a Value<'a>>,
    Option<Vec<std::borrow::Cow<'a, str>>>,
);

// ---------------------------------------------------------------------------
// Evaluator
// ---------------------------------------------------------------------------

pub struct Evaluator<'a, 'env> {
    arena: &'a Bump,
    env: &'env dyn Environment,
    scratch: EvalScratch<'a>,
    chain_ast: Option<Ast>,
    depth: std::cell::Cell<usize>,
    max_depth: usize,
    time_limit_ms: Option<u64>,
    start_timestamp: u64,
    scope: std::cell::RefCell<ScopeStack<'a>>,
}

impl<'a, 'env> Evaluator<'a, 'env> {
    pub fn new(
        arena: &'a Bump,
        env: &'env dyn Environment,
        chain_ast: Option<Ast>,
        max_depth: usize,
        time_limit_ms: Option<u64>,
    ) -> Self {
        let scratch = EvalScratch::new(arena);
        let start_timestamp = env.timestamp();
        Self {
            arena,
            env,
            scratch,
            chain_ast,
            depth: std::cell::Cell::new(0),
            max_depth,
            time_limit_ms,
            start_timestamp,
            scope: std::cell::RefCell::new(ScopeStack::new()),
        }
    }

    /// Bind all built-in JSONata functions into the evaluator's scope.
    pub fn bind_natives(&self) {
        super::functions::bind_all_natives(&mut self.scope.borrow_mut(), self.arena);
    }

    /// Bind a variable in the evaluator's scope.
    pub fn bind(&self, name: &'a str, value: &'a Value<'a>) {
        self.scope.borrow_mut().bind(name, value);
    }

    // -- Sentinel helpers --

    #[inline]
    fn undefined(&self) -> &'a Value<'a> {
        self.scratch.undefined
    }

    #[inline]
    fn val_bool(&self, b: bool) -> &'a Value<'a> {
        if b {
            self.scratch.val_true
        } else {
            self.scratch.val_false
        }
    }

    // -- Resource limit checks --

    fn check_limits(&self, span: Span) -> Result<()> {
        if self.depth.get() > self.max_depth {
            return Err(Error::DepthLimitExceeded {
                limit: self.max_depth,
                span: Some(span),
            });
        }
        if let Some(limit) = self.time_limit_ms {
            if self.env.elapsed_millis(self.start_timestamp) >= limit {
                return Err(Error::TimeLimitExceeded { limit_ms: limit });
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Top-level evaluate
    // -----------------------------------------------------------------------

    pub fn evaluate(&self, node: &Ast, input: &'a Value<'a>) -> Result<&'a Value<'a>> {
        self.depth.set(self.depth.get() + 1);
        self.check_limits(node.span)?;

        let result = self.evaluate_inner(node, input);

        self.depth.set(self.depth.get() - 1);

        let mut result = result?;

        // Apply predicates
        if let Some(ref filters) = node.predicates {
            for filter in filters {
                if let AstKind::Filter(ref expr) = filter.kind {
                    result = self.evaluate_filter(expr, result)?;
                }
            }
        }

        // Sequence post-processing
        Ok(
            if result.has_flags(ArrayFlags::SEQUENCE) && !result.has_flags(ArrayFlags::TUPLE_STREAM)
            {
                if node.keep_array {
                    result = result.clone_array_with_flags(
                        self.arena,
                        result.get_flags() | ArrayFlags::SINGLETON,
                    );
                }
                if result.is_empty() {
                    self.undefined()
                } else if result.len() == 1 {
                    if result.has_flags(ArrayFlags::SINGLETON) {
                        result
                    } else {
                        result.get_member(0).unwrap_or_else(|| self.undefined())
                    }
                } else {
                    result
                }
            } else {
                result
            },
        )
    }

    fn evaluate_inner(&self, node: &Ast, input: &'a Value<'a>) -> Result<&'a Value<'a>> {
        match node.kind {
            AstKind::Null => Ok(Value::null(self.arena)),
            AstKind::Bool(b) => Ok(self.val_bool(b)),
            AstKind::String(ref s) => Ok(Value::string(self.arena, s)),
            AstKind::Number(n) => Ok(Value::number(self.arena, n)),
            AstKind::Regex(ref r) => Ok(self.arena.alloc(Value::Regex(r.clone()))),

            AstKind::Block(ref exprs) => self.evaluate_block(exprs, input),
            AstKind::Var(ref name) => self.evaluate_var(name, input),
            AstKind::Name(ref name) => Ok(self.fn_lookup_internal(input, name)),

            AstKind::Unary(ref op) => self.evaluate_unary(node, op, input),
            AstKind::Binary(ref op, ref lhs, ref rhs) => {
                self.evaluate_binary(node, op, lhs, rhs, input)
            }

            AstKind::Ternary {
                ref cond,
                ref truthy,
                ref falsy,
            } => self.evaluate_ternary(cond, truthy, falsy.as_deref(), input),

            AstKind::Path(ref steps) => self.evaluate_path(node, steps, input),

            AstKind::Lambda { .. } => {
                let boxed = bumpalo::boxed::Box::new_in(node.clone(), self.arena);
                let captures = self.scope.borrow().capture(self.arena);
                Ok(self.arena.alloc(Value::Lambda {
                    ast: boxed,
                    input,
                    captures,
                }))
            }

            AstKind::Function {
                ref proc,
                ref args,
                is_partial,
                ..
            } => self.evaluate_function(input, proc, args, is_partial, None),

            AstKind::Wildcard => self.evaluate_wildcard(input),
            AstKind::Descendent => self.evaluate_descendants(input),
            AstKind::Parent => {
                // Parent (%) access is resolved via the seeking_parent flag
                // on path steps. As a standalone node, it returns the input.
                Ok(input)
            }

            AstKind::Transform {
                ref pattern,
                ref update,
                ref delete,
            } => Ok(self.arena.alloc(Value::Transformer {
                pattern: pattern.clone(),
                update: update.clone(),
                delete: delete.clone(),
            })),

            AstKind::Empty => Ok(self.undefined()),

            AstKind::Filter(_)
            | AstKind::Sort(_)
            | AstKind::Index(_)
            | AstKind::GroupBy(_, _)
            | AstKind::OrderBy(_, _)
            | AstKind::PartialArg => Err(Error::UnsupportedNode(
                node.span,
                format!("{:?}", std::mem::discriminant(&node.kind)),
            )),
        }
    }

    // -----------------------------------------------------------------------
    // Block
    // -----------------------------------------------------------------------

    fn evaluate_block(&self, exprs: &[Ast], input: &'a Value<'a>) -> Result<&'a Value<'a>> {
        if exprs.is_empty() {
            return Ok(self.undefined());
        }
        self.scope.borrow_mut().push_scope();
        let mut result = self.undefined();
        for expr in exprs {
            result = self.evaluate(expr, input)?;
        }
        self.scope.borrow_mut().pop_scope();
        Ok(result)
    }

    // -----------------------------------------------------------------------
    // Var
    // -----------------------------------------------------------------------

    fn evaluate_var(&self, name: &str, input: &'a Value<'a>) -> Result<&'a Value<'a>> {
        if name.is_empty() {
            // $ — context reference
            if input.has_flags(ArrayFlags::WRAPPED) {
                Ok(input.get_member(0).unwrap_or_else(|| self.undefined()))
            } else {
                Ok(input)
            }
        } else if let Some(value) = self.scope.borrow().lookup(name) {
            Ok(value)
        } else {
            Ok(self.undefined())
        }
    }

    // -----------------------------------------------------------------------
    // Ternary
    // -----------------------------------------------------------------------

    fn evaluate_ternary(
        &self,
        cond: &Ast,
        truthy: &Ast,
        falsy: Option<&Ast>,
        input: &'a Value<'a>,
    ) -> Result<&'a Value<'a>> {
        let c = self.evaluate(cond, input)?;
        if c.is_truthy() {
            self.evaluate(truthy, input)
        } else if let Some(f) = falsy {
            self.evaluate(f, input)
        } else {
            Ok(self.undefined())
        }
    }

    // -----------------------------------------------------------------------
    // Unary operators
    // -----------------------------------------------------------------------

    fn evaluate_unary(
        &self,
        node: &Ast,
        op: &UnaryOp,
        input: &'a Value<'a>,
    ) -> Result<&'a Value<'a>> {
        match op {
            UnaryOp::Minus(ref value) => {
                let result = self.evaluate(value, input)?;
                match result {
                    Value::Undefined => Ok(self.undefined()),
                    Value::Number(n) if result.is_valid_number()? => {
                        Ok(Value::number(self.arena, -*n))
                    }
                    _ => Err(Error::D1002NegatingNonNumeric(
                        node.span,
                        format!("{result:?}"),
                    )),
                }
            }
            UnaryOp::ArrayConstructor(ref items) => {
                let mut values = BumpVec::new_in(self.arena);
                for item in items {
                    let value = self.evaluate(item, input)?;
                    if matches!(item.kind, AstKind::Unary(UnaryOp::ArrayConstructor(..))) {
                        values.push(value);
                    } else {
                        fn_append_internal(&mut values, value);
                    }
                }
                let flags = if node.cons_array {
                    ArrayFlags::CONS
                } else {
                    ArrayFlags::empty()
                };
                Ok(Value::array_from(self.arena, values, flags))
            }
            UnaryOp::ObjectConstructor(ref object) => {
                self.evaluate_group_expression(node.span, object, input)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Binary operators
    // -----------------------------------------------------------------------

    fn evaluate_binary(
        &self,
        node: &Ast,
        op: &BinaryOp,
        lhs_ast: &Ast,
        rhs_ast: &Ast,
        input: &'a Value<'a>,
    ) -> Result<&'a Value<'a>> {
        // Bind (:=) is special: lhs is a var name, not evaluated
        if *op == BinaryOp::Bind {
            if let AstKind::Var(ref name) = lhs_ast.kind {
                let rhs = self.evaluate(rhs_ast, input)?;
                let name = BumpString::from_str_in(name, self.arena);
                self.scope.borrow_mut().bind(name.into_bump_str(), rhs);
                return Ok(rhs);
            }
            return Err(Error::UnsupportedNode(
                node.span,
                "Bind with non-var LHS".into(),
            ));
        }

        // Evaluate LHS (rhs is lazy for short-circuit)
        let lhs = self.evaluate(lhs_ast, input)?;

        match op {
            // -- Arithmetic --
            BinaryOp::Add
            | BinaryOp::Subtract
            | BinaryOp::Multiply
            | BinaryOp::Divide
            | BinaryOp::Modulus => {
                let rhs = self.evaluate(rhs_ast, input)?;

                let l = if lhs.is_undefined() {
                    return Ok(self.undefined());
                } else if lhs.is_valid_number()? {
                    lhs.as_f64()
                } else {
                    return Err(Error::T2001LeftSideNotNumber(node.span, op.to_string()));
                };

                let r = if rhs.is_undefined() {
                    return Ok(self.undefined());
                } else if rhs.is_valid_number()? {
                    rhs.as_f64()
                } else {
                    return Err(Error::T2002RightSideNotNumber(node.span, op.to_string()));
                };

                let result = match op {
                    BinaryOp::Add => l + r,
                    BinaryOp::Subtract => l - r,
                    BinaryOp::Multiply => l * r,
                    BinaryOp::Divide => l / r,
                    BinaryOp::Modulus => l % r,
                    _ => unreachable!(),
                };
                Ok(Value::number(self.arena, result))
            }

            // -- Comparison --
            BinaryOp::LessThan
            | BinaryOp::LessThanEqual
            | BinaryOp::GreaterThan
            | BinaryOp::GreaterThanEqual => {
                let rhs = self.evaluate(rhs_ast, input)?;

                // Check that both operands are number or string (or undefined).
                // If an operand is defined but not number/string, error T2010.
                if !lhs.is_undefined() && !lhs.is_number() && !lhs.is_string() {
                    return Err(Error::T2010BinaryOpTypes(node.span, op.to_string()));
                }
                if !rhs.is_undefined() && !rhs.is_number() && !rhs.is_string() {
                    return Err(Error::T2010BinaryOpTypes(node.span, op.to_string()));
                }

                if lhs.is_undefined() || rhs.is_undefined() {
                    return Ok(self.undefined());
                }

                if !((lhs.is_number() || lhs.is_string()) && (rhs.is_number() || rhs.is_string())) {
                    return Err(Error::T2010BinaryOpTypes(node.span, op.to_string()));
                }

                if lhs.is_number() && rhs.is_number() {
                    let l = lhs.as_f64();
                    let r = rhs.as_f64();
                    return Ok(self.val_bool(match op {
                        BinaryOp::LessThan => l < r,
                        BinaryOp::LessThanEqual => l <= r,
                        BinaryOp::GreaterThan => l > r,
                        BinaryOp::GreaterThanEqual => l >= r,
                        _ => unreachable!(),
                    }));
                }

                if let (Value::String(ref ls), Value::String(ref rs)) = (lhs, rhs) {
                    return Ok(self.val_bool(match op {
                        BinaryOp::LessThan => ls < rs,
                        BinaryOp::LessThanEqual => ls <= rs,
                        BinaryOp::GreaterThan => ls > rs,
                        BinaryOp::GreaterThanEqual => ls >= rs,
                        _ => unreachable!(),
                    }));
                }

                Err(Error::T2009BinaryOpMismatch(
                    node.span,
                    format!("{lhs:?}"),
                    format!("{rhs:?}"),
                    op.to_string(),
                ))
            }

            // -- Equality --
            BinaryOp::Equal | BinaryOp::NotEqual => {
                let rhs = self.evaluate(rhs_ast, input)?;
                if lhs.is_undefined() || rhs.is_undefined() {
                    return Ok(self.val_bool(false));
                }
                Ok(self.val_bool(match op {
                    BinaryOp::Equal => lhs == rhs,
                    BinaryOp::NotEqual => lhs != rhs,
                    _ => unreachable!(),
                }))
            }

            // -- Range (..) --
            BinaryOp::Range => {
                let rhs = self.evaluate(rhs_ast, input)?;

                if !lhs.is_undefined() && !lhs.is_integer() {
                    return Err(Error::T2003LeftSideNotInteger(node.span));
                }
                if !rhs.is_undefined() && !rhs.is_integer() {
                    return Err(Error::T2004RightSideNotInteger(node.span));
                }
                if lhs.is_undefined() || rhs.is_undefined() {
                    return Ok(self.undefined());
                }

                let l = lhs.as_isize();
                let r = rhs.as_isize();
                if l > r {
                    return Ok(self.undefined());
                }

                let size = r - l + 1;
                if size > 10_000_000 {
                    return Err(Error::D2014RangeOutOfBounds(node.span, size));
                }
                Ok(Value::range(self.arena, l, r))
            }

            // -- Concatenation (&) — simple stringify, no fn_string yet --
            BinaryOp::Concat => {
                let rhs = self.evaluate(rhs_ast, input)?;
                let mut buf = String::new();
                if !lhs.is_undefined() {
                    simple_stringify(lhs, &mut buf);
                }
                if !rhs.is_undefined() {
                    simple_stringify(rhs, &mut buf);
                }
                Ok(Value::string(self.arena, &buf))
            }

            // -- Boolean (short-circuit) --
            BinaryOp::And => {
                let lb = lhs.is_truthy();
                if !lb {
                    return Ok(self.val_bool(false));
                }
                let rhs = self.evaluate(rhs_ast, input)?;
                Ok(self.val_bool(rhs.is_truthy()))
            }
            BinaryOp::Or => {
                let lb = lhs.is_truthy();
                if lb {
                    return Ok(self.val_bool(true));
                }
                let rhs = self.evaluate(rhs_ast, input)?;
                Ok(self.val_bool(rhs.is_truthy()))
            }

            // -- In --
            BinaryOp::In => {
                let rhs = self.evaluate(rhs_ast, input)?;
                if lhs.is_undefined() || rhs.is_undefined() {
                    return Ok(self.val_bool(false));
                }
                let rhs = Value::wrap_in_array_if_needed(self.arena, rhs, ArrayFlags::empty());
                for item in rhs.members() {
                    if item == lhs {
                        return Ok(self.val_bool(true));
                    }
                }
                Ok(self.val_bool(false))
            }

            // -- Apply (~>) --
            BinaryOp::Apply => {
                if let AstKind::Function {
                    ref proc,
                    ref args,
                    is_partial,
                    ..
                } = rhs_ast.kind
                {
                    self.evaluate_function(input, proc, args, is_partial, Some(lhs))
                } else {
                    let rhs = self.evaluate(rhs_ast, input)?;
                    if !rhs.is_function() {
                        return Err(Error::T2006RightSideNotFunction(rhs_ast.span));
                    }
                    if lhs.is_function() {
                        // Function chaining
                        if let Some(ref chain_ast) = self.chain_ast.clone() {
                            let chain = self.evaluate(chain_ast, self.undefined())?;
                            self.apply_function(lhs_ast.span, self.undefined(), chain, &[lhs, rhs])
                        } else {
                            self.apply_function(rhs_ast.span, self.undefined(), rhs, &[lhs])
                        }
                    } else {
                        self.apply_function(rhs_ast.span, self.undefined(), rhs, &[lhs])
                    }
                }
            }

            // -- Map (.) handled in Path, but can appear as binary --
            BinaryOp::Map | BinaryOp::FocusBind | BinaryOp::IndexBind | BinaryOp::Predicate => {
                // These are processed during AST post-processing into Path nodes.
                // If they appear here, treat as path with two steps.
                let steps = vec![lhs_ast.clone(), rhs_ast.clone()];
                self.evaluate_path(node, &steps, input)
            }

            BinaryOp::Bind => {
                // Already handled above
                unreachable!()
            }
        }
    }

    // -----------------------------------------------------------------------
    // Path evaluation
    // -----------------------------------------------------------------------

    fn evaluate_path(
        &self,
        node: &Ast,
        steps: &[Ast],
        input: &'a Value<'a>,
    ) -> Result<&'a Value<'a>> {
        if steps.is_empty() {
            return Ok(self.undefined());
        }

        let mut input: &'a Value<'a> =
            if input.is_array() && !matches!(steps[0].kind, AstKind::Var(..)) {
                input
            } else {
                Value::wrap_in_array(self.arena, input, ArrayFlags::SEQUENCE)
            };

        let mut result: &'a Value<'a> = self.undefined();
        let mut is_tuple_stream = false;
        let mut tuple_bindings: &'a Value<'a> = self.undefined();

        for (step_index, step) in steps.iter().enumerate() {
            if step.tuple {
                is_tuple_stream = true;
            }

            if step_index == 0 && step.cons_array {
                result = self.evaluate(step, input)?;
            } else if is_tuple_stream {
                tuple_bindings = self.evaluate_tuple_step(step, input, tuple_bindings)?;
            } else {
                result = self.evaluate_step(step, input, step_index == steps.len() - 1)?;
            }

            if !is_tuple_stream
                && (result.is_undefined() || (result.is_array() && result.is_empty()))
            {
                break;
            }

            if !is_tuple_stream {
                input = result;
            }
        }

        if is_tuple_stream {
            if node.tuple {
                result = tuple_bindings;
            } else {
                let new_result = Value::array_with_capacity(
                    self.arena,
                    if tuple_bindings.is_array() {
                        tuple_bindings.len()
                    } else {
                        0
                    },
                    ArrayFlags::SEQUENCE,
                );
                if tuple_bindings.is_array() {
                    for binding in tuple_bindings.members() {
                        let at = binding.get_entry("@").unwrap_or_else(|| self.undefined());
                        new_result.push(at);
                    }
                }
                result = new_result;
            }
        }

        if node.keep_singleton_array && result.is_array() {
            let flags = result.get_flags();
            if flags.contains(ArrayFlags::CONS) && !flags.contains(ArrayFlags::SEQUENCE) {
                result = Value::wrap_in_array(
                    self.arena,
                    result,
                    flags | ArrayFlags::SEQUENCE | ArrayFlags::SINGLETON,
                );
            }
            result = result.clone_array_with_flags(self.arena, flags | ArrayFlags::SINGLETON);
        }

        if let Some((ref _group_span, ref object)) = node.group_by {
            self.evaluate_group_expression(
                node.span,
                object,
                if is_tuple_stream {
                    tuple_bindings
                } else {
                    result
                },
            )
        } else {
            Ok(result)
        }
    }

    fn evaluate_step(
        &self,
        step: &Ast,
        input: &'a Value<'a>,
        last_step: bool,
    ) -> Result<&'a Value<'a>> {
        // Sort step
        if let AstKind::Sort(ref sort_terms) = step.kind {
            let mut result = self.evaluate_sort(step.span, sort_terms, input)?;
            if let Some(ref stages) = step.stages {
                result = self.evaluate_stages(stages, result)?;
            }
            return Ok(result);
        }

        // Determine whether stages (filters) should be applied per-item or
        // to the aggregated result. When input is a SEQUENCE (from a previous
        // path step or from evaluate_path wrapping), stages are per-item.
        // When input is raw data (no SEQUENCE flag), stages apply to aggregate.
        let stages_per_item = input.has_flags(ArrayFlags::SEQUENCE);

        let mut results: Vec<&'a Value<'a>> = Vec::new();

        for (item_index, item) in input.members().enumerate() {
            if let Some(ref index_var) = step.index {
                let idx_name = BumpString::from_str_in(index_var, self.arena);
                self.scope.borrow_mut().bind(
                    idx_name.into_bump_str(),
                    Value::number(self.arena, item_index as f64),
                );
            }

            let mut item_result = self.evaluate(step, item)?;

            // Apply stages per-item when input is from a previous path step
            if stages_per_item {
                if let Some(ref stages) = step.stages {
                    for stage in stages {
                        if let AstKind::Filter(ref expr) = stage.kind {
                            item_result = self.evaluate_filter(expr, item_result)?;
                        }
                    }
                }
            }

            if !item_result.is_undefined() {
                results.push(item_result);
            }
        }

        let mut result: &'a Value<'a> = if last_step
            && results.len() == 1
            && results[0].is_array()
            && !results[0].has_flags(ArrayFlags::SEQUENCE)
        {
            results.remove(0)
        } else {
            let result_seq =
                Value::array_with_capacity(self.arena, results.len(), ArrayFlags::SEQUENCE);
            for ri in results {
                if !ri.is_array() || ri.has_flags(ArrayFlags::CONS) {
                    result_seq.push(ri);
                } else {
                    for item in ri.members() {
                        result_seq.push(item);
                    }
                }
            }
            result_seq
        };

        // Apply stages to aggregate when input is raw data
        if !stages_per_item {
            if let Some(ref stages) = step.stages {
                result = self.evaluate_stages(stages, result)?;
            }
        }

        Ok(result)
    }

    fn evaluate_tuple_step(
        &self,
        step: &Ast,
        input: &'a Value<'a>,
        tuple_bindings: &'a Value<'a>,
    ) -> Result<&'a Value<'a>> {
        // Sort within tuple stream
        if let AstKind::Sort(ref sort_terms) = step.kind {
            let mut result = if tuple_bindings.is_undefined() {
                let sorted = self.evaluate_sort(step.span, sort_terms, input)?;
                let arr = Value::array(self.arena, ArrayFlags::SEQUENCE | ArrayFlags::TUPLE_STREAM);
                for (idx, item) in sorted.members().enumerate() {
                    let tuple = Value::object(self.arena);
                    tuple.insert("@", item);
                    if let Some(ref index_var) = step.index {
                        tuple.insert(index_var, Value::number(self.arena, idx as f64));
                    }
                    arr.push(tuple);
                }
                &*arr
            } else {
                self.evaluate_sort(step.span, sort_terms, tuple_bindings)?
            };

            if let Some(ref stages) = step.stages {
                result = self.evaluate_stages(stages, result)?;
            }
            return Ok(result);
        }

        let tuple_bindings = if tuple_bindings.is_undefined() {
            let arr = Value::array_with_capacity(self.arena, input.len(), ArrayFlags::empty());
            for member in input.members() {
                let tuple = Value::object(self.arena);
                tuple.insert("@", member);
                arr.push(tuple);
            }
            &*arr
        } else {
            tuple_bindings
        };

        let result = Value::array(self.arena, ArrayFlags::SEQUENCE | ArrayFlags::TUPLE_STREAM);

        for tuple in tuple_bindings.members() {
            let context = tuple.get_entry("@").unwrap_or_else(|| self.undefined());

            // Restore tuple bindings into scope
            self.scope.borrow_mut().push_scope();
            if tuple.is_object() {
                for (key, value) in tuple.entries() {
                    let k = BumpString::from_str_in(key.as_str(), self.arena);
                    self.scope.borrow_mut().bind(k.into_bump_str(), value);
                }
            }

            let mut binding_sequence = self.evaluate(step, context)?;
            self.scope.borrow_mut().pop_scope();

            if !binding_sequence.is_undefined() {
                binding_sequence = Value::wrap_in_array_if_needed(
                    self.arena,
                    binding_sequence,
                    ArrayFlags::empty(),
                );
                for (binding_index, binding) in binding_sequence.members().enumerate() {
                    let output_tuple = Value::object(self.arena);
                    if tuple.is_object() {
                        for (key, value) in tuple.entries() {
                            output_tuple.insert(key.as_str(), value);
                        }
                    }
                    if binding_sequence.has_flags(ArrayFlags::TUPLE_STREAM) {
                        if binding.is_object() {
                            for (key, value) in binding.entries() {
                                output_tuple.insert(key.as_str(), value);
                            }
                        }
                    } else {
                        if let Some(ref focus_var) = step.focus {
                            output_tuple.insert(focus_var, binding);
                            output_tuple.insert("@", context);
                        } else {
                            output_tuple.insert("@", binding);
                        }
                        if let Some(ref index_var) = step.index {
                            output_tuple
                                .insert(index_var, Value::number(self.arena, binding_index as f64));
                        }
                    }
                    result.push(output_tuple);
                }
            }
        }

        let mut result: &'a Value<'a> = result;
        if let Some(ref stages) = step.stages {
            result = self.evaluate_stages(stages, result)?;
        }

        Ok(result)
    }

    // -----------------------------------------------------------------------
    // Sort
    // -----------------------------------------------------------------------

    fn evaluate_sort(
        &self,
        span: Span,
        sort_terms: &SortTerms,
        input: &'a Value<'a>,
    ) -> Result<&'a Value<'a>> {
        if input.is_undefined() {
            return Ok(self.undefined());
        }

        if !input.is_array() || input.len() <= 1 {
            return Ok(Value::wrap_in_array_if_needed(
                self.arena,
                input,
                ArrayFlags::empty(),
            ));
        }

        let is_tuple_sort = input.has_flags(ArrayFlags::TUPLE_STREAM);
        let items: Vec<&'a Value<'a>> = input.members().collect();

        // Evaluate sort keys up-front for each item
        let mut keys: Vec<Vec<&'a Value<'a>>> = Vec::with_capacity(items.len());
        for item in &items {
            let mut item_keys = Vec::with_capacity(sort_terms.len());
            for (term, _) in sort_terms {
                let ctx = if is_tuple_sort {
                    item.get_entry("@").unwrap_or_else(|| self.undefined())
                } else {
                    item
                };

                if is_tuple_sort {
                    self.scope.borrow_mut().push_scope();
                    if item.is_object() {
                        for (k, v) in item.entries() {
                            let key = BumpString::from_str_in(k.as_str(), self.arena);
                            self.scope.borrow_mut().bind(key.into_bump_str(), v);
                        }
                    }
                }

                let key = self.evaluate(term, ctx)?;

                if is_tuple_sort {
                    self.scope.borrow_mut().pop_scope();
                }

                item_keys.push(key);
            }
            keys.push(item_keys);
        }

        // Build indices and sort them
        let mut indices: Vec<usize> = (0..items.len()).collect();
        let mut sort_error: Option<Error> = None;

        indices.sort_by(|&a, &b| {
            if sort_error.is_some() {
                return std::cmp::Ordering::Equal;
            }
            for (term_idx, (_, descending)) in sort_terms.iter().enumerate() {
                let aa = keys[a][term_idx];
                let bb = keys[b][term_idx];

                if aa.is_undefined() && bb.is_undefined() {
                    continue;
                }
                if aa.is_undefined() {
                    return std::cmp::Ordering::Greater;
                }
                if bb.is_undefined() {
                    return std::cmp::Ordering::Less;
                }

                if !(aa.is_string() || aa.is_number()) || !(bb.is_string() || bb.is_number()) {
                    sort_error = Some(Error::T2008InvalidOrderBy(span));
                    return std::cmp::Ordering::Equal;
                }

                let ord = match (aa, bb) {
                    (Value::String(ref sa), Value::String(ref sb)) => sa.cmp(sb),
                    (Value::Number(na), Value::Number(nb)) => na.total_cmp(nb),
                    _ => {
                        sort_error = Some(Error::T2007CompareTypeMismatch(
                            span,
                            format!("{aa:?}"),
                            format!("{bb:?}"),
                        ));
                        std::cmp::Ordering::Equal
                    }
                };

                if ord != std::cmp::Ordering::Equal {
                    return if *descending { ord.reverse() } else { ord };
                }
            }
            std::cmp::Ordering::Equal
        });

        if let Some(err) = sort_error {
            return Err(err);
        }

        let result = Value::array_with_capacity(self.arena, items.len(), input.get_flags());
        for idx in indices {
            result.push(items[idx]);
        }
        Ok(result)
    }

    // -----------------------------------------------------------------------
    // Stages (filter / index)
    // -----------------------------------------------------------------------

    fn evaluate_stages(&self, stages: &[Ast], input: &'a Value<'a>) -> Result<&'a Value<'a>> {
        let mut result = input;
        for stage in stages {
            match stage.kind {
                AstKind::Filter(ref pred) => {
                    result = self.evaluate_filter(pred, result)?;
                }
                AstKind::Index(ref index_var) => {
                    if result.is_array() {
                        let new_result = Value::array_with_capacity(
                            self.arena,
                            result.len(),
                            result.get_flags(),
                        );
                        for (tuple_index, tuple) in result.members().enumerate() {
                            let new_tuple = if tuple.is_object() {
                                let nt = Value::object_with_capacity(self.arena, 8);
                                for (key, value) in tuple.entries() {
                                    nt.insert(key.as_str(), value);
                                }
                                nt
                            } else {
                                Value::object(self.arena)
                            };
                            new_tuple
                                .insert(index_var, Value::number(self.arena, tuple_index as f64));
                            new_result.push(new_tuple);
                        }
                        result = new_result;
                    }
                }
                _ => {
                    return Err(Error::UnsupportedNode(
                        stage.span,
                        "Unexpected stage kind".into(),
                    ));
                }
            }
        }
        Ok(result)
    }

    // -----------------------------------------------------------------------
    // Filter
    // -----------------------------------------------------------------------

    fn evaluate_filter(&self, predicate: &Ast, input: &'a Value<'a>) -> Result<&'a Value<'a>> {
        let flags = if input.has_flags(ArrayFlags::TUPLE_STREAM) {
            ArrayFlags::SEQUENCE | ArrayFlags::TUPLE_STREAM
        } else {
            ArrayFlags::SEQUENCE
        };
        let result = Value::array(self.arena, flags);
        let input = Value::wrap_in_array_if_needed(self.arena, input, ArrayFlags::empty());

        let get_index = |n: f64, len: usize| -> usize {
            let mut idx = n.floor() as isize;
            if idx < 0 {
                idx += len as isize;
            }
            idx as usize
        };

        match predicate.kind {
            AstKind::Number(n) => {
                let index = get_index(n, input.len());
                if let Some(item) = input.get_member(index) {
                    if !item.is_undefined() {
                        if item.is_array() {
                            return Ok(item);
                        } else {
                            result.push(item);
                        }
                    }
                }
            }
            _ => {
                for (item_index, item) in input.members().enumerate() {
                    let mut index = if input.has_flags(ArrayFlags::TUPLE_STREAM) {
                        self.scope.borrow_mut().push_scope();
                        if item.is_object() {
                            for (key, value) in item.entries() {
                                let k = BumpString::from_str_in(key.as_str(), self.arena);
                                self.scope.borrow_mut().bind(k.into_bump_str(), value);
                            }
                        }
                        let ctx = item.get_entry("@").unwrap_or_else(|| self.undefined());
                        let r = self.evaluate(predicate, ctx)?;
                        self.scope.borrow_mut().pop_scope();
                        r
                    } else {
                        self.evaluate(predicate, item)?
                    };

                    if index.is_valid_number()? {
                        index = Value::wrap_in_array(self.arena, index, ArrayFlags::empty());
                    }

                    if is_array_of_valid_numbers(index)? {
                        for v in index.members() {
                            let i = get_index(v.as_f64(), input.len());
                            if i == item_index {
                                result.push(item);
                            }
                        }
                    } else if index.is_truthy() {
                        result.push(item);
                    }
                }
            }
        }

        Ok(result)
    }

    // -----------------------------------------------------------------------
    // Group expression (object constructors)
    // -----------------------------------------------------------------------

    fn evaluate_group_expression(
        &self,
        span: Span,
        object: &[(Ast, Ast)],
        input: &'a Value<'a>,
    ) -> Result<&'a Value<'a>> {
        let is_tuple_stream = input.has_flags(ArrayFlags::TUPLE_STREAM);

        if is_tuple_stream {
            return self.evaluate_group_expression_tuple(span, object, input);
        }

        // Non-tuple-stream path (original logic)
        let mut groups: Vec<(String, &'a Value<'a>, usize)> = Vec::new();

        let input = if input.is_array() && input.is_empty() {
            let arr = Value::array_with_capacity(self.arena, 1, input.get_flags());
            arr.push(self.undefined());
            &*arr
        } else if !input.is_array() {
            let wrapped = Value::array_with_capacity(self.arena, 1, ArrayFlags::SEQUENCE);
            wrapped.push(input);
            &*wrapped
        } else {
            input
        };

        for item in input.members() {
            for (index, (key_ast, _val_ast)) in object.iter().enumerate() {
                let key = self.evaluate(key_ast, item)?;
                if !key.is_string() {
                    return Err(Error::T1003NonStringKey(span, format!("{key:?}")));
                }
                let key_str = key.as_str().to_string();

                if let Some(group) = groups.iter_mut().find(|(k, _, _)| *k == key_str) {
                    if group.2 != index {
                        return Err(Error::D1009MultipleKeys(span, key_str));
                    }
                    let mut vals = BumpVec::new_in(self.arena);
                    fn_append_internal(&mut vals, group.1);
                    fn_append_internal(&mut vals, item);
                    group.1 = Value::array_from(self.arena, vals, ArrayFlags::empty());
                } else {
                    groups.push((key_str, item, index));
                }
            }
        }

        let result = Value::object(self.arena);
        for (key, data, idx) in &groups {
            let value = self.evaluate(&object[*idx].1, data)?;
            if !value.is_undefined() {
                result.insert(key, value);
            }
        }

        Ok(result)
    }

    /// Group expression for tuple streams: evaluate with tuple bindings in scope.
    fn evaluate_group_expression_tuple(
        &self,
        span: Span,
        object: &[(Ast, Ast)],
        input: &'a Value<'a>,
    ) -> Result<&'a Value<'a>> {
        // Each item in input is a tuple object with "@", "$e", "$c", etc.
        // We group by key (evaluated with tuple bindings in scope),
        // then for each group, evaluate the value for EACH contributing tuple
        // and collect the results.

        // groups: key -> (pair_index, vec of tuples)
        struct Group<'a> {
            key: String,
            pair_index: usize,
            tuples: Vec<&'a Value<'a>>,
        }

        let mut groups: Vec<Group<'a>> = Vec::new();

        for item in input.members() {
            let context = if item.is_object() {
                self.scope.borrow_mut().push_scope();
                for (key, value) in item.entries() {
                    let k = BumpString::from_str_in(key.as_str(), self.arena);
                    self.scope.borrow_mut().bind(k.into_bump_str(), value);
                }
                item.get_entry("@").unwrap_or_else(|| self.undefined())
            } else {
                item
            };

            for (index, (key_ast, _)) in object.iter().enumerate() {
                let key = self.evaluate(key_ast, context)?;
                if !key.is_string() {
                    if item.is_object() {
                        self.scope.borrow_mut().pop_scope();
                    }
                    return Err(Error::T1003NonStringKey(span, format!("{key:?}")));
                }
                let key_str = key.as_str().to_string();

                if let Some(group) = groups.iter_mut().find(|g| g.key == key_str) {
                    if group.pair_index != index {
                        if item.is_object() {
                            self.scope.borrow_mut().pop_scope();
                        }
                        return Err(Error::D1009MultipleKeys(span, key_str));
                    }
                    group.tuples.push(item);
                } else {
                    groups.push(Group {
                        key: key_str,
                        pair_index: index,
                        tuples: vec![item],
                    });
                }
            }

            if item.is_object() {
                self.scope.borrow_mut().pop_scope();
            }
        }

        let result = Value::object(self.arena);
        for group in &groups {
            // Build a "reduce" tuple: merge all tuple bindings into arrays
            // For each binding key (except "@"), collect all values into an array.
            // For "@", collect all context values into an array.
            let reduce_tuple = Value::object(self.arena);
            let mut at_values = BumpVec::new_in(self.arena);

            // Collect all binding keys across all tuples
            let mut binding_keys: Vec<String> = Vec::new();
            for tuple in &group.tuples {
                if tuple.is_object() {
                    for (key, _) in tuple.entries() {
                        if key.as_str() != "@" && !binding_keys.iter().any(|k| k == key.as_str()) {
                            binding_keys.push(key.to_string());
                        }
                    }
                }
            }

            for tuple in &group.tuples {
                if let Some(at) = tuple.get_entry("@") {
                    at_values.push(at);
                }
            }

            // For each binding key, collect values from all tuples
            for bk in &binding_keys {
                let mut bk_values = BumpVec::new_in(self.arena);
                for tuple in &group.tuples {
                    if let Some(v) = tuple.get_entry(bk) {
                        bk_values.push(v);
                    }
                }
                let arr_val = if bk_values.len() == 1 {
                    bk_values[0]
                } else {
                    Value::array_from(self.arena, bk_values, ArrayFlags::SEQUENCE) as &Value
                };
                reduce_tuple.insert(bk, arr_val);
            }

            // Set "@" to the array of context values
            let at_val: &'a Value<'a> = if at_values.len() == 1 {
                at_values[0]
            } else {
                Value::array_from(self.arena, at_values, ArrayFlags::SEQUENCE)
            };
            reduce_tuple.insert("@", at_val);

            // Push bindings from the reduce tuple into scope
            self.scope.borrow_mut().push_scope();
            for (key, value) in reduce_tuple.entries() {
                if key.as_str() != "@" {
                    let k = BumpString::from_str_in(key.as_str(), self.arena);
                    self.scope.borrow_mut().bind(k.into_bump_str(), value);
                }
            }

            let val = self.evaluate(&object[group.pair_index].1, at_val)?;
            self.scope.borrow_mut().pop_scope();

            if !val.is_undefined() {
                result.insert(&group.key, val);
            }
        }

        Ok(result)
    }

    // -----------------------------------------------------------------------
    // Wildcard (*)
    // -----------------------------------------------------------------------

    fn evaluate_wildcard(&self, input: &'a Value<'a>) -> Result<&'a Value<'a>> {
        let mut values = BumpVec::new_in(self.arena);

        let input = if input.is_array() && input.has_flags(ArrayFlags::WRAPPED) && !input.is_empty()
        {
            input.get_member(0).unwrap_or_else(|| self.undefined())
        } else {
            input
        };

        if input.is_object() {
            for (_key, value) in input.entries() {
                if value.is_array() {
                    let flat = value.flatten(self.arena);
                    fn_append_internal(&mut values, flat);
                } else {
                    values.push(value);
                }
            }
        }

        Ok(Value::array_from(self.arena, values, ArrayFlags::SEQUENCE))
    }

    // -----------------------------------------------------------------------
    // Descendants (**)
    // -----------------------------------------------------------------------

    fn evaluate_descendants(&self, input: &'a Value<'a>) -> Result<&'a Value<'a>> {
        if input.is_undefined() {
            return Ok(self.undefined());
        }
        let mut collected = BumpVec::new_in(self.arena);
        Self::recurse_descendants_into(input, &mut collected);

        if collected.len() == 1 {
            Ok(collected[0])
        } else {
            Ok(Value::array_from(
                self.arena,
                collected,
                ArrayFlags::SEQUENCE,
            ))
        }
    }

    fn recurse_descendants_into(input: &'a Value<'a>, out: &mut BumpVec<'a, &'a Value<'a>>) {
        if !input.is_array() {
            out.push(input);
        }

        if input.is_array() {
            for member in input.members() {
                Self::recurse_descendants_into(member, out);
            }
        } else if input.is_object() {
            for (_key, value) in input.entries() {
                Self::recurse_descendants_into(value, out);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Function evaluation
    // -----------------------------------------------------------------------

    fn evaluate_function(
        &self,
        input: &'a Value<'a>,
        proc_ast: &Ast,
        args: &[Ast],
        _is_partial: bool,
        context: Option<&'a Value<'a>>,
    ) -> Result<&'a Value<'a>> {
        let evaluated_proc = self.evaluate(proc_ast, input)?;

        // Help user if they forgot '$'
        if evaluated_proc.is_undefined() {
            if let AstKind::Path(ref steps) = proc_ast.kind {
                if let Some(first) = steps.first() {
                    if let AstKind::Name(ref name) = first.kind {
                        if self.scope.borrow().lookup(name).is_some() {
                            return Err(Error::T1005InvokedNonFunctionSuggest(
                                proc_ast.span,
                                name.clone(),
                            ));
                        }
                    }
                }
            }
        }

        let mut evaluated_args: Vec<&'a Value<'a>> = Vec::with_capacity(args.len() + 1);
        if let Some(ctx) = context {
            evaluated_args.push(ctx);
        }
        for arg in args {
            let val = self.evaluate(arg, input)?;
            evaluated_args.push(val);
        }

        let result = self.apply_function(proc_ast.span, input, evaluated_proc, &evaluated_args)?;

        // Trampoline for thunks (TCO)
        self.trampoline(result, input)
    }

    fn trampoline(&self, mut result: &'a Value<'a>, input: &'a Value<'a>) -> Result<&'a Value<'a>> {
        while let Value::Lambda {
            ref ast,
            input: lambda_input,
            ref captures,
            ..
        } = result
        {
            if let AstKind::Lambda {
                ref body,
                thunk: true,
                ..
            } = ast.kind
            {
                if let AstKind::Function {
                    ref proc, ref args, ..
                } = body.kind
                {
                    {
                        let mut scope = self.scope.borrow_mut();
                        scope.push_scope();
                        scope.restore_captures(captures);
                    }
                    let next = self.evaluate(proc, lambda_input)?;
                    let mut evaluated_args = Vec::with_capacity(args.len());
                    for arg in args {
                        evaluated_args.push(self.evaluate(arg, lambda_input)?);
                    }
                    self.scope.borrow_mut().pop_scope();

                    result = self.apply_function(proc.span, input, next, &evaluated_args)?;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        Ok(result)
    }

    pub fn apply_function(
        &self,
        span: Span,
        input: &'a Value<'a>,
        proc: &'a Value<'a>,
        args: &[&'a Value<'a>],
    ) -> Result<&'a Value<'a>> {
        match proc {
            Value::Lambda {
                ref ast,
                input: lambda_input,
                ref captures,
                ..
            } => {
                if let AstKind::Lambda {
                    ref body,
                    args: ref param_names,
                    ..
                } = ast.kind
                {
                    {
                        let mut scope = self.scope.borrow_mut();
                        scope.push_scope();
                        scope.restore_captures(captures);
                        for (i, param) in param_names.iter().enumerate() {
                            if let AstKind::Var(ref name) = param.kind {
                                let val = args.get(i).copied().unwrap_or_else(|| self.undefined());
                                let n = BumpString::from_str_in(name, self.arena);
                                scope.bind(n.into_bump_str(), val);
                            }
                        }
                    }
                    let result = self.evaluate(body, lambda_input)?;
                    self.scope.borrow_mut().pop_scope();
                    // Trampoline thunks (TCO) -- needed when apply_function
                    // is called directly (e.g., from HOF apply_fn callbacks)
                    // rather than through evaluate_function which does its own
                    // trampoline.
                    self.trampoline(result, input)
                } else {
                    Err(Error::T1006InvokedNonFunction(span))
                }
            }
            Value::NativeFn {
                ref name, ref func, ..
            } => {
                // Handle environment-dependent functions that need access to
                // self.env, which is not available in FnContext.
                match name.as_str() {
                    "now" => {
                        // $now(picture?, timezone?)
                        if args.is_empty() {
                            return Ok(Value::string(self.arena, &self.env.now_iso()));
                        }
                        // With picture/timezone args, format from millis
                        let millis_val = Value::number(self.arena, self.env.now_millis() as f64);
                        let mut from_millis_args: Vec<&'a Value<'a>> =
                            Vec::with_capacity(args.len() + 1);
                        from_millis_args.push(millis_val);
                        from_millis_args.extend_from_slice(args);
                        // Look up $fromMillis and call it
                        if let Some(fm) = self.scope.borrow().lookup("fromMillis") {
                            return self.apply_function(span, input, fm, &from_millis_args);
                        }
                        return Ok(Value::string(self.arena, &self.env.now_iso()));
                    }
                    "millis" => {
                        return Ok(Value::number(self.arena, self.env.now_millis() as f64));
                    }
                    "random" => {
                        return Ok(Value::number(self.arena, self.env.random_f64()));
                    }
                    "uuid" => {
                        return Ok(Value::string(self.arena, &self.env.random_uuid()));
                    }
                    _ => {}
                }

                // The scope lives inside self.scope (RefCell), so nested
                // native-fn calls simply borrow it as needed -- no conflict.
                let apply_fn = |span: Span,
                                input: &'a Value<'a>,
                                proc: &'a Value<'a>,
                                args: &[&'a Value<'a>]|
                 -> Result<&'a Value<'a>> {
                    self.apply_function(span, input, proc, args)
                };
                let context = FnContext {
                    name,
                    char_index: span.start,
                    input,
                    arena: self.arena,
                    apply_fn: &apply_fn,
                };
                func(context, args)
            }
            Value::Transformer {
                ref pattern,
                ref update,
                ref delete,
            } => {
                let input = args.first().copied().unwrap_or_else(|| self.undefined());
                self.apply_transformer(input, pattern, update, delete)
            }
            _ => Err(Error::T1006InvokedNonFunction(span)),
        }
    }

    // -----------------------------------------------------------------------
    // Transform — clone-and-rebuild, NO in-place mutation
    // -----------------------------------------------------------------------

    fn apply_transformer(
        &self,
        input: &'a Value<'a>,
        pattern_ast: &Ast,
        update_ast: &Ast,
        delete_ast: &Option<Box<Ast>>,
    ) -> Result<&'a Value<'a>> {
        if input.is_undefined() {
            return Ok(self.undefined());
        }

        if !input.is_object() && !input.is_array() {
            return Err(Error::T0410ArgumentNotValid(
                pattern_ast.span,
                1,
                "undefined".to_string(),
            ));
        }

        // Step 1: Deep clone the input into the arena
        let cloned: &'a Value<'a> = input.clone_in(self.arena);

        // Step 2: Evaluate the pattern against the clone to find matches
        let wrapped = Value::wrap_in_array(self.arena, cloned, ArrayFlags::empty());
        let matches = self.evaluate(pattern_ast, wrapped)?;

        if matches.is_undefined() {
            return Ok(cloned);
        }

        let matches = Value::wrap_in_array_if_needed(self.arena, matches, ArrayFlags::empty());

        // Step 3: For each match, compute update+delete and store by pointer
        let mut updates: Vec<TransformUpdate<'a>> = Vec::new();

        for m in matches.members() {
            let ptr = m as *const Value<'a>;
            let update = self.evaluate(update_ast, m)?;
            let mut update_val = None;
            let mut delete_keys = None;

            if !update.is_undefined() {
                if !update.is_object() {
                    return Err(Error::T2011UpdateNotObject(
                        update_ast.span,
                        format!("{update:?}"),
                    ));
                }
                update_val = Some(update);
            }

            if let Some(ref del_ast) = delete_ast {
                let deletions = self.evaluate(del_ast, m)?;
                if !deletions.is_undefined() {
                    let deletions =
                        Value::wrap_in_array_if_needed(self.arena, deletions, ArrayFlags::empty());
                    let mut keys = Vec::new();
                    for deletion in deletions.members() {
                        if !deletion.is_string() {
                            return Err(Error::T2012DeleteNotStrings(
                                del_ast.span,
                                format!("{deletions:?}"),
                            ));
                        }
                        keys.push(deletion.as_str());
                    }
                    delete_keys = Some(keys);
                }
            }

            updates.push((ptr, update_val, delete_keys));
        }

        // Step 4: Walk the cloned tree, rebuilding nodes that match
        let result = self.transform_walk(cloned, &updates);
        Ok(result)
    }

    /// Recursively walk a value tree and replace matched nodes with updated copies.
    fn transform_walk(
        &self,
        node: &'a Value<'a>,
        updates: &[TransformUpdate<'a>],
    ) -> &'a Value<'a> {
        let ptr = node as *const Value<'a>;

        // Check if this node is a match target
        if let Some((_, ref update_val, ref delete_keys)) =
            updates.iter().find(|(p, _, _)| *p == ptr)
        {
            if node.is_object() {
                let new_obj = Value::object_with_capacity(self.arena, 8);
                for (k, v) in node.entries() {
                    // Recurse into child values
                    let new_v = self.transform_walk(v, updates);
                    new_obj.insert(k.as_str(), new_v);
                }
                // Apply update fields
                if let Some(upd) = update_val {
                    for (k, v) in upd.entries() {
                        new_obj.insert(k.as_str(), v);
                    }
                }
                // Apply deletions
                if let Some(keys) = delete_keys {
                    for key in keys {
                        new_obj.remove_entry(key);
                    }
                }
                return new_obj;
            }
        }

        // Not a match target — recurse into children
        match node {
            Value::Object(ref map) => {
                let new_obj = Value::object_with_capacity(self.arena, map.len());
                for (k, v) in map.iter() {
                    let new_v = self.transform_walk(v, updates);
                    new_obj.insert(k.as_str(), new_v);
                }
                new_obj
            }
            Value::Array(ref arr, flags) => {
                let new_arr = Value::array_with_capacity(self.arena, arr.len(), *flags);
                for item in arr.iter() {
                    let new_item = self.transform_walk(item, updates);
                    new_arr.push(new_item);
                }
                new_arr
            }
            // Leaf values are returned as-is
            _ => node,
        }
    }

    // -----------------------------------------------------------------------
    // Name lookup (field access)
    // -----------------------------------------------------------------------

    fn fn_lookup_internal(&self, input: &'a Value<'a>, key: &str) -> &'a Value<'a> {
        match input {
            Value::Array(..) => {
                let result = Value::array(self.arena, ArrayFlags::SEQUENCE);
                for item in input.members() {
                    let res = self.fn_lookup_internal(item, key);
                    match res {
                        Value::Undefined => {}
                        Value::Array(..) => {
                            for sub in res.members() {
                                result.push(sub);
                            }
                        }
                        _ => result.push(res),
                    }
                }
                result
            }
            Value::Object(..) => input.get_entry(key).unwrap_or_else(|| self.undefined()),
            _ => self.undefined(),
        }
    }
}

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------

/// Append a value's members (or the value itself) into a bump vector.
pub fn fn_append_internal<'a>(values: &mut BumpVec<'a, &'a Value<'a>>, value: &'a Value<'a>) {
    if value.is_undefined() {
        return;
    }
    match value {
        Value::Array(ref a, _) => values.extend_from_slice(a),
        Value::Range(_) => values.extend(value.members()),
        _ => values.push(value),
    }
}

/// Check if a value is an array of valid numbers.
fn is_array_of_valid_numbers(value: &Value<'_>) -> Result<bool> {
    match value {
        Value::Array(ref a, _) => {
            for member in a.iter() {
                if !member.is_valid_number()? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// Simple stringify for concatenation. Full `$string()` comes in Phase 4.
fn simple_stringify<'a>(value: &'a Value<'a>, buf: &mut String) {
    match value {
        Value::Undefined => {}
        Value::Null => buf.push_str("null"),
        Value::Bool(b) => buf.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            if n.is_nan() || n.is_infinite() {
                buf.push_str("null");
            } else {
                // Use the same formatting as Value serialization for consistency
                buf.push_str(&value.serialize(false));
            }
        }
        Value::String(ref s) => buf.push_str(s.as_str()),
        Value::Array(..) | Value::Range(..) => {
            // JSONata concatenation: arrays are serialized as JSON arrays
            buf.push_str(&value.serialize(false));
        }
        Value::Object(..) => {
            // JSONata $string on objects produces JSON
            buf.push_str(&value.serialize(false));
        }
        Value::Regex(_) => {}
        Value::Lambda { .. } | Value::NativeFn { .. } | Value::Transformer { .. } => {
            buf.push_str("");
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::MockEnvironment;
    use crate::parser;

    /// Helper: parse + evaluate an expression against a JSON input.
    fn eval_with_input(expr: &str, json: &serde_json::Value) -> serde_json::Value {
        let arena = Bump::new();
        let env = MockEnvironment::new(42);
        let ast = parser::parse(expr).expect("parse failed");
        let input = Value::from_json(&arena, json);
        let evaluator = Evaluator::new(&arena, &env, None, 1000, Some(5000));
        let result = evaluator.evaluate(&ast, input).expect("eval failed");
        value_to_json(result)
    }

    /// Helper: parse + evaluate an expression with no input.
    fn eval(expr: &str) -> serde_json::Value {
        eval_with_input(expr, &serde_json::Value::Null)
    }

    /// Convert a Value back to serde_json::Value for easy assertion.
    fn value_to_json(val: &Value<'_>) -> serde_json::Value {
        match val {
            Value::Undefined => serde_json::Value::Null,
            Value::Null => serde_json::Value::Null,
            Value::Bool(b) => serde_json::Value::Bool(*b),
            Value::Number(n) => serde_json::json!(*n),
            Value::String(ref s) => serde_json::Value::String(s.as_str().to_string()),
            Value::Array(ref a, _) => {
                let arr: Vec<serde_json::Value> = a.iter().map(|v| value_to_json(v)).collect();
                serde_json::Value::Array(arr)
            }
            Value::Object(ref o) => {
                let mut map = serde_json::Map::new();
                for (k, v) in o.iter() {
                    map.insert(k.as_str().to_string(), value_to_json(v));
                }
                serde_json::Value::Object(map)
            }
            Value::Range(ref r) => {
                let arr: Vec<serde_json::Value> = (r.start()..=r.end())
                    .map(|i| serde_json::json!(i as f64))
                    .collect();
                serde_json::Value::Array(arr)
            }
            _ => serde_json::Value::Null,
        }
    }

    // -- Literal evaluation --

    #[test]
    fn literal_number() {
        assert_eq!(eval("42"), serde_json::json!(42.0));
    }

    #[test]
    fn literal_string() {
        assert_eq!(eval(r#""hello""#), serde_json::json!("hello"));
    }

    #[test]
    fn literal_bool_true() {
        assert_eq!(eval("true"), serde_json::json!(true));
    }

    #[test]
    fn literal_bool_false() {
        assert_eq!(eval("false"), serde_json::json!(false));
    }

    #[test]
    fn literal_null() {
        assert_eq!(eval("null"), serde_json::Value::Null);
    }

    // -- Simple path access --

    #[test]
    fn name_lookup_on_object() {
        let input = serde_json::json!({"name": "Alice", "age": 30});
        assert_eq!(eval_with_input("name", &input), serde_json::json!("Alice"));
    }

    #[test]
    fn nested_path_access() {
        let input = serde_json::json!({"address": {"city": "Paris"}});
        assert_eq!(
            eval_with_input("address.city", &input),
            serde_json::json!("Paris")
        );
    }

    #[test]
    fn missing_field_returns_undefined() {
        let input = serde_json::json!({"a": 1});
        // Undefined gets serialized as null
        assert_eq!(eval_with_input("b", &input), serde_json::Value::Null);
    }

    // -- Binary arithmetic --

    #[test]
    fn addition() {
        assert_eq!(eval("2 + 3"), serde_json::json!(5.0));
    }

    #[test]
    fn subtraction() {
        assert_eq!(eval("10 - 4"), serde_json::json!(6.0));
    }

    #[test]
    fn multiplication() {
        assert_eq!(eval("6 * 7"), serde_json::json!(42.0));
    }

    #[test]
    fn division() {
        assert_eq!(eval("15 / 3"), serde_json::json!(5.0));
    }

    #[test]
    fn modulus() {
        assert_eq!(eval("17 % 5"), serde_json::json!(2.0));
    }

    #[test]
    fn arithmetic_with_fields() {
        let input = serde_json::json!({"x": 10, "y": 3});
        assert_eq!(eval_with_input("x + y", &input), serde_json::json!(13.0));
    }

    // -- Comparison operators --

    #[test]
    fn less_than() {
        assert_eq!(eval("1 < 2"), serde_json::json!(true));
        assert_eq!(eval("2 < 1"), serde_json::json!(false));
    }

    #[test]
    fn greater_than() {
        assert_eq!(eval("5 > 3"), serde_json::json!(true));
        assert_eq!(eval("3 > 5"), serde_json::json!(false));
    }

    #[test]
    fn less_than_equal() {
        assert_eq!(eval("1 <= 1"), serde_json::json!(true));
        assert_eq!(eval("2 <= 1"), serde_json::json!(false));
    }

    #[test]
    fn greater_than_equal() {
        assert_eq!(eval("5 >= 5"), serde_json::json!(true));
        assert_eq!(eval("3 >= 5"), serde_json::json!(false));
    }

    #[test]
    fn equal() {
        assert_eq!(eval("1 = 1"), serde_json::json!(true));
        assert_eq!(eval("1 = 2"), serde_json::json!(false));
    }

    #[test]
    fn not_equal() {
        assert_eq!(eval("1 != 2"), serde_json::json!(true));
        assert_eq!(eval("1 != 1"), serde_json::json!(false));
    }

    // -- Boolean operators --

    #[test]
    fn and_operator() {
        assert_eq!(eval("true and true"), serde_json::json!(true));
        assert_eq!(eval("true and false"), serde_json::json!(false));
        assert_eq!(eval("false and true"), serde_json::json!(false));
    }

    #[test]
    fn or_operator() {
        assert_eq!(eval("true or false"), serde_json::json!(true));
        assert_eq!(eval("false or true"), serde_json::json!(true));
        assert_eq!(eval("false or false"), serde_json::json!(false));
    }

    // -- Array construction --

    #[test]
    fn array_constructor() {
        assert_eq!(eval("[1, 2, 3]"), serde_json::json!([1.0, 2.0, 3.0]));
    }

    #[test]
    fn empty_array() {
        assert_eq!(eval("[]"), serde_json::json!([]));
    }

    // -- Ternary --

    #[test]
    fn ternary_true_branch() {
        assert_eq!(eval("true ? 1 : 2"), serde_json::json!(1.0));
    }

    #[test]
    fn ternary_false_branch() {
        assert_eq!(eval("false ? 1 : 2"), serde_json::json!(2.0));
    }

    #[test]
    fn ternary_no_false_branch() {
        // Without false branch, undefined -> null
        assert_eq!(eval("false ? 1"), serde_json::Value::Null);
    }

    // -- Concatenation --

    #[test]
    fn string_concat() {
        assert_eq!(
            eval(r#""hello" & " " & "world""#),
            serde_json::json!("hello world")
        );
    }

    #[test]
    fn concat_mixed_types() {
        assert_eq!(eval(r#""count: " & 42"#), serde_json::json!("count: 42"));
    }

    // -- Variable binding --

    #[test]
    fn variable_binding_and_use() {
        assert_eq!(eval("($x := 10; $x + 5)"), serde_json::json!(15.0));
    }

    // -- Range operator --

    #[test]
    fn range_operator() {
        assert_eq!(eval("[1..3]"), serde_json::json!([1.0, 2.0, 3.0]));
    }

    // -- Unary minus --

    #[test]
    fn unary_minus() {
        assert_eq!(eval("-5"), serde_json::json!(-5.0));
    }

    // -- Lambda and function calls --

    #[test]
    fn lambda_definition_and_call() {
        assert_eq!(
            eval("($f := function($x){ $x * 2 }; $f(5))"),
            serde_json::json!(10.0)
        );
    }

    // -- Context variable ($) --

    #[test]
    fn context_variable() {
        let input = serde_json::json!(42);
        assert_eq!(eval_with_input("$", &input), serde_json::json!(42.0));
    }

    // -- Depth limit --

    #[test]
    fn depth_limit_exceeded() {
        let arena = Bump::new();
        let env = MockEnvironment::new(42);
        // Build a deeply nested expression that forces many evaluate() calls.
        // Each nested parenthesized block adds depth.
        // With max_depth=5, nesting 10 levels should trigger it.
        let mut expr = String::from("1");
        for _ in 0..20 {
            expr = format!("({expr} + 1)");
        }
        let ast = parser::parse(&expr).expect("parse failed");
        let input = Value::null(&arena);
        let evaluator = Evaluator::new(&arena, &env, None, 5, Some(5000));
        let result = evaluator.evaluate(&ast, input);
        assert!(result.is_err());
        match result.unwrap_err() {
            Error::DepthLimitExceeded { .. } => {} // expected
            other => panic!("Expected DepthLimitExceeded, got: {other}"),
        }
    }

    // -- Wildcard --

    #[test]
    fn wildcard_on_object() {
        let input = serde_json::json!({"a": 1, "b": 2});
        let result = eval_with_input("*", &input);
        // Wildcard returns all values; order may vary
        if let serde_json::Value::Array(arr) = result {
            assert_eq!(arr.len(), 2);
            assert!(arr.contains(&serde_json::json!(1.0)));
            assert!(arr.contains(&serde_json::json!(2.0)));
        } else {
            panic!("Expected array from wildcard, got: {result:?}");
        }
    }

    #[test]
    fn phone_filter_on_array_input() {
        let input = serde_json::json!([
            {"phone": [{"number": 0}]},
            {"phone": [{"number": 1}]},
            {"phone": [{"number": 2}]}
        ]);
        let result = eval_with_input("phone[0]", &input);
        assert_eq!(result, serde_json::json!({"number": 0.0}));
    }
}
