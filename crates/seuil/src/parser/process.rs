//! AST post-processing: path optimization, predicate flattening, tail-call optimization.

use crate::{Error, Result, Span};
use std::mem::take;

use super::ast::*;

impl Ast {
    pub fn process(self) -> Result<Ast> {
        process_ast(self)
    }
}

pub fn process_ast(node: Ast) -> Result<Ast> {
    let mut node = node;
    let keep_array = node.keep_array;

    let mut result = match node.kind {
        AstKind::Name(..) => process_name(node)?,
        AstKind::Block(..) => process_block(node)?,
        AstKind::Unary(..) => process_unary(node)?,
        AstKind::Binary(..) => process_binary(node)?,
        AstKind::GroupBy(ref mut lhs, ref mut rhs) => process_group_by(node.span, lhs, rhs)?,
        AstKind::OrderBy(ref mut lhs, ref mut rhs) => process_order_by(node.span, lhs, rhs)?,
        AstKind::Function {
            ref mut proc,
            ref mut args,
            ..
        } => {
            process_function(proc, args)?;
            node
        }
        AstKind::Lambda { ref mut body, .. } => {
            process_lambda(body)?;
            node
        }
        AstKind::Ternary { .. } => process_ternary(node)?,
        AstKind::Transform { .. } => process_transform(node)?,
        // Parent operator — no longer panics, just passes through for evaluator to handle
        AstKind::Parent => {
            let mut result = Ast::new(AstKind::Parent, node.span);
            result.seeking_parent = true;
            result
        }
        _ => node,
    };

    if keep_array {
        result.keep_array = true;
    }

    Ok(result)
}

fn process_name(node: Ast) -> Result<Ast> {
    let span = node.span;
    let keep_singleton_array = node.keep_array;
    let mut result = Ast::new(AstKind::Path(vec![node]), span);
    result.keep_singleton_array = keep_singleton_array;
    Ok(result)
}

fn process_block(node: Ast) -> Result<Ast> {
    let mut node = node;
    if let AstKind::Block(ref mut exprs) = node.kind {
        for expr in exprs {
            *expr = process_ast(take(expr))?;
        }
    }
    Ok(node)
}

fn process_ternary(node: Ast) -> Result<Ast> {
    let mut node = node;
    if let AstKind::Ternary {
        ref mut cond,
        ref mut truthy,
        ref mut falsy,
    } = node.kind
    {
        **cond = process_ast(take(cond))?;
        **truthy = process_ast(take(truthy))?;
        if let Some(ref mut falsy) = falsy {
            **falsy = process_ast(take(falsy))?;
        }
    }
    Ok(node)
}

fn process_transform(node: Ast) -> Result<Ast> {
    let mut node = node;
    if let AstKind::Transform {
        ref mut pattern,
        ref mut update,
        ref mut delete,
    } = node.kind
    {
        **pattern = process_ast(take(pattern))?;
        **update = process_ast(take(update))?;
        if let Some(ref mut delete) = delete {
            **delete = process_ast(take(delete))?;
        }
    }
    Ok(node)
}

fn process_unary(node: Ast) -> Result<Ast> {
    let mut node = node;
    match node.kind {
        AstKind::Unary(UnaryOp::Minus(value)) => {
            let mut result = process_ast(*value)?;
            match result.kind {
                AstKind::Number(ref mut v) => {
                    *v = -*v;
                    Ok(result)
                }
                _ => Ok(Ast::new(
                    AstKind::Unary(UnaryOp::Minus(Box::new(result))),
                    node.span,
                )),
            }
        }
        AstKind::Unary(UnaryOp::ArrayConstructor(ref mut exprs)) => {
            for expr in exprs {
                *expr = process_ast(take(expr))?;
            }
            Ok(node)
        }
        AstKind::Unary(UnaryOp::ObjectConstructor(ref mut object)) => {
            for pair in object {
                let key = take(&mut pair.0);
                let value = take(&mut pair.1);
                *pair = (process_ast(key)?, process_ast(value)?);
            }
            Ok(node)
        }
        _ => Ok(node),
    }
}

fn process_binary(node: Ast) -> Result<Ast> {
    let mut node = node;
    match node.kind {
        AstKind::Binary(BinaryOp::Map, ref mut lhs, ref mut rhs) => {
            process_path(node.span, lhs, rhs)
        }
        AstKind::Binary(BinaryOp::Predicate, ref mut lhs, ref mut rhs) => {
            process_predicate(node.span, lhs, rhs)
        }
        AstKind::Binary(BinaryOp::FocusBind, ref mut lhs, ref mut rhs) => {
            process_focus_bind(node.span, node.keep_array, lhs, rhs)
        }
        AstKind::Binary(BinaryOp::IndexBind, ref mut lhs, ref mut rhs) => {
            process_index_bind(node.span, lhs, rhs)
        }
        AstKind::Binary(_, ref mut lhs, ref mut rhs) => {
            **lhs = process_ast(take(lhs))?;
            **rhs = process_ast(take(rhs))?;
            Ok(node)
        }
        _ => Ok(node),
    }
}

fn process_path(span: Span, lhs: &mut Box<Ast>, rhs: &mut Box<Ast>) -> Result<Ast> {
    let left_step = process_ast(take(lhs))?;
    let mut rest = process_ast(take(rhs))?;

    let mut result = if matches!(left_step.kind, AstKind::Path(_)) {
        left_step
    } else {
        Ast::new(AstKind::Path(vec![left_step]), span)
    };

    if let AstKind::Path(ref mut steps) = result.kind {
        if let AstKind::Path(ref mut rest_steps) = rest.kind {
            steps.append(rest_steps);
        } else {
            rest.stages = rest.predicates.take();
            steps.push(rest);
        }

        let mut keep_singleton_array = false;
        let last_index = steps.len() - 1;
        let mut has_parent_ref = false;

        for (step_index, step) in steps.iter_mut().enumerate() {
            match step.kind {
                AstKind::Number(..) | AstKind::Bool(..) | AstKind::Null => {
                    return Err(Error::S0213InvalidStep(step.span, "TODO".to_string()));
                }
                AstKind::String(ref s) => {
                    step.kind = AstKind::Name(s.clone());
                }
                AstKind::Unary(UnaryOp::ArrayConstructor(..)) => {
                    if step_index == 0 || step_index == last_index {
                        step.cons_array = true;
                    }
                }
                AstKind::Parent => {
                    has_parent_ref = true;
                    step.seeking_parent = true;
                }
                _ => (),
            }
            keep_singleton_array = keep_singleton_array || step.keep_array;
        }

        result.keep_singleton_array = keep_singleton_array;
        if has_parent_ref {
            result.seeking_parent = true;
        }
    }

    Ok(result)
}

fn process_predicate(span: Span, lhs: &mut Box<Ast>, rhs: &mut Box<Ast>) -> Result<Ast> {
    let mut result = process_ast(take(lhs))?;
    let mut in_path = false;

    let node = if let AstKind::Path(ref mut steps) = result.kind {
        in_path = true;
        let last_index = steps.len() - 1;
        &mut steps[last_index]
    } else {
        &mut result
    };

    if node.group_by.is_some() {
        return Err(Error::S0209InvalidPredicate(span));
    }

    let filter = Ast::new(AstKind::Filter(Box::new(process_ast(take(rhs))?)), span);

    if in_path {
        match node.stages {
            None => node.stages = Some(vec![filter]),
            Some(ref mut stages) => stages.push(filter),
        }
    } else {
        match node.predicates {
            None => node.predicates = Some(vec![filter]),
            Some(ref mut predicates) => predicates.push(filter),
        }
    }

    Ok(result)
}

fn process_focus_bind(
    span: Span,
    keep_array: bool,
    lhs: &mut Box<Ast>,
    rhs: &mut Box<Ast>,
) -> Result<Ast> {
    let mut result = process_ast(take(lhs))?;
    let step = if let AstKind::Path(ref mut steps) = result.kind {
        let last_index = steps.len() - 1;
        &mut steps[last_index]
    } else {
        &mut result
    };

    if step.stages.is_some() || step.predicates.is_some() {
        return Err(Error::S0215BindingAfterPredicates(span));
    }

    if let AstKind::Sort(..) = step.kind {
        return Err(Error::S0216BindingAfterSort(span));
    }

    if keep_array {
        step.keep_array = true;
    }

    let focus = if let AstKind::Var(ref var) = rhs.kind {
        var.clone()
    } else {
        unreachable!()
    };
    step.focus = Some(focus);
    step.tuple = true;

    Ok(result)
}

fn process_index_bind(span: Span, lhs: &mut Box<Ast>, rhs: &mut Box<Ast>) -> Result<Ast> {
    let mut result = process_ast(take(lhs))?;
    let mut is_path = false;

    let step = if let AstKind::Path(ref mut steps) = result.kind {
        is_path = true;
        let last_index = steps.len() - 1;
        &mut steps[last_index]
    } else {
        if result.predicates.is_some() {
            result.stages = result.predicates.take();
        }
        &mut result
    };

    step.tuple = true;

    let index = if let AstKind::Var(ref var) = rhs.kind {
        var.clone()
    } else {
        unreachable!()
    };

    match step.stages {
        None => step.index = Some(index),
        Some(ref mut stages) => {
            let index = Ast::new(AstKind::Index(index), span);
            stages.push(index);
        }
    }

    Ok(if !is_path {
        Ast::new(AstKind::Path(vec![result]), span)
    } else {
        result
    })
}

fn process_group_by(span: Span, lhs: &mut Box<Ast>, rhs: &mut Object) -> Result<Ast> {
    let mut result = process_ast(take(lhs))?;

    if result.group_by.is_some() {
        return Err(Error::S0210MultipleGroupBy(span));
    }

    for pair in rhs.iter_mut() {
        let key = take(&mut pair.0);
        let value = take(&mut pair.1);
        *pair = (process_ast(key)?, process_ast(value)?);
    }

    result.group_by = Some((span, take(rhs)));
    Ok(result)
}

fn process_order_by(span: Span, lhs: &mut Box<Ast>, rhs: &mut SortTerms) -> Result<Ast> {
    let lhs = process_ast(take(lhs))?;

    let mut result = if matches!(lhs.kind, AstKind::Path(_)) {
        lhs
    } else {
        Ast::new(AstKind::Path(vec![lhs]), span)
    };

    for pair in rhs.iter_mut() {
        *pair = (process_ast(take(&mut pair.0))?, pair.1);
    }

    if let AstKind::Path(ref mut steps) = result.kind {
        steps.push(Ast::new(AstKind::Sort(take(rhs)), span));
    }

    Ok(result)
}

fn process_function(proc: &mut Box<Ast>, args: &mut [Ast]) -> Result<()> {
    **proc = process_ast(take(&mut **proc))?;
    for arg in args.iter_mut() {
        *arg = process_ast(take(arg))?;
    }
    Ok(())
}

fn process_lambda(body: &mut Box<Ast>) -> Result<()> {
    let new_body = process_ast(take(body))?;
    let new_body = tail_call_optimize(new_body)?;
    **body = new_body;
    Ok(())
}

fn tail_call_optimize(mut expr: Ast) -> Result<Ast> {
    match &mut expr.kind {
        AstKind::Function { .. } if expr.predicates.is_none() => {
            let span = expr.span;
            let thunk = Ast::new(
                AstKind::Lambda {
                    name: String::from("thunk"),
                    args: vec![],
                    thunk: true,
                    body: Box::new(expr),
                },
                span,
            );
            Ok(thunk)
        }
        AstKind::Ternary { truthy, falsy, .. } => {
            **truthy = tail_call_optimize(take(truthy))?;
            if let Some(inner) = falsy {
                *falsy = Some(Box::new(tail_call_optimize(take(inner))?));
            }
            Ok(expr)
        }
        AstKind::Block(statements) => {
            let length = statements.len();
            if length > 0 {
                statements[length - 1] = tail_call_optimize(take(&mut statements[length - 1]))?;
            }
            Ok(expr)
        }
        _ => Ok(expr),
    }
}
