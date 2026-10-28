//! Initialized enum payloads. Identity remains the enum discriminant; source
//! constructors and field slots, not getter-time evaluation, initialize state.
use super::*;
use crate::rust_ir::RustParam;
use crate::slot_render::default_expr_for_slot_with_facts;

fn enabled(ty: &TypeDecl) -> bool {
    crate::constructor_plan::enum_requires_instance_initialization(ty)
}

/// Source Enum.valueOf has a nullable Java String input despite returning a
/// required enum reference. Keep null until the enum's own checked-name boundary
/// so its exception kind/message are not replaced by an arbitrary slot unwrap.
pub(super) fn value_of(expr: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> Option<RustExpr> {
    let Expr::Call {
        name,
        args,
        resolved,
        ..
    } = expr
    else {
        return None;
    };
    if !matches!(name.as_str(), "valueOf" | "value_of") {
        return None;
    }
    // An enum may declare other valueOf overloads. Only the compiler-provided
    // String overload uses this checked nullable runtime entry point.
    let call = resolved.as_ref()?;
    if call.rust_name != crate::java_naming::ENUM_VALUE_OF_RESULT_METHOD {
        return None;
    }
    let [argument] = args.as_slice() else {
        return None;
    };
    // Resolution already identifies the compiler-provided overload and its
    // declaring enum. Rediscovering it from a qualified receiver can fail when
    // primary and support contexts both contain the enclosing declaration.
    let ty = find_source_enum_type_decl_for_java_type(
        &JavaType::Named {
            name: call.declaring_type.clone(),
            args: vec![],
        },
        ctx,
    )?;
    let mut input = value_shape(crate::types::optional_java_type(JavaType::String));
    input.ownership = OwnershipShape::SharedBorrow;
    input.view = ViewShape::Str;
    let argument = lower_argument_for_explicit_slot(argument, env, Slot::Argument(input), ctx);
    Some(apply_result_value_policy_from_ctx(
        function_call_projection(
            owner_associated_path_projection(
                &crate::java_naming::rust_name_for_java_named_type(&ty.name),
                "__java_value_of_nullable_result",
            ),
            vec![argument],
        ),
        ctx,
    ))
}

pub(super) fn constructor_metadata(expr: &Expr, ctx: LowerCtx) -> Option<(RustExpr, JavaType)> {
    let instance = ctx.current_enum_instance?;
    let Expr::Call {
        receiver,
        name,
        args,
        ..
    } = expr
    else {
        return None;
    };
    if !args.is_empty()
        || receiver
            .as_deref()
            .is_some_and(|receiver| !is_this_receiver_expr(receiver))
    {
        return None;
    }
    let ty = match name.as_str() {
        "ordinal" => JavaType::Primitive(PrimitiveType::Int),
        "name" => JavaType::String,
        _ => return None,
    };
    let value = receiver_method_call_projection_exact(RustExpr::Var(instance.into()), name, vec![]);
    Some((
        if name == "name" {
            receiver_method_call_projection_exact(value, "to_string", vec![])
        } else {
            value
        },
        ty,
    ))
}

/// Accessor-backed enum fields are not Rust places. Evaluate the receiver
/// once, perform the ordinary Java primitive update on a local, and publish
/// the result through the same setter used by assignment. No callback runs
/// while a payload mutex is held.
pub(super) fn update(
    op: UnaryOp,
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    if !matches!(
        op,
        UnaryOp::PreInc | UnaryOp::PreDec | UnaryOp::PostInc | UnaryOp::PostDec
    ) {
        return None;
    }
    let this = Expr::This;
    let (receiver, field) = match expr {
        Expr::Var(name) if env.local_shape(name).is_none() => (&this, name.as_str()),
        Expr::FieldAccess(receiver, field) => (receiver.as_ref(), field.as_str()),
        _ => return None,
    };
    let accessor = source_java_enum_field_accessor_name_for_receiver(receiver, field, env, ctx)?;
    let owner = if is_this_receiver_expr(receiver) {
        find_source_enum_type_decl_for_current_type(ctx.current_type?, ctx)?
    } else {
        let shape = infer_expr_shape_with_context(receiver, env, ctx.expr_shape_context());
        find_source_enum_type_decl_for_java_type(&shape.java_ty, ctx)?
    };
    if !enabled(owner) {
        return None;
    }
    let JavaType::Primitive(primitive) = numeric_operand_java_type(expr, env, ctx) else {
        return None;
    };
    let value_name = "__java_enum_update_value";
    let result_name = "__java_enum_update_result";
    let receiver_name = "__java_enum_update_receiver";
    let update = java_primitive_update_expression_projection(
        RustExpr::Var(value_name.into()),
        matches!(op, UnaryOp::PreInc | UnaryOp::PostInc),
        primitive,
        matches!(op, UnaryOp::PreInc | UnaryOp::PreDec),
    )?;
    let lowered = adapt_optional_instance_receiver_with_slot_plan(
        receiver,
        lower_expr_with_refinements(receiver, env, ctx, refinements),
        env,
        ctx,
        false,
        false,
    );
    Some(RustExpr::Block {
        stmts: vec![
            RustStmt::Let {
                mutable: false,
                name: receiver_name.into(),
                ty: None,
                value: Some(lowered),
            },
            RustStmt::Let {
                mutable: true,
                name: value_name.into(),
                ty: None,
                value: Some(receiver_method_call_projection_exact(
                    RustExpr::Var(receiver_name.into()),
                    &accessor,
                    vec![],
                )),
            },
            RustStmt::Let {
                mutable: false,
                name: result_name.into(),
                ty: None,
                value: Some(update),
            },
            expr_stmt(receiver_method_call_projection_exact(
                RustExpr::Var(receiver_name.into()),
                &format!("__java_set_{}", crate::parser::to_snake_case(field)),
                vec![RustExpr::Var(value_name.into())],
            )),
        ],
        tail: Some(Box::new(RustExpr::Var(result_name.into()))),
    })
}

pub(super) fn support(ty: &TypeDecl, ctx: LowerCtx) -> Option<String> {
    if !enabled(ty) {
        return None;
    }
    let plan = crate::constructor_plan::EnumInitializationPlan::for_type(ty)?;
    let name = crate::java_naming::rust_name_for_java_named_type(&ty.name);
    let payload = format!("__JavaEnumPayload{name}");
    let mut declarations = vec!["__published: bool".to_string()];
    let mut defaults = vec!["__published: false".to_string()];
    let mut methods = Vec::new();
    for field in &plan.instance_fields {
        let field_name = crate::parser::to_snake_case(&field.name);
        let storage_name = crate::java_naming::source_instance_field_rust_name(ty, &field.name);
        let field_token = crate::representation_lowering::rust_identifier_token(&storage_name);
        let shape = ctx.signatures.required_declared_field_shape(ty, field);
        let storage_type = rust_type_for_slot_with_facts_and_representations(
            &shape,
            ctx.facts,
            ctx.representations,
        );
        let rust_ty = emit::emit_type(&storage_type);
        declarations.push(format!("{field_token}: {rust_ty}"));
        defaults.push(format!(
            "{field_token}: {}",
            emit::emit_expr(&default_expr_for_slot_with_facts(&shape, ctx.facts))
        ));
        let mut borrowed = shape.clone();
        borrowed.ownership = OwnershipShape::SharedBorrow;
        let value = RustExpr::Field(
            Box::new(RustExpr::Var("__java_payload_guard".into())),
            storage_name.clone(),
        );
        // Project the optional element, not its Option shell. Slot conversion
        // then copies values or clones retained identities without moving state.
        let storage_payload = match &storage_type {
            RustType::Option(payload) => payload.as_ref(),
            payload => payload,
        };
        let copied = if matches!(storage_payload, RustType::Generic { base, .. }
            if matches!(base.as_str(), "std::rc::Rc" | "std::sync::Arc"))
        {
            // A field read copies its stored canonical handle. A borrowed
            // nominal view may dereference that handle to its Mutex/RefCell;
            // recovering ownership from that view would clone the payload.
            crate::coercion::clone_reference_carrier_for_evaluation(
                value,
                &shape,
                RustExpr::path(["std", "clone", "Clone", "clone"]),
            )
        } else if let Some(element) = shape.optional_element_type() {
            let lowered = optional_element_borrow_lowered_expr_with_facts(
                value,
                &shape,
                element,
                OwnershipShape::SharedBorrow,
                ctx.facts,
            )
            .expect("nullable payload must expose an element borrow");
            let conversion =
                plan_lowered_slot_conversion(lowered.actual(), Slot::Assignment(shape), ctx.facts);
            apply_lowered_slot_expr_conversion(lowered, &conversion)
        } else {
            apply_expr_to_slot_via_plan(
                force_shared_borrow_projection(value),
                borrowed,
                &Slot::Assignment(shape),
                ctx.facts,
            )
        };
        let accessor = crate::representation_lowering::rust_identifier_token(
            &crate::java_naming::source_enum_field_accessor_name(ty, &field_name),
        );
        methods.push(format!("pub fn {accessor}(&self) -> {rust_ty} {{ Self::__java_class_init(); let __java_payload_guard = Self::__java_payloads()[self.ordinal() as usize].lock().expect(\"Java enum payload mutex poisoned\"); {} }}\npub fn __java_set_{field_name}(&self, value: {rust_ty}) {{ Self::__java_class_init(); Self::__java_payloads()[self.ordinal() as usize].lock().expect(\"Java enum payload mutex poisoned\").{field_token} = value; }}", emit::emit_expr(&copied)));
    }
    Some(format!(
        "struct {payload} {{ {} }}\nimpl {name} {{\nfn __java_payloads() -> &'static Vec<std::sync::Mutex<{payload}>> {{ static STATE: std::sync::OnceLock<Vec<std::sync::Mutex<{payload}>>> = std::sync::OnceLock::new(); STATE.get_or_init(|| (0..{}).map(|_| std::sync::Mutex::new({payload} {{ {} }})).collect()) }}\npub fn __java_enum_constant(&self) -> Option<Self> {{ Self::__java_class_init(); if Self::__java_payloads()[self.ordinal() as usize].lock().expect(\"Java enum payload mutex poisoned\").__published {{ Some(*self) }} else {{ None }} }}\nfn __java_publish(&self) {{ Self::__java_payloads()[self.ordinal() as usize].lock().expect(\"Java enum payload mutex poisoned\").__published = true; }}\nfn __java_values_ready() -> bool {{ Self::__java_payloads().iter().all(|value| value.lock().expect(\"Java enum payload mutex poisoned\").__published) }}\n{}\n}}",
        declarations.join(", "),
        plan.constants.len(),
        defaults.join(", "),
        methods.join("\n")
    ))
}

pub(super) fn constructors(ty: &TypeDecl, ctx: LowerCtx) -> Vec<RustFunction> {
    if !enabled(ty) {
        return vec![];
    }
    let declared = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Constructor(ctor) => Some(ctor),
            _ => None,
        })
        .collect::<Vec<_>>();
    let implicit;
    let constructors = if declared.is_empty() {
        implicit = ConstructorDecl {
            name: ty.name.clone(),
            rust_name: Some("new".into()),
            type_params: vec![],
            type_param_bounds: vec![],
            params: vec![],
            throws: vec![],
            modifiers: vec![Modifier::Private],
            body: Some(Block { statements: vec![] }),
            span: ty.span,
        };
        vec![&implicit]
    } else {
        declared
    };
    constructors
        .into_iter()
        .map(|ctor| {
            let signature = ctx.signatures.constructor_signature(ty, ctor);
            let mut env = signature
                .map(|signature| shape_env_for_constructor(ty, ctor, signature, ctx.signatures))
                .unwrap_or_else(|| shape_env_for_type(ty, ctx.signatures));
            seed_imported_static_type_aliases(&mut env, ctx.program);
            seed_context_static_type_aliases(&mut env, ctx);
            seed_support_type_fields(&mut env, ctx.support_local_type_program, ctx.signatures);
            let instance = env.fresh_local_name("__java_enum_instance");
            let returns_result =
                signature.is_some() && source_constructor_returns_result(ty, ctor, ctx);
            let liveness = ctor
                .body
                .as_ref()
                .map(crate::liveness::LivenessAnalysis::for_body)
                .unwrap_or_default();
            let ctor_ctx = LowerCtx {
                current_type: Some(&ty.name),
                current_constructor: Some(ctor.rust_name.as_deref().unwrap_or("new")),
                current_constructor_decl: Some(ctor),
                constructor_frame_owner: Some(ty),
                current_method: None,
                current_method_decl: None,
                current_method_contract: None,
                current_receiver: Some(ReceiverPlan::Shared),
                current_self_handle: None,
                current_enum_instance: Some(&instance),
                current_return_shape: None,
                current_local_shapes: signature.map(|signature| signature.locals()).unwrap_or(&[]),
                constructor_return_signal_only: true,
                liveness: &liveness,
                result_propagation_mode: if returns_result {
                    ResultPropagationMode::Propagate
                } else {
                    ResultPropagationMode::UncaughtPanic
                },
                ..ctx
            };
            let source = ctor
                .body
                .as_ref()
                .expect("source enum constructor must have a body");
            let mut statements = Vec::new();
            let mut skip = 0;
            if let Some(Stmt::ConstructorInvocation {
                kind: crate::java_ir::ConstructorInvocationKind::This,
                args,
                resolved,
                ..
            }) = source.statements.first()
            {
                let call = resolved
                    .as_ref()
                    .expect("resolved enum delegation required");
                let mut arguments = vec![RustExpr::Var(instance.clone())];
                arguments.extend(
                    lower_resolved_constructor_varargs_args(args, &env, call, ctor_ctx)
                        .unwrap_or_else(|| {
                            lower_constructor_args_with_plan(
                                args,
                                &env,
                                plan_constructor_arguments(Some(call), args.len(), ctx.signatures),
                                ctor_ctx,
                            )
                        }),
                );
                let selected = ty
                    .members
                    .iter()
                    .find_map(|member| match member {
                        Member::Constructor(candidate)
                            if crate::signature::ConstructorKey::source_constructor(
                                ty, candidate,
                            ) == crate::signature::ConstructorKey::resolved_constructor(
                                call,
                            ) =>
                        {
                            Some(candidate)
                        }
                        _ => None,
                    })
                    .expect("selected enum delegation declaration required");
                let value = function_call_projection(
                    self_associated_path_projection(&format!("__java_enum_{}", call.rust_name)),
                    arguments,
                );
                statements.push(expr_stmt(
                    if source_constructor_returns_result(ty, selected, ctx) {
                        apply_result_value_policy_from_ctx(value, ctor_ctx)
                    } else {
                        value
                    },
                ));
                skip = 1;
            } else {
                statements.extend(lower_delegated_constructor_trailing_stmts(
                    ty,
                    &crate::constructor_plan::instance_field_initializer_statements(ty),
                    &env,
                    ctor_ctx,
                    returns_result,
                ));
            }
            statements.extend(lower_block(
                &Block {
                    statements: source.statements[skip..].to_vec(),
                },
                &mut env,
                returns_result,
                ctor_ctx,
            ));
            let replacement = SelfReceiverReplacement {
                expr: RustExpr::Var(instance.clone()),
                field_expr: RustExpr::Var(instance.clone()),
                mutable_field_expr: RustExpr::Var(instance.clone()),
                mutable_field_access_borrows_owner: false,
            };
            statements = statements
                .into_iter()
                .map(|stmt| replace_self_receiver_in_stmt_with_policy(stmt, &replacement, false))
                .collect();
            if returns_result {
                statements.push(RustStmt::Return(Some(function_call_projection(
                    RustExpr::path(["Ok"]),
                    vec![RustExpr::Tuple(vec![])],
                ))));
            }
            let mut function = if signature.is_some() {
                apply_representation_aware_constructor_signature(
                    constructor_function_shell_with_facts_and_support_contexts(
                        ty,
                        ctor,
                        ctx.signatures,
                        vec![],
                        ctx.facts,
                        &source_bound_support_programs(ctx),
                        Some(ctx.clone_requirement_cache),
                    ),
                    ty,
                    ctor,
                    ctx,
                )
            } else {
                RustFunction {
                    name: "new".into(),
                    type_params: vec![],
                    params: vec![],
                    return_type: None,
                    where_bounds: vec![],
                    body: vec![],
                    is_pub: false,
                    is_pub_crate: false,
                    receiver: None,
                }
            };
            function.name = format!("__java_enum_{}", function.name);
            function.params.insert(
                0,
                RustParam {
                    name: instance,
                    ty: RustType::Named(crate::java_naming::rust_name_for_java_named_type(
                        &ty.name,
                    )),
                },
            );
            function.return_type = returns_result.then(|| exception_result_type(RustType::Unit));
            function.body = statements;
            function
        })
        .collect()
}

pub(super) fn initialize_constant(stmt: &Stmt, env: &ShapeEnv, ctx: LowerCtx) -> Option<RustStmt> {
    if !ctx
        .current_method_decl
        .is_some_and(crate::class_initialization::is_initializer)
    {
        return None;
    }
    let owner = find_context_type_decl_by_erased_name(ctx, ctx.current_type?)?;
    if !enabled(owner) {
        return None;
    }
    let Stmt::Assign {
        target: Expr::Var(name),
        value: Expr::New { args, resolved, .. },
    } = stmt
    else {
        return None;
    };
    let plan = crate::constructor_plan::EnumInitializationPlan::for_type(owner)?;
    let constant = plan
        .constants
        .iter()
        .find(|constant| constant.field.name == *name)?;
    let (method, returns_result) = match &constant.constructor {
        crate::constructor_plan::EnumConstructorSelection::Resolved { declaration, .. } => (
            declaration.rust_name.as_deref().unwrap_or("new"),
            source_constructor_returns_result(owner, declaration, ctx),
        ),
        crate::constructor_plan::EnumConstructorSelection::ImplicitDefault => ("new", false),
        crate::constructor_plan::EnumConstructorSelection::Unresolved => {
            return Some(expr_stmt(crate::unsupported::compile_error_expr(
                "unresolved enum constant constructor",
            )));
        }
    };
    let identity = owner_associated_path_projection(
        &crate::java_naming::rust_name_for_java_named_type(&owner.name),
        &crate::object_model::rust_enum_variant_name_from_parsed_constant(name),
    );
    let mut arguments = vec![identity.clone()];
    arguments.extend(
        resolved
            .as_ref()
            .and_then(|call| lower_resolved_constructor_varargs_args(args, env, call, ctx))
            .unwrap_or_else(|| {
                lower_constructor_args_with_plan(
                    args,
                    env,
                    plan_constructor_arguments(resolved.as_ref(), args.len(), ctx.signatures),
                    ctx,
                )
            }),
    );
    let call = function_call_projection(
        self_associated_path_projection(&format!("__java_enum_{method}")),
        arguments,
    );
    let call = if returns_result {
        apply_result_value_policy_from_ctx(call, ctx)
    } else {
        call
    };
    Some(RustStmt::Expr(RustExpr::Block {
        stmts: vec![
            expr_stmt(call),
            expr_stmt(receiver_method_call_projection_exact(
                identity,
                "__java_publish",
                vec![],
            )),
        ],
        tail: None,
    }))
}
