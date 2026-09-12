use crate::rust_ir::{RustExpr, RustStmt};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TailRecursiveLoopRender {
    params: Vec<String>,
    condition: RustExpr,
    base_return: RustExpr,
    next_args: Vec<RustExpr>,
}

impl TailRecursiveLoopRender {
    pub(crate) fn new(
        params: Vec<String>,
        condition: RustExpr,
        base_return: RustExpr,
        next_args: Vec<RustExpr>,
    ) -> Self {
        Self {
            params,
            condition,
            base_return,
            next_args,
        }
    }

    fn into_parts(self) -> (Vec<String>, RustExpr, RustExpr, Vec<RustExpr>) {
        (
            self.params,
            self.condition,
            self.base_return,
            self.next_args,
        )
    }
}

pub(crate) fn render_tail_recursive_loop_body(plan: TailRecursiveLoopRender) -> Vec<RustStmt> {
    let (params, condition, base_return, next_args) = plan.into_parts();
    let mut stmts = params
        .iter()
        .map(|param| RustStmt::Let {
            mutable: true,
            name: param.clone(),
            ty: None,
            value: Some(RustExpr::Var(param.clone())),
        })
        .collect::<Vec<_>>();

    let mut loop_body = vec![RustStmt::If {
        condition,
        then_body: vec![RustStmt::Return(Some(base_return))],
        else_body: Vec::new(),
    }];
    loop_body.extend(
        next_args
            .into_iter()
            .enumerate()
            .map(|(index, arg)| RustStmt::Let {
                mutable: false,
                name: tail_recursive_arg_temp(index),
                ty: None,
                value: Some(arg),
            }),
    );
    loop_body.extend(
        params
            .into_iter()
            .enumerate()
            .map(|(index, param)| RustStmt::Assign {
                target: RustExpr::Var(param),
                value: RustExpr::Var(tail_recursive_arg_temp(index)),
            }),
    );
    loop_body.push(RustStmt::Continue(None));
    stmts.push(RustStmt::Loop {
        label: None,
        body: loop_body,
    });
    stmts
}

fn tail_recursive_arg_temp(index: usize) -> String {
    format!("__java_tail_arg_{index}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn var(name: &str) -> RustExpr {
        RustExpr::Var(name.to_string())
    }

    fn binary(op: &str, left: RustExpr, right: RustExpr) -> RustExpr {
        RustExpr::Binary {
            op: op.to_string(),
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    #[test]
    fn tail_recursive_loop_body_is_structured() {
        let stmts = render_tail_recursive_loop_body(TailRecursiveLoopRender::new(
            vec!["n".to_string(), "acc".to_string()],
            binary("<=", var("n"), RustExpr::Literal("1".to_string())),
            var("acc"),
            vec![
                binary("-", var("n"), RustExpr::Literal("1".to_string())),
                binary("*", var("acc"), var("n")),
            ],
        ));

        assert_eq!(
            stmts,
            vec![
                RustStmt::Let {
                    mutable: true,
                    name: "n".to_string(),
                    ty: None,
                    value: Some(var("n")),
                },
                RustStmt::Let {
                    mutable: true,
                    name: "acc".to_string(),
                    ty: None,
                    value: Some(var("acc")),
                },
                RustStmt::Loop {
                    label: None,
                    body: vec![
                        RustStmt::If {
                            condition: binary("<=", var("n"), RustExpr::Literal("1".to_string())),
                            then_body: vec![RustStmt::Return(Some(var("acc")))],
                            else_body: Vec::new(),
                        },
                        RustStmt::Let {
                            mutable: false,
                            name: "__java_tail_arg_0".to_string(),
                            ty: None,
                            value: Some(binary("-", var("n"), RustExpr::Literal("1".to_string()),)),
                        },
                        RustStmt::Let {
                            mutable: false,
                            name: "__java_tail_arg_1".to_string(),
                            ty: None,
                            value: Some(binary("*", var("acc"), var("n"))),
                        },
                        RustStmt::Assign {
                            target: var("n"),
                            value: var("__java_tail_arg_0"),
                        },
                        RustStmt::Assign {
                            target: var("acc"),
                            value: var("__java_tail_arg_1"),
                        },
                        RustStmt::Continue(None),
                    ],
                },
            ]
        );
        assert!(!stmts.iter().any(|stmt| matches!(stmt, RustStmt::Raw(_))));
    }
}
