//! Backward liveness over a callable body.
//!
//! Replaces the syntactic "textual suffix" approximation of *which names can be
//! read after this point*. The suffix scan cannot see back edges, so a value
//! consumed inside a loop looked like a last use and was moved without a clone.
//! Each loop form previously needed its own patch to re-add the loop-carried
//! reads; this computes the relation once, as a fixpoint, so a new
//! loop-producing lowering cannot reintroduce that hole.
//!
//! Java statements are structured (no arbitrary `goto`), so the dataflow is
//! expressed compositionally — `live_in(stmt, live_out)` — rather than over an
//! explicit CFG. Loops iterate to a fixpoint, which is what makes the back edge
//! visible. The result is deliberately a *may* analysis: over-approximating
//! liveness costs an unnecessary clone, while under-approximating produces code
//! that does not compile, so every uncertain construct widens the set.

use std::collections::{HashMap, HashSet};

use crate::java_ir::{BinaryOp, Block, Expr, Stmt, UnaryOp};

/// Identity of a declared variable: its name plus the declaration it came from.
///
/// Keying on the name alone conflates two different variables that happen to
/// share it. The declaration id distinguishes them, so a live set can never
/// claim an inner binding keeps an outer one alive.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Binding {
    name: String,
    /// Declaration site, or `OUTER_DECL` for anything declared outside the
    /// analyzed body (parameters, fields).
    decl: usize,
}

/// Declaration id for names that enter from outside the analyzed body.
const OUTER_DECL: usize = 0;

impl Binding {
    fn new(name: &str, decl: usize) -> Self {
        Self {
            name: name.to_string(),
            decl,
        }
    }
}

/// Name -> declaration id currently in scope.
type ScopeMap = HashMap<String, usize>;

/// Which relation the backward walk is computing. The control-flow structure —
/// loop fixpoints, the `try` live floor, scope-correct kills — is identical for
/// both; only what a statement *generates* differs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Pass {
    /// Gen on `use`: which bindings may be read after this point.
    #[default]
    Reads,
    /// Gen on `def`: which bindings may be written after this point.
    Writes,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct LivenessAnalysis {
    live_out: HashMap<usize, HashSet<Binding>>,
    /// May-be-written-after: the same backward fixpoint as `live_out`, but
    /// generating on `def` instead of `use`. Also a *may* analysis — an
    /// over-approximated write costs a defensive copy, an under-approximated
    /// one is an aliasing bug.
    mutated_after: HashMap<usize, HashSet<Binding>>,
    /// Scope in effect at each statement, so a read can be resolved to the
    /// declaration it actually refers to.
    scope_at: HashMap<usize, ScopeMap>,
    pass: Pass,
}

fn stmt_id(stmt: &Stmt) -> usize {
    std::ptr::from_ref(stmt) as usize
}

/// The name a statement introduces, if it introduces one.
fn declared_binding_name(stmt: &Stmt) -> Option<&str> {
    match stmt {
        Stmt::VarDecl { name, .. } => Some(name),
        Stmt::EnhancedFor { var_name, .. } => Some(var_name),
        _ => None,
    }
}

fn add_true_pattern_bindings_to_scope(expr: &Expr, decl: usize, scope: &mut ScopeMap) {
    match expr {
        Expr::InstanceOf {
            binding: Some(name),
            ..
        } => {
            scope.insert(name.clone(), decl);
        }
        Expr::Unary {
            op: UnaryOp::Not,
            expr,
        } => add_false_pattern_bindings_to_scope(expr, decl, scope),
        Expr::Binary {
            op: BinaryOp::And,
            left,
            right,
        } => {
            add_true_pattern_bindings_to_scope(left, decl, scope);
            add_true_pattern_bindings_to_scope(right, decl, scope);
        }
        _ => {}
    }
}

fn add_false_pattern_bindings_to_scope(expr: &Expr, decl: usize, scope: &mut ScopeMap) {
    match expr {
        Expr::Unary {
            op: UnaryOp::Not,
            expr,
        } => add_true_pattern_bindings_to_scope(expr, decl, scope),
        Expr::Binary {
            op: BinaryOp::Or,
            left,
            right,
        } => {
            add_false_pattern_bindings_to_scope(left, decl, scope);
            add_false_pattern_bindings_to_scope(right, decl, scope);
        }
        _ => {}
    }
}

fn add_switch_case_pattern_bindings_to_scope(
    case: &crate::java_ir::SwitchCase,
    decl: usize,
    scope: &mut ScopeMap,
) {
    for label in &case.labels {
        if let crate::java_ir::SwitchLabel::Expr(expr) = label {
            add_true_pattern_bindings_to_scope(expr, decl, scope);
        }
    }
}

fn remove_bindings_declared_at(bindings: &mut HashSet<Binding>, decl: usize) {
    bindings.retain(|binding| binding.decl != decl);
}

fn remove_scope_bindings_declared_at(scope: &mut ScopeMap, decl: usize) {
    scope.retain(|_, binding_decl| *binding_decl != decl);
}

impl LivenessAnalysis {
    /// A nested declaration with the same spelling must not invalidate a
    /// provenance fact about a binding supplied by the enclosing scope.
    pub(crate) fn body_assigns_outer_binding(body: &Block, name: &str) -> bool {
        let mut analysis = Self::default();
        analysis.record_scopes_block(body, &mut ScopeMap::new());
        analysis.pass = Pass::Writes;
        analysis
            .block_live_in(body, &HashSet::new(), None)
            .contains(&Binding::new(name, OUTER_DECL))
    }

    /// Analyze a callable body. Nothing is live on normal exit.
    pub(crate) fn for_body(body: &Block) -> Self {
        let mut analysis = Self::default();
        analysis.record_scopes_block(body, &mut ScopeMap::new());
        let exit = HashSet::new();
        analysis.block_live_in(body, &exit, None);
        analysis.pass = Pass::Writes;
        analysis.block_live_in(body, &exit, None);
        analysis.pass = Pass::Reads;
        analysis
    }

    /// Analyze a block that a lowering turns into a *loop body* — a lambda
    /// passed to `forEach`, for instance. The block's statements are nested in
    /// an expression, so the enclosing body's statement walk never reaches them,
    /// yet they still execute repeatedly and need the same back-edge treatment.
    ///
    /// `rebound` names (the lambda parameters) are fresh on each iteration, so
    /// reads of them are not carried across the back edge.
    pub(crate) fn for_loop_body(
        block: &Block,
        rebound: &[String],
        enclosing_live: &[String],
    ) -> Self {
        let mut analysis = Self::default();
        analysis.record_scopes_block(block, &mut ScopeMap::new());
        let enclosing = enclosing_live
            .iter()
            .map(|name| Binding::new(name, OUTER_DECL))
            .collect::<HashSet<_>>();
        analysis.pass = Pass::Reads;
        analysis.loop_fixpoint(&enclosing, |analysis, carried| {
            let mut live = analysis.block_live_in(block, carried, Some(carried));
            live.retain(|binding| !rebound.contains(&binding.name));
            live
        });
        analysis.pass = Pass::Writes;
        analysis.loop_fixpoint(&enclosing, |analysis, carried| {
            let mut live = analysis.block_live_in(block, carried, Some(carried));
            live.retain(|binding| !rebound.contains(&binding.name));
            live
        });
        analysis.pass = Pass::Reads;
        analysis
    }

    /// A callback can return and subsequently be invoked again. Only its
    /// captured bindings survive that boundary: parameters and body locals
    /// are fresh on each invocation, and a same-named local is not a capture.
    pub(crate) fn for_callback_body(
        block: &Block,
        rebound: &[String],
        captures: &[String],
        enclosing_live: &[String],
    ) -> Self {
        let mut analysis = Self::for_loop_body(block, rebound, enclosing_live);
        let floor = captures
            .iter()
            .filter(|name| !rebound.contains(name))
            .map(|name| Binding::new(name, OUTER_DECL))
            .collect();
        analysis.add_live_floor_to_block(block, &floor);
        analysis
    }

    /// How many times each name is read *within* `stmt`.
    ///
    /// Not a dataflow relation — within-statement multiplicity — but it answers
    /// "how is this binding used here", so it belongs to the same owner rather
    /// than being a separate context field computed at each call site.
    pub(crate) fn use_counts_in(&self, stmt: &Stmt) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        crate::lower::collect_stmt_read_counts_for_liveness(stmt, &mut counts);
        counts
    }

    /// Names read by an expression that is evaluated again on a loop back edge.
    ///
    /// A single textual read in a loop condition is dynamically repeated.  It
    /// therefore has the same move-safety consequence as two reads in an
    /// ordinary statement: consuming an owned value requires recovery even
    /// when no read is live after the loop statement itself.
    pub(crate) fn repeated_use_counts_in(&self, stmt: &Stmt) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        match stmt {
            Stmt::While { condition, .. } | Stmt::DoWhile { condition, .. } => {
                crate::lower::collect_expr_read_counts_for_liveness(condition, &mut counts);
            }
            Stmt::For {
                condition, update, ..
            } => {
                if let Some(condition) = condition {
                    crate::lower::collect_expr_read_counts_for_liveness(condition, &mut counts);
                }
                for expression in update {
                    crate::lower::collect_expr_read_counts_for_liveness(expression, &mut counts);
                }
            }
            _ => {}
        }
        counts
    }

    /// Whether the binding `name` refers to at `stmt` may be read afterwards.
    ///
    /// Resolves the name to the binding in scope at that point and compares
    /// bindings, so a same-named variable from another scope cannot answer for
    /// it. `None` means the statement was not analyzed.
    pub(crate) fn binding_is_live_after(&self, stmt: &Stmt, name: &str) -> Option<bool> {
        let bindings = self.live_out.get(&stmt_id(stmt))?;
        Some(bindings.contains(&self.binding_at(stmt, name)))
    }

    /// The write-side equivalent of [`Self::binding_is_live_after`].
    pub(crate) fn binding_is_mutated_after(&self, stmt: &Stmt, name: &str) -> Option<bool> {
        let bindings = self.mutated_after.get(&stmt_id(stmt))?;
        Some(bindings.contains(&self.binding_at(stmt, name)))
    }

    /// Declaration identity at this program point. The token is only for
    /// matching declarations in the same analyzed tree, never dereferenced.
    pub(crate) fn declaration_site_at(&self, stmt: &Stmt, name: &str) -> Option<usize> {
        let declaration = self.binding_at(stmt, name).decl;
        (declaration != OUTER_DECL).then_some(declaration)
    }

    /// The binding a name refers to at `stmt`.
    ///
    /// `scope_at` deliberately records the scope *before* a statement's own
    /// declaration, so that `int x = x + 1` reads the outer `x`. Both callers
    /// here ask about the program point *after* the statement, where a name the
    /// statement declares refers to the new binding — so a declaration is
    /// resolved to itself. Without this, "is `values` written after
    /// `byte[] values = …`?" resolved to `OUTER_DECL`, matched nothing in the
    /// write set, and the local came out non-`mut`.
    fn binding_at(&self, stmt: &Stmt, name: &str) -> Binding {
        if let Some(declared) = declared_binding_name(stmt)
            && declared == name
        {
            return Binding::new(name, stmt_id(stmt));
        }
        let decl = self
            .scope_at
            .get(&stmt_id(stmt))
            .and_then(|scope| scope.get(name).copied())
            .unwrap_or(OUTER_DECL);
        Binding::new(name, decl)
    }

    /// Names that may be *written* after `stmt` completes.
    #[cfg(test)]
    fn mutated_after_for(&self, stmt: &Stmt) -> Option<HashSet<String>> {
        self.mutated_after.get(&stmt_id(stmt)).map(|bindings| {
            bindings
                .iter()
                .map(|binding| binding.name.clone())
                .collect()
        })
    }

    /// Bindings that may be read after `stmt` completes.
    #[cfg(test)]
    fn live_out_bindings_for(&self, stmt: &Stmt) -> Option<&HashSet<Binding>> {
        self.live_out.get(&stmt_id(stmt))
    }

    /// Names that may be read after `stmt` completes.
    ///
    /// A projection of [`Self::live_out_bindings_for`] for consumers that still
    /// work in names. Exact at each program point: a declaration kills its name
    /// for everything before it, so at most one binding of a given name is ever
    /// live at once.
    ///
    /// `None` means the statement was not part of the analyzed body (for
    /// example a synthesized statement), and the caller should fall back.
    pub(crate) fn live_out_for(&self, stmt: &Stmt) -> Option<HashSet<String>> {
        self.live_out.get(&stmt_id(stmt)).map(|bindings| {
            bindings
                .iter()
                .map(|binding| binding.name.clone())
                .collect()
        })
    }

    /// Record the scope in effect at each statement, so reads can be resolved to
    /// declarations. Runs forward, which is the order scoping is defined in.
    fn record_scopes_block(&mut self, block: &Block, scope: &mut ScopeMap) {
        let outer = scope.clone();
        for stmt in &block.statements {
            self.record_scopes_stmt(stmt, scope);
        }
        *scope = outer;
    }

    fn record_scopes_stmt(&mut self, stmt: &Stmt, scope: &mut ScopeMap) {
        // The statement's own reads see the scope *before* any declaration it
        // makes: `int x = x + 1` reads the outer `x`.
        self.scope_at.insert(stmt_id(stmt), scope.clone());
        match stmt {
            Stmt::VarDecl { name, .. } => {
                scope.insert(name.clone(), stmt_id(stmt));
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
            } => {
                let mut then_scope = scope.clone();
                add_true_pattern_bindings_to_scope(condition, stmt_id(stmt), &mut then_scope);
                self.record_scopes_block(then_block, &mut then_scope);
                if let Some(else_block) = else_block {
                    let mut else_scope = scope.clone();
                    add_false_pattern_bindings_to_scope(condition, stmt_id(stmt), &mut else_scope);
                    self.record_scopes_block(else_block, &mut else_scope);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                let mut body_scope = scope.clone();
                add_true_pattern_bindings_to_scope(condition, stmt_id(stmt), &mut body_scope);
                self.record_scopes_block(body, &mut body_scope);
            }
            Stmt::DoWhile { body, .. } => {
                self.record_scopes_block(body, &mut scope.clone());
            }
            Stmt::For {
                init,
                condition,
                body,
                ..
            } => {
                let mut inner = scope.clone();
                for stmt in init {
                    self.record_scopes_stmt(stmt, &mut inner);
                }
                let mut body_scope = inner;
                if let Some(condition) = condition {
                    add_true_pattern_bindings_to_scope(condition, stmt_id(stmt), &mut body_scope);
                }
                self.record_scopes_block(body, &mut body_scope);
            }
            Stmt::EnhancedFor { var_name, body, .. } => {
                let mut inner = scope.clone();
                inner.insert(var_name.clone(), stmt_id(stmt));
                self.record_scopes_block(body, &mut inner);
            }
            Stmt::Switch { cases, .. } => {
                let mut inner = scope.clone();
                for case in cases {
                    let mut case_scope = inner.clone();
                    add_switch_case_pattern_bindings_to_scope(case, stmt_id(stmt), &mut case_scope);
                    for stmt in &case.statements {
                        self.record_scopes_stmt(stmt, &mut case_scope);
                    }
                    inner = case_scope;
                    remove_scope_bindings_declared_at(&mut inner, stmt_id(stmt));
                }
            }
            Stmt::Try {
                resources,
                body,
                catches,
                finally_block,
            } => {
                let mut inner = scope.clone();
                for stmt in resources {
                    self.record_scopes_stmt(stmt, &mut inner);
                }
                self.record_scopes_block(body, &mut inner);
                for catch in catches {
                    let mut catch_scope = scope.clone();
                    catch_scope.insert(catch.source_name.clone(), stmt_id(stmt));
                    self.record_scopes_block(&catch.body, &mut catch_scope);
                }
                if let Some(finally_block) = finally_block {
                    self.record_scopes_block(finally_block, &mut scope.clone());
                }
            }
            Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
                self.record_scopes_block(block, &mut scope.clone());
            }
            _ => {}
        }
    }

    /// Resolve the names read by `expr` to bindings, using the scope at `stmt`.
    fn read_bindings(&self, stmt: &Stmt, expr: &Expr, out: &mut HashSet<Binding>) {
        let scope = self.scope_at.get(&stmt_id(stmt));
        self.read_bindings_in_scope(expr, scope, out);
    }

    fn read_bindings_in_scope(
        &self,
        expr: &Expr,
        scope: Option<&ScopeMap>,
        out: &mut HashSet<Binding>,
    ) {
        let mut names = HashSet::new();
        match self.pass {
            Pass::Reads => read_names(expr, &mut names),
            Pass::Writes => written_names(expr, &mut names),
        }
        for name in names {
            let decl = scope
                .and_then(|scope| scope.get(&name).copied())
                .unwrap_or(OUTER_DECL);
            out.insert(Binding::new(&name, decl));
        }
    }

    fn scope_after_for_init(&self, stmt: &Stmt, init: &[Stmt]) -> ScopeMap {
        let mut scope = self
            .scope_at
            .get(&stmt_id(stmt))
            .cloned()
            .unwrap_or_default();
        for init_stmt in init {
            if let Some(name) = declared_binding_name(init_stmt) {
                scope.insert(name.to_string(), stmt_id(init_stmt));
            }
        }
        scope
    }

    fn block_live_in(
        &mut self,
        block: &Block,
        live_out: &HashSet<Binding>,
        loop_live: Option<&HashSet<Binding>>,
    ) -> HashSet<Binding> {
        let mut live = live_out.clone();
        for stmt in block.statements.iter().rev() {
            live = self.stmt_live_in(stmt, &live, loop_live);
        }
        live
    }

    /// Union `floor` into the live-out of every statement in `block`.
    ///
    /// Used for the guarded region of a `try`: an exception can leave *any*
    /// statement for the catch, so a catch's reads stay live throughout — even
    /// past a `return`, which otherwise kills everything after it.
    fn add_live_floor_to_block(&mut self, block: &Block, floor: &HashSet<Binding>) {
        for stmt in &block.statements {
            self.add_live_floor_to_stmt(stmt, floor);
        }
    }

    fn add_live_floor_to_stmt(&mut self, stmt: &Stmt, floor: &HashSet<Binding>) {
        let target = match self.pass {
            Pass::Reads => &mut self.live_out,
            Pass::Writes => &mut self.mutated_after,
        };
        if let Some(live) = target.get_mut(&stmt_id(stmt)) {
            live.extend(floor.iter().cloned());
        }
        match stmt {
            Stmt::If {
                then_block,
                else_block,
                ..
            } => {
                self.add_live_floor_to_block(then_block, floor);
                if let Some(else_block) = else_block {
                    self.add_live_floor_to_block(else_block, floor);
                }
            }
            Stmt::While { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::For { body, .. }
            | Stmt::EnhancedFor { body, .. }
            | Stmt::Block(body)
            | Stmt::LabeledBlock { body, .. } => self.add_live_floor_to_block(body, floor),
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    for stmt in &case.statements {
                        self.add_live_floor_to_stmt(stmt, floor);
                    }
                }
            }
            Stmt::Try {
                resources,
                body,
                catches,
                finally_block,
            } => {
                for stmt in resources {
                    self.add_live_floor_to_stmt(stmt, floor);
                }
                self.add_live_floor_to_block(body, floor);
                for catch in catches {
                    self.add_live_floor_to_block(&catch.body, floor);
                }
                if let Some(finally_block) = finally_block {
                    self.add_live_floor_to_block(finally_block, floor);
                }
            }
            _ => {}
        }
    }

    fn stmt_live_in(
        &mut self,
        stmt: &Stmt,
        live_out: &HashSet<Binding>,
        loop_live: Option<&HashSet<Binding>>,
    ) -> HashSet<Binding> {
        let recorded_live_out = if matches!(stmt, Stmt::Return(_) | Stmt::Throw(_)) {
            HashSet::new()
        } else {
            live_out.clone()
        };
        match self.pass {
            Pass::Reads => self.live_out.insert(stmt_id(stmt), recorded_live_out),
            Pass::Writes => self.mutated_after.insert(stmt_id(stmt), recorded_live_out),
        };
        match stmt {
            Stmt::VarDecl { initializer, .. } => {
                let mut live = live_out.clone();
                // Kill exactly this declaration's binding; a same-named binding
                // from an outer scope stays live.
                live.retain(|binding| binding.decl != stmt_id(stmt));
                if let Some(initializer) = initializer {
                    self.read_bindings(stmt, initializer, &mut live);
                }
                live
            }
            Stmt::Assign { target, value } => {
                let mut live = live_out.clone();
                if self.pass == Pass::Writes {
                    // The target expression is the write itself. Reads still
                    // flow through the expression walker below.
                    let mut names = HashSet::new();
                    crate::lower::collect_assignment_target_assigned_names_for_liveness(
                        target, &mut names,
                    );
                    for name in names {
                        let decl = self
                            .scope_at
                            .get(&stmt_id(stmt))
                            .and_then(|scope| scope.get(&name).copied())
                            .unwrap_or(OUTER_DECL);
                        live.insert(Binding::new(&name, decl));
                    }
                }
                self.read_bindings(stmt, target, &mut live);
                self.read_bindings(stmt, value, &mut live);
                live
            }
            Stmt::Expr(expr) => {
                let mut live = live_out.clone();
                self.read_bindings(stmt, expr, &mut live);
                live
            }
            // Nothing in this callable runs after a return or throw.
            Stmt::Return(expr) => {
                let mut live = HashSet::new();
                if let Some(expr) = expr {
                    self.read_bindings(stmt, expr, &mut live);
                }
                live
            }
            Stmt::Throw(expr) => {
                let mut live = HashSet::new();
                self.read_bindings(stmt, expr, &mut live);
                live
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
            } => {
                let mut live = self.block_live_in(then_block, live_out, loop_live);
                match else_block {
                    Some(else_block) => {
                        live.extend(self.block_live_in(else_block, live_out, loop_live));
                    }
                    // No else: the condition may fall straight through.
                    None => live.extend(live_out.iter().cloned()),
                }
                remove_bindings_declared_at(&mut live, stmt_id(stmt));
                self.read_bindings(stmt, condition, &mut live);
                live
            }
            Stmt::While {
                condition, body, ..
            } => self.loop_fixpoint(live_out, |analysis, carried| {
                let mut live = analysis.block_live_in(body, carried, Some(carried));
                remove_bindings_declared_at(&mut live, stmt_id(stmt));
                analysis.read_bindings(stmt, condition, &mut live);
                live
            }),
            Stmt::DoWhile {
                condition, body, ..
            } => self.loop_fixpoint(live_out, |analysis, carried| {
                let mut header = carried.clone();
                analysis.read_bindings(stmt, condition, &mut header);
                analysis.block_live_in(body, &header, Some(carried))
            }),
            Stmt::For {
                init,
                condition,
                update,
                body,
                ..
            } => {
                let loop_scope = self.scope_after_for_init(stmt, init);
                let carried = self.loop_fixpoint(live_out, |analysis, carried| {
                    let mut header = analysis.block_live_in(body, carried, Some(carried));
                    remove_bindings_declared_at(&mut header, stmt_id(stmt));
                    for expr in update {
                        analysis.read_bindings_in_scope(expr, Some(&loop_scope), &mut header);
                    }
                    if let Some(condition) = condition {
                        analysis.read_bindings_in_scope(condition, Some(&loop_scope), &mut header);
                    }
                    header
                });
                let mut live = carried;
                for stmt in init.iter().rev() {
                    live = self.stmt_live_in(stmt, &live, loop_live);
                }
                live
            }
            Stmt::EnhancedFor { iterable, body, .. } => {
                let mut live = self.loop_fixpoint(live_out, |analysis, carried| {
                    let mut header = analysis.block_live_in(body, carried, Some(carried));
                    // The loop variable is rebound each iteration, so a read of
                    // it inside the body is not carried across the back edge.
                    header.retain(|binding| binding.decl != stmt_id(stmt));
                    header
                });
                live.retain(|binding| binding.decl != stmt_id(stmt));
                self.read_bindings(stmt, iterable, &mut live);
                live
            }
            Stmt::Switch { selector, cases } => {
                // Cases may fall through. Walk from bottom to top so a case
                // without an effective break carries the next case's live-in.
                let mut live = live_out.clone();
                let mut trailing_case_live = live_out.clone();
                for case in cases.iter().rev() {
                    let mut trailing = trailing_case_live.clone();
                    for stmt in case.statements.iter().rev() {
                        trailing = self.stmt_live_in(stmt, &trailing, loop_live);
                    }
                    remove_bindings_declared_at(&mut trailing, stmt_id(stmt));
                    trailing_case_live = trailing.clone();
                    live.extend(trailing);
                }
                self.read_bindings(stmt, selector, &mut live);
                live
            }
            Stmt::Try {
                resources,
                body,
                catches,
                finally_block,
            } => {
                // An exception can transfer control from anywhere in the body to
                // any catch, so treat everything reachable here as live rather
                // than modelling per-statement exception edges.
                // Compute the abrupt-completion floor before the authoritative
                // normal continuation. The empty-exit query also records
                // per-statement facts; doing it last erased the finally's
                // live-out bindings that are read after the entire try.
                let finally_floor = finally_block
                    .as_ref()
                    .map(|block| self.block_live_in(block, &HashSet::new(), loop_live))
                    .unwrap_or_default();
                let mut live = live_out.clone();
                if let Some(finally_block) = finally_block {
                    live = self.block_live_in(finally_block, &live, loop_live);
                }
                let after_body = live.clone();
                for catch in catches {
                    let mut catch_live = self.block_live_in(&catch.body, &after_body, loop_live);
                    remove_catch_binding(stmt, catch, &mut catch_live);
                    live.extend(catch_live);
                }
                // Everything a catch reads must stay live across the whole
                // guarded region, because the exception edge leaves from any
                // statement in it — not only from the entry. A `finally` needs
                // the same treatment for a different reason: it runs however the
                // body exits, including through a `return` that would otherwise
                // kill everything after it.
                let mut guarded_floor = finally_floor;
                for catch in catches {
                    let mut reads = self.block_live_in(&catch.body, &after_body, loop_live);
                    remove_catch_binding(stmt, catch, &mut reads);
                    guarded_floor.extend(reads);
                }
                live = self.block_live_in(body, &live, loop_live);
                self.add_live_floor_to_block(body, &guarded_floor);
                // A terminating guarded body (for example, `return`) clears
                // its normal predecessor set. Catch and finally regions are
                // still reachable across that abrupt completion, so their
                // reads/writes must also flow back to statements before the
                // try. Without this union a local assigned only by catch or
                // finally was rendered immutable when the try returned.
                live.extend(guarded_floor);
                for stmt in resources.iter().rev() {
                    live = self.stmt_live_in(stmt, &live, loop_live);
                }
                live
            }
            Stmt::Block(block) => self.block_live_in(block, live_out, loop_live),
            Stmt::LabeledBlock { body, .. } => {
                // A labelled break can leave from anywhere inside, so the block
                // exit stays live throughout.
                self.block_live_in(body, live_out, Some(live_out))
            }
            // `break`/`continue` jump to the loop exit or header. Both are
            // covered by the enclosing loop's carried set, which is the sound
            // over-approximation here.
            Stmt::Break(_) | Stmt::Continue(_) => {
                loop_live.cloned().unwrap_or_else(|| live_out.clone())
            }
            Stmt::LocalType(_) | Stmt::ConstructorInvocation { .. } | Stmt::Unsupported(_) => {
                live_out.clone()
            }
        }
    }

    /// Iterate a loop body until the carried live set stops growing, so reads
    /// reached through the back edge are included.
    fn loop_fixpoint<F>(&mut self, live_out: &HashSet<Binding>, mut body: F) -> HashSet<Binding>
    where
        F: FnMut(&mut Self, &HashSet<Binding>) -> HashSet<Binding>,
    {
        let mut carried = live_out.clone();
        loop {
            let mut next = body(self, &carried);
            next.extend(live_out.iter().cloned());
            if next.len() == carried.len() && next.iter().all(|binding| carried.contains(binding)) {
                return next;
            }
            carried = next;
        }
    }
}

fn remove_catch_binding(
    try_stmt: &Stmt,
    catch: &crate::java_ir::CatchClause,
    bindings: &mut HashSet<Binding>,
) {
    let catch_binding = Binding::new(&catch.source_name, stmt_id(try_stmt));
    bindings.retain(|binding| binding != &catch_binding);
}

fn read_names(expr: &Expr, names: &mut HashSet<String>) {
    crate::lower::collect_expr_read_names_for_liveness(expr, names);
}

fn written_names(expr: &Expr, names: &mut HashSet<String>) {
    crate::lower::collect_expr_assigned_names_for_liveness(expr, names);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java_ir::{Block, CatchClause, Expr, JavaType, Stmt};

    fn var(name: &str) -> Expr {
        Expr::Var(name.to_string())
    }

    fn decl(name: &str, init: Expr) -> Stmt {
        Stmt::VarDecl {
            name: name.to_string(),
            source_name: name.to_string(),
            ty: crate::java_ir::JavaType::String,
            initializer: Some(init),
            binding: None,
        }
    }

    fn int(value: i32) -> Expr {
        Expr::Literal(crate::java_ir::Literal::Int(value))
    }

    #[test]
    fn callback_exit_keeps_only_outer_captures_live() {
        let body = Block {
            statements: vec![Stmt::Return(Some(var("capture")))],
        };
        let analysis = LivenessAnalysis::for_callback_body(
            &body,
            &["argument".into()],
            &["capture".into()],
            &[],
        );
        let stmt = &body.statements[0];
        assert_eq!(analysis.binding_is_live_after(stmt, "capture"), Some(true));
        assert_eq!(
            analysis.binding_is_live_after(stmt, "argument"),
            Some(false)
        );
        let ordinary = LivenessAnalysis::for_loop_body(&body, &[], &[]);
        assert_eq!(ordinary.binding_is_live_after(stmt, "capture"), Some(false));

        let shadowed = Block {
            statements: vec![
                decl("capture", var("argument")),
                Stmt::Return(Some(var("capture"))),
            ],
        };
        let analysis = LivenessAnalysis::for_callback_body(
            &shadowed,
            &["argument".into()],
            &["capture".into()],
            &[],
        );
        assert_eq!(
            analysis.binding_is_live_after(&shadowed.statements[1], "capture"),
            Some(false)
        );
    }

    fn int_decl(name: &str, init: i32) -> Stmt {
        Stmt::VarDecl {
            name: name.to_string(),
            source_name: name.to_string(),
            ty: crate::java_ir::JavaType::Primitive(crate::java_ir::PrimitiveType::Int),
            initializer: Some(int(init)),
            binding: None,
        }
    }

    fn string_instanceof_binding(binding: &str) -> Expr {
        Expr::InstanceOf {
            expr: Box::new(var("input")),
            ty: JavaType::String,
            binding: Some(binding.to_string()),
        }
    }

    #[test]
    fn finally_floor_does_not_erase_the_normal_continuation() {
        let body = Block {
            statements: vec![
                decl("value", var("initial")),
                Stmt::Try {
                    resources: vec![],
                    body: Block {
                        statements: vec![Stmt::Expr(var("work"))],
                    },
                    catches: vec![],
                    finally_block: Some(Block {
                        statements: vec![Stmt::Expr(var("value"))],
                    }),
                },
                Stmt::Return(Some(var("value"))),
            ],
        };
        let analysis = LivenessAnalysis::for_body(&body);
        let Stmt::Try {
            finally_block: Some(finally),
            ..
        } = &body.statements[1]
        else {
            panic!()
        };
        assert_eq!(
            analysis.binding_is_live_after(&finally.statements[0], "value"),
            Some(true)
        );
        assert_eq!(
            analysis.binding_is_live_after(&body.statements[2], "value"),
            Some(false)
        );
    }

    #[test]
    fn catch_binding_filter_preserves_distinct_same_name_bindings() {
        let try_stmt = Stmt::Try {
            resources: Vec::new(),
            body: Block {
                statements: Vec::new(),
            },
            catches: Vec::new(),
            finally_block: None,
        };
        let catch = CatchClause {
            types: vec![crate::java_ir::JavaType::Named {
                name: "Exception".to_string(),
                args: Vec::new(),
            }],
            name: "e".to_string(),
            source_name: "e".to_string(),
            body: Block {
                statements: Vec::new(),
            },
            binding: None,
        };
        let catch_binding = Binding::new("e", stmt_id(&try_stmt));
        let outer_binding = Binding::new("e", OUTER_DECL);
        let other_binding = Binding::new("other", OUTER_DECL);
        let mut bindings = HashSet::from([
            catch_binding.clone(),
            outer_binding.clone(),
            other_binding.clone(),
        ]);

        remove_catch_binding(&try_stmt, &catch, &mut bindings);

        assert!(!bindings.contains(&catch_binding));
        assert!(bindings.contains(&outer_binding));
        assert!(bindings.contains(&other_binding));
    }

    #[test]
    fn catch_and_finally_writes_reach_declaration_before_terminating_try_body() {
        let declaration = decl("out", var("initial"));
        let try_stmt = Stmt::Try {
            resources: Vec::new(),
            body: Block {
                statements: vec![Stmt::Return(Some(var("result")))],
            },
            catches: vec![CatchClause {
                types: vec![crate::java_ir::JavaType::Named {
                    name: "RuntimeException".to_string(),
                    args: Vec::new(),
                }],
                name: "ex".to_string(),
                source_name: "ex".to_string(),
                body: Block {
                    statements: vec![Stmt::Assign {
                        target: var("out"),
                        value: var("caught"),
                    }],
                },
                binding: None,
            }],
            finally_block: Some(Block {
                statements: vec![Stmt::Assign {
                    target: var("out"),
                    value: var("finalized"),
                }],
            }),
        };
        let body = Block {
            statements: vec![declaration, try_stmt],
        };
        let analysis = LivenessAnalysis::for_body(&body);

        assert_eq!(
            analysis.binding_is_mutated_after(&body.statements[0], "out"),
            Some(true),
            "abrupt completion of the guarded body must not erase catch/finally writes"
        );
    }

    /// Two sibling scopes each declaring `x` are different variables. A live set
    /// must not let the second one keep the first alive, or every such move
    /// would take a needless clone.
    #[test]
    fn sibling_scopes_declaring_the_same_name_are_distinct_bindings() {
        let body = Block {
            statements: vec![
                Stmt::Block(Block {
                    statements: vec![decl("x", var("a")), Stmt::Expr(var("x"))],
                }),
                Stmt::Block(Block {
                    statements: vec![decl("x", var("b")), Stmt::Expr(var("x"))],
                }),
            ],
        };
        let analysis = LivenessAnalysis::for_body(&body);
        let Stmt::Block(first) = &body.statements[0] else {
            panic!("expected a block");
        };
        // After `use(x)` in the first block nothing more reads that `x`.
        let live = analysis
            .live_out_for(&first.statements[1])
            .expect("statement was analyzed");
        assert!(
            !live.contains("x"),
            "the second block's `x` is a different binding: {live:?}"
        );
    }

    /// Reads of the same name resolve to the declaration actually in scope.
    #[test]
    fn reads_resolve_to_the_enclosing_declaration() {
        let inner = Stmt::Block(Block {
            statements: vec![decl("x", var("a")), Stmt::Expr(var("x"))],
        });
        let body = Block {
            statements: vec![inner, Stmt::Expr(var("x"))],
        };
        let analysis = LivenessAnalysis::for_body(&body);
        let Stmt::Block(inner_block) = &body.statements[0] else {
            panic!("expected a block");
        };
        // The trailing `use(x)` reads an outer binding, so it is live across the
        // inner block, but the inner declaration's own binding is not.
        let live = analysis
            .live_out_bindings_for(&inner_block.statements[1])
            .expect("statement was analyzed");
        assert!(
            live.iter().all(|binding| binding.decl == OUTER_DECL),
            "inner `x` must not appear as live: {live:?}"
        );
    }

    #[test]
    fn if_pattern_binding_read_does_not_keep_outer_same_name_live() {
        let body = Block {
            statements: vec![
                Stmt::Expr(var("probe")),
                Stmt::If {
                    condition: string_instanceof_binding("value"),
                    then_block: Block {
                        statements: vec![Stmt::Expr(var("value"))],
                    },
                    else_block: None,
                },
            ],
        };
        let analysis = LivenessAnalysis::for_body(&body);
        let live = analysis
            .live_out_bindings_for(&body.statements[0])
            .expect("probe statement was analyzed");

        assert!(
            !live.contains(&Binding::new("value", OUTER_DECL)),
            "a read of the pattern binding must not keep the outer same-name binding live: {live:?}"
        );
    }

    #[test]
    fn if_pattern_binding_write_does_not_mutate_outer_same_name() {
        let body = Block {
            statements: vec![
                Stmt::Expr(var("probe")),
                Stmt::If {
                    condition: string_instanceof_binding("value"),
                    then_block: Block {
                        statements: vec![Stmt::Assign {
                            target: var("value"),
                            value: var("next"),
                        }],
                    },
                    else_block: None,
                },
            ],
        };
        let analysis = LivenessAnalysis::for_body(&body);
        let mutated = analysis
            .mutated_after
            .get(&stmt_id(&body.statements[0]))
            .expect("probe statement was analyzed by write pass");

        assert!(
            !mutated.contains(&Binding::new("value", OUTER_DECL)),
            "a write to the pattern binding must not count as mutating the outer same-name binding: {mutated:?}"
        );
    }

    #[test]
    fn negated_else_pattern_binding_read_does_not_keep_outer_same_name_live() {
        let body = Block {
            statements: vec![
                Stmt::Expr(var("probe")),
                Stmt::If {
                    condition: Expr::Unary {
                        op: crate::java_ir::UnaryOp::Not,
                        expr: Box::new(string_instanceof_binding("value")),
                    },
                    then_block: Block {
                        statements: Vec::new(),
                    },
                    else_block: Some(Block {
                        statements: vec![Stmt::Expr(var("value"))],
                    }),
                },
            ],
        };
        let analysis = LivenessAnalysis::for_body(&body);
        let live = analysis
            .live_out_bindings_for(&body.statements[0])
            .expect("probe statement was analyzed");

        assert!(
            !live.contains(&Binding::new("value", OUTER_DECL)),
            "a read of a negated-pattern else binding must not keep the outer same-name binding live: {live:?}"
        );
    }

    #[test]
    fn while_pattern_binding_write_does_not_mutate_outer_same_name() {
        let body = Block {
            statements: vec![
                Stmt::Expr(var("probe")),
                Stmt::While {
                    condition: string_instanceof_binding("value"),
                    body: Block {
                        statements: vec![Stmt::Assign {
                            target: var("value"),
                            value: var("next"),
                        }],
                    },
                    label: None,
                },
            ],
        };
        let analysis = LivenessAnalysis::for_body(&body);
        let mutated = analysis
            .mutated_after
            .get(&stmt_id(&body.statements[0]))
            .expect("probe statement was analyzed by write pass");

        assert!(
            !mutated.contains(&Binding::new("value", OUTER_DECL)),
            "a write to a while-pattern binding must not count as mutating the outer same-name binding: {mutated:?}"
        );
    }

    #[test]
    fn switch_pattern_label_read_does_not_keep_outer_same_name_live() {
        let body = Block {
            statements: vec![
                Stmt::Expr(var("probe")),
                Stmt::Switch {
                    selector: var("input"),
                    cases: vec![crate::java_ir::SwitchCase {
                        labels: vec![crate::java_ir::SwitchLabel::Expr(
                            string_instanceof_binding("value"),
                        )],
                        statements: vec![Stmt::Expr(var("value"))],
                    }],
                },
            ],
        };
        let analysis = LivenessAnalysis::for_body(&body);
        let live = analysis
            .live_out_bindings_for(&body.statements[0])
            .expect("probe statement was analyzed");

        assert!(
            !live.contains(&Binding::new("value", OUTER_DECL)),
            "a read of a switch pattern-label binding must not keep the outer same-name binding live: {live:?}"
        );
    }

    #[test]
    fn return_live_out_does_not_report_unreachable_continuation_reads() {
        let body = Block {
            statements: vec![Stmt::Return(Some(var("x"))), Stmt::Expr(var("later"))],
        };
        let analysis = LivenessAnalysis::for_body(&body);
        let live = analysis
            .live_out_for(&body.statements[0])
            .expect("return statement was analyzed");
        assert!(
            live.is_empty(),
            "a return has no normal continuation live-out: {live:?}"
        );
    }

    #[test]
    fn switch_case_fallthrough_keeps_next_case_reads_live() {
        let first_stmt = Stmt::Expr(var("first"));
        let second_stmt = Stmt::Expr(var("x"));
        let body = Block {
            statements: vec![Stmt::Switch {
                selector: int(1),
                cases: vec![
                    crate::java_ir::SwitchCase {
                        labels: vec![crate::java_ir::SwitchLabel::Expr(int(1))],
                        statements: vec![first_stmt],
                    },
                    crate::java_ir::SwitchCase {
                        labels: vec![crate::java_ir::SwitchLabel::Expr(int(2))],
                        statements: vec![second_stmt],
                    },
                ],
            }],
        };
        let analysis = LivenessAnalysis::for_body(&body);
        let Stmt::Switch { cases, .. } = &body.statements[0] else {
            panic!("expected switch");
        };
        let live = analysis
            .live_out_for(&cases[0].statements[0])
            .expect("case statement was analyzed");
        assert!(
            live.contains("x"),
            "fallthrough into the next case should keep its reads live: {live:?}"
        );
    }

    #[test]
    fn loop_body_analysis_computes_write_pass() {
        let first_stmt = Stmt::Expr(var("first"));
        let body = Block {
            statements: vec![
                first_stmt,
                Stmt::Assign {
                    target: var("x"),
                    value: var("value"),
                },
            ],
        };
        let analysis = LivenessAnalysis::for_loop_body(&body, &[], &[]);
        let mutated = analysis
            .mutated_after_for(&body.statements[0])
            .expect("loop body statement was analyzed by write pass");
        assert!(
            mutated.contains("x"),
            "writes later in a lowered loop body should be visible: {mutated:?}"
        );
    }

    #[test]
    fn for_update_mutates_initializer_binding() {
        let body = Block {
            statements: vec![Stmt::For {
                init: vec![int_decl("i", 0)],
                condition: Some(Expr::Binary {
                    op: crate::java_ir::BinaryOp::Lt,
                    left: Box::new(var("i")),
                    right: Box::new(int(10)),
                }),
                update: vec![Expr::Unary {
                    op: crate::java_ir::UnaryOp::PostInc,
                    expr: Box::new(var("i")),
                }],
                body: Block {
                    statements: vec![Stmt::Expr(var("i"))],
                },
                label: None,
            }],
        };
        let analysis = LivenessAnalysis::for_body(&body);
        let Stmt::For { init, .. } = &body.statements[0] else {
            panic!("expected for loop");
        };
        let init_stmt = &init[0];
        assert_eq!(
            analysis.binding_is_mutated_after(init_stmt, "i"),
            Some(true),
            "the update expression should resolve `i` to the for initializer binding"
        );
    }

    #[test]
    fn field_assignment_mutates_receiver_binding() {
        let body = Block {
            statements: vec![
                decl("first", var("source")),
                Stmt::Assign {
                    target: Expr::FieldAccess(Box::new(var("first")), "value".to_string()),
                    value: int(7),
                },
            ],
        };
        let analysis = LivenessAnalysis::for_body(&body);
        assert_eq!(
            analysis.binding_is_mutated_after(&body.statements[0], "first"),
            Some(true),
            "field assignment should mark the receiver binding mutable"
        );
    }

    #[test]
    fn unary_array_element_update_mutates_receiver_binding() {
        let body = Block {
            statements: vec![
                decl("values", Expr::ArrayInitializer(vec![int(1)])),
                Stmt::Expr(Expr::Unary {
                    op: crate::java_ir::UnaryOp::PostInc,
                    expr: Box::new(Expr::ArrayAccess(Box::new(var("values")), Box::new(int(0)))),
                }),
            ],
        };
        let analysis = LivenessAnalysis::for_body(&body);
        assert_eq!(
            analysis.binding_is_mutated_after(&body.statements[0], "values"),
            Some(true),
            "array element unary update should mark the receiver binding mutable"
        );
    }
}
