//! Additive, static representation complexity audit. Costs are policy units,
//! not timings. This deliberately parses emitted Rust: opaque IR/runtime bodies
//! must remain visible to the audit. No score establishes semantic admissibility.
use std::collections::{BTreeMap, HashMap, HashSet};
use tree_sitter::{Node, Parser};

pub const MODEL: &str = "representation-cost-v1";

#[derive(Clone, Debug)]
pub struct CostWeights(pub BTreeMap<String, u64>);

impl Default for CostWeights {
    fn default() -> Self {
        Self(
            [
                ("type.borrow", 1),
                ("type.box", 8),
                ("type.rc", 16),
                ("type.arc", 64),
                ("type.refcell", 12),
                ("type.mutex", 128),
                ("type.rwlock", 128),
                ("type.dynamic_dispatch", 16),
                ("op.box_allocate", 24),
                ("op.rc_allocate", 48),
                ("op.arc_allocate", 128),
                ("op.refcell_create", 8),
                ("op.mutex_create", 96),
                ("op.rwlock_create", 96),
                ("op.lock", 192),
                ("op.read_lock", 160),
                ("op.write_lock", 192),
                ("op.borrow", 2),
                ("op.borrow_mut", 4),
                ("op.rc_clone", 12),
                ("op.arc_clone", 48),
                ("op.box_clone", 64),
                ("op.value_clone", 16),
                ("op.unresolved_clone", 64),
                ("op.unresolved_lock", 192),
                ("op.downcast", 12),
                ("op.collect", 24),
                ("op.to_owned", 24),
                ("op.erased_conversion", 12),
                ("op.vec_allocate", 16),
                ("op.format_allocate", 16),
            ]
            .into_iter()
            .map(|(name, value)| (name.into(), value))
            .collect(),
        )
    }
}

impl CostWeights {
    /// Override known metrics with `metric=nonnegative_integer` lines.
    pub fn with_overrides(mut self, source: &str) -> Result<Self, String> {
        for (index, line) in source.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let (name, value) = line
                .split_once('=')
                .ok_or_else(|| format!("weight line {}: expected metric=value", index + 1))?;
            let slot = self
                .0
                .get_mut(name.trim())
                .ok_or_else(|| format!("unknown cost metric: {}", name.trim()))?;
            *slot = value
                .trim()
                .parse()
                .map_err(|_| format!("invalid weight for {}", name.trim()))?;
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Count {
    pub occurrences: u64,
    pub max_loop_depth: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CostSite {
    pub scope: String,
    pub metric: String,
    pub line: usize,
    pub loop_depth: usize,
}

#[derive(Clone, Debug, Default)]
pub struct CostReport {
    pub counts: BTreeMap<(String, String), Count>,
    pub sites: Vec<CostSite>,
    /// Coverage issues are separate from costs; zero weight cannot erase them.
    pub coverage: BTreeMap<String, u64>,
}

impl CostReport {
    fn add(&mut self, scope: &str, metric: &str, line: usize, depth: usize) {
        let count = self
            .counts
            .entry((scope.into(), metric.into()))
            .or_default();
        count.occurrences += 1;
        count.max_loop_depth = count.max_loop_depth.max(depth);
        self.sites.push(CostSite {
            scope: scope.into(),
            metric: metric.into(),
            line,
            loop_depth: depth,
        });
    }
    pub fn merge(&mut self, other: Self) {
        for (key, value) in other.counts {
            let count = self.counts.entry(key).or_default();
            count.occurrences += value.occurrences;
            count.max_loop_depth = count.max_loop_depth.max(value.max_loop_depth);
        }
        self.sites.extend(other.sites);
        for (key, value) in other.coverage {
            *self.coverage.entry(key).or_default() += value;
        }
    }
    pub fn metrics(&self) -> BTreeMap<String, u64> {
        let mut result = BTreeMap::new();
        for ((_, metric), count) in &self.counts {
            *result.entry(metric.clone()).or_default() += count.occurrences;
        }
        result
    }
    pub fn score(&self, weights: &CostWeights) -> u128 {
        self.counts
            .iter()
            .map(|((_, metric), count)| {
                u128::from(count.occurrences) * u128::from(*weights.0.get(metric).unwrap_or(&0))
            })
            .sum()
    }
    pub fn render(&self, weights: &CostWeights, baseline: Option<&Self>, sites: bool) -> String {
        let mut out = format!(
            "# model={MODEL}; units=static-policy; correctness=not-evaluated\n# Includes emitted helper definitions; no execution-frequency or reachability weighting.\n# Syntax audit with file-local name inference; no Rust type checking or macro expansion.\n# Loop depth is context only. Unknown clone/lock operations retain conservative costs.\n"
        );
        out.push_str(&format!("# total={}\n", self.score(weights)));
        if let Some(old) = baseline {
            out.push_str(&format!(
                "# baseline_total={}; delta={}\n",
                old.score(weights),
                self.score(weights) as i128 - old.score(weights) as i128
            ));
        }
        for (issue, count) in &self.coverage {
            out.push_str(&format!("# coverage.{issue}={count}\n"));
        }
        out.push_str(
            "scope\tmetric\tcount\tweight\tcost\tmax_loop_depth\tbaseline_count\tdelta_count\n",
        );
        let previous = baseline.map(Self::metrics).unwrap_or_default();
        let current = self.metrics();
        let all = current
            .keys()
            .chain(previous.keys())
            .collect::<std::collections::BTreeSet<_>>();
        for metric in all {
            let count = current.get(metric).copied().unwrap_or(0);
            let old = previous.get(metric).copied().unwrap_or(0);
            let weight = weights.0.get(metric).copied().unwrap_or(0);
            out.push_str(&format!(
                "TOTAL\t{metric}\t{count}\t{weight}\t{}\t0\t{old}\t{}\n",
                u128::from(count) * u128::from(weight),
                i128::from(count) - i128::from(old)
            ));
        }
        let mut rows = self.counts.iter().collect::<Vec<_>>();
        rows.sort_by_key(|((scope, metric), count)| {
            (
                std::cmp::Reverse(
                    u128::from(count.occurrences)
                        * u128::from(*weights.0.get(metric).unwrap_or(&0)),
                ),
                scope.as_str(),
                metric.as_str(),
            )
        });
        for ((scope, metric), count) in rows {
            let weight = weights.0.get(metric).copied().unwrap_or(0);
            let old = baseline
                .and_then(|report| report.counts.get(&(scope.clone(), metric.clone())))
                .map_or(0, |c| c.occurrences);
            out.push_str(&format!(
                "{scope}\t{metric}\t{}\t{weight}\t{}\t{}\t{old}\t{}\n",
                count.occurrences,
                u128::from(count.occurrences) * u128::from(weight),
                count.max_loop_depth,
                i128::from(count.occurrences) - i128::from(old)
            ));
        }
        if sites {
            out.push_str("# sites: scope\tmetric\tline\tloop_depth\n");
            for site in &self.sites {
                out.push_str(&format!(
                    "# site\t{}\t{}\t{}\t{}\n",
                    site.scope, site.metric, site.line, site.loop_depth
                ));
            }
        }
        out
    }
}

#[derive(Clone, Debug, Default)]
struct Ty {
    head: String,
    args: Vec<Ty>,
}
impl Ty {
    fn parse(node: Node<'_>, source: &str) -> Self {
        let text = |n: Node<'_>| source[n.byte_range()].to_string();
        match node.kind() {
            "generic_type" => Self {
                head: node
                    .child_by_field_name("type")
                    .map(text)
                    .unwrap_or_default(),
                args: node
                    .child_by_field_name("type_arguments")
                    .map(|a| {
                        children(a)
                            .into_iter()
                            .filter(|n| n.kind() != "lifetime")
                            .map(|n| Self::parse(n, source))
                            .collect()
                    })
                    .unwrap_or_default(),
            },
            "reference_type" => Self {
                head: "&".into(),
                args: node
                    .child_by_field_name("type")
                    .map(|n| vec![Self::parse(n, source)])
                    .unwrap_or_default(),
            },
            "dynamic_type" => Self {
                head: "dyn".into(),
                args: vec![],
            },
            "tuple_type" | "array_type" | "slice_type" | "function_type" => Self {
                head: node.kind().into(),
                args: children(node)
                    .into_iter()
                    .filter(|n| n.kind().ends_with("type") || n.kind().ends_with("identifier"))
                    .map(|n| Self::parse(n, source))
                    .collect(),
            },
            _ => Self {
                head: text(node),
                args: vec![],
            },
        }
    }
}
fn children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}
fn compact(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}
fn leaf(text: &str) -> &str {
    text.rsplit("::").next().unwrap_or(text)
}

#[derive(Clone, Default)]
struct Symbols {
    imports: HashMap<String, String>,
    aliases: HashMap<String, (Vec<String>, Ty)>,
    fields: HashMap<String, HashMap<String, Ty>>,
    wrappers: HashMap<String, (Vec<String>, Ty)>,
    declarations: HashSet<String>,
}
impl Symbols {
    fn parameters(node: Node<'_>, source: &str) -> Vec<String> {
        node.child_by_field_name("type_parameters")
            .map(|params| {
                children(params)
                    .into_iter()
                    .filter_map(|p| p.child_by_field_name("name"))
                    .map(|p| source[p.byte_range()].to_string())
                    .collect()
            })
            .unwrap_or_default()
    }
    fn substitute(ty: &Ty, parameters: &[String], args: &[Ty]) -> Ty {
        if let Some(index) = parameters.iter().position(|p| p == &ty.head) {
            if let Some(arg) = args.get(index) {
                return arg.clone();
            }
        }
        Ty {
            head: ty.head.clone(),
            args: ty
                .args
                .iter()
                .map(|t| Self::substitute(t, parameters, args))
                .collect(),
        }
    }

    fn collect(&mut self, node: Node<'_>, source: &str) {
        let text = |n: Node<'_>| &source[n.byte_range()];
        if node.kind() == "use_declaration" {
            if let Some(arg) = node.child_by_field_name("argument") {
                self.import(arg, "", source);
            }
        }
        if matches!(node.kind(), "struct_item" | "enum_item" | "type_item") {
            if let Some(name) = node.child_by_field_name("name") {
                self.declarations.insert(text(name).into());
            }
        }
        if node.kind() == "type_item" {
            if let (Some(name), Some(ty)) = (
                node.child_by_field_name("name"),
                node.child_by_field_name("type"),
            ) {
                self.aliases.insert(
                    text(name).into(),
                    (Self::parameters(node, source), Ty::parse(ty, source)),
                );
            }
        }
        if node.kind() == "struct_item" {
            if let (Some(name), Some(body)) = (
                node.child_by_field_name("name"),
                node.child_by_field_name("body"),
            ) {
                let mut fields = HashMap::new();
                let mut index = 0;
                for field in children(body) {
                    if field.kind() == "field_declaration" {
                        if let (Some(name), Some(ty)) = (
                            field.child_by_field_name("name"),
                            field.child_by_field_name("type"),
                        ) {
                            fields.insert(text(name).into(), Ty::parse(ty, source));
                        }
                    } else if field.kind().ends_with("type") || field.kind().ends_with("identifier")
                    {
                        fields.insert(index.to_string(), Ty::parse(field, source));
                        index += 1;
                    }
                }
                if fields.len() == 1 {
                    self.wrappers.insert(
                        text(name).into(),
                        (
                            Self::parameters(node, source),
                            fields.values().next().unwrap().clone(),
                        ),
                    );
                }
                self.fields.insert(text(name).into(), fields);
            }
        }
        for child in children(node) {
            self.collect(child, source);
        }
    }
    fn import(&mut self, node: Node<'_>, prefix: &str, source: &str) {
        let text = &source[node.byte_range()];
        if node.kind() == "scoped_use_list" {
            let path = node
                .child_by_field_name("path")
                .map(|n| &source[n.byte_range()])
                .unwrap_or("");
            if let Some(list) = node.child_by_field_name("list") {
                self.import(list, &format!("{prefix}{path}::"), source);
            }
        } else if node.kind() == "use_list" {
            for child in children(node) {
                self.import(child, prefix, source);
            }
        } else if node.kind() == "use_as_clause" {
            if let (Some(path), Some(alias)) = (
                node.child_by_field_name("path"),
                node.child_by_field_name("alias"),
            ) {
                self.imports.insert(
                    source[alias.byte_range()].into(),
                    format!("{prefix}{}", &source[path.byte_range()]),
                );
            }
        } else if node.kind() != "use_wildcard" {
            self.imports
                .insert(leaf(text).into(), format!("{prefix}{text}"));
        }
    }
    fn standard(&self, name: &str) -> Option<&'static str> {
        let name = name.trim_start_matches("::");
        let resolved = self.imports.get(name).map(String::as_str).unwrap_or(name);
        match resolved {
            "std::rc::Rc" | "alloc::rc::Rc" => Some("rc"),
            "std::sync::Arc" | "alloc::sync::Arc" => Some("arc"),
            "std::sync::Mutex" => Some("mutex"),
            "std::sync::RwLock" => Some("rwlock"),
            "std::cell::RefCell" | "core::cell::RefCell" => Some("refcell"),
            "std::boxed::Box" | "alloc::boxed::Box" => Some("box"),
            "Box" if !self.declarations.contains("Box") => Some("box"),
            _ => None,
        }
    }
    fn resolved(&self, ty: &Ty, seen: &mut HashSet<String>) -> Ty {
        if self.standard(&ty.head).is_some() {
            return ty.clone();
        }
        let key = leaf(&ty.head);
        if seen.insert(key.into()) {
            if let Some((parameters, alias)) = self.aliases.get(key) {
                return self.resolved(&Self::substitute(alias, parameters, &ty.args), seen);
            }
            if let Some((parameters, wrapper)) = self.wrappers.get(key) {
                let resolved =
                    self.resolved(&Self::substitute(wrapper, parameters, &ty.args), seen);
                if self.standard(&resolved.head).is_some() {
                    return resolved;
                }
            }
        }
        ty.clone()
    }
    fn receiver_kind(&self, ty: &Ty, method: &str) -> Option<&'static str> {
        let resolved = self.resolved(ty, &mut HashSet::new());
        let kind = self.kind(&resolved);
        if matches!(
            method,
            "lock"
                | "try_lock"
                | "read"
                | "try_read"
                | "write"
                | "try_write"
                | "borrow"
                | "borrow_mut"
                | "try_borrow"
                | "try_borrow_mut"
        ) && matches!(kind, Some("arc" | "rc" | "box"))
        {
            return resolved
                .args
                .first()
                .and_then(|t| self.receiver_kind(t, method));
        }
        kind
    }
    fn kind(&self, ty: &Ty) -> Option<&'static str> {
        if ty.head == "&" {
            return ty.args.first().and_then(|t| self.kind(t));
        }
        let resolved = self.resolved(ty, &mut HashSet::new());
        self.standard(&resolved.head)
    }
}

struct Audit<'a> {
    source: &'a str,
    symbols: Symbols,
    report: CostReport,
}
impl Audit<'_> {
    fn text(&self, node: Node<'_>) -> &str {
        &self.source[node.byte_range()]
    }
    fn types(&mut self, ty: &Ty, scope: &str, line: usize, seen: &mut HashSet<String>) {
        let resolved = self.symbols.resolved(ty, seen);
        let category = if resolved.head == "&" {
            Some("borrow")
        } else if resolved.head == "dyn" {
            Some("dynamic_dispatch")
        } else {
            self.symbols.standard(&resolved.head)
        };
        if let Some(kind) = category {
            self.report.add(scope, &format!("type.{kind}"), line, 0);
        }
        for arg in &resolved.args {
            self.types(arg, scope, line, &mut seen.clone());
        }
    }
    fn infer(&self, node: Node<'_>, env: &HashMap<String, Ty>, owner: &str) -> Ty {
        match node.kind() {
            "identifier" | "self" => env.get(self.text(node)).cloned().unwrap_or_default(),
            "reference_expression" | "parenthesized_expression" | "try_expression" => {
                children(node)
                    .last()
                    .map(|n| self.infer(*n, env, owner))
                    .unwrap_or_default()
            }
            "field_expression" => {
                let base = node
                    .child_by_field_name("value")
                    .map(|n| self.infer(n, env, owner))
                    .unwrap_or_default();
                let base = if base.head == "Self" {
                    owner
                } else {
                    leaf(&base.head)
                };
                node.child_by_field_name("field")
                    .and_then(|n| {
                        self.symbols
                            .fields
                            .get(base)
                            .and_then(|fields| fields.get(self.text(n)))
                    })
                    .cloned()
                    .unwrap_or_default()
            }
            "call_expression" => {
                let Some(mut function) = node.child_by_field_name("function") else {
                    return Ty::default();
                };
                if function.kind() == "generic_function" {
                    function = function.child_by_field_name("function").unwrap_or(function);
                }
                if function.kind() == "field_expression" {
                    let receiver = function
                        .child_by_field_name("value")
                        .map(|n| self.infer(n, env, owner))
                        .unwrap_or_default();
                    let method = function
                        .child_by_field_name("field")
                        .map(|n| self.text(n))
                        .unwrap_or("");
                    if method == "clone" {
                        return receiver;
                    }
                    if matches!(
                        method,
                        "borrow"
                            | "borrow_mut"
                            | "lock"
                            | "read"
                            | "write"
                            | "unwrap"
                            | "expect"
                            | "as_ref"
                    ) {
                        return receiver.args.first().cloned().unwrap_or(receiver);
                    }
                }
                let path = self.text(function);
                if let Some((base, method)) = path.rsplit_once("::") {
                    if matches!(method, "new" | "default" | "clone") {
                        let ty = Ty {
                            head: base.into(),
                            args: vec![],
                        };
                        if self.symbols.kind(&ty).is_some() {
                            return ty;
                        }
                    }
                }
                Ty::default()
            }
            _ => Ty::default(),
        }
    }
    fn call(
        &mut self,
        node: Node<'_>,
        scope: &str,
        env: &HashMap<String, Ty>,
        owner: &str,
        depth: usize,
    ) {
        let Some(mut fun) = node.child_by_field_name("function") else {
            return;
        };
        if fun.kind() == "generic_function" {
            fun = fun.child_by_field_name("function").unwrap_or(fun);
        }
        let mut kind = None;
        let method;
        if fun.kind() == "field_expression" {
            let receiver = fun
                .child_by_field_name("value")
                .map(|n| self.infer(n, env, owner))
                .unwrap_or_default();
            method = fun
                .child_by_field_name("field")
                .map(|n| self.text(n))
                .unwrap_or("")
                .to_string();
            kind = self.symbols.receiver_kind(&receiver, &method);
        } else {
            let path = compact(self.text(fun));
            method = path.rsplit("::").next().unwrap_or("").to_string();
            if let Some((base, _)) = path.rsplit_once("::") {
                // Qualified Clone::clone often spells its receiver type in
                // <Arc<_> as Clone>; consult its structured type node too.
                kind = self.symbols.standard(base);
                if kind.is_none() {
                    fn find_type(
                        node: Node<'_>,
                        source: &str,
                        symbols: &Symbols,
                    ) -> Option<&'static str> {
                        if node.kind() == "generic_type" {
                            return symbols.kind(&Ty::parse(node, source));
                        }
                        children(node)
                            .into_iter()
                            .find_map(|n| find_type(n, source, symbols))
                    }
                    kind = find_type(fun, self.source, &self.symbols);
                }
            }
            if method == "clone" && kind.is_none() {
                kind = node
                    .child_by_field_name("arguments")
                    .and_then(|n| children(n).first().copied())
                    .and_then(|n| self.symbols.kind(&self.infer(n, env, owner)));
            }
        }
        let metric = match (method.as_str(), kind) {
            ("new" | "new_cyclic" | "new_uninit", Some("box")) => Some("op.box_allocate"),
            ("new" | "new_cyclic" | "new_uninit", Some("rc")) => Some("op.rc_allocate"),
            ("new" | "new_cyclic" | "new_uninit", Some("arc")) => Some("op.arc_allocate"),
            ("new", Some("mutex")) => Some("op.mutex_create"),
            ("new", Some("rwlock")) => Some("op.rwlock_create"),
            ("new", Some("refcell")) => Some("op.refcell_create"),
            ("clone", Some("arc")) => Some("op.arc_clone"),
            ("clone", Some("rc")) => Some("op.rc_clone"),
            ("clone", Some("box")) => Some("op.box_clone"),
            ("clone", _) => Some("op.unresolved_clone"),
            ("lock" | "try_lock", Some("mutex")) => Some("op.lock"),
            ("lock" | "try_lock", _) => Some("op.unresolved_lock"),
            ("read" | "try_read", Some("rwlock")) => Some("op.read_lock"),
            ("write" | "try_write", Some("rwlock")) => Some("op.write_lock"),
            ("borrow" | "try_borrow", Some("refcell")) => Some("op.borrow"),
            ("borrow_mut" | "try_borrow_mut", Some("refcell")) => Some("op.borrow_mut"),
            (
                "downcast" | "downcast_ref" | "downcast_mut" | "downcast_clone"
                | "try_downcast_clone",
                _,
            ) => Some("op.downcast"),
            ("collect", _) => Some("op.collect"),
            ("to_owned" | "to_vec", _) => Some("op.to_owned"),
            (
                "java_retarget_erased_generic_value" | "java_retarget_erased_generic_value_result",
                _,
            ) => Some("op.erased_conversion"),
            _ => None,
        };
        if let Some(metric) = metric {
            self.report
                .add(scope, metric, node.start_position().row + 1, depth);
            if metric.contains("unresolved") {
                *self.report.coverage.entry(metric.into()).or_default() += 1;
            }
        }
    }
    fn walk(
        &mut self,
        node: Node<'_>,
        scope: &str,
        env: &mut HashMap<String, Ty>,
        owner: &str,
        depth: usize,
    ) {
        let line = node.start_position().row + 1;
        if node.is_error() || node.is_missing() {
            *self
                .report
                .coverage
                .entry("parse_errors".into())
                .or_default() += 1;
        }
        if matches!(
            node.kind(),
            "line_comment"
                | "block_comment"
                | "string_literal"
                | "raw_string_literal"
                | "char_literal"
        ) {
            return;
        }
        if node.kind() == "mod_item" {
            let name = node
                .child_by_field_name("name")
                .map(|n| self.text(n).to_string())
                .unwrap_or_else(|| "?".into());
            let nested = format!("{scope}::{name}");
            for child in children(node) {
                self.walk(child, &nested, &mut env.clone(), owner, depth);
            }
            return;
        }
        if node.kind() == "impl_item" {
            let name = node
                .child_by_field_name("type")
                .map(|n| compact(self.text(n)))
                .unwrap_or_default();
            let trait_name = node
                .child_by_field_name("trait")
                .map(|n| format!(" as {}", compact(self.text(n))))
                .unwrap_or_default();
            for child in children(node) {
                self.walk(
                    child,
                    &format!("{scope}::impl {name}{trait_name}"),
                    &mut env.clone(),
                    &name,
                    depth,
                );
            }
            return;
        }
        if matches!(node.kind(), "struct_item" | "enum_item" | "trait_item") {
            let name = node
                .child_by_field_name("name")
                .map(|n| self.text(n).to_string())
                .unwrap_or_else(|| "?".into());
            for child in children(node) {
                self.walk(
                    child,
                    &format!("{scope}::{name}"),
                    &mut env.clone(),
                    owner,
                    depth,
                );
            }
            return;
        }
        if node.kind() == "type_item" {
            *self
                .report
                .coverage
                .entry("type_alias_definitions".into())
                .or_default() += 1;
            return;
        }
        if node.kind() == "function_item" || node.kind() == "function_signature_item" {
            let name = node
                .child_by_field_name("name")
                .map(|n| self.text(n).to_string())
                .unwrap_or_else(|| "?".into());
            let nested = format!("{scope}::{name}");
            let mut local = env.clone();
            local.insert(
                "self".into(),
                Ty {
                    head: owner.split('<').next().unwrap_or(owner).into(),
                    args: vec![],
                },
            );
            if let Some(ty) = node.child_by_field_name("return_type") {
                self.types(
                    &Ty::parse(ty, self.source),
                    &nested,
                    line,
                    &mut HashSet::new(),
                );
            }
            if let Some(params) = node.child_by_field_name("parameters") {
                self.walk(params, &nested, &mut local, owner, 0);
            }
            if let Some(body) = node.child_by_field_name("body") {
                self.walk(body, &nested, &mut local, owner, 0);
            }
            return;
        }
        if matches!(
            node.kind(),
            "field_declaration" | "parameter" | "let_declaration" | "static_item" | "const_item"
        ) {
            if let Some(ty) = node.child_by_field_name("type") {
                self.types(
                    &Ty::parse(ty, self.source),
                    scope,
                    line,
                    &mut HashSet::new(),
                );
            }
            let pattern = node
                .child_by_field_name("pattern")
                .or_else(|| node.child_by_field_name("name"));
            if let Some(pattern) = pattern {
                if matches!(pattern.kind(), "identifier" | "self") {
                    let ty = node
                        .child_by_field_name("type")
                        .map(|n| Ty::parse(n, self.source))
                        .or_else(|| {
                            node.child_by_field_name("value")
                                .map(|n| self.infer(n, env, owner))
                        });
                    if let Some(ty) = ty {
                        env.insert(self.text(pattern).into(), ty);
                    }
                }
            }
        }
        if node.kind() == "ordered_field_declaration_list" {
            for child in children(node) {
                if child.kind().ends_with("type") || child.kind().ends_with("identifier") {
                    self.types(
                        &Ty::parse(child, self.source),
                        scope,
                        line,
                        &mut HashSet::new(),
                    );
                }
            }
            return;
        }
        if node.kind() == "call_expression" {
            self.call(node, scope, env, owner, depth);
        }
        if node.kind() == "macro_invocation" {
            let name = node
                .child_by_field_name("macro")
                .map(|n| leaf(self.text(n)))
                .unwrap_or("")
                .to_string();
            if name == "vec" {
                self.report.add(scope, "op.vec_allocate", line, depth);
            }
            if name == "format" {
                self.report.add(scope, "op.format_allocate", line, depth);
            }
            if name == "compile_error" {
                *self
                    .report
                    .coverage
                    .entry("compile_error_markers".into())
                    .or_default() += 1;
            }
            if matches!(
                name.as_str(),
                "vec"
                    | "format"
                    | "print"
                    | "println"
                    | "eprint"
                    | "eprintln"
                    | "write"
                    | "writeln"
                    | "panic"
                    | "assert"
                    | "assert_eq"
                    | "assert_ne"
                    | "dbg"
                    | "format_args"
            ) {
                if let Some(tokens) = children(node)
                    .into_iter()
                    .find(|n| n.kind() == "token_tree")
                {
                    let text = self.text(tokens);
                    let fragment = format!("const _: () = {{ [{}]; }};", &text[1..text.len() - 1]);
                    let mut parser = Parser::new();
                    if parser
                        .set_language(&tree_sitter_rust::LANGUAGE.into())
                        .is_ok()
                    {
                        if let Some(tree) = parser.parse(&fragment, None) {
                            if !tree.root_node().has_error() {
                                let mut nested = Audit {
                                    source: &fragment,
                                    symbols: self.symbols.clone(),
                                    report: CostReport::default(),
                                };
                                nested.walk(
                                    tree.root_node(),
                                    scope,
                                    &mut env.clone(),
                                    owner,
                                    depth,
                                );
                                for site in &mut nested.report.sites {
                                    site.line += line - 1;
                                }
                                self.report.merge(nested.report);
                                return;
                            }
                        }
                    }
                }
            } else if matches!(
                name.as_str(),
                "include_str" | "include_bytes" | "concat" | "env" | "option_env" | "stringify"
            ) {
                return;
            }
            // Other macro token trees need expansion knowledge. Keep this
            // visible; a score with coverage gaps cannot certify an improvement.
            *self
                .report
                .coverage
                .entry("opaque_macro_invocations".into())
                .or_default() += 1;
            return;
        }
        if node.kind() == "macro_definition" {
            *self
                .report
                .coverage
                .entry("opaque_macro_definitions".into())
                .or_default() += 1;
            return;
        }
        let depth = depth
            + usize::from(matches!(
                node.kind(),
                "for_expression" | "while_expression" | "loop_expression"
            ));
        let mut block_env = env.clone();
        let target = if node.kind() == "block" {
            &mut block_env
        } else {
            env
        };
        for child in children(node) {
            self.walk(child, scope, target, owner, depth);
        }
    }
}

/// Audit a final Rust file, including helper definitions and opaque IR leaves.
pub fn analyze_rust(source: &str) -> Result<CostReport, String> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .map_err(|e| e.to_string())?;
    let tree = parser.parse(source, None).ok_or("Rust parse cancelled")?;
    let mut audit = Audit {
        source,
        symbols: Symbols::default(),
        report: CostReport::default(),
    };
    audit.symbols.collect(tree.root_node(), source);
    audit.walk(tree.root_node(), "crate", &mut HashMap::new(), "", 0);
    Ok(audit.report)
}

/// Entry point for emission-stage callers while Raw IR remains in use.
pub fn analyze_ir(file: &crate::rust_ir::RustFile) -> Result<CostReport, String> {
    analyze_rust(&crate::emit::emit_file(file))
}

/// Analyze one Rust file or a source directory. Use an assembled crate's src
/// directory to avoid counting duplicated per-unit runtime definitions.
pub fn analyze_path(path: &std::path::Path) -> Result<CostReport, String> {
    fn collect(path: &std::path::Path, files: &mut Vec<std::path::PathBuf>) -> std::io::Result<()> {
        if path.is_dir() {
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                if entry.file_type()?.is_symlink() {
                    continue;
                }
                if matches!(entry.file_name().to_str(), Some("target" | ".git")) {
                    continue;
                }
                collect(&entry.path(), files)?;
            }
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path.into());
        }
        Ok(())
    }
    if path.to_str() == Some("-") {
        return analyze_rust(
            &std::io::read_to_string(std::io::stdin()).map_err(|e| e.to_string())?,
        );
    }
    if !path.is_dir() {
        return analyze_rust(
            &std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?,
        );
    }
    let mut files = Vec::new();
    collect(path, &mut files).map_err(|e| e.to_string())?;
    files.sort();
    if files.is_empty() {
        return Err(format!("no Rust source files in {}", path.display()));
    }
    let mut report = CostReport::default();
    for file in files {
        let source = std::fs::read_to_string(&file).map_err(|e| e.to_string())?;
        let mut current = analyze_rust(&source)?;
        let prefix = file
            .strip_prefix(path)
            .unwrap_or(&file)
            .display()
            .to_string();
        current.counts = current
            .counts
            .into_iter()
            .map(|((scope, metric), count)| ((format!("{prefix}::{scope}"), metric), count))
            .collect();
        for site in &mut current.sites {
            site.scope = format!("{prefix}::{}", site.scope);
        }
        report.merge(current);
    }
    Ok(report)
}
