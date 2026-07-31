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

#[derive(Debug, Default, Clone)]
pub struct LivenessAnalysis {
    live_out: HashMap<usize, HashSet<String>>,
}

fn stmt_id(stmt: &Stmt) -> usize {
    std::ptr::from_ref(stmt) as usize
}

impl LivenessAnalysis {
    /// Analyze a callable body. Nothing is live on normal exit.
    pub fn for_body(body: &Block) -> Self {
        let mut analysis = Self::default();
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
    pub fn for_loop_body(
        block: &Block,
        rebound: &[String],
        enclosing_live: &HashSet<String>,
    ) -> Self {
        let mut analysis = Self::default();
        analysis.loop_fixpoint(enclosing_live, |analysis, carried| {
            let mut live = analysis.block_live_in(block, carried, Some(carried));
            for name in rebound {
                live.remove(name);
            }
            live
        });
        analysis
    }

    /// Names that may be read after `stmt` completes.
    ///
    /// `None` means the statement was not part of the analyzed body (for
    /// example a synthesized statement), and the caller should fall back.
    pub fn live_out_for(&self, stmt: &Stmt) -> Option<&HashSet<String>> {
        self.live_out.get(&stmt_id(stmt))
    }

    fn block_live_in(
        &mut self,
        block: &Block,
        live_out: &HashSet<String>,
        loop_live: Option<&HashSet<String>>,
    ) -> HashSet<String> {
        let mut live = live_out.clone();
        for stmt in block.statements.iter().rev() {
            live = self.stmt_live_in(stmt, &live, loop_live);
        }
        live
    }

    fn stmt_live_in(
        &mut self,
        stmt: &Stmt,
        live_out: &HashSet<String>,
        loop_live: Option<&HashSet<String>>,
    ) -> HashSet<String> {
        self.live_out.insert(stmt_id(stmt), live_out.clone());
        match stmt {
            Stmt::VarDecl {
                name, initializer, ..
            } => {
                let mut live = live_out.clone();
                live.remove(name);
                if let Some(initializer) = initializer {
                    read_names(initializer, &mut live);
                }
                live
            }
            Stmt::Assign { target, value } => {
                let mut live = live_out.clone();
                read_names(target, &mut live);
                read_names(value, &mut live);
                live
            }
            Stmt::Expr(expr) => {
                let mut live = live_out.clone();
                read_names(expr, &mut live);
                live
            }
            // Nothing in this callable runs after a return or throw.
            Stmt::Return(expr) => {
                let mut live = HashSet::new();
                if let Some(expr) = expr {
                    read_names(expr, &mut live);
                }
                live
            }
            Stmt::Throw(expr) => {
                let mut live = HashSet::new();
                read_names(expr, &mut live);
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
                read_names(condition, &mut live);
                live
            }
            Stmt::While {
                condition, body, ..
            } => self.loop_fixpoint(live_out, |analysis, carried| {
                let mut live = analysis.block_live_in(body, carried, Some(carried));
                read_names(condition, &mut live);
                live
            }),
            Stmt::DoWhile {
                condition, body, ..
            } => self.loop_fixpoint(live_out, |analysis, carried| {
                let mut header = carried.clone();
                read_names(condition, &mut header);
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
                        read_names(expr, &mut header);
                    }
                    if let Some(condition) = condition {
                        read_names(condition, &mut header);
                    }
                    header
                });
                let mut live = carried;
                for stmt in init.iter().rev() {
                    live = self.stmt_live_in(stmt, &live, loop_live);
                }
                live
            }
            Stmt::EnhancedFor {
                var_name,
                iterable,
                body,
                ..
            } => {
                let mut live = self.loop_fixpoint(live_out, |analysis, carried| {
                    let mut header = analysis.block_live_in(body, carried, Some(carried));
                    // The loop variable is rebound each iteration, so a read of
                    // it inside the body is not carried across the back edge.
                    header.remove(var_name);
                    header
                });
                live.remove(var_name);
                read_names(iterable, &mut live);
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
                read_names(selector, &mut live);
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
                    catch_live.remove(&catch.name);
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
    fn loop_fixpoint<F>(&mut self, live_out: &HashSet<String>, mut body: F) -> HashSet<String>
    where
        F: FnMut(&mut Self, &HashSet<String>) -> HashSet<String>,
    {
        let mut carried = live_out.clone();
        loop {
            let mut next = body(self, &carried);
            next.extend(live_out.iter().cloned());
            if next.len() == carried.len() && next.iter().all(|name| carried.contains(name)) {
                return next;
            }
            carried = next;
        }
    }
}

fn read_names(expr: &Expr, names: &mut HashSet<String>) {
    crate::lower::collect_expr_read_names_for_liveness(expr, names);
}
