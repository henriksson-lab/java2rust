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

use crate::java_ir::{Block, Expr, Stmt};

/// Identity of a declared variable: its name plus the declaration it came from.
///
/// Keying on the name alone conflates two different variables that happen to
/// share it. The declaration id distinguishes them, so a live set can never
/// claim an inner binding keeps an outer one alive.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Binding {
    pub name: String,
    /// Declaration site, or `OUTER_DECL` for anything declared outside the
    /// analyzed body (parameters, fields).
    pub decl: usize,
}

/// Declaration id for names that enter from outside the analyzed body.
pub const OUTER_DECL: usize = 0;

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

#[derive(Debug, Default, Clone)]
pub struct LivenessAnalysis {
    live_out: HashMap<usize, HashSet<Binding>>,
    /// Scope in effect at each statement, so a read can be resolved to the
    /// declaration it actually refers to.
    scope_at: HashMap<usize, ScopeMap>,
}

fn stmt_id(stmt: &Stmt) -> usize {
    std::ptr::from_ref(stmt) as usize
}

impl LivenessAnalysis {
    /// Analyze a callable body. Nothing is live on normal exit.
    pub fn for_body(body: &Block) -> Self {
        let mut analysis = Self::default();
        analysis.record_scopes_block(body, &mut ScopeMap::new());
        let exit = HashSet::new();
        analysis.block_live_in(body, &exit, None);
        analysis
    }

    /// Analyze a block that a lowering turns into a *loop body* — a lambda
    /// passed to `forEach`, for instance. The block's statements are nested in
    /// an expression, so the enclosing body's statement walk never reaches them,
    /// yet they still execute repeatedly and need the same back-edge treatment.
    ///
    /// `rebound` names (the lambda parameters) are fresh on each iteration, so
    /// reads of them are not carried across the back edge.
    pub fn for_loop_body(block: &Block, rebound: &[String], enclosing_live: &[String]) -> Self {
        let mut analysis = Self::default();
        analysis.record_scopes_block(block, &mut ScopeMap::new());
        let enclosing = enclosing_live
            .iter()
            .map(|name| Binding::new(name, OUTER_DECL))
            .collect::<HashSet<_>>();
        analysis.loop_fixpoint(&enclosing, |analysis, carried| {
            let mut live = analysis.block_live_in(block, carried, Some(carried));
            live.retain(|binding| !rebound.contains(&binding.name));
            live
        });
        analysis
    }

    /// Bindings that may be read after `stmt` completes.
    pub fn live_out_bindings_for(&self, stmt: &Stmt) -> Option<&HashSet<Binding>> {
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
    pub fn live_out_for(&self, stmt: &Stmt) -> Option<HashSet<String>> {
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
                then_block,
                else_block,
                ..
            } => {
                self.record_scopes_block(then_block, &mut scope.clone());
                if let Some(else_block) = else_block {
                    self.record_scopes_block(else_block, &mut scope.clone());
                }
            }
            Stmt::While { body, .. } | Stmt::DoWhile { body, .. } => {
                self.record_scopes_block(body, &mut scope.clone());
            }
            Stmt::For { init, body, .. } => {
                let mut inner = scope.clone();
                for stmt in init {
                    self.record_scopes_stmt(stmt, &mut inner);
                }
                self.record_scopes_block(body, &mut inner);
            }
            Stmt::EnhancedFor { var_name, body, .. } => {
                let mut inner = scope.clone();
                inner.insert(var_name.clone(), stmt_id(stmt));
                self.record_scopes_block(body, &mut inner);
            }
            Stmt::Switch { cases, .. } => {
                let mut inner = scope.clone();
                for case in cases {
                    for stmt in &case.statements {
                        self.record_scopes_stmt(stmt, &mut inner);
                    }
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
                    catch_scope.insert(catch.name.clone(), stmt_id(stmt));
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
        let mut names = HashSet::new();
        read_names(expr, &mut names);
        let scope = self.scope_at.get(&stmt_id(stmt));
        for name in names {
            let decl = scope
                .and_then(|scope| scope.get(&name).copied())
                .unwrap_or(OUTER_DECL);
            out.insert(Binding::new(&name, decl));
        }
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

    fn stmt_live_in(
        &mut self,
        stmt: &Stmt,
        live_out: &HashSet<Binding>,
        loop_live: Option<&HashSet<Binding>>,
    ) -> HashSet<Binding> {
        self.live_out.insert(stmt_id(stmt), live_out.clone());
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
                self.read_bindings(stmt, condition, &mut live);
                live
            }
            Stmt::While {
                condition, body, ..
            } => self.loop_fixpoint(live_out, |analysis, carried| {
                let mut live = analysis.block_live_in(body, carried, Some(carried));
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
                let carried = self.loop_fixpoint(live_out, |analysis, carried| {
                    let mut header = analysis.block_live_in(body, carried, Some(carried));
                    for expr in update {
                        analysis.read_bindings(stmt, expr, &mut header);
                    }
                    if let Some(condition) = condition {
                        analysis.read_bindings(stmt, condition, &mut header);
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
                // Cases may fall through, so union every case's live-in and the
                // fall-out of the switch itself.
                let mut live = live_out.clone();
                for case in cases {
                    let case_block = Block {
                        statements: case.statements.clone(),
                    };
                    let case_live = self.block_live_in(&case_block, live_out, loop_live);
                    live.extend(case_live);
                    // Record live-out for the real statement nodes too, since
                    // the temporary block above holds clones.
                    let mut trailing = live_out.clone();
                    for stmt in case.statements.iter().rev() {
                        trailing = self.stmt_live_in(stmt, &trailing, loop_live);
                    }
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
                let mut live = live_out.clone();
                if let Some(finally_block) = finally_block {
                    live = self.block_live_in(finally_block, &live, loop_live);
                }
                let after_body = live.clone();
                for catch in catches {
                    let mut catch_live = self.block_live_in(&catch.body, &after_body, loop_live);
                    catch_live.retain(|binding| binding.name != catch.name);
                    live.extend(catch_live);
                }
                live = self.block_live_in(body, &live, loop_live);
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

fn read_names(expr: &Expr, names: &mut HashSet<String>) {
    crate::lower::collect_expr_read_names_for_liveness(expr, names);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java_ir::{Block, Expr, Stmt};

    fn var(name: &str) -> Expr {
        Expr::Var(name.to_string())
    }

    fn decl(name: &str, init: Expr) -> Stmt {
        Stmt::VarDecl {
            name: name.to_string(),
            ty: crate::java_ir::JavaType::String,
            initializer: Some(init),
        }
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
}
