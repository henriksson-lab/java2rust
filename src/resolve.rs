use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::binding_id::{BindingId, BindingKind};
use crate::diagnostics::DiagnosticBag;
use crate::facts::{
    Capture, CaptureFacts, CaptureOrigin, CaptureOwner, CaptureOwnerKind, CaptureSource,
    ClosedHierarchyFact, ExternalFactLookupIndex, ExternalFieldFact, ExternalFieldKind,
    FactProvenance, FactSet, FieldKey, MethodEffectFact, MethodKey, NodeId, NullabilityState,
    ObjectSafetyFact, ObjectSafetyViolation, ReceiverMutabilityFact, SealedPermitsFact,
    TypeCapabilityOverride, UnresolvedTargetFact, UnresolvedTargetKind,
};
use crate::java_ir::{
    Block, CallKind, ConstructorDecl, ConstructorInvocationKind, Expr, JavaCoercionEvidence,
    JavaCoercionKind, JavaType, LambdaBody, Member, MethodDecl, Modifier, Param, PrimitiveType,
    Program, ResolvedCall, ResolvedConstructor, Stmt, SwitchLabel, TypeDecl, TypeKind,
    WildcardBound,
};
use crate::java_signature::JavaOwnerKey;
use crate::types::{TypeEnv, infer_expr_type};

#[derive(Debug, Clone)]
pub struct ResolvedProgram {
    pub program: Program,
    pub diagnostics: DiagnosticBag,
    pub facts: FactSet,
}

#[derive(Debug, Clone)]
pub struct SourceSymbols {
    symbols: ProgramSymbols,
}

impl SourceSymbols {
    /// The source SAM's declaration identity and specialized Java signature.
    /// Semantic consumers must not replace a co-translated interface with a
    /// similarly named native functional facade.
    pub(crate) fn source_functional_interface_call(&self, ty: &JavaType) -> Option<ResolvedCall> {
        let method = self.symbols.source_functional_interface_method(ty)?;
        let substitutions = functional_interface_type_substitutions(ty, &self.symbols);
        Some(ResolvedCall {
            declaring_type: method.declaring_type.clone(),
            rust_name: method.rust_name.clone(),
            kind: CallKind::Instance,
            type_args: Vec::new(),
            return_type: substitute_method_type_variables(&method.return_type, &substitutions),
            arg_target_types: method
                .params
                .iter()
                .map(|param| substitute_method_type_variables(&param.ty, &substitutions))
                .collect(),
            arg_coercions: Vec::new(),
        })
    }

    pub(crate) fn source_type_name<'a>(&'a self, type_name: &'a str) -> &'a str {
        self.symbols.source_type_name(type_name)
    }

    /// Prove the receiver of an unqualified instance call in an anonymous
    /// body. A scoped implementation/helper or inherited member binds before
    /// an enclosing instance. Share this proof between captures and emission.
    pub(crate) fn anonymous_enclosing_call_receiver(
        &self,
        expr: &Expr,
        base: &JavaType,
        methods: &[&MethodDecl],
        enclosing: &[JavaType],
        env: &TypeEnv,
        facts: &FactSet,
    ) -> Option<JavaType> {
        let Expr::Call {
            receiver: None,
            name,
            args,
            resolved: Some(call),
            ..
        } = expr
        else {
            return None;
        };
        if call.kind != CallKind::Instance
            || crate::stdlib::is_known_object_inherited_method(name, args.len())
        {
            return None;
        }
        let arg_types = args
            .iter()
            .map(|arg| infer_expr_type(arg, env))
            .collect::<Vec<_>>();
        let base_name = java_type_simple_name(base);
        if select_scoped_instance_method(methods, name, &arg_types, &base_name, self, env, facts)
            .is_some()
        {
            return None;
        }
        if resolve_same_class_call(
            name,
            &arg_types,
            &self.symbols.methods_for_hierarchy(&base_name),
            &self.symbols,
            facts,
            Some(base),
            env,
        )
        .ok()
        .flatten()
        .is_some()
        {
            return None;
        }
        enclosing
            .iter()
            .rev()
            .find(|outer| {
                self.symbols
                    .source_subtype_distance(
                        &java_type_simple_name(outer),
                        &call.declaring_type,
                        facts,
                    )
                    .is_some()
            })
            .cloned()
    }

    pub(crate) fn source_field_identity(
        &self,
        owner: &str,
        name: &str,
    ) -> Option<(String, String, JavaType)> {
        let canonical = crate::parser::to_snake_case(name);
        self.symbols
            .fields_for_hierarchy(owner)
            .into_iter()
            .find(|field| field_name_matches(field, name, &canonical))
            .map(|field| (field.declaring_type, field.java_name, field.ty))
    }

    pub(crate) fn source_field_identity_for_receiver(
        &self,
        receiver: &JavaType,
        name: &str,
    ) -> Option<(String, String, JavaType)> {
        let receiver =
            crate::types::optional_element_type(receiver).unwrap_or_else(|| receiver.clone());
        let owner = java_type_simple_name(&receiver);
        let field = self.symbols.field_for_from(&owner, name, &owner)?;
        Some((
            field.declaring_type.clone(),
            field.java_name.clone(),
            self.symbols.field_type_for_receiver(field, &receiver),
        ))
    }

    pub(crate) fn source_static_field_names(
        &self,
        owner: &str,
    ) -> std::collections::HashSet<String> {
        self.symbols
            .fields_for_hierarchy(owner)
            .into_iter()
            .filter(|field| field.is_static)
            .flat_map(|field| {
                let rust_name = crate::parser::to_snake_case(&field.java_name);
                [field.java_name, rust_name]
            })
            .collect()
    }
}

/// Immutable external facts plus the lookup index shared by one resolver run.
///
/// Building `ExternalFactLookupIndex` is proportional to the complete external
/// surface. Rebuilding it for every nested expression made resolution
/// effectively quadratic for large dependency packs such as HTSJDK.
struct ResolverExternalFacts<'a> {
    facts: &'a FactSet,
    lookup: ExternalFactLookupIndex<'a>,
    static_member_imports: HashMap<String, Option<String>>,
}

impl<'a> ResolverExternalFacts<'a> {
    fn new(facts: &'a FactSet) -> Self {
        Self {
            facts,
            lookup: facts.external_lookup_index(),
            static_member_imports: HashMap::new(),
        }
    }

    fn with_imports(mut self, program: &Program) -> Self {
        for import in &program.imports {
            if !import.is_static || import.is_wildcard {
                continue;
            }
            let Some((owner, member)) = import.path.rsplit_once('.') else {
                continue;
            };
            self.static_member_imports
                .entry(member.to_string())
                .and_modify(|existing| {
                    if existing.as_deref() != Some(owner) {
                        *existing = None;
                    }
                })
                .or_insert_with(|| Some(owner.to_string()));
        }
        self
    }
}

impl std::ops::Deref for ResolverExternalFacts<'_> {
    type Target = FactSet;

    fn deref(&self) -> &Self::Target {
        self.facts
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverloadApplicabilityMode {
    FixedArity,
    VariableArity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverloadArgumentConversion {
    Exact,
    PrimitiveWidening,
    Boxing,
    Unboxing,
    BoxingWidening,
    ReferenceWidening,
    GenericInference,
    NullToReference,
    LambdaToFunctionalInterface,
    MethodReferenceToFunctionalInterface,
    UnknownFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverloadRejectionReason {
    ArityMismatch {
        expected: usize,
        actual: usize,
        varargs: bool,
    },
    MissingVarargsArrayParam,
    ArgumentTypeMismatch {
        arg_index: usize,
        actual: JavaType,
        target: JavaType,
    },
    NullToPrimitive {
        arg_index: usize,
        target: JavaType,
    },
    LambdaTargetNotFunctionalInterface {
        arg_index: usize,
        target: JavaType,
    },
    MethodReferenceTargetNotFunctionalInterface {
        arg_index: usize,
        target: JavaType,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverloadArgumentApplicability {
    arg_index: usize,
    actual: JavaType,
    target: JavaType,
    conversion: OverloadArgumentConversion,
    score: u32,
}

impl OverloadArgumentApplicability {
    pub fn arg_index(&self) -> usize {
        self.arg_index
    }

    pub fn actual(&self) -> &JavaType {
        &self.actual
    }

    pub fn target(&self) -> &JavaType {
        &self.target
    }

    pub fn conversion(&self) -> &OverloadArgumentConversion {
        &self.conversion
    }

    pub fn score(&self) -> u32 {
        self.score
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverloadApplicabilityReport {
    mode: Option<OverloadApplicabilityMode>,
    score: Option<u32>,
    arguments: Vec<OverloadArgumentApplicability>,
    rejections: Vec<OverloadRejectionReason>,
}

impl OverloadApplicabilityReport {
    pub fn mode(&self) -> Option<OverloadApplicabilityMode> {
        self.mode
    }

    pub fn score(&self) -> Option<u32> {
        self.score
    }

    pub fn arguments(&self) -> &[OverloadArgumentApplicability] {
        &self.arguments
    }

    pub fn rejections(&self) -> &[OverloadRejectionReason] {
        &self.rejections
    }

    pub fn is_applicable(&self) -> bool {
        self.score.is_some() && self.rejections.is_empty()
    }
}

pub fn resolve_program(program: &Program, diagnostics: DiagnosticBag) -> ResolvedProgram {
    resolve_program_with_source_program(program, diagnostics, program)
}

pub fn resolve_program_with_source_program(
    program: &Program,
    diagnostics: DiagnosticBag,
    source_program: &Program,
) -> ResolvedProgram {
    let source_symbols = source_symbols_from_program(source_program);
    resolve_program_with_source_symbols(program, diagnostics, &source_symbols)
}

pub fn resolve_program_with_source_program_and_facts(
    program: &Program,
    diagnostics: DiagnosticBag,
    source_program: &Program,
    external_facts: &FactSet,
) -> ResolvedProgram {
    let source_symbols = source_symbols_from_program(source_program);
    resolve_program_with_source_symbols_and_facts(
        program,
        diagnostics,
        &source_symbols,
        external_facts,
    )
}

pub fn source_symbols_from_program(source_program: &Program) -> SourceSymbols {
    let mut symbol_program = source_program.clone();
    crate::class_initialization::materialize(&mut symbol_program);
    let mut diagnostics = DiagnosticBag::new();
    qualify_nested_type_names(&mut symbol_program);
    fn has_number_parent(ty: &TypeDecl) -> bool {
        matches!(ty.extends.as_deref(), Some("Number" | "java.lang.Number"))
            || ty
                .members
                .iter()
                .any(|member| matches!(member, Member::Type(nested) if has_number_parent(nested)))
    }
    if symbol_program.types.iter().any(has_number_parent) {
        let original_symbols = ProgramSymbols::from_program(&symbol_program);
        materialize_platform_number_defaults(&mut symbol_program, &original_symbols);
    }
    let mut program_counts = ProgramMethodCounts::new();
    for ty in &symbol_program.types {
        collect_program_method_counts(ty, &mut program_counts);
    }
    for ty in &mut symbol_program.types {
        assign_type_rust_names(ty, &mut diagnostics, &program_counts);
    }
    SourceSymbols {
        symbols: ProgramSymbols::from_program(&symbol_program),
    }
}

/// Give every method in `program` its overload-disambiguated Rust name.
///
/// The primary program gets this during resolution; **context and summary
/// programs did not**, so a `MethodDecl` reaching lowering from one of them had
/// `rust_name: None` and `source_method_rust_name` fell back to the bare
/// snake-cased Java name. Two overloads then became one Rust method:
///
/// ```text
/// Util::pick_int(1), Util::pick_string("ab")   // static path:   resolved
/// u.inst(1),         u.inst("ab")              // instance path: collapsed
/// ```
///
/// The static path escaped because it re-solves signatures on an isolated
/// one-type program, which does name them. Two answers to one question, which is
/// what [0009](../claude_audits/done/0009-overload-identity-collapses-onto-rust-name.md)
/// is about — and the naming pass was already correct, it was simply not applied
/// here.
///
/// Deliberately does **not** call `qualify_nested_type_names`: these programs
/// are looked up by the names they already carry, and requalifying them would
/// change lookups rather than names.
pub fn assign_rust_names_to_context_program(program: &mut Program) {
    let mut diagnostics = DiagnosticBag::new();
    let mut program_counts = ProgramMethodCounts::new();
    for ty in &program.types {
        collect_program_method_counts(ty, &mut program_counts);
    }
    for ty in &mut program.types {
        assign_type_rust_names(ty, &mut diagnostics, &program_counts);
    }
}

pub fn resolve_program_with_source_symbols(
    program: &Program,
    diagnostics: DiagnosticBag,
    source_symbols: &SourceSymbols,
) -> ResolvedProgram {
    resolve_program_with_source_symbols_and_facts(
        program,
        diagnostics,
        source_symbols,
        &FactSet::default(),
    )
}

pub fn resolve_program_with_source_symbols_and_facts(
    program: &Program,
    diagnostics: DiagnosticBag,
    source_symbols: &SourceSymbols,
    external_facts: &FactSet,
) -> ResolvedProgram {
    let mut diagnostics = diagnostics;
    let mut program = program.clone();
    crate::class_initialization::materialize(&mut program);
    qualify_explicitly_imported_nested_type_names(&mut program);
    qualify_nested_type_names(&mut program);
    qualify_imported_jdk_map_entry_implements(&mut program);
    let symbols = &source_symbols.symbols;
    qualify_inherited_nested_type_names(&mut program, symbols);
    materialize_platform_number_defaults(&mut program, symbols);
    assign_local_binding_ids(&mut program);
    let mut program_counts = ProgramMethodCounts::new();
    for ty in &program.types {
        collect_program_method_counts(ty, &mut program_counts);
    }
    for ty in &mut program.types {
        assign_type_rust_names_with_source_symbols(ty, &mut diagnostics, &program_counts, symbols);
    }
    let external_facts = ResolverExternalFacts::new(external_facts).with_imports(&program);
    let mut resolver_facts = FactSet::default();
    for ty in &mut program.types {
        resolve_type_calls(
            ty,
            symbols,
            &mut diagnostics,
            &mut resolver_facts,
            &external_facts,
        );
    }
    let mut facts = collect_resolved_facts(&program, source_symbols, external_facts.facts);
    facts.extend_from(resolver_facts);
    ResolvedProgram {
        program,
        diagnostics,
        facts,
    }
}

/// Model concrete platform defaults before binding/effect/storage analysis.
/// Keeping delegation in ordinary Java IR lets the existing virtual-call,
/// receiver-mode, exception and narrowing machinery own its complete contract.
fn materialize_platform_number_defaults(program: &mut Program, symbols: &ProgramSymbols) {
    let imported_other_number = program.imports.iter().any(|import| {
        !import.is_static
            && !import.is_wildcard
            && import.path.rsplit('.').next() == Some("Number")
            && import.path != "java.lang.Number"
    });
    fn visit(ty: &mut TypeDecl, symbols: &ProgramSymbols, imported_other_number: bool) {
        for member in &mut ty.members {
            if let Member::Type(nested) = member {
                visit(nested, symbols, imported_other_number);
            }
        }
        if ty.kind != TypeKind::Class {
            return;
        }
        let Some(parent) = ty.extends.as_deref() else {
            return;
        };
        let parent = crate::parser::parse_hierarchy_type(parent);
        let parent = symbols.member_type_in_owner(&parent, &ty.name, &[]);
        let JavaType::Named { name, args } = parent else {
            return;
        };
        if !args.is_empty()
            || !matches!(name.as_str(), "Number" | "java.lang.Number")
            || (name == "Number" && imported_other_number)
            || symbols.kind_by_type.contains_key(&name)
        {
            return;
        }
        let has_method = |ty: &TypeDecl, name: &str| {
            ty.members.iter().any(|member| {
            matches!(member, Member::Method(method) if method.name == name && method.params.is_empty())
        })
        };
        for (name, primitive) in [
            ("byteValue", PrimitiveType::Byte),
            ("shortValue", PrimitiveType::Short),
        ] {
            if has_method(ty, name) {
                continue;
            }
            ty.members.push(Member::Method(MethodDecl {
                name: name.into(),
                rust_name: None,
                type_params: Vec::new(),
                type_param_bounds: Vec::new(),
                return_type: JavaType::Primitive(primitive),
                throws: Vec::new(),
                params: Vec::new(),
                modifiers: vec![Modifier::Public],
                body: Some(Block {
                    statements: vec![Stmt::Return(Some(Expr::Cast {
                        ty: JavaType::Primitive(primitive),
                        expr: Box::new(Expr::Call {
                            receiver: Some(Box::new(Expr::This)),
                            name: "intValue".into(),
                            type_args: Vec::new(),
                            args: Vec::new(),
                            resolved: None,
                        }),
                    }))],
                }),
                span: ty.span,
            }));
        }
        if ty.modifiers.contains(&Modifier::Abstract) {
            // A Java abstract subclass may inherit these abstract declarations
            // without spelling them itself. They are contracts, never bodies.
            for (name, primitive) in [
                ("intValue", PrimitiveType::Int),
                ("longValue", PrimitiveType::Long),
                ("floatValue", PrimitiveType::Float),
                ("doubleValue", PrimitiveType::Double),
            ] {
                if has_method(ty, name) {
                    continue;
                }
                ty.members.push(Member::Method(MethodDecl {
                    name: name.into(),
                    rust_name: None,
                    type_params: Vec::new(),
                    type_param_bounds: Vec::new(),
                    return_type: JavaType::Primitive(primitive),
                    throws: Vec::new(),
                    params: Vec::new(),
                    modifiers: vec![Modifier::Public, Modifier::Abstract],
                    body: None,
                    span: ty.span,
                }));
            }
        }
    }
    for ty in &mut program.types {
        visit(ty, symbols, imported_other_number);
    }
}

/// Resolve an unqualified nested type inherited from a superclass.
///
/// Java makes static member types available through subclasses, so a method
/// in `Child extends Base` may use `Mode` when the declaration is
/// `Base.Mode`. Lexical nested-name qualification cannot see that relation;
/// the hierarchy index can. Qualifying it before signature planning prevents
/// each generated child module from synthesizing an unrelated external
/// `Mode` facade.
fn qualify_inherited_nested_type_names(program: &mut Program, symbols: &ProgramSymbols) {
    fn qualify_type(ty: &mut TypeDecl, symbols: &ProgramSymbols) {
        let mut candidates = HashMap::<String, Vec<String>>::new();
        for field in symbols.fields_for_hierarchy(&ty.name) {
            if !field.is_static {
                continue;
            }
            let JavaType::Named { name, args } = &field.ty else {
                continue;
            };
            if !args.is_empty() {
                continue;
            }
            let canonical = symbols.source_type_name(name);
            if !symbols.outer_by_type.contains_key(canonical) {
                continue;
            }
            let simple = canonical.rsplit('.').next().unwrap_or(canonical);
            candidates
                .entry(simple.to_string())
                .or_default()
                .push(canonical.to_string());
        }
        let aliases = candidates
            .into_iter()
            .filter_map(|(simple, mut names)| {
                names.sort();
                names.dedup();
                (names.len() == 1).then(|| (simple, names.remove(0)))
            })
            .collect::<HashMap<_, _>>();
        if !aliases.is_empty() {
            rewrite_type_decl_references(ty, &aliases, &HashMap::new());
        }
        for member in &mut ty.members {
            if let Member::Type(nested) = member {
                qualify_type(nested, symbols);
            }
        }
    }

    for ty in &mut program.types {
        qualify_type(ty, symbols);
    }
}

/// Qualify the simple name introduced by an explicit import of a nested Java
/// type (`import p.Owner.Item`) to the source identity `Owner.Item`.
///
/// Ordinary top-level imports intentionally retain their simple spelling, but
/// a nested type needs its owner to select the shared flattened Rust identity
/// (`Owner_Item`).  Leaving it as `Item` causes external-facade synthesis and
/// makes different translation units invent a second Rust type for the same
/// Java declaration.
fn qualify_explicitly_imported_nested_type_names(program: &mut Program) {
    let mut candidates = HashMap::<String, Vec<String>>::new();
    for import in program
        .imports
        .iter()
        .filter(|import| !import.is_static && !import.is_wildcard)
    {
        let segments = import.path.split('.').collect::<Vec<_>>();
        let Some(first_type) = segments
            .iter()
            .position(|segment| segment.chars().next().is_some_and(char::is_uppercase))
        else {
            continue;
        };
        if first_type + 1 >= segments.len() {
            continue;
        }
        let simple = segments.last().expect("non-empty import path").to_string();
        candidates
            .entry(simple)
            .or_default()
            .push(segments[first_type..].join("."));
    }
    let aliases = candidates
        .into_iter()
        .filter_map(|(simple, mut candidates)| {
            candidates.sort();
            candidates.dedup();
            (candidates.len() == 1).then(|| (simple, candidates.remove(0)))
        })
        .collect::<HashMap<_, _>>();
    if aliases.is_empty() {
        return;
    }
    let captured = HashMap::new();
    for ty in &mut program.types {
        rewrite_type_decl_references(ty, &aliases, &captured);
    }
}

fn qualify_imported_jdk_map_entry_implements(program: &mut Program) {
    let imports_map = program
        .imports
        .iter()
        .any(|import| !import.is_static && import.path == "java.util.Map");
    let imports_entry = program
        .imports
        .iter()
        .any(|import| !import.is_static && import.path == "java.util.Map.Entry");
    if !imports_map && !imports_entry {
        return;
    }
    fn source_declares_simple_name(ty: &TypeDecl, simple: &str) -> bool {
        ty.name.rsplit('.').next() == Some(simple)
            || ty.members.iter().any(|member| match member {
                Member::Type(nested) => source_declares_simple_name(nested, simple),
                _ => false,
            })
    }
    let source_declares_map = program
        .types
        .iter()
        .any(|ty| source_declares_simple_name(ty, "Map"));
    let source_declares_entry = program
        .types
        .iter()
        .any(|ty| source_declares_simple_name(ty, "Entry"));
    fn qualify_type(ty: &mut TypeDecl, map: bool, entry: bool) {
        for implemented in &mut ty.implements {
            let head = implemented.split('<').next().unwrap_or(implemented).trim();
            let qualifies = (map && head == "Map.Entry") || (entry && head == "Entry");
            if qualifies {
                *implemented = implemented.replacen(head, "java.util.Map.Entry", 1);
            }
        }
        for member in &mut ty.members {
            if let Member::Type(nested) = member {
                qualify_type(nested, map, entry);
            }
        }
    }
    for ty in &mut program.types {
        qualify_type(
            ty,
            imports_map && !source_declares_map,
            imports_entry && !source_declares_entry,
        );
    }
}

pub fn declaration_facts_from_program(program: &Program) -> FactSet {
    let mut program = program.clone();
    qualify_nested_type_names(&mut program);
    let mut facts = FactSet::default();
    collect_type_capability_facts(&program, &mut facts);
    collect_generic_parameter_variance_facts(&program, &mut facts);
    collect_field_declaration_facts(&program, &mut facts);
    collect_static_final_constant_value_facts(&program, &mut facts);
    collect_method_declaration_effect_facts(&program, &mut facts);
    facts
}

fn assign_local_binding_ids(program: &mut Program) {
    for ty in &mut program.types {
        assign_local_binding_ids_for_type(ty);
    }
}

fn assign_local_binding_ids_for_type(ty: &mut TypeDecl) {
    for member in &mut ty.members {
        match member {
            Member::Method(method) => {
                let mut occurrences = HashMap::new();
                for param in &mut method.params {
                    param.binding = Some(next_binding_id(
                        BindingKind::Param,
                        &param.source_name,
                        &mut occurrences,
                    ));
                }
                if let Some(body) = &mut method.body {
                    assign_local_binding_ids_for_block(body, &mut occurrences);
                }
            }
            Member::Constructor(constructor) => {
                let mut occurrences = HashMap::new();
                for param in &mut constructor.params {
                    param.binding = Some(next_binding_id(
                        BindingKind::Param,
                        &param.source_name,
                        &mut occurrences,
                    ));
                }
                if let Some(body) = &mut constructor.body {
                    assign_local_binding_ids_for_block(body, &mut occurrences);
                }
            }
            Member::Type(nested) => assign_local_binding_ids_for_type(nested),
            _ => {}
        }
    }
}

fn assign_local_binding_ids_for_block(
    block: &mut Block,
    occurrences: &mut HashMap<(BindingKind, String), usize>,
) {
    for stmt in &mut block.statements {
        assign_local_binding_ids_for_stmt(stmt, occurrences);
    }
}

fn assign_local_binding_ids_for_stmt(
    stmt: &mut Stmt,
    occurrences: &mut HashMap<(BindingKind, String), usize>,
) {
    match stmt {
        Stmt::VarDecl {
            source_name,
            binding,
            ..
        } => {
            *binding = Some(next_binding_id(
                BindingKind::Local,
                source_name,
                occurrences,
            ));
        }
        Stmt::For { init, body, .. } => {
            for init_stmt in init {
                assign_local_binding_ids_for_stmt(init_stmt, occurrences);
            }
            assign_local_binding_ids_for_block(body, occurrences);
        }
        Stmt::EnhancedFor {
            source_var_name,
            body,
            binding,
            ..
        } => {
            *binding = Some(next_binding_id(
                BindingKind::LoopVariable,
                source_var_name,
                occurrences,
            ));
            assign_local_binding_ids_for_block(body, occurrences);
        }
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            assign_local_binding_ids_for_block(then_block, occurrences);
            if let Some(else_block) = else_block {
                assign_local_binding_ids_for_block(else_block, occurrences);
            }
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::Block(body)
        | Stmt::LabeledBlock { body, .. } => {
            assign_local_binding_ids_for_block(body, occurrences);
        }
        Stmt::Switch { cases, .. } => {
            for case in cases {
                for stmt in &mut case.statements {
                    assign_local_binding_ids_for_stmt(stmt, occurrences);
                }
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            for resource in resources {
                assign_local_binding_ids_for_stmt(resource, occurrences);
            }
            assign_local_binding_ids_for_block(body, occurrences);
            for catch in catches {
                catch.binding = Some(next_binding_id(
                    BindingKind::CatchParam,
                    &catch.source_name,
                    occurrences,
                ));
                assign_local_binding_ids_for_block(&mut catch.body, occurrences);
            }
            if let Some(finally_block) = finally_block {
                assign_local_binding_ids_for_block(finally_block, occurrences);
            }
        }
        Stmt::LocalType(local) => assign_local_binding_ids_for_type(local),
        Stmt::Assign { .. }
        | Stmt::Expr(_)
        | Stmt::Return(_)
        | Stmt::Throw(_)
        | Stmt::ConstructorInvocation { .. }
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => {}
    }
}

fn next_binding_id(
    kind: BindingKind,
    name: &str,
    occurrences: &mut HashMap<(BindingKind, String), usize>,
) -> BindingId {
    let key = (kind, name.to_string());
    let occurrence = occurrences.entry(key).or_default();
    let binding = BindingId::new(kind, name, *occurrence);
    *occurrence += 1;
    binding
}

fn collect_method_declaration_effect_facts(program: &Program, facts: &mut FactSet) {
    for ty in &program.types {
        collect_type_method_declaration_effect_facts(ty, facts);
    }
}

fn collect_type_method_declaration_effect_facts(ty: &TypeDecl, facts: &mut FactSet) {
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                let return_nullability = source_summary_method_return_nullability(method);
                if return_nullability != NullabilityState::Unknown || !method.throws.is_empty() {
                    facts.method_effect_facts.push(MethodEffectFact {
                        method: MethodKey::source_method(ty, method),
                        receiver_mutability: ReceiverMutabilityFact::Unknown,
                        return_nullability,
                        thrown: method.throws.clone(),
                        provenance: FactProvenance::ParsedSource,
                    });
                }
            }
            Member::Type(nested) => collect_type_method_declaration_effect_facts(nested, facts),
            _ => {}
        }
    }
}

fn source_summary_method_return_nullability(method: &MethodDecl) -> NullabilityState {
    crate::signature::source_method_return_nullability_constraints(method)
        .into_iter()
        .find_map(|constraint| match constraint {
            crate::signature::SignatureConstraint::RequiredStorageNullability(state) => Some(state),
            _ => None,
        })
        .unwrap_or_else(|| crate::semantic_analysis::method_return_nullability_effect(method))
}

fn qualify_nested_type_names(program: &mut Program) {
    // A Java compilation unit can refer to a nested type through a sibling
    // top-level owner (`FieldInfo.Builder`).  Lexical qualification alone only
    // sees nested types of the declaration currently being rewritten, leaving
    // those references as `Unknown` and later erasing otherwise concrete
    // builder values to `Box<dyn Any>`.
    let mut captured_type_params = HashMap::new();
    for ty in &mut program.types {
        let qualified_name = ty.name.clone();
        materialize_non_static_inner_type_params(
            ty,
            &qualified_name,
            &[],
            &mut captured_type_params,
        );
    }
    let mut compilation_unit_nested = HashMap::new();
    for ty in &program.types {
        collect_compilation_unit_nested_type_aliases(ty, &mut compilation_unit_nested);
    }
    for ty in &mut program.types {
        qualify_nested_type_names_for_type(ty, &compilation_unit_nested, &captured_type_params);
    }
}

#[derive(Debug, Clone)]
struct CapturedInnerTypeParams {
    explicit_arity: usize,
    args: Vec<String>,
}

/// Rust makes the hidden enclosing instance of a non-static Java inner class
/// part of the struct, so the enclosing class's type parameters must also be
/// parameters of that struct.  Java leaves those arguments implicit at inner
/// type use sites; record the expansion here so qualification can materialize
/// them consistently in declarations and references.
fn materialize_non_static_inner_type_params(
    ty: &mut TypeDecl,
    qualified_name: &str,
    inherited: &[String],
    captured: &mut HashMap<String, CapturedInnerTypeParams>,
) {
    let mut effective = ty.type_params.clone();
    for param in inherited {
        if !effective.contains(param) {
            effective.push(param.clone());
            ty.type_params.push(param.clone());
            ty.type_param_bounds.push(None);
        }
    }

    for member in &mut ty.members {
        let Member::Type(nested) = member else {
            continue;
        };
        let nested_simple = nested.name.rsplit('.').next().unwrap_or(&nested.name);
        let qualified = format!("{qualified_name}.{nested_simple}");
        let captures_outer = nested.has_enclosing_instance_in(ty.kind);
        let inherited = if captures_outer {
            effective.as_slice()
        } else {
            &[]
        };
        if !inherited.is_empty() {
            captured.insert(
                qualified.clone(),
                CapturedInnerTypeParams {
                    explicit_arity: nested.type_params.len(),
                    args: inherited.to_vec(),
                },
            );
        }
        materialize_non_static_inner_type_params(nested, &qualified, inherited, captured);
    }
}

fn collect_compilation_unit_nested_type_aliases(
    ty: &TypeDecl,
    aliases: &mut HashMap<String, String>,
) {
    let owner_simple = ty.name.rsplit('.').next().unwrap_or(&ty.name);
    for member in &ty.members {
        let Member::Type(nested) = member else {
            continue;
        };
        let nested_simple = nested.name.rsplit('.').next().unwrap_or(&nested.name);
        let qualified = format!("{}.{}", ty.name, nested_simple);
        aliases.insert(format!("{owner_simple}.{nested_simple}"), qualified.clone());
        aliases.insert(qualified.clone(), qualified.clone());
        collect_nested_type_path_aliases(
            nested,
            format!("{owner_simple}.{nested_simple}"),
            qualified,
            aliases,
        );
    }
}

fn qualify_nested_type_names_for_type(
    ty: &mut TypeDecl,
    enclosing_nested: &HashMap<String, String>,
    captured_type_params: &HashMap<String, CapturedInnerTypeParams>,
) {
    let mut direct_nested = HashMap::new();
    for member in &ty.members {
        if let Member::Type(nested) = member {
            let nested_simple = nested.name.rsplit('.').next().unwrap_or(&nested.name);
            let nested_qualified = format!("{}.{}", ty.name, nested_simple);
            direct_nested.insert(nested.name.clone(), nested_qualified.clone());
            collect_nested_type_path_aliases(
                nested,
                nested_simple.to_string(),
                nested_qualified,
                &mut direct_nested,
            );
        }
    }

    let mut visible_nested = enclosing_nested.clone();
    visible_nested.extend(direct_nested.clone());

    rewrite_type_decl_references(ty, &visible_nested, captured_type_params);

    for member in &mut ty.members {
        if let Member::Type(nested) = member {
            if let Some(qualified) = direct_nested.get(&nested.name).cloned() {
                nested.name = qualified;
            }
            qualify_nested_type_names_for_type(nested, &visible_nested, captured_type_params);
        }
    }
}

fn collect_nested_type_path_aliases(
    ty: &TypeDecl,
    reference_prefix: String,
    qualified_prefix: String,
    aliases: &mut HashMap<String, String>,
) {
    for member in &ty.members {
        if let Member::Type(nested) = member {
            let nested_simple = nested.name.rsplit('.').next().unwrap_or(&nested.name);
            let reference_name = format!("{reference_prefix}.{nested_simple}");
            let qualified_name = format!("{qualified_prefix}.{nested_simple}");
            aliases.insert(reference_name.clone(), qualified_name.clone());
            collect_nested_type_path_aliases(nested, reference_name, qualified_name, aliases);
        }
    }
}

fn aliases_outside_type_parameter_scope<'a>(
    aliases: &'a HashMap<String, String>,
    parameters: &[String],
) -> std::borrow::Cow<'a, HashMap<String, String>> {
    if !parameters.iter().any(|name| aliases.contains_key(name)) {
        return std::borrow::Cow::Borrowed(aliases);
    }
    std::borrow::Cow::Owned(
        aliases
            .iter()
            .filter(|(name, _)| {
                let root = name.split('.').next().unwrap_or(name);
                !parameters.iter().any(|parameter| parameter == root)
            })
            .map(|(name, target)| (name.clone(), target.clone()))
            .collect(),
    )
}

fn rewrite_type_decl_references(
    ty: &mut TypeDecl,
    direct_nested: &HashMap<String, String>,
    captured_type_params: &HashMap<String, CapturedInnerTypeParams>,
) {
    let class_aliases = aliases_outside_type_parameter_scope(direct_nested, &ty.type_params);
    let direct_nested = class_aliases.as_ref();
    for edge in ty.extends.iter_mut().chain(ty.implements.iter_mut()) {
        // Hierarchy edges are parameterized types too. Qualifying only an
        // exact string key misses both the owner and arguments in I<Nested>,
        // disconnecting override identity/effects from method declarations.
        let original = crate::parser::parse_hierarchy_type(edge);
        let mut qualified = original.clone();
        qualify_nested_java_type(&mut qualified, direct_nested, captured_type_params);
        if qualified != original {
            *edge = crate::java_type_substitution::java_type_text(&qualified);
        }
    }
    for bound in &mut ty.type_param_bounds {
        if let Some(bound) = bound {
            qualify_nested_java_type(bound, direct_nested, captured_type_params);
        }
    }
    for member in &mut ty.members {
        match member {
            Member::Field(field) => {
                qualify_nested_java_type(&mut field.ty, direct_nested, captured_type_params);
                if let Some(initializer) = &mut field.initializer {
                    qualify_nested_expr_types(initializer, direct_nested, captured_type_params);
                }
            }
            Member::Method(method) => {
                let member_aliases =
                    aliases_outside_type_parameter_scope(direct_nested, &method.type_params);
                let direct_nested = member_aliases.as_ref();
                qualify_nested_java_type(
                    &mut method.return_type,
                    direct_nested,
                    captured_type_params,
                );
                for bound in &mut method.type_param_bounds {
                    if let Some(bound) = bound {
                        qualify_nested_java_type(bound, direct_nested, captured_type_params);
                    }
                }
                for param in &mut method.params {
                    qualify_nested_java_type(&mut param.ty, direct_nested, captured_type_params);
                }
                for thrown in &mut method.throws {
                    qualify_nested_java_type(thrown, direct_nested, captured_type_params);
                }
                if let Some(body) = &mut method.body {
                    qualify_nested_block_types(body, direct_nested, captured_type_params);
                }
            }
            Member::Constructor(ctor) => {
                let member_aliases =
                    aliases_outside_type_parameter_scope(direct_nested, &ctor.type_params);
                let direct_nested = member_aliases.as_ref();
                for bound in &mut ctor.type_param_bounds {
                    if let Some(bound) = bound {
                        qualify_nested_java_type(bound, direct_nested, captured_type_params);
                    }
                }
                for param in &mut ctor.params {
                    qualify_nested_java_type(&mut param.ty, direct_nested, captured_type_params);
                }
                for thrown in &mut ctor.throws {
                    qualify_nested_java_type(thrown, direct_nested, captured_type_params);
                }
                if let Some(body) = &mut ctor.body {
                    qualify_nested_block_types(body, direct_nested, captured_type_params);
                }
            }
            Member::Type(_) | Member::Unsupported { .. } => {}
        }
    }
}

fn qualify_nested_block_types(
    block: &mut Block,
    direct_nested: &HashMap<String, String>,
    captured_type_params: &HashMap<String, CapturedInnerTypeParams>,
) {
    for stmt in &mut block.statements {
        qualify_nested_stmt_types(stmt, direct_nested, captured_type_params);
    }
}

fn qualify_nested_stmt_types(
    stmt: &mut Stmt,
    direct_nested: &HashMap<String, String>,
    captured_type_params: &HashMap<String, CapturedInnerTypeParams>,
) {
    match stmt {
        Stmt::VarDecl {
            ty, initializer, ..
        } => {
            qualify_nested_java_type(ty, direct_nested, captured_type_params);
            if let Some(initializer) = initializer {
                qualify_nested_expr_types(initializer, direct_nested, captured_type_params);
            }
        }
        Stmt::Assign { target, value } => {
            qualify_nested_expr_types(target, direct_nested, captured_type_params);
            qualify_nested_expr_types(value, direct_nested, captured_type_params);
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            qualify_nested_expr_types(expr, direct_nested, captured_type_params);
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            qualify_nested_expr_types(condition, direct_nested, captured_type_params);
            qualify_nested_block_types(then_block, direct_nested, captured_type_params);
            if let Some(else_block) = else_block {
                qualify_nested_block_types(else_block, direct_nested, captured_type_params);
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            qualify_nested_expr_types(condition, direct_nested, captured_type_params);
            qualify_nested_block_types(body, direct_nested, captured_type_params);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                qualify_nested_stmt_types(stmt, direct_nested, captured_type_params);
            }
            if let Some(condition) = condition {
                qualify_nested_expr_types(condition, direct_nested, captured_type_params);
            }
            for expr in update {
                qualify_nested_expr_types(expr, direct_nested, captured_type_params);
            }
            qualify_nested_block_types(body, direct_nested, captured_type_params);
        }
        Stmt::EnhancedFor {
            var_type,
            iterable,
            body,
            ..
        } => {
            qualify_nested_java_type(var_type, direct_nested, captured_type_params);
            qualify_nested_expr_types(iterable, direct_nested, captured_type_params);
            qualify_nested_block_types(body, direct_nested, captured_type_params);
        }
        Stmt::Switch { selector, cases } => {
            qualify_nested_expr_types(selector, direct_nested, captured_type_params);
            for case in cases {
                for stmt in &mut case.statements {
                    qualify_nested_stmt_types(stmt, direct_nested, captured_type_params);
                }
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            for resource in resources {
                qualify_nested_stmt_types(resource, direct_nested, captured_type_params);
            }
            qualify_nested_block_types(body, direct_nested, captured_type_params);
            for catch in catches {
                for ty in &mut catch.types {
                    qualify_nested_java_type(ty, direct_nested, captured_type_params);
                }
                qualify_nested_block_types(&mut catch.body, direct_nested, captured_type_params);
            }
            if let Some(finally_block) = finally_block {
                qualify_nested_block_types(finally_block, direct_nested, captured_type_params);
            }
        }
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                qualify_nested_expr_types(arg, direct_nested, captured_type_params);
            }
        }
        Stmt::LocalType(local) => {
            rewrite_type_decl_references(local, direct_nested, captured_type_params)
        }
        Stmt::Block(body) | Stmt::LabeledBlock { body, .. } => {
            qualify_nested_block_types(body, direct_nested, captured_type_params)
        }
        Stmt::Return(None) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
    }
}

fn qualify_nested_expr_types(
    expr: &mut Expr,
    direct_nested: &HashMap<String, String>,
    captured_type_params: &HashMap<String, CapturedInnerTypeParams>,
) {
    match expr {
        Expr::FieldAccess(receiver, _) | Expr::Unary { expr: receiver, .. } => {
            qualify_nested_expr_types(receiver, direct_nested, captured_type_params);
        }
        Expr::ArrayAccess(array, index)
        | Expr::Binary {
            left: array,
            right: index,
            ..
        } => {
            qualify_nested_expr_types(array, direct_nested, captured_type_params);
            qualify_nested_expr_types(index, direct_nested, captured_type_params);
        }
        Expr::ArrayCreation {
            element_type,
            dimensions,
            initializer,
        } => {
            qualify_nested_java_type(element_type, direct_nested, captured_type_params);
            for expr in dimensions {
                qualify_nested_expr_types(expr, direct_nested, captured_type_params);
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    qualify_nested_expr_types(expr, direct_nested, captured_type_params);
                }
            }
        }
        Expr::ArrayInitializer(values) => {
            for expr in values {
                qualify_nested_expr_types(expr, direct_nested, captured_type_params);
            }
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            qualify_nested_expr_types(condition, direct_nested, captured_type_params);
            qualify_nested_expr_types(then_expr, direct_nested, captured_type_params);
            qualify_nested_expr_types(else_expr, direct_nested, captured_type_params);
        }
        Expr::Switch { selector, cases } => {
            qualify_nested_expr_types(selector, direct_nested, captured_type_params);
            for case in cases {
                for label in &mut case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        qualify_nested_expr_types(expr, direct_nested, captured_type_params);
                    }
                }
                qualify_nested_expr_types(&mut case.value, direct_nested, captured_type_params);
            }
        }
        Expr::Call {
            receiver,
            args,
            type_args,
            ..
        } => {
            for ty in type_args {
                qualify_nested_java_type(ty, direct_nested, captured_type_params);
            }
            if let Some(receiver) = receiver {
                qualify_nested_expr_types(receiver, direct_nested, captured_type_params);
            }
            for arg in args {
                qualify_nested_expr_types(arg, direct_nested, captured_type_params);
            }
        }
        Expr::New { ty, args, body, .. } => {
            qualify_nested_java_type(ty, direct_nested, captured_type_params);
            for arg in args {
                qualify_nested_expr_types(arg, direct_nested, captured_type_params);
            }
            if let Some(members) = body {
                for member in members {
                    match member {
                        Member::Field(field) => {
                            qualify_nested_java_type(
                                &mut field.ty,
                                direct_nested,
                                captured_type_params,
                            );
                            if let Some(initializer) = &mut field.initializer {
                                qualify_nested_expr_types(
                                    initializer,
                                    direct_nested,
                                    captured_type_params,
                                );
                            }
                        }
                        Member::Method(method) => {
                            qualify_nested_java_type(
                                &mut method.return_type,
                                direct_nested,
                                captured_type_params,
                            );
                            for param in &mut method.params {
                                qualify_nested_java_type(
                                    &mut param.ty,
                                    direct_nested,
                                    captured_type_params,
                                );
                            }
                            if let Some(body) = &mut method.body {
                                qualify_nested_block_types(
                                    body,
                                    direct_nested,
                                    captured_type_params,
                                );
                            }
                        }
                        Member::Constructor(ctor) => {
                            for param in &mut ctor.params {
                                qualify_nested_java_type(
                                    &mut param.ty,
                                    direct_nested,
                                    captured_type_params,
                                );
                            }
                            if let Some(body) = &mut ctor.body {
                                qualify_nested_block_types(
                                    body,
                                    direct_nested,
                                    captured_type_params,
                                );
                            }
                        }
                        Member::Type(_) | Member::Unsupported { .. } => {}
                    }
                }
            }
        }
        Expr::Cast { ty, expr } | Expr::InstanceOf { ty, expr, .. } => {
            qualify_nested_java_type(ty, direct_nested, captured_type_params);
            qualify_nested_expr_types(expr, direct_nested, captured_type_params);
        }
        Expr::Lambda { params, body } => {
            for param in params {
                if let Some(ty) = &mut param.ty {
                    qualify_nested_java_type(ty, direct_nested, captured_type_params);
                }
            }
            match body {
                LambdaBody::Expr(expr) => {
                    qualify_nested_expr_types(expr, direct_nested, captured_type_params)
                }
                LambdaBody::Block(block) => {
                    qualify_nested_block_types(block, direct_nested, captured_type_params)
                }
            }
        }
        Expr::ClassLiteral(ty) => qualify_nested_java_type(ty, direct_nested, captured_type_params),
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => {}
    }
}

fn qualify_nested_java_type(
    ty: &mut JavaType,
    direct_nested: &HashMap<String, String>,
    captured_type_params: &HashMap<String, CapturedInnerTypeParams>,
) {
    match ty {
        JavaType::Intersection(bounds) => {
            for bound in bounds {
                qualify_nested_java_type(bound, direct_nested, captured_type_params);
            }
        }
        JavaType::Named { name, args } => {
            if let Some(qualified) = direct_nested.get(name) {
                *name = qualified.clone();
            }
            for arg in args.iter_mut() {
                qualify_nested_java_type(arg, direct_nested, captured_type_params);
            }
            if let Some(captured) = captured_type_params.get(name)
                && args.len() == captured.explicit_arity
            {
                args.extend(captured.args.iter().cloned().map(|name| JavaType::Named {
                    name,
                    args: Vec::new(),
                }));
            }
        }
        JavaType::Unknown(name) => {
            if let Some(qualified) = direct_nested.get(name) {
                *name = qualified.clone();
            }
            if let Some(captured) = captured_type_params.get(name)
                && captured.explicit_arity == 0
            {
                *ty = JavaType::Named {
                    name: name.clone(),
                    args: captured
                        .args
                        .iter()
                        .cloned()
                        .map(|name| JavaType::Named {
                            name,
                            args: Vec::new(),
                        })
                        .collect(),
                };
            }
        }
        JavaType::Array(inner) => {
            qualify_nested_java_type(inner, direct_nested, captured_type_params)
        }
        JavaType::Wildcard(Some(bound)) => match bound {
            WildcardBound::Extends(inner) | WildcardBound::Super(inner) => {
                qualify_nested_java_type(inner, direct_nested, captured_type_params)
            }
        },
        JavaType::Void | JavaType::Primitive(_) | JavaType::String | JavaType::Wildcard(None) => {}
    }
}

/// Method-name counts per type, used to decide whether a name needs an overload
/// suffix. Counting only a type's *declared* methods gave a subclass that
/// overrides one of several overloads a different Rust name from the superclass
/// it overrides — same Java method, two names, which breaks the moment dispatch
/// is virtual.
type ProgramMethodCounts = HashMap<String, HashMap<String, usize>>;

fn declared_and_implicit_method_counts(ty: &TypeDecl) -> HashMap<String, usize> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for member in &ty.members {
        if let Member::Method(method) = member {
            *counts.entry(method.name.clone()).or_default() += 1;
        }
    }
    if ty.kind == TypeKind::Enum {
        *counts.entry("valueOf".into()).or_default() += 1;
    }
    counts
}

fn collect_program_method_counts(ty: &TypeDecl, out: &mut ProgramMethodCounts) {
    let counts = declared_and_implicit_method_counts(ty);
    // Index under both names: nested types are stored qualified (`Outer.Inner`)
    // while an `extends` clause usually names them simply (`Inner`).
    let simple = ty.name.rsplit('.').next().unwrap_or(&ty.name).to_string();
    for key in [ty.name.clone(), simple] {
        let entry = out.entry(key).or_default();
        for (name, count) in &counts {
            let slot = entry.entry(name.clone()).or_default();
            *slot = (*slot).max(*count);
        }
    }
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_program_method_counts(nested, out);
        }
    }
}

/// Counts for `ty` including everything it inherits, so an override is named the
/// same way as the method it overrides.
fn method_counts_including_supertypes(
    ty: &TypeDecl,
    program_counts: &ProgramMethodCounts,
) -> HashMap<String, usize> {
    let mut counts = declared_and_implicit_method_counts(ty);
    let mut seen = std::collections::HashSet::new();
    let mut pending = ty.extends.clone().into_iter().collect::<Vec<_>>();
    while let Some(super_name) = pending.pop() {
        let simple = super_name
            .rsplit('.')
            .next()
            .unwrap_or(&super_name)
            .to_string();
        if !seen.insert(simple.clone()) {
            continue;
        }
        for key in [super_name.as_str(), simple.as_str()] {
            if let Some(super_counts) = program_counts.get(key) {
                for (name, count) in super_counts {
                    let entry = counts.entry(name.clone()).or_default();
                    *entry = (*entry).max(*count);
                }
            }
        }
    }
    counts
}

fn assign_type_rust_names(
    ty: &mut TypeDecl,
    diagnostics: &mut DiagnosticBag,
    program_counts: &ProgramMethodCounts,
) {
    let method_counts = method_counts_including_supertypes(ty, program_counts);
    let mut ctor_count = 0usize;
    for member in &ty.members {
        if let Member::Constructor(_) = member {
            ctor_count += 1;
        }
    }
    assign_type_rust_names_with_counts(ty, diagnostics, program_counts, method_counts, ctor_count)
}

fn assign_type_rust_names_with_source_symbols(
    ty: &mut TypeDecl,
    diagnostics: &mut DiagnosticBag,
    program_counts: &ProgramMethodCounts,
    symbols: &ProgramSymbols,
) {
    let method_counts = method_counts_including_source_hierarchy(ty, program_counts, symbols);
    let mut ctor_count = 0usize;
    for member in &ty.members {
        if let Member::Constructor(_) = member {
            ctor_count += 1;
        }
    }
    assign_type_rust_names_with_source_symbols_and_counts(
        ty,
        diagnostics,
        program_counts,
        symbols,
        method_counts,
        ctor_count,
    )
}

fn method_counts_including_source_hierarchy(
    ty: &TypeDecl,
    program_counts: &ProgramMethodCounts,
    symbols: &ProgramSymbols,
) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for method in symbols.methods_for_hierarchy(&ty.name) {
        *counts.entry(method.java_name).or_default() += 1;
    }
    if counts.is_empty() {
        method_counts_including_supertypes(ty, program_counts)
    } else {
        counts
    }
}

fn assign_type_rust_names_with_counts(
    ty: &mut TypeDecl,
    diagnostics: &mut DiagnosticBag,
    program_counts: &ProgramMethodCounts,
    method_counts: HashMap<String, usize>,
    ctor_count: usize,
) {
    let extends = ty.extends.clone();
    for member in &mut ty.members {
        match member {
            Member::Method(method) => resolve_method(method, &method_counts, extends.as_deref()),
            Member::Constructor(ctor) => resolve_constructor(ctor, ctor_count),
            Member::Type(nested) => assign_type_rust_names(nested, diagnostics, program_counts),
            Member::Unsupported { reason, .. } => {
                diagnostics.unsupported("unsupported_member", reason.clone());
            }
            _ => {}
        }
    }
    allocate_unique_declared_method_rust_names(ty);
}

fn assign_type_rust_names_with_source_symbols_and_counts(
    ty: &mut TypeDecl,
    diagnostics: &mut DiagnosticBag,
    program_counts: &ProgramMethodCounts,
    symbols: &ProgramSymbols,
    method_counts: HashMap<String, usize>,
    ctor_count: usize,
) {
    let extends = ty.extends.clone();
    for member in &mut ty.members {
        match member {
            Member::Method(method) => resolve_method(method, &method_counts, extends.as_deref()),
            Member::Constructor(ctor) => resolve_constructor(ctor, ctor_count),
            Member::Type(nested) => assign_type_rust_names_with_source_symbols(
                nested,
                diagnostics,
                program_counts,
                symbols,
            ),
            Member::Unsupported { reason, .. } => {
                diagnostics.unsupported("unsupported_member", reason.clone());
            }
            _ => {}
        }
    }
    allocate_unique_declared_method_rust_names(ty);
}

fn resolve_type_calls(
    ty: &mut TypeDecl,
    symbols: &ProgramSymbols,
    diagnostics: &mut DiagnosticBag,
    facts: &mut FactSet,
    external_facts: &ResolverExternalFacts<'_>,
) {
    let methods = symbols.methods_for_hierarchy(&ty.name);
    let constructors = symbols.constructors_for(&ty.name);
    let fields = collect_field_env(ty, symbols);
    for member in &mut ty.members {
        match member {
            Member::Field(field) => {
                if let Some(initializer) = &mut field.initializer {
                    // Initializers are executable class-scope expressions,
                    // not constructor bodies. Resolve calls and diamond
                    // target typing with class bounds but no ctor bindings.
                    let mut env = fields.clone();
                    insert_scoped_type_param_bounds(
                        &mut env,
                        &ty.type_params,
                        &ty.type_param_bounds,
                    );
                    resolve_expr_calls(
                        initializer,
                        &ty.name,
                        symbols,
                        &methods,
                        &mut env,
                        diagnostics,
                        facts,
                        external_facts,
                        Some(&field.name),
                    );
                    apply_target_type_to_constructor_expr(
                        initializer,
                        &field.ty,
                        symbols,
                        external_facts,
                    );
                    resolve_lambda_with_expected_type(
                        initializer,
                        &field.ty,
                        &ty.name,
                        symbols,
                        &methods,
                        &env,
                        diagnostics,
                        facts,
                        external_facts,
                        Some(&field.name),
                    );
                }
            }
            Member::Method(method) => {
                if let Some(body) = &mut method.body {
                    let mut env = fields.clone();
                    insert_scoped_type_param_bounds(
                        &mut env,
                        &ty.type_params,
                        &ty.type_param_bounds,
                    );
                    insert_scoped_type_param_bounds(
                        &mut env,
                        &method.type_params,
                        &method.type_param_bounds,
                    );
                    for param in &method.params {
                        env.insert_local(param.name.clone(), param.ty.clone());
                    }
                    resolve_block_calls(
                        body,
                        &ty.name,
                        symbols,
                        &methods,
                        constructors,
                        &mut env,
                        diagnostics,
                        facts,
                        external_facts,
                        Some(&method.name),
                        Some(&method.return_type),
                    );
                }
            }
            Member::Constructor(ctor) => {
                if let Some(body) = &mut ctor.body {
                    let mut env = fields.clone();
                    insert_scoped_type_param_bounds(
                        &mut env,
                        &ty.type_params,
                        &ty.type_param_bounds,
                    );
                    insert_scoped_type_param_bounds(
                        &mut env,
                        &ctor.type_params,
                        &ctor.type_param_bounds,
                    );
                    for param in &ctor.params {
                        env.insert_local(param.name.clone(), param.ty.clone());
                    }
                    resolve_block_calls(
                        body,
                        &ty.name,
                        symbols,
                        &methods,
                        constructors,
                        &mut env,
                        diagnostics,
                        facts,
                        external_facts,
                        Some("<init>"),
                        None,
                    );
                }
            }
            Member::Type(nested) => {
                resolve_type_calls(nested, symbols, diagnostics, facts, external_facts)
            }
            _ => {}
        }
    }
}

fn resolve_method(
    method: &mut MethodDecl,
    counts: &HashMap<String, usize>,
    direct_superclass: Option<&str>,
) {
    if crate::class_initialization::is_initializer(method) {
        // The ordinary declaration allocator still disambiguates the emitted
        // Rust name against user methods after all methods have been named.
        return;
    }
    let base = crate::parser::to_snake_case(&method.name);
    if source_method_overrides_jdk_input_stream_read_3(method, direct_superclass) {
        method.rust_name = Some("read_3".to_string());
        return;
    }
    // Object.toString() is also an inherited overload when only an
    // argument-taking source declaration is present. Preserve the zero-arg
    // protocol name for both Java dispatch and the emitted Object fallback.
    let overloads_object_to_string = method.name == "toString" && !method.params.is_empty();
    if counts.get(&method.name).copied().unwrap_or(0) <= 1 && !overloads_object_to_string {
        method.rust_name = Some(base);
        return;
    }

    // The zero-argument overload keeps the bare name. Only one overload of a
    // name can take no arguments, so this cannot collide, and the alternative
    // is a trailing underscore (`to_string_encoding_`) from joining an empty
    // suffix.
    if method.params.is_empty() {
        method.rust_name = Some(base);
        return;
    }

    method.rust_name = Some(crate::java_signature::source_method_overload_rust_name(
        method,
    ));
}

fn allocate_unique_declared_method_rust_names(ty: &mut TypeDecl) {
    let mut occupied = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Constructor(ctor) => ctor.rust_name.clone(),
            _ => None,
        })
        .collect::<HashSet<_>>();

    for member in &mut ty.members {
        let Member::Method(method) = member else {
            continue;
        };
        let base = method
            .rust_name
            .clone()
            .unwrap_or_else(|| crate::parser::to_snake_case(&method.name));
        let rust_name = crate::java_naming::allocate_rust_ident(&base, &occupied);
        occupied.insert(rust_name.clone());
        method.rust_name = Some(rust_name);
    }
}

fn source_method_overrides_jdk_input_stream_read_3(
    method: &MethodDecl,
    direct_superclass: Option<&str>,
) -> bool {
    method.name == "read"
        && method.params.len() == 3
        && direct_superclass
            .and_then(|name| name.rsplit('.').next())
            .is_some_and(|name| {
                matches!(
                    name,
                    "InputStream"
                        | "FileInputStream"
                        | "ByteArrayInputStream"
                        | "BufferedInputStream"
                        | "DataInputStream"
                        | "GZIPInputStream"
                        | "ZipInputStream"
                        | "ObjectInputStream"
                        | "FilterInputStream"
                        | "PushbackInputStream"
                        | "SequenceInputStream"
                )
            })
        && method.params.first().is_some_and(|param| {
            matches!(
                &param.ty,
                JavaType::Array(element)
                    if matches!(element.as_ref(), JavaType::Primitive(PrimitiveType::Byte))
            )
        })
        && method
            .params
            .get(1)
            .is_some_and(|param| matches!(param.ty, JavaType::Primitive(PrimitiveType::Int)))
        && method
            .params
            .get(2)
            .is_some_and(|param| matches!(param.ty, JavaType::Primitive(PrimitiveType::Int)))
}

fn resolve_constructor(ctor: &mut ConstructorDecl, ctor_count: usize) {
    ctor.rust_name =
        Some(crate::java_signature::source_constructor_rust_name_with_count(ctor, ctor_count));
}

fn insert_scoped_type_param_bounds(
    env: &mut TypeEnv,
    type_params: &[String],
    type_param_bounds: &[Option<JavaType>],
) {
    for (index, name) in type_params.iter().enumerate() {
        env.insert_type_param_bound(
            name.clone(),
            type_param_bounds
                .get(index)
                .and_then(Clone::clone)
                .unwrap_or_else(|| JavaType::Named {
                    name: "Object".to_string(),
                    args: Vec::new(),
                }),
        );
    }
}

#[derive(Debug, Clone)]
struct MethodSymbol {
    declaring_type: String,
    owner_type_params: Vec<String>,
    owner_type_param_bounds: Vec<Option<JavaType>>,
    method_type_params: Vec<String>,
    method_type_param_bounds: Vec<Option<JavaType>>,
    java_name: String,
    rust_name: String,
    params: Vec<Param>,
    return_type: JavaType,
    is_static: bool,
}

#[derive(Debug, Clone)]
struct MethodResolution {
    call: ResolvedCall,
    params: Vec<Param>,
}

#[derive(Debug, Clone)]
struct ConstructorSymbol {
    declaring_type: String,
    outer_type: Option<String>,
    rust_name: String,
    type_params: Vec<String>,
    params: Vec<Param>,
}

#[derive(Debug, Clone)]
struct FieldSymbol {
    declaring_type: String,
    outer_type: Option<String>,
    java_name: String,
    ty: JavaType,
    is_static: bool,
}

fn collect_method_symbols(ty: &TypeDecl) -> Vec<MethodSymbol> {
    let mut methods = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) => Some(MethodSymbol {
                declaring_type: ty.name.clone(),
                owner_type_params: ty.type_params.clone(),
                owner_type_param_bounds: ty.type_param_bounds.clone(),
                method_type_params: method.type_params.clone(),
                method_type_param_bounds: method.type_param_bounds.clone(),
                java_name: method.name.clone(),
                rust_name: method
                    .rust_name
                    .clone()
                    .unwrap_or_else(|| crate::parser::to_snake_case(&method.name)),
                params: method.params.clone(),
                return_type: method.return_type.clone(),
                is_static: method.modifiers.contains(&Modifier::Static),
            }),
            _ => None,
        })
        .collect::<Vec<_>>();
    // javac supplies this real method without a source MethodDecl. Retain its
    // typed owner/signature so calls use the same graph and effect propagation
    // as declared methods, rather than becoming unresolved external calls.
    if ty.kind == TypeKind::Enum {
        methods.push(MethodSymbol {
            declaring_type: ty.name.clone(),
            owner_type_params: vec![],
            owner_type_param_bounds: vec![],
            method_type_params: vec![],
            method_type_param_bounds: vec![],
            java_name: "valueOf".into(),
            rust_name: crate::java_naming::ENUM_VALUE_OF_RESULT_METHOD.into(),
            params: vec![Param {
                name: "name".into(),
                source_name: "name".into(),
                ty: JavaType::String,
                is_varargs: false,
                binding: None,
            }],
            return_type: JavaType::Named {
                name: ty.name.clone(),
                args: vec![],
            },
            is_static: true,
        });
    }
    methods
}

fn collect_constructor_symbols(ty: &TypeDecl, outer: Option<&TypeDecl>) -> Vec<ConstructorSymbol> {
    let mut constructors = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Constructor(ctor) => Some(ConstructorSymbol {
                declaring_type: ty.name.clone(),
                outer_type: outer.map(|outer| outer.name.clone()),
                rust_name: ctor.rust_name.clone().unwrap_or_else(|| "new".to_string()),
                type_params: ctor.type_params.clone(),
                params: ctor.params.clone(),
            }),
            _ => None,
        })
        .collect::<Vec<_>>();
    if constructors.is_empty() && matches!(ty.kind, TypeKind::Class | TypeKind::Record) {
        constructors.push(ConstructorSymbol {
            declaring_type: ty.name.clone(),
            outer_type: outer.map(|outer| outer.name.clone()),
            rust_name: "new".to_string(),
            type_params: Vec::new(),
            params: Vec::new(),
        });
    }
    constructors
}

fn collect_field_symbols(ty: &TypeDecl, outer: Option<&TypeDecl>) -> Vec<FieldSymbol> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) => Some(FieldSymbol {
                declaring_type: ty.name.clone(),
                outer_type: outer.map(|outer| outer.name.clone()),
                java_name: field.name.clone(),
                ty: field.ty.clone(),
                is_static: field.modifiers.contains(&Modifier::Static),
            }),
            Member::Type(nested) => Some(FieldSymbol {
                declaring_type: ty.name.clone(),
                outer_type: outer.map(|outer| outer.name.clone()),
                java_name: crate::parser::to_snake_case(nested_simple_type_name(&nested.name)),
                ty: JavaType::Named {
                    name: nested.name.clone(),
                    args: Vec::new(),
                },
                is_static: true,
            }),
            _ => None,
        })
        .collect()
}

#[derive(Debug, Clone, Default)]
struct ProgramSymbols {
    methods_by_type: HashMap<String, Vec<MethodSymbol>>,
    constructors_by_type: HashMap<String, Vec<ConstructorSymbol>>,
    fields_by_type: HashMap<String, Vec<FieldSymbol>>,
    hierarchy_methods_by_type: HashMap<String, Vec<MethodSymbol>>,
    hierarchy_fields_by_type: HashMap<String, Vec<FieldSymbol>>,
    source_hierarchy_walk_by_type: Arc<Mutex<HashMap<String, Vec<(String, u32)>>>>,
    extends_by_type: HashMap<String, String>,
    extends_type_by_type: HashMap<String, JavaType>,
    implements_by_type: HashMap<String, Vec<String>>,
    implements_type_by_type: HashMap<String, Vec<JavaType>>,
    outer_by_type: HashMap<String, String>,
    static_by_type: HashMap<String, bool>,
    canonical_type_names: HashMap<String, String>,
    kind_by_type: HashMap<String, TypeKind>,
    type_params_by_type: HashMap<String, Vec<String>>,
    type_param_bounds_by_type: HashMap<String, Vec<Option<JavaType>>>,
    functional_interfaces: HashSet<String>,
    functional_interface_methods: HashMap<String, MethodSymbol>,
}

impl ProgramSymbols {
    fn from_program(program: &Program) -> Self {
        let mut symbols = Self::default();
        for ty in &program.types {
            symbols.collect_type(ty, None);
        }
        symbols.canonicalize_source_hierarchy();
        symbols.rebuild_hierarchy_indexes();
        symbols
    }

    fn canonicalize_source_hierarchy(&mut self) {
        // Collect every declaration before interpreting parent names. A sibling
        // nested parent may be declared later, and a global short-name lookup
        // cannot distinguish two lexical owners' identically named parents.
        // Canonicalize this shared inventory once, so method/field lookup,
        // assignability and generic ancestry all walk the same source edges.
        self.extends_type_by_type = self
            .extends_type_by_type
            .iter()
            .map(|(owner, parent)| (owner.clone(), self.member_type_in_owner(parent, owner, &[])))
            .collect();
        self.implements_type_by_type = self
            .implements_type_by_type
            .iter()
            .map(|(owner, parents)| {
                (
                    owner.clone(),
                    parents
                        .iter()
                        .map(|parent| self.member_type_in_owner(parent, owner, &[]))
                        .collect(),
                )
            })
            .collect();
        self.extends_by_type = self
            .extends_type_by_type
            .iter()
            .map(|(owner, parent)| (owner.clone(), java_type_simple_name(parent)))
            .collect();
        self.implements_by_type = self
            .implements_type_by_type
            .iter()
            .map(|(owner, parents)| {
                (
                    owner.clone(),
                    parents.iter().map(java_type_simple_name).collect(),
                )
            })
            .collect();
    }

    fn rebuild_hierarchy_indexes(&mut self) {
        self.hierarchy_methods_by_type.clear();
        self.hierarchy_fields_by_type.clear();
        let type_names = self
            .canonical_type_names
            .values()
            .cloned()
            .collect::<HashSet<_>>();
        for type_name in type_names {
            self.hierarchy_methods_by_type.insert(
                type_name.clone(),
                self.compute_methods_for_hierarchy(&type_name),
            );
            self.hierarchy_fields_by_type.insert(
                type_name.clone(),
                self.compute_fields_for_hierarchy(&type_name),
            );
        }
    }

    fn collect_type(&mut self, ty: &TypeDecl, outer: Option<&TypeDecl>) {
        let methods = collect_method_symbols(ty);
        self.methods_by_type
            .insert(ty.name.clone(), methods.clone());
        self.constructors_by_type
            .entry(ty.name.clone())
            .or_default()
            .extend(collect_constructor_symbols(ty, outer));
        self.fields_by_type
            .entry(ty.name.clone())
            .or_default()
            .extend(collect_field_symbols(ty, outer));
        self.kind_by_type.insert(ty.name.clone(), ty.kind);
        self.type_params_by_type
            .insert(ty.name.clone(), ty.type_params.clone());
        self.type_param_bounds_by_type
            .insert(ty.name.clone(), ty.type_param_bounds.clone());
        self.static_by_type
            .insert(ty.name.clone(), ty.modifiers.contains(&Modifier::Static));
        self.canonical_type_names
            .insert(ty.name.clone(), ty.name.clone());
        // Batch programs retain per-compilation-unit names even after their
        // aggregate package is erased. Imports still name those original
        // canonical/binary owners; resolve them to the same declaration used
        // for methods, fields and constructors rather than dropping overloads.
        if let Some(names) = &ty.source_names {
            for alias in [&names.canonical, &names.binary] {
                self.canonical_type_names
                    .insert(alias.clone(), ty.name.clone());
            }
        }
        if let Some(outer) = outer {
            self.outer_by_type
                .insert(ty.name.clone(), outer.name.clone());
            let outer_aliases = self
                .canonical_type_names
                .iter()
                .filter_map(|(alias, source_name)| {
                    (source_name == &outer.name).then(|| alias.clone())
                })
                .collect::<Vec<_>>();
            for outer_alias in outer_aliases {
                self.canonical_type_names.insert(
                    format!("{outer_alias}.{}", nested_simple_type_name(&ty.name)),
                    ty.name.clone(),
                );
            }
        }
        if is_source_functional_interface(ty) {
            self.functional_interfaces.insert(ty.name.clone());
            if let Some(method) = methods.iter().find(|method| !method.is_static).cloned() {
                self.functional_interface_methods
                    .insert(ty.name.clone(), method);
            }
        }
        self.canonical_type_names
            .insert(crate::parser::to_snake_case(&ty.name), ty.name.clone());
        if let Some(super_type) = &ty.extends {
            let parsed_super_type = crate::parser::parse_type(super_type);
            self.extends_by_type
                .insert(ty.name.clone(), java_type_simple_name(&parsed_super_type));
            self.extends_type_by_type
                .insert(ty.name.clone(), parsed_super_type);
        }
        if !ty.implements.is_empty() {
            let implements_types = ty
                .implements
                .iter()
                .map(|interface| crate::parser::parse_type(interface))
                .collect::<Vec<_>>();
            self.implements_by_type.insert(
                ty.name.clone(),
                implements_types.iter().map(java_type_simple_name).collect(),
            );
            self.implements_type_by_type
                .insert(ty.name.clone(), implements_types);
        }
        for member in &ty.members {
            if let Member::Type(nested) = member {
                self.collect_type(nested, Some(ty));
            }
        }
        for member in &ty.members {
            match member {
                Member::Method(method) => {
                    if let Some(body) = &method.body {
                        self.collect_local_types_from_block(body, ty);
                    }
                }
                Member::Constructor(constructor) => {
                    if let Some(body) = &constructor.body {
                        self.collect_local_types_from_block(body, ty);
                    }
                }
                _ => {}
            }
        }
    }

    fn collect_local_types_from_block(&mut self, block: &Block, enclosing_type: &TypeDecl) {
        for stmt in &block.statements {
            self.collect_local_types_from_stmt(stmt, enclosing_type);
        }
    }

    fn collect_local_types_from_stmt(&mut self, stmt: &Stmt, enclosing_type: &TypeDecl) {
        match stmt {
            Stmt::LocalType(local) => self.collect_type(local, Some(enclosing_type)),
            Stmt::VarDecl {
                initializer: Some(initializer),
                ..
            } => self.collect_local_types_from_expr(initializer, enclosing_type),
            Stmt::Assign { target, value } => {
                self.collect_local_types_from_expr(target, enclosing_type);
                self.collect_local_types_from_expr(value, enclosing_type);
            }
            Stmt::Expr(expr) | Stmt::Throw(expr) => {
                self.collect_local_types_from_expr(expr, enclosing_type);
            }
            Stmt::Return(Some(expr)) => {
                self.collect_local_types_from_expr(expr, enclosing_type);
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
                ..
            } => {
                self.collect_local_types_from_expr(condition, enclosing_type);
                self.collect_local_types_from_block(then_block, enclosing_type);
                if let Some(else_block) = else_block {
                    self.collect_local_types_from_block(else_block, enclosing_type);
                }
            }
            Stmt::While {
                condition, body, ..
            }
            | Stmt::DoWhile {
                condition, body, ..
            } => {
                self.collect_local_types_from_expr(condition, enclosing_type);
                self.collect_local_types_from_block(body, enclosing_type);
            }
            Stmt::For {
                init,
                condition,
                update,
                body,
                ..
            } => {
                for stmt in init {
                    self.collect_local_types_from_stmt(stmt, enclosing_type);
                }
                if let Some(condition) = condition {
                    self.collect_local_types_from_expr(condition, enclosing_type);
                }
                for expr in update {
                    self.collect_local_types_from_expr(expr, enclosing_type);
                }
                self.collect_local_types_from_block(body, enclosing_type);
            }
            Stmt::EnhancedFor { iterable, body, .. } => {
                self.collect_local_types_from_expr(iterable, enclosing_type);
                self.collect_local_types_from_block(body, enclosing_type);
            }
            Stmt::Block(body) | Stmt::LabeledBlock { body, .. } => {
                self.collect_local_types_from_block(body, enclosing_type)
            }
            Stmt::Switch {
                selector, cases, ..
            } => {
                self.collect_local_types_from_expr(selector, enclosing_type);
                for case in cases {
                    for stmt in &case.statements {
                        self.collect_local_types_from_stmt(stmt, enclosing_type);
                    }
                }
            }
            Stmt::Try {
                resources,
                body,
                catches,
                finally_block,
            } => {
                for resource in resources {
                    self.collect_local_types_from_stmt(resource, enclosing_type);
                }
                self.collect_local_types_from_block(body, enclosing_type);
                for catch in catches {
                    self.collect_local_types_from_block(&catch.body, enclosing_type);
                }
                if let Some(finally_block) = finally_block {
                    self.collect_local_types_from_block(finally_block, enclosing_type);
                }
            }
            Stmt::ConstructorInvocation { args, .. } => {
                for arg in args {
                    self.collect_local_types_from_expr(arg, enclosing_type);
                }
            }
            _ => {}
        }
    }

    fn collect_local_types_from_expr(&mut self, expr: &Expr, enclosing_type: &TypeDecl) {
        match expr {
            Expr::FieldAccess(receiver, _) | Expr::Unary { expr: receiver, .. } => {
                self.collect_local_types_from_expr(receiver, enclosing_type);
            }
            Expr::ArrayAccess(array, index)
            | Expr::Binary {
                left: array,
                right: index,
                ..
            } => {
                self.collect_local_types_from_expr(array, enclosing_type);
                self.collect_local_types_from_expr(index, enclosing_type);
            }
            Expr::ArrayCreation {
                dimensions,
                initializer,
                ..
            } => {
                for expr in dimensions {
                    self.collect_local_types_from_expr(expr, enclosing_type);
                }
                if let Some(initializer) = initializer {
                    for expr in initializer {
                        self.collect_local_types_from_expr(expr, enclosing_type);
                    }
                }
            }
            Expr::ArrayInitializer(dimensions) => {
                for expr in dimensions {
                    self.collect_local_types_from_expr(expr, enclosing_type);
                }
            }
            Expr::Ternary {
                condition,
                then_expr,
                else_expr,
            } => {
                self.collect_local_types_from_expr(condition, enclosing_type);
                self.collect_local_types_from_expr(then_expr, enclosing_type);
                self.collect_local_types_from_expr(else_expr, enclosing_type);
            }
            Expr::Switch { selector, cases } => {
                self.collect_local_types_from_expr(selector, enclosing_type);
                for case in cases {
                    for label in &case.labels {
                        if let SwitchLabel::Expr(expr) = label {
                            self.collect_local_types_from_expr(expr, enclosing_type);
                        }
                    }
                    self.collect_local_types_from_expr(&case.value, enclosing_type);
                }
            }
            Expr::Call { receiver, args, .. } => {
                if let Some(receiver) = receiver {
                    self.collect_local_types_from_expr(receiver, enclosing_type);
                }
                for arg in args {
                    self.collect_local_types_from_expr(arg, enclosing_type);
                }
            }
            Expr::New {
                qualifier,
                args,
                body,
                ..
            } => {
                if let Some(qualifier) = qualifier {
                    self.collect_local_types_from_expr(qualifier, enclosing_type);
                }
                for arg in args {
                    self.collect_local_types_from_expr(arg, enclosing_type);
                }
                if let Some(members) = body {
                    self.collect_local_types_from_members(members, enclosing_type);
                }
            }
            Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
                self.collect_local_types_from_expr(expr, enclosing_type);
            }
            Expr::Lambda { body, .. } => match body {
                LambdaBody::Expr(expr) => self.collect_local_types_from_expr(expr, enclosing_type),
                LambdaBody::Block(block) => {
                    self.collect_local_types_from_block(block, enclosing_type)
                }
            },
            Expr::Literal(_)
            | Expr::Var(_)
            | Expr::This
            | Expr::Super
            | Expr::MethodReference { .. }
            | Expr::ClassLiteral(_)
            | Expr::Raw(_)
            | Expr::Unsupported { .. } => {}
        }
    }

    fn collect_local_types_from_members(&mut self, members: &[Member], enclosing_type: &TypeDecl) {
        for member in members {
            match member {
                Member::Type(local) => self.collect_type(local, Some(enclosing_type)),
                Member::Method(method) => {
                    if let Some(body) = &method.body {
                        self.collect_local_types_from_block(body, enclosing_type);
                    }
                }
                Member::Constructor(constructor) => {
                    if let Some(body) = &constructor.body {
                        self.collect_local_types_from_block(body, enclosing_type);
                    }
                }
                _ => {}
            }
        }
    }

    fn methods_for_hierarchy(&self, type_name: &str) -> Vec<MethodSymbol> {
        let source_name = self.source_type_name(type_name);
        if let Some(methods) = self.hierarchy_methods_by_type.get(source_name) {
            return methods.clone();
        }
        self.compute_methods_for_hierarchy(source_name)
    }

    fn compute_methods_for_hierarchy(&self, type_name: &str) -> Vec<MethodSymbol> {
        let mut out = Vec::new();
        let mut seen_signatures = HashMap::new();
        let mut visited = HashSet::new();
        self.collect_methods_for_hierarchy(
            self.source_type_name(type_name),
            &HashMap::new(),
            &mut visited,
            &mut seen_signatures,
            &mut out,
        );
        out
    }

    fn collect_methods_for_hierarchy(
        &self,
        type_name: &str,
        substitutions: &HashMap<String, JavaType>,
        visited: &mut HashSet<String>,
        seen_signatures: &mut HashMap<String, usize>,
        out: &mut Vec<MethodSymbol>,
    ) {
        let type_name = self.source_type_name(type_name).to_string();
        let visit_key = format!("{type_name}::{substitutions:?}");
        if !visited.insert(visit_key) {
            return;
        }
        if let Some(methods) = self.methods_by_type.get(&type_name) {
            for method in methods {
                let substituted = substitute_method_symbol(method, substitutions);
                let key = method_signature_key(&substituted);
                if let Some(&index) = seen_signatures.get(&key) {
                    let inherited = &out[index];
                    let facts = FactSet::default();
                    if java_type_assignable(
                        &substituted.return_type,
                        &inherited.return_type,
                        self,
                        &facts,
                    ) && !java_type_assignable(
                        &inherited.return_type,
                        &substituted.return_type,
                        self,
                        &facts,
                    ) {
                        out[index] = substituted;
                    }
                } else {
                    seen_signatures.insert(key, out.len());
                    out.push(substituted);
                }
            }
        }

        if let Some(super_type) = self.extends_type_by_type.get(&type_name) {
            let mut super_substitutions = substitutions.clone();
            if let Some(super_name) = extend_supertype_substitutions(
                self,
                &type_name,
                super_type,
                &mut super_substitutions,
            ) {
                self.collect_methods_for_hierarchy(
                    &super_name,
                    &super_substitutions,
                    visited,
                    seen_signatures,
                    out,
                );
            }
        }

        if let Some(interfaces) = self.implements_type_by_type.get(&type_name) {
            for interface_type in interfaces {
                let mut interface_substitutions = substitutions.clone();
                if let Some(interface_name) = extend_supertype_substitutions(
                    self,
                    &type_name,
                    interface_type,
                    &mut interface_substitutions,
                ) {
                    self.collect_methods_for_hierarchy(
                        &interface_name,
                        &interface_substitutions,
                        visited,
                        seen_signatures,
                        out,
                    );
                }
            }
        }
    }

    fn methods_for_unqualified_call(&self, type_name: &str) -> Vec<MethodSymbol> {
        let mut out = Vec::new();
        let mut seen_signatures = HashSet::new();
        let current_type_is_static = self.type_is_static(type_name);
        for (index, lexical_type) in self.lexical_type_scope(type_name).into_iter().enumerate() {
            for method in self.methods_for_hierarchy(&lexical_type) {
                if index > 0 && current_type_is_static && !method.is_static {
                    continue;
                }
                if seen_signatures.insert(method_signature_key(&method)) {
                    out.push(method);
                }
            }
        }
        out
    }

    fn type_is_static(&self, type_name: &str) -> bool {
        self.static_by_type
            .get(self.source_type_name(type_name))
            .copied()
            .unwrap_or(false)
    }

    fn constructors_for(&self, type_name: &str) -> &[ConstructorSymbol] {
        let type_name = self.source_type_name(type_name);
        self.constructors_by_type
            .get(type_name)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn constructors_for_new(&self, type_name: &str, current_type: &str) -> Vec<ConstructorSymbol> {
        let direct_type_name = self.source_type_name(type_name).to_string();
        let lexical_type_name = self.lexical_constructor_type_name(type_name, current_type);
        let constructor_type_name = lexical_type_name.as_deref().unwrap_or(&direct_type_name);
        let constructors = self
            .constructors_by_type
            .get(constructor_type_name)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let scoped = constructors
            .iter()
            .filter(|constructor| self.constructor_visible_from(constructor, current_type))
            .cloned()
            .collect::<Vec<_>>();
        let mut selected = if scoped.is_empty() {
            constructors.to_vec()
        } else {
            scoped
        };
        for constructor in &mut selected {
            for param in &mut constructor.params {
                param.ty = self.member_type_in_owner(
                    &param.ty,
                    &constructor.declaring_type,
                    &constructor.type_params,
                );
            }
        }
        selected
    }

    fn lexical_constructor_type_name(&self, type_name: &str, current_type: &str) -> Option<String> {
        if type_name.contains('.') {
            return None;
        }
        self.lexical_source_type_name(type_name, current_type)
            .filter(|name| self.constructors_by_type.contains_key(name))
    }

    fn lexical_source_type_name(&self, type_name: &str, current_type: &str) -> Option<String> {
        self.lexical_type_scope(current_type)
            .into_iter()
            .filter_map(|scope_type| {
                let candidate = format!("{scope_type}.{type_name}");
                let source_name = self.source_type_name(&candidate).to_string();
                self.kind_by_type
                    .contains_key(&source_name)
                    .then_some(source_name)
            })
            .next()
    }

    fn member_type_in_owner(
        &self,
        ty: &JavaType,
        owner: &str,
        member_type_params: &[String],
    ) -> JavaType {
        match ty {
            JavaType::Named { name, args } => {
                let is_parameter = member_type_params.contains(name)
                    || self
                        .type_params_for(owner)
                        .is_some_and(|params| params.contains(name));
                let name = if is_parameter {
                    name.clone()
                } else {
                    self.lexical_source_type_name(name, owner)
                        .unwrap_or_else(|| self.source_type_name(name).to_string())
                };
                JavaType::Named {
                    name,
                    args: args
                        .iter()
                        .map(|arg| self.member_type_in_owner(arg, owner, member_type_params))
                        .collect(),
                }
            }
            JavaType::Array(element) => JavaType::Array(Box::new(self.member_type_in_owner(
                element,
                owner,
                member_type_params,
            ))),
            JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Extends(bound))) => {
                JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Extends(Box::new(
                    self.member_type_in_owner(bound, owner, member_type_params),
                ))))
            }
            JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Super(bound))) => {
                JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Super(Box::new(
                    self.member_type_in_owner(bound, owner, member_type_params),
                ))))
            }
            JavaType::Intersection(bounds) => JavaType::Intersection(
                bounds
                    .iter()
                    .map(|bound| self.member_type_in_owner(bound, owner, member_type_params))
                    .collect(),
            ),
            _ => ty.clone(),
        }
    }

    fn constructor_visible_from(
        &self,
        constructor: &ConstructorSymbol,
        current_type: &str,
    ) -> bool {
        let Some(outer_type) = constructor.outer_type.as_deref() else {
            return false;
        };
        self.lexical_type_scope(current_type)
            .iter()
            .any(|type_name| type_name == outer_type)
    }

    fn lexical_type_scope(&self, type_name: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut current = Some(self.source_type_name(type_name).to_string());
        let mut visited = HashSet::new();
        while let Some(type_name) = current {
            let source_name = self.source_type_name(&type_name).to_string();
            if !visited.insert(source_name.clone()) {
                break;
            }
            out.push(source_name.clone());
            // Source summaries can flatten a nested declaration into a
            // top-level entry while retaining its dotted Java owner. Its
            // lexical siblings remain visible even without a capture edge.
            current = self.outer_by_type.get(&source_name).cloned().or_else(|| {
                source_name
                    .rsplit_once('.')
                    .map(|(owner, _)| owner.to_string())
            });
        }
        out
    }

    fn field_for_from(
        &self,
        type_name: &str,
        field_name: &str,
        current_type: &str,
    ) -> Option<&FieldSymbol> {
        let canonical_field_name = crate::parser::to_snake_case(field_name);
        let mut current = Some(self.field_lookup_start_type(type_name, current_type));
        let mut visited = HashSet::new();
        while let Some(type_name) = current.as_deref() {
            if !visited.insert(type_name.to_string()) {
                return None;
            }
            if let Some(fields) = self.fields_by_type.get(type_name) {
                let matches = fields
                    .iter()
                    .filter(|field| field_name_matches(field, field_name, &canonical_field_name))
                    .collect::<Vec<_>>();
                if let Some(field) = self.preferred_scoped_field(matches, current_type) {
                    return Some(field);
                }
            }
            current = self.extends_by_type.get(type_name).cloned();
        }
        None
    }

    fn field_type_for_receiver(&self, field: &FieldSymbol, receiver: &JavaType) -> JavaType {
        if field.is_static {
            return field.ty.clone();
        }
        // Field lookup selects a declaration, but its type is read in the
        // receiver's actual owner frame. Reuse hierarchy argument projection
        // and the same simultaneous owner substitution as methods/constructors.
        let Some(args) = self.source_supertype_args(receiver, &field.declaring_type) else {
            return field.ty.clone();
        };
        let substitutions = owner_type_argument_substitutions(
            self.type_params_for(&field.declaring_type).unwrap_or(&[]),
            &[],
            &args,
            self.type_param_bounds_for(&field.declaring_type)
                .unwrap_or(&[]),
        );
        substitute_method_type_variables(&field.ty, &substitutions)
    }

    fn external_super_field_type_for(&self, type_name: &str, field_name: &str) -> Option<JavaType> {
        let mut current = Some(self.source_type_name(type_name).to_string());
        let mut visited = HashSet::new();
        while let Some(type_name) = current.as_deref() {
            if !visited.insert(type_name.to_string()) {
                return None;
            }
            let Some(super_ty) = self.extends_type_by_type.get(type_name) else {
                return None;
            };
            let super_name = java_type_simple_name(super_ty);
            if self.fields_by_type.contains_key(&super_name)
                || self.methods_by_type.contains_key(&super_name)
            {
                current = Some(super_name);
                continue;
            }
            if let Some(java_ty) = known_external_or_pack_field_type_for_receiver(
                super_ty,
                field_name,
                &FactSet::default(),
            ) {
                return Some(java_ty);
            }
            return None;
        }
        None
    }

    fn field_lookup_start_type(&self, type_name: &str, current_type: &str) -> String {
        let source_name = self.source_type_name(type_name);
        if self.fields_by_type.contains_key(source_name)
            || self.methods_by_type.contains_key(source_name)
        {
            return source_name.to_string();
        }
        self.type_param_bound_for(current_type, type_name)
            .as_ref()
            .map(java_type_simple_name)
            .unwrap_or_else(|| source_name.to_string())
    }

    fn static_field_for_from(
        &self,
        type_name: &str,
        field_name: &str,
        current_type: &str,
    ) -> Option<&FieldSymbol> {
        let canonical_field_name = crate::parser::to_snake_case(field_name);
        let mut current = Some(self.source_type_name(type_name));
        let mut visited = HashSet::new();
        while let Some(type_name) = current {
            if !visited.insert(type_name.to_string()) {
                return None;
            }
            if let Some(fields) = self.fields_by_type.get(type_name) {
                let matches = fields
                    .iter()
                    .filter(|field| {
                        field.is_static
                            && field_name_matches(field, field_name, &canonical_field_name)
                    })
                    .collect::<Vec<_>>();
                if let Some(field) = self.preferred_scoped_field(matches, current_type) {
                    return Some(field);
                }
            }
            current = self.extends_by_type.get(type_name).map(String::as_str);
        }
        None
    }

    fn preferred_scoped_field<'a>(
        &self,
        fields: Vec<&'a FieldSymbol>,
        current_type: &str,
    ) -> Option<&'a FieldSymbol> {
        if fields.len() <= 1 {
            return fields.into_iter().next();
        }
        let scoped = fields
            .iter()
            .copied()
            .filter(|field| self.field_visible_from(field, current_type))
            .collect::<Vec<_>>();
        match scoped.as_slice() {
            [field] => Some(*field),
            [] => fields.into_iter().next(),
            _ => scoped.into_iter().next(),
        }
    }

    fn field_visible_from(&self, field: &FieldSymbol, current_type: &str) -> bool {
        let Some(outer_type) = field.outer_type.as_deref() else {
            return false;
        };
        self.lexical_type_scope(current_type)
            .iter()
            .any(|type_name| type_name == outer_type)
    }

    fn fields_for_hierarchy(&self, type_name: &str) -> Vec<FieldSymbol> {
        let source_name = self.source_type_name(type_name);
        if let Some(fields) = self.hierarchy_fields_by_type.get(source_name) {
            return fields.clone();
        }
        self.compute_fields_for_hierarchy(source_name)
    }

    fn compute_fields_for_hierarchy(&self, type_name: &str) -> Vec<FieldSymbol> {
        let mut out = Vec::new();
        let mut seen_fields = HashSet::new();
        let mut current = Some(self.source_type_name(type_name).to_string());
        let mut substitutions = HashMap::new();
        let mut visited = HashSet::new();
        while let Some(type_name) = current.as_deref() {
            if !visited.insert(type_name.to_string()) {
                break;
            }
            if let Some(fields) = self.fields_by_type.get(type_name) {
                for field in fields {
                    if seen_fields.insert(field.java_name.clone()) {
                        out.push(substitute_field_symbol(field, &substitutions));
                    }
                }
            }
            current = self
                .extends_type_by_type
                .get(type_name)
                .and_then(|super_type| {
                    extend_supertype_substitutions(self, type_name, super_type, &mut substitutions)
                });
        }
        out
    }

    fn fields_for_enclosing_types(&self, type_name: &str) -> Vec<FieldSymbol> {
        let mut out = Vec::new();
        let mut seen_fields = HashSet::new();
        let mut current = self
            .outer_by_type
            .get(self.source_type_name(type_name))
            .map(String::as_str);
        let mut visited = HashSet::new();
        while let Some(type_name) = current {
            let type_name = self.source_type_name(type_name);
            if !visited.insert(type_name.to_string()) {
                break;
            }
            for field in self.fields_for_hierarchy(type_name) {
                if seen_fields.insert(field.java_name.clone()) {
                    out.push(field);
                }
            }
            current = self.outer_by_type.get(type_name).map(String::as_str);
        }
        out
    }

    fn source_type_name<'a>(&'a self, type_name: &'a str) -> &'a str {
        self.canonical_type_names
            .get(type_name)
            .map(String::as_str)
            .unwrap_or(type_name)
    }

    fn is_functional_interface_type(&self, type_name: &str) -> bool {
        let source_name = self.source_type_name(type_name);
        self.functional_interfaces.contains(source_name)
            || crate::stdlib::is_known_functional_interface(source_name)
    }

    fn source_functional_interface_method(&self, ty: &JavaType) -> Option<&MethodSymbol> {
        let (JavaType::Named { name, .. } | JavaType::Unknown(name)) = ty else {
            return None;
        };
        let source_name = self.source_type_name(name);
        self.functional_interface_methods.get(source_name)
    }

    fn source_subtype_distance(
        &self,
        actual: &str,
        target: &str,
        external_facts: &FactSet,
    ) -> Option<u32> {
        let target = self.source_type_name(target);
        for (type_name, distance) in self.source_hierarchy_walk(actual) {
            let type_name = self.source_type_name(&type_name);
            if type_name == target {
                return Some(distance);
            }
            if self.fact_subtype_reaches(type_name, target, external_facts) {
                return Some(distance + 2);
            }
            if crate::stdlib::is_known_java_subtype(type_name, target) {
                return Some(distance + 5);
            }
        }

        None
    }

    fn source_hierarchy_walk(&self, actual: &str) -> Vec<(String, u32)> {
        let actual = self.source_type_name(actual).to_string();
        if let Some(walk) = self
            .source_hierarchy_walk_by_type
            .lock()
            .expect("source hierarchy walk cache mutex poisoned")
            .get(&actual)
            .cloned()
        {
            return walk;
        }

        let mut walk = Vec::new();
        let mut pending = vec![(actual.clone(), 0_u32)];
        let mut visited = HashSet::new();
        while let Some((type_name, distance)) = pending.pop() {
            let type_name = self.source_type_name(&type_name).to_string();
            if !visited.insert(type_name.clone()) {
                continue;
            }
            walk.push((type_name.clone(), distance));
            if let Some(super_type) = self.extends_by_type.get(&type_name) {
                pending.push((self.source_type_name(super_type).to_string(), distance + 1));
            }
            if let Some(interfaces) = self.implements_by_type.get(&type_name) {
                for interface in interfaces {
                    pending.push((self.source_type_name(interface).to_string(), distance + 1));
                }
            }
        }

        self.source_hierarchy_walk_by_type
            .lock()
            .expect("source hierarchy walk cache mutex poisoned")
            .insert(actual, walk.clone());
        walk
    }

    fn fact_subtype_reaches(&self, actual: &str, target: &str, external_facts: &FactSet) -> bool {
        let actual_ty = JavaType::Named {
            name: self.source_type_name(actual).to_string(),
            args: Vec::new(),
        };
        let target_ty = JavaType::Named {
            name: self.source_type_name(target).to_string(),
            args: Vec::new(),
        };
        external_facts
            .hierarchy_subtype_policy(&actual_ty, &target_ty)
            .reaches()
    }

    fn is_source_enum_type(&self, type_name: &str) -> bool {
        let source_name = self.source_type_name(type_name);
        self.kind_by_type
            .get(source_name)
            .is_some_and(|kind| *kind == TypeKind::Enum)
    }

    fn source_type_kind(&self, type_name: &str) -> Option<TypeKind> {
        let source_name = self.source_type_name(type_name);
        self.kind_by_type.get(source_name).copied()
    }

    fn type_params_for(&self, type_name: &str) -> Option<&[String]> {
        self.type_params_by_type
            .get(self.source_type_name(type_name))
            .map(Vec::as_slice)
    }

    fn type_param_bounds_for(&self, type_name: &str) -> Option<&[Option<JavaType>]> {
        self.type_param_bounds_by_type
            .get(self.source_type_name(type_name))
            .map(Vec::as_slice)
    }

    fn type_param_bound_for(&self, type_name: &str, param_name: &str) -> Option<JavaType> {
        let source_name = self.source_type_name(type_name);
        let params = self.type_params_by_type.get(source_name)?;
        let bounds = self.type_param_bounds_by_type.get(source_name)?;
        let index = params.iter().position(|param| param == param_name)?;
        bounds.get(index).and_then(Clone::clone)
    }

    fn source_type_accepts_actual_type(&self, actual_ty: &JavaType) -> bool {
        let JavaType::Named { name, args } = actual_ty else {
            return false;
        };
        let source_name = self.source_type_name(name);
        if !self.methods_by_type.contains_key(source_name) {
            return false;
        }
        let Some(params) = self.type_params_by_type.get(source_name) else {
            return args.is_empty();
        };
        args.is_empty() || params.len() == args.len()
    }

    fn source_single_arg_supertype_arg(
        &self,
        actual_ty: &JavaType,
        target_type: &str,
    ) -> Option<JavaType> {
        let args = self.source_supertype_args(actual_ty, target_type)?;
        (args.len() == 1).then(|| args[0].clone())
    }

    fn source_supertype_args(
        &self,
        actual_ty: &JavaType,
        target_type: &str,
    ) -> Option<Vec<JavaType>> {
        let JavaType::Named { name, args } = actual_ty else {
            return None;
        };
        let target_type = self.source_type_name(target_type);
        let mut pending = vec![(self.source_type_name(name).to_string(), args.clone())];
        let mut visited = HashSet::new();
        while let Some((current, args)) = pending.pop() {
            if !visited.insert(current.clone()) {
                continue;
            }
            if current == target_type {
                return Some(args);
            }
            let substitutions = self
                .type_params_for(&current)
                .unwrap_or(&[])
                .iter()
                .cloned()
                .zip(args)
                .collect();
            for super_type in self.extends_type_by_type.get(&current).into_iter().chain(
                self.implements_type_by_type
                    .get(&current)
                    .into_iter()
                    .flatten(),
            ) {
                if let JavaType::Named { name, args } =
                    substitute_method_type_variables(super_type, &substitutions)
                {
                    pending.push((self.source_type_name(&name).to_string(), args));
                }
            }
        }
        None
    }
}

fn field_name_matches(field: &FieldSymbol, field_name: &str, canonical_field_name: &str) -> bool {
    field.java_name == field_name
        || crate::parser::to_snake_case(&field.java_name) == canonical_field_name
}

fn is_source_functional_interface(ty: &TypeDecl) -> bool {
    if ty.kind != TypeKind::Interface {
        return false;
    }
    let abstract_method_count = ty
        .members
        .iter()
        .filter(|member| match member {
            Member::Method(method) => !method.modifiers.contains(&Modifier::Static),
            _ => false,
        })
        .count();
    abstract_method_count == 1
}

fn extend_supertype_substitutions(
    symbols: &ProgramSymbols,
    _current_type: &str,
    super_type: &JavaType,
    substitutions: &mut HashMap<String, JavaType>,
) -> Option<String> {
    let JavaType::Named { name, args } = super_type else {
        return None;
    };
    let super_name = symbols.source_type_name(name).to_string();
    if let Some(super_params) = symbols
        .methods_by_type
        .get(&super_name)
        .and_then(|_| symbols.type_params_for(&super_name))
    {
        let super_bounds = symbols.type_param_bounds_for(&super_name).unwrap_or(&[]);
        // Evaluate every argument in the child's scope before introducing the
        // parent's binders. In `Child<A,B> extends Parent<B,A>`, inserting A
        // before evaluating the second argument would incorrectly turn both
        // parent arguments into B. Bindings from older owners are not visible
        // in the new lexical owner either.
        let parent_bindings = super_params
            .iter()
            .zip(args)
            .enumerate()
            .map(|(index, (param, arg))| {
                let substituted_arg = substitute_method_type_variables(arg, substitutions);
                (
                    param.clone(),
                    wildcard_actual_or_declared_bound(&substituted_arg, super_bounds.get(index)),
                )
            })
            .collect();
        *substitutions = parent_bindings;
    }
    Some(super_name)
}

fn substitute_method_symbol(
    method: &MethodSymbol,
    substitutions: &HashMap<String, JavaType>,
) -> MethodSymbol {
    let substitutions = crate::java_type_substitution::without_shadowed_binders(
        substitutions,
        &method.method_type_params,
    );
    MethodSymbol {
        declaring_type: method.declaring_type.clone(),
        owner_type_params: method.owner_type_params.clone(),
        owner_type_param_bounds: method.owner_type_param_bounds.clone(),
        method_type_params: method.method_type_params.clone(),
        method_type_param_bounds: method
            .method_type_param_bounds
            .iter()
            .map(|bound| {
                bound
                    .as_ref()
                    .map(|bound| substitute_method_type_variables(bound, &substitutions))
            })
            .collect(),
        java_name: method.java_name.clone(),
        rust_name: method.rust_name.clone(),
        params: method
            .params
            .iter()
            .map(|param| Param {
                name: param.name.clone(),
                source_name: param.source_name.clone(),
                ty: substitute_method_type_variables(&param.ty, &substitutions),
                is_varargs: param.is_varargs,
                binding: param.binding.clone(),
            })
            .collect(),
        return_type: substitute_method_type_variables(&method.return_type, &substitutions),
        is_static: method.is_static,
    }
}

fn substitute_field_symbol(
    field: &FieldSymbol,
    substitutions: &HashMap<String, JavaType>,
) -> FieldSymbol {
    FieldSymbol {
        declaring_type: field.declaring_type.clone(),
        outer_type: field.outer_type.clone(),
        java_name: field.java_name.clone(),
        ty: substitute_method_type_variables(&field.ty, substitutions),
        is_static: field.is_static,
    }
}

fn method_signature_key(method: &MethodSymbol) -> String {
    let mut key = method.java_name.clone();
    key.push('(');
    for param in &method.params {
        key.push_str(&java_type_signature_key(&param.ty));
        key.push(';');
    }
    key.push(')');
    key
}

fn java_type_signature_key(java_ty: &JavaType) -> String {
    match java_ty {
        JavaType::Void => "void".to_string(),
        JavaType::Primitive(primitive) => format!("{primitive:?}"),
        JavaType::String => "String".to_string(),
        JavaType::Array(inner) => format!("{}[]", java_type_signature_key(inner)),
        JavaType::Named { name, args } => {
            if args.is_empty() {
                name.clone()
            } else {
                format!(
                    "{}<{}>",
                    name,
                    args.iter()
                        .map(java_type_signature_key)
                        .collect::<Vec<_>>()
                        .join(",")
                )
            }
        }
        JavaType::Intersection(bounds) => bounds
            .iter()
            .map(java_type_signature_key)
            .collect::<Vec<_>>()
            .join("&"),
        JavaType::Wildcard(bound) => format!("?{bound:?}"),
        JavaType::Unknown(name) => format!("unknown:{name}"),
    }
}

fn collect_field_env(ty: &TypeDecl, symbols: &ProgramSymbols) -> TypeEnv {
    let mut env = TypeEnv::new();
    for field in symbols.fields_for_hierarchy(&ty.name) {
        env.insert_field(field.java_name, field.ty);
    }
    for field in symbols.fields_for_enclosing_types(&ty.name) {
        if env.field_type(&field.java_name).is_none() {
            env.insert_field(field.java_name, field.ty);
        }
    }
    env
}

fn resolve_block_calls(
    block: &mut Block,
    current_type: &str,
    symbols: &ProgramSymbols,
    methods: &[MethodSymbol],
    constructors: &[ConstructorSymbol],
    env: &mut TypeEnv,
    diagnostics: &mut DiagnosticBag,
    facts: &mut FactSet,
    external_facts: &ResolverExternalFacts<'_>,
    enclosing_member: Option<&str>,
    expected_return_type: Option<&JavaType>,
) {
    for stmt in &mut block.statements {
        resolve_stmt_calls(
            stmt,
            current_type,
            symbols,
            methods,
            constructors,
            env,
            diagnostics,
            facts,
            external_facts,
            enclosing_member,
            expected_return_type,
        );
        env.learn_stmt(stmt);
    }
}

fn resolve_stmt_calls(
    stmt: &mut Stmt,
    current_type: &str,
    symbols: &ProgramSymbols,
    methods: &[MethodSymbol],
    constructors: &[ConstructorSymbol],
    env: &mut TypeEnv,
    diagnostics: &mut DiagnosticBag,
    facts: &mut FactSet,
    external_facts: &ResolverExternalFacts<'_>,
    enclosing_member: Option<&str>,
    expected_return_type: Option<&JavaType>,
) {
    match stmt {
        Stmt::VarDecl {
            initializer: Some(expr),
            ty,
            ..
        } => {
            resolve_expr_calls(
                expr,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
            apply_target_type_to_constructor_expr(expr, ty, symbols, external_facts);
            resolve_lambda_with_expected_type(
                expr,
                ty,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
        }
        Stmt::VarDecl { .. } => {}
        Stmt::Assign { target, value } => {
            resolve_expr_calls(
                target,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
            resolve_expr_calls(
                value,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
            let target_type = infer_resolver_expr_type_with_facts(
                target,
                current_type,
                symbols,
                env,
                external_facts,
            );
            apply_target_type_to_constructor_expr(value, &target_type, symbols, external_facts);
            resolve_lambda_with_expected_type(
                value,
                &target_type,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
        }
        Stmt::Expr(expr) => resolve_expr_calls(
            expr,
            current_type,
            symbols,
            methods,
            env,
            diagnostics,
            facts,
            external_facts,
            enclosing_member,
        ),
        Stmt::Return(Some(expr)) => {
            resolve_expr_calls(
                expr,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
            if let Some(target) = expected_return_type {
                resolve_lambda_with_expected_type(
                    expr,
                    target,
                    current_type,
                    symbols,
                    methods,
                    env,
                    diagnostics,
                    facts,
                    external_facts,
                    enclosing_member,
                );
                apply_target_type_to_constructor_expr(expr, target, symbols, external_facts);
            }
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            resolve_expr_calls(
                condition,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
            resolve_block_calls(
                then_block,
                current_type,
                symbols,
                methods,
                constructors,
                &mut env.clone(),
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
                expected_return_type,
            );
            if let Some(block) = else_block {
                resolve_block_calls(
                    block,
                    current_type,
                    symbols,
                    methods,
                    constructors,
                    &mut env.clone(),
                    diagnostics,
                    facts,
                    external_facts,
                    enclosing_member,
                    expected_return_type,
                );
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            resolve_expr_calls(
                condition,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
            resolve_block_calls(
                body,
                current_type,
                symbols,
                methods,
                constructors,
                &mut env.clone(),
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
                expected_return_type,
            );
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                resolve_stmt_calls(
                    stmt,
                    current_type,
                    symbols,
                    methods,
                    constructors,
                    env,
                    diagnostics,
                    facts,
                    external_facts,
                    enclosing_member,
                    expected_return_type,
                );
                env.learn_stmt(stmt);
            }
            if let Some(condition) = condition {
                resolve_expr_calls(
                    condition,
                    current_type,
                    symbols,
                    methods,
                    env,
                    diagnostics,
                    facts,
                    external_facts,
                    enclosing_member,
                );
            }
            for expr in update {
                resolve_expr_calls(
                    expr,
                    current_type,
                    symbols,
                    methods,
                    env,
                    diagnostics,
                    facts,
                    external_facts,
                    enclosing_member,
                );
            }
            resolve_block_calls(
                body,
                current_type,
                symbols,
                methods,
                constructors,
                &mut env.clone(),
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
                expected_return_type,
            );
        }
        Stmt::EnhancedFor {
            var_type,
            var_name,
            iterable,
            body,
            ..
        } => {
            resolve_expr_calls(
                iterable,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
            let mut body_env = env.clone();
            body_env.insert_local(var_name.clone(), var_type.clone());
            resolve_block_calls(
                body,
                current_type,
                symbols,
                methods,
                constructors,
                &mut body_env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
                expected_return_type,
            );
        }
        Stmt::Switch { selector, cases } => {
            resolve_expr_calls(
                selector,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
            for case in cases {
                for label in &mut case.labels {
                    if let crate::java_ir::SwitchLabel::Expr(expr) = label {
                        resolve_expr_calls(
                            expr,
                            current_type,
                            symbols,
                            methods,
                            env,
                            diagnostics,
                            facts,
                            external_facts,
                            enclosing_member,
                        );
                    }
                }
                let mut case_env = env.clone();
                for stmt in &mut case.statements {
                    resolve_stmt_calls(
                        stmt,
                        current_type,
                        symbols,
                        methods,
                        constructors,
                        &mut case_env,
                        diagnostics,
                        facts,
                        external_facts,
                        enclosing_member,
                        expected_return_type,
                    );
                    case_env.learn_stmt(stmt);
                }
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            for resource in resources {
                resolve_stmt_calls(
                    resource,
                    current_type,
                    symbols,
                    methods,
                    constructors,
                    env,
                    diagnostics,
                    facts,
                    external_facts,
                    enclosing_member,
                    expected_return_type,
                );
                env.learn_stmt(resource);
            }
            resolve_block_calls(
                body,
                current_type,
                symbols,
                methods,
                constructors,
                &mut env.clone(),
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
                expected_return_type,
            );
            for catch in catches {
                let mut catch_env = env.clone();
                for ty in &catch.types {
                    catch_env.insert_local(catch.source_name.clone(), ty.clone());
                }
                resolve_block_calls(
                    &mut catch.body,
                    current_type,
                    symbols,
                    methods,
                    constructors,
                    &mut catch_env,
                    diagnostics,
                    facts,
                    external_facts,
                    enclosing_member,
                    expected_return_type,
                );
            }
            if let Some(block) = finally_block {
                resolve_block_calls(
                    block,
                    current_type,
                    symbols,
                    methods,
                    constructors,
                    &mut env.clone(),
                    diagnostics,
                    facts,
                    external_facts,
                    enclosing_member,
                    expected_return_type,
                );
            }
        }
        Stmt::Throw(expr) => resolve_expr_calls(
            expr,
            current_type,
            symbols,
            methods,
            env,
            diagnostics,
            facts,
            external_facts,
            enclosing_member,
        ),
        Stmt::ConstructorInvocation {
            kind,
            args,
            resolved,
        } => {
            for arg in args.iter_mut() {
                resolve_expr_calls(
                    arg,
                    current_type,
                    symbols,
                    methods,
                    env,
                    diagnostics,
                    facts,
                    external_facts,
                    enclosing_member,
                );
            }
            let arg_types = args
                .iter()
                .map(|arg| {
                    infer_resolver_expr_type_with_facts(
                        arg,
                        current_type,
                        symbols,
                        env,
                        external_facts,
                    )
                })
                .collect::<Vec<_>>();
            let constructor_owner = match kind {
                ConstructorInvocationKind::This => Some(current_type.to_string()),
                ConstructorInvocationKind::Super => symbols
                    .extends_by_type
                    .get(symbols.source_type_name(current_type))
                    .cloned(),
            };
            if let Some(constructor_owner) = constructor_owner {
                let constructed_type = match kind {
                    ConstructorInvocationKind::This => JavaType::Named {
                        name: current_type.to_string(),
                        args: symbols
                            .type_params_for(current_type)
                            .unwrap_or(&[])
                            .iter()
                            .map(|name| JavaType::Named {
                                name: name.clone(),
                                args: Vec::new(),
                            })
                            .collect(),
                    },
                    ConstructorInvocationKind::Super => symbols
                        .extends_type_by_type
                        .get(symbols.source_type_name(current_type))
                        .cloned()
                        .unwrap_or_else(|| JavaType::Named {
                            name: constructor_owner.clone(),
                            args: Vec::new(),
                        }),
                };
                let constructor_candidates = if matches!(kind, ConstructorInvocationKind::This) {
                    constructors.to_vec()
                } else {
                    symbols.constructors_for(&constructor_owner).to_vec()
                };
                match resolve_same_class_constructor(
                    &arg_types,
                    &constructor_candidates,
                    symbols,
                    external_facts,
                    Some(&constructed_type),
                    env,
                ) {
                    Ok(Some(mut constructor)) => {
                        refresh_constructor_derived_fields(
                            &mut constructor,
                            Some(&arg_types),
                            symbols,
                            external_facts,
                        );
                        *resolved = Some(constructor);
                    }
                    Ok(None) => facts.unresolved_target_facts.push(UnresolvedTargetFact {
                        enclosing_type: current_type.to_string(),
                        enclosing_member: enclosing_member.map(str::to_string),
                        kind: UnresolvedTargetKind::ConstructorDelegation,
                        target_name: match kind {
                            ConstructorInvocationKind::This => "this".to_string(),
                            ConstructorInvocationKind::Super => "super".to_string(),
                        },
                        receiver_type: Some(JavaType::Named {
                            name: constructor_owner.clone(),
                            args: Vec::new(),
                        }),
                        arg_types,
                        is_anonymous_class: false,
                        provenance: FactProvenance::ParsedSource,
                    }),
                    Err(message) => diagnostics.error(
                        "resolve_constructor_ambiguous",
                        format!("constructor {:?}: {message}", kind),
                    ),
                }
            }
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => resolve_block_calls(
            block,
            current_type,
            symbols,
            methods,
            constructors,
            &mut env.clone(),
            diagnostics,
            facts,
            external_facts,
            enclosing_member,
            expected_return_type,
        ),
        _ => {}
    }
}

fn apply_target_type_to_constructor_expr(
    expr: &mut Expr,
    target_ty: &JavaType,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) {
    match expr {
        Expr::Ternary {
            then_expr,
            else_expr,
            ..
        } => {
            apply_target_type_to_constructor_expr(then_expr, target_ty, symbols, external_facts);
            apply_target_type_to_constructor_expr(else_expr, target_ty, symbols, external_facts);
            return;
        }
        Expr::Switch { cases, .. } => {
            for case in cases {
                apply_target_type_to_constructor_expr(
                    &mut case.value,
                    target_ty,
                    symbols,
                    external_facts,
                );
            }
            return;
        }
        _ => {}
    }
    // Native collection constructors need not have a modeled overload. Their
    // diamond still has a contextual element/key/value type. Retain it on the
    // expression so later capability inference sees the same instantiated
    // payload that lowering uses. Source constructors continue through the
    // hierarchy-aware binder solver below, never this positional platform ABI.
    if let Expr::New {
        ty,
        infer_type_arguments: true,
        resolved: None,
        ..
    } = expr
        && let JavaType::Named { name, .. } = &*ty
        && let JavaType::Named {
            name: target_name, ..
        } = target_ty
        && symbols.type_params_for(name).is_none()
        && symbols.type_params_for(target_name).is_none()
        && crate::stdlib::is_known_java_platform_subtype(name, target_name)
        && crate::stdlib::java_collection_kind_for_type(ty).is_some()
        && crate::stdlib::java_collection_kind_for_type(target_ty).is_some()
    {
        *ty = inferred_constructed_type(ty, Some(target_ty), name, symbols, external_facts);
        return;
    }
    let Expr::New {
        ty,
        infer_type_arguments,
        args,
        resolved: Some(resolved),
        ..
    } = expr
    else {
        return;
    };
    if !*infer_type_arguments {
        return;
    }
    let arg_types = constructor_argument_types_after_targeting(args, resolved);
    let Some(contextual) = contextual_constructed_type(
        ty,
        target_ty,
        resolved,
        arg_types.as_deref(),
        symbols,
        external_facts,
    ) else {
        // Object/raw/non-nominal targets contribute no generic equations.
        // In particular they must not erase argument-inferred witnesses.
        return;
    };
    resolved.constructed_type = contextual;
    refresh_constructor_derived_fields(resolved, None, symbols, external_facts);
    apply_constructor_param_targets(args, resolved, symbols, external_facts);
    // Nested diamond arguments now have their own target constraints. Refresh
    // coercion evidence with those instantiated types, not the old raw types.
    let arg_types = constructor_argument_types_after_targeting(args, resolved);
    refresh_constructor_derived_fields(resolved, arg_types.as_deref(), symbols, external_facts);
}

fn constructor_argument_types_after_targeting(
    args: &[Expr],
    constructor: &ResolvedConstructor,
) -> Option<Vec<JavaType>> {
    let previous = constructor_arg_actual_types(constructor).unwrap_or_default();
    args.iter()
        .enumerate()
        .map(|(index, argument)| match argument {
            Expr::New {
                resolved: Some(nested),
                ..
            } => Some(nested.constructed_type.clone()),
            _ => previous.get(index).cloned(),
        })
        .collect()
}

fn contextual_constructed_type(
    parsed_ty: &JavaType,
    target_ty: &JavaType,
    constructor: &ResolvedConstructor,
    arg_types: Option<&[JavaType]>,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Option<JavaType> {
    let target_ty = crate::types::generic_read_type(target_ty);
    let JavaType::Named {
        name: target_name,
        args: target_args,
    } = &target_ty
    else {
        return None;
    };
    if target_args.is_empty() {
        return None;
    }
    let Some(type_params) = symbols.type_params_for(&constructor.declaring_type) else {
        let inferred = inferred_constructed_type(
            parsed_ty,
            Some(&target_ty),
            &constructor.declaring_type,
            symbols,
            external_facts,
        );
        return matches!(&inferred, JavaType::Named { args, .. } if !args.is_empty())
            .then_some(inferred);
    };
    let formal_owner = JavaType::Named {
        name: constructor.declaring_type.clone(),
        args: type_params
            .iter()
            .map(|name| JavaType::Named {
                name: name.clone(),
                args: vec![],
            })
            .collect(),
    };
    // Project the owner's formal frame through its real hierarchy. Copying
    // target arguments by position is wrong for Child<A,B> extends Base<B,A>.
    let formal_target_args = symbols.source_supertype_args(&formal_owner, target_name)?;
    let formal_target = JavaType::Named {
        name: target_name.clone(),
        args: formal_target_args,
    };
    let mut context = HashMap::new();
    collect_method_type_substitutions(
        &formal_target,
        &target_ty,
        &mut context,
        symbols,
        Some(type_params),
    );
    let arguments = arg_types
        .and_then(|types| {
            constructor_argument_type_substitutions(constructor, types, symbols, external_facts)
        })
        .unwrap_or_default();
    let bounds = symbols
        .type_param_bounds_for(&constructor.declaring_type)
        .unwrap_or(&[]);
    let args = type_params
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let actual = arguments
                .get(parameter)
                .filter(|ty| constructor_witness_is_instantiated(ty, symbols));
            let target = context.get(parameter).or(actual)?;
            Some(match target {
                JavaType::Wildcard(Some(WildcardBound::Extends(bound))) => actual
                    .filter(|actual| java_type_assignable(actual, bound, symbols, external_facts))
                    .cloned()
                    .unwrap_or_else(|| (**bound).clone()),
                JavaType::Wildcard(Some(WildcardBound::Super(bound))) => actual
                    .filter(|actual| java_type_assignable(bound, actual, symbols, external_facts))
                    .cloned()
                    .unwrap_or_else(|| (**bound).clone()),
                JavaType::Wildcard(None) => actual.cloned().unwrap_or_else(|| {
                    bounds
                        .get(index)
                        .and_then(Option::as_ref)
                        .map(|bound| substitute_method_type_variables(bound, &context))
                        .unwrap_or_else(|| JavaType::Named {
                            name: "Object".into(),
                            args: vec![],
                        })
                }),
                other => other.clone(),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(JavaType::Named {
        name: constructor.declaring_type.clone(),
        args,
    })
}

fn constructor_witness_is_instantiated(ty: &JavaType, symbols: &ProgramSymbols) -> bool {
    match ty {
        JavaType::Named { name, args } => {
            symbols
                .type_params_for(name)
                .is_none_or(|params| params.len() == args.len())
                && args
                    .iter()
                    .all(|arg| constructor_witness_is_instantiated(arg, symbols))
        }
        JavaType::Array(element) => constructor_witness_is_instantiated(element, symbols),
        JavaType::Wildcard(_) | JavaType::Unknown(_) => false,
        _ => true,
    }
}

fn apply_constructor_param_targets(
    args: &mut [Expr],
    constructor: &ResolvedConstructor,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) {
    for (arg, param) in args.iter_mut().zip(&constructor.params) {
        apply_target_type_to_constructor_expr(arg, &param.ty, symbols, external_facts);
    }
}

fn constructor_params_for_constructed_type(
    constructor: &ResolvedConstructor,
    symbols: &ProgramSymbols,
) -> Vec<Param> {
    constructor_params_for_type(constructor, &constructor.constructed_type, symbols)
}

fn constructor_params_for_type(
    constructor: &ResolvedConstructor,
    constructed_type: &JavaType,
    symbols: &ProgramSymbols,
) -> Vec<Param> {
    let declaration = symbols
        .constructors_for(&constructor.declaring_type)
        .iter()
        .find(|declaration| {
            declaration.rust_name == constructor.rust_name
                && declaration.params.len() == constructor.params.len()
        });
    let member_type_params = declaration
        .map(|decl| decl.type_params.as_slice())
        .unwrap_or(&[]);
    let declared_params = declaration
        .map(|declaration| declaration.params.as_slice())
        .unwrap_or(&constructor.params);
    let JavaType::Named { name, args } = constructed_type else {
        return constructor.params.clone();
    };
    let Some(type_params) = symbols.type_params_for(name) else {
        return constructor.params.clone();
    };
    let substitutions = type_params
        .iter()
        .zip(args)
        .filter(|(param, _)| !member_type_params.contains(param))
        .map(|(param, arg)| (param.clone(), arg.clone()))
        .collect::<HashMap<_, _>>();
    declared_params
        .iter()
        .map(|param| Param {
            name: param.name.clone(),
            source_name: param.source_name.clone(),
            ty: substitute_method_type_variables(
                &symbols.member_type_in_owner(
                    &param.ty,
                    &constructor.declaring_type,
                    member_type_params,
                ),
                &substitutions,
            ),
            is_varargs: param.is_varargs,
            binding: param.binding.clone(),
        })
        .collect()
}

fn refresh_constructor_derived_fields(
    constructor: &mut ResolvedConstructor,
    arg_types: Option<&[JavaType]>,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) {
    let recovered_arg_types = arg_types
        .is_none()
        .then(|| constructor_arg_actual_types(constructor))
        .flatten();
    let effective_arg_types = arg_types.or(recovered_arg_types.as_deref());

    constructor.params = constructor_params_for_constructed_type(constructor, symbols);

    if let Some(arg_types) = effective_arg_types {
        let param_types = constructor
            .params
            .iter()
            .map(|param| param.ty.clone())
            .collect::<Vec<_>>();
        constructor.arg_coercions = java_coercion_evidence_for_params(
            &constructor.params,
            &param_types,
            arg_types,
            symbols,
            external_facts,
        );
    }
}

fn constructor_arg_actual_types(constructor: &ResolvedConstructor) -> Option<Vec<JavaType>> {
    let arg_count = constructor
        .arg_coercions
        .iter()
        .map(|coercion| coercion.arg_index)
        .max()
        .map_or(0, |index| index + 1);
    let mut arg_types = vec![None; arg_count];
    for coercion in &constructor.arg_coercions {
        if let Some(slot) = arg_types.get_mut(coercion.arg_index) {
            *slot = Some(coercion.actual.clone());
        }
    }
    arg_types.into_iter().collect()
}

fn inferred_constructed_type(
    parsed_ty: &JavaType,
    target_ty: Option<&JavaType>,
    declaring_type: &str,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> JavaType {
    let JavaType::Named { name, args } = parsed_ty else {
        return parsed_ty.clone();
    };
    if !args.is_empty() {
        return parsed_ty.clone();
    }
    let Some(JavaType::Named {
        name: target_name,
        args: target_args,
    }) = target_ty
    else {
        return JavaType::Named {
            name: declaring_type.to_string(),
            args: Vec::new(),
        };
    };
    if target_args.is_empty() {
        return JavaType::Named {
            name: declaring_type.to_string(),
            args: Vec::new(),
        };
    }
    let source_name = symbols.source_type_name(name);
    let source_target = symbols.source_type_name(target_name);
    if source_name != source_target
        && !crate::stdlib::is_known_java_subtype(source_name, source_target)
        && symbols
            .source_subtype_distance(source_name, source_target, external_facts)
            .is_none()
    {
        return JavaType::Named {
            name: declaring_type.to_string(),
            args: Vec::new(),
        };
    }
    JavaType::Named {
        name: declaring_type.to_string(),
        args: target_args.clone(),
    }
}

fn diamond_constructed_type_from_arguments(
    constructor: &ResolvedConstructor,
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Option<JavaType> {
    let type_params = symbols.type_params_for(&constructor.declaring_type)?;
    if type_params.is_empty() {
        return None;
    }
    let substitutions =
        constructor_argument_type_substitutions(constructor, arg_types, symbols, external_facts)?;
    // Unconstrained parameters still need target typing; do not manufacture
    // an erasure and then treat it as inference evidence at later call sites.
    let args = type_params
        .iter()
        .map(|param| substitutions.get(param).cloned())
        .collect::<Option<Vec<_>>>()?;
    if constructor
        .params
        .iter()
        .zip(arg_types)
        .any(|(param, actual)| {
            !java_type_assignable(
                actual,
                &substitute_method_type_variables(&param.ty, &substitutions),
                symbols,
                external_facts,
            )
        })
    {
        return None;
    }
    Some(JavaType::Named {
        name: constructor.declaring_type.clone(),
        args,
    })
}

/// Retain partial lower constraints for contextual inference. A constructor
/// taking only Actual cannot infer its owner's other parameters from arguments
/// alone, but its Actual evidence must not be discarded for that reason.
fn constructor_argument_type_substitutions(
    constructor: &ResolvedConstructor,
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Option<HashMap<String, JavaType>> {
    let type_params = symbols.type_params_for(&constructor.declaring_type)?;
    let raw_owner = JavaType::Named {
        name: constructor.declaring_type.clone(),
        args: vec![],
    };
    let declared_params = constructor_params_for_type(constructor, &raw_owner, symbols);
    let mut substitutions = HashMap::new();
    for (param, actual) in declared_params.iter().zip(arg_types) {
        let mut evidence = HashMap::new();
        collect_method_type_substitutions(
            &param.ty,
            actual,
            &mut evidence,
            symbols,
            Some(type_params),
        );
        for (name, actual) in evidence {
            if is_null_type(&actual) {
                continue;
            }
            if let Some(previous) = substitutions.get(&name) {
                let common = resolver_common_type(previous, &actual, symbols, external_facts);
                if matches!(common, JavaType::Unknown(_)) {
                    return None;
                }
                substitutions.insert(name, common);
            } else {
                substitutions.insert(name, actual);
            }
        }
    }
    Some(substitutions)
}

fn resolve_expr_calls(
    expr: &mut Expr,
    current_type: &str,
    symbols: &ProgramSymbols,
    methods: &[MethodSymbol],
    env: &TypeEnv,
    diagnostics: &mut DiagnosticBag,
    facts: &mut FactSet,
    external_facts: &ResolverExternalFacts<'_>,
    enclosing_member: Option<&str>,
) {
    // Bind explicit imported value fields before call/type inference. Deferring
    // this to rendering makes `VALUE.method()` invent a receiver type VALUE,
    // contaminating effects, storage and generated facade inventories.
    if let Expr::Var(name) = expr
        && env
            .local_type(name)
            .or_else(|| env.local_type(&crate::parser::to_snake_case(name)))
            .is_none()
        && env
            .field_type(name)
            .or_else(|| env.field_type(&crate::parser::to_snake_case(name)))
            .is_none()
        && let Some(Some(owner)) = external_facts.static_member_imports.get(name)
        && external_facts
            .lookup
            .external_field_for(owner, name)
            .cloned()
            .or_else(|| {
                (!external_facts.disable_builtin_external_packs)
                    .then(|| crate::external_packs::external_field_for(owner, name))
                    .flatten()
            })
            .is_some_and(|field| {
                matches!(
                    field.field_kind,
                    ExternalFieldKind::StaticFinal | ExternalFieldKind::EnumVariant
                )
            })
    {
        *expr = Expr::FieldAccess(Box::new(Expr::Var(owner.clone())), name.clone());
    }
    match expr {
        Expr::FieldAccess(receiver, _) => {
            resolve_expr_calls(
                receiver,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
            record_unresolved_field_access(
                expr,
                current_type,
                symbols,
                env,
                facts,
                external_facts,
                enclosing_member,
            );
        }
        Expr::ArrayAccess(array, index) => {
            resolve_expr_calls(
                array,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
            resolve_expr_calls(
                index,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
        }
        Expr::Binary { left, right, .. } => {
            resolve_expr_calls(
                left,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
            resolve_expr_calls(
                right,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
        }
        Expr::Unary { expr, .. } => resolve_expr_calls(
            expr,
            current_type,
            symbols,
            methods,
            env,
            diagnostics,
            facts,
            external_facts,
            enclosing_member,
        ),
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            resolve_expr_calls(
                condition,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
            resolve_expr_calls(
                then_expr,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
            resolve_expr_calls(
                else_expr,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
        }
        Expr::Call {
            receiver,
            name,
            args,
            resolved,
            type_args,
        } => {
            if let Some(receiver) = receiver {
                resolve_expr_calls(
                    receiver,
                    current_type,
                    symbols,
                    methods,
                    env,
                    diagnostics,
                    facts,
                    external_facts,
                    enclosing_member,
                );
            }
            let receiver_type = receiver.as_deref().map(|receiver| {
                infer_resolver_expr_type_with_facts(
                    receiver,
                    current_type,
                    symbols,
                    env,
                    external_facts,
                )
            });
            let defer_stream_lambda_arg =
                should_defer_stream_lambda_arg_resolution(receiver_type.as_ref(), name, args);
            for (arg_index, arg) in args.iter_mut().enumerate() {
                if defer_stream_lambda_arg && arg_index == 0 {
                    continue;
                }
                resolve_expr_calls(
                    arg,
                    current_type,
                    symbols,
                    methods,
                    env,
                    diagnostics,
                    facts,
                    external_facts,
                    enclosing_member,
                );
            }

            resolve_stream_map_lambda_arg(
                receiver_type.as_ref(),
                name,
                args,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );

            let arg_types = args
                .iter()
                .map(|arg| {
                    infer_resolver_expr_type_with_facts(
                        arg,
                        current_type,
                        symbols,
                        env,
                        external_facts,
                    )
                })
                .collect::<Vec<_>>();
            if receiver.is_none()
                || receiver
                    .as_deref()
                    .is_some_and(|expr| matches!(expr, Expr::This))
            {
                if receiver.is_none()
                    && let Some(enum_call) =
                        source_enum_synthetic_call(current_type, name, arg_types.len(), symbols)
                {
                    *resolved = Some(enum_call);
                    return;
                }
                let unqualified_methods = if receiver.is_none() {
                    symbols.methods_for_unqualified_call(current_type)
                } else {
                    methods.to_vec()
                };
                match resolve_same_class_call_with_type_args(
                    name,
                    &arg_types,
                    &unqualified_methods,
                    symbols,
                    external_facts,
                    None,
                    env,
                    type_args,
                ) {
                    Ok(Some(selection)) => {
                        resolve_target_typed_lambda_args(
                            args,
                            &selection.params,
                            current_type,
                            symbols,
                            methods,
                            env,
                            diagnostics,
                            facts,
                            external_facts,
                            enclosing_member,
                        );
                        *resolved = Some(selection.call);
                    }
                    Ok(None) => facts.unresolved_target_facts.push(UnresolvedTargetFact {
                        enclosing_type: current_type.to_string(),
                        enclosing_member: enclosing_member.map(str::to_string),
                        kind: UnresolvedTargetKind::MethodCall,
                        target_name: name.clone(),
                        receiver_type: if receiver.is_none()
                            && crate::stdlib::is_known_receiverless_static_utility_method(
                                name,
                                arg_types.len(),
                            ) {
                            None
                        } else {
                            Some(JavaType::Named {
                                name: current_type.to_string(),
                                args: Vec::new(),
                            })
                        },
                        arg_types,
                        is_anonymous_class: false,
                        provenance: FactProvenance::ParsedSource,
                    }),
                    Err(message) => diagnostics.error("resolve_call_ambiguous", message),
                }
            } else if let Some(receiver_ty) = receiver.as_deref().and_then(|receiver| {
                source_static_type_receiver(receiver, current_type, symbols, env)
            }) {
                let receiver_methods = symbols
                    .methods_for_hierarchy(&receiver_ty)
                    .into_iter()
                    .filter(|method| method.is_static)
                    .collect::<Vec<_>>();
                match resolve_same_class_call_with_type_args(
                    name,
                    &arg_types,
                    &receiver_methods,
                    symbols,
                    external_facts,
                    None,
                    env,
                    type_args,
                ) {
                    Ok(Some(selection)) => {
                        resolve_target_typed_lambda_args(
                            args,
                            &selection.params,
                            current_type,
                            symbols,
                            methods,
                            env,
                            diagnostics,
                            facts,
                            external_facts,
                            enclosing_member,
                        );
                        *resolved = Some(selection.call);
                    }
                    Ok(None) => facts.unresolved_target_facts.push(UnresolvedTargetFact {
                        enclosing_type: current_type.to_string(),
                        enclosing_member: enclosing_member.map(str::to_string),
                        kind: UnresolvedTargetKind::MethodCall,
                        target_name: name.clone(),
                        receiver_type: Some(JavaType::Named {
                            name: receiver_ty,
                            args: Vec::new(),
                        }),
                        arg_types,
                        is_anonymous_class: false,
                        provenance: FactProvenance::ParsedSource,
                    }),
                    Err(message) => diagnostics.error("resolve_call_ambiguous", message),
                }
            } else if let Some((receiver_ty, receiver_actual_ty)) =
                receiver.as_deref().and_then(|receiver| {
                    source_receiver_type_for_method(
                        receiver,
                        name,
                        arg_types.len(),
                        current_type,
                        symbols,
                        env,
                    )
                })
            {
                let receiver_methods = symbols.methods_for_hierarchy(&receiver_ty);
                match resolve_same_class_call_with_type_args(
                    name,
                    &arg_types,
                    &receiver_methods,
                    symbols,
                    external_facts,
                    Some(&receiver_actual_ty),
                    env,
                    type_args,
                ) {
                    Ok(Some(selection)) => {
                        resolve_target_typed_lambda_args(
                            args,
                            &selection.params,
                            current_type,
                            symbols,
                            methods,
                            env,
                            diagnostics,
                            facts,
                            external_facts,
                            enclosing_member,
                        );
                        *resolved = Some(selection.call);
                    }
                    Ok(None) => facts.unresolved_target_facts.push(UnresolvedTargetFact {
                        enclosing_type: current_type.to_string(),
                        enclosing_member: enclosing_member.map(str::to_string),
                        kind: UnresolvedTargetKind::MethodCall,
                        target_name: name.clone(),
                        receiver_type: Some(JavaType::Named {
                            name: receiver_ty,
                            args: Vec::new(),
                        }),
                        arg_types,
                        is_anonymous_class: false,
                        provenance: FactProvenance::ParsedSource,
                    }),
                    Err(message) => diagnostics.error("resolve_call_ambiguous", message),
                }
            } else if let Some(identity_call) = receiver.as_deref().and_then(|receiver| {
                let receiver_ty = infer_resolver_expr_type_with_facts(
                    receiver,
                    current_type,
                    symbols,
                    env,
                    external_facts,
                );
                let owner = match &receiver_ty {
                    JavaType::Named { name, .. } | JavaType::Unknown(name) => name,
                    _ => return None,
                };
                if symbols.source_type_kind(owner).is_some() {
                    return None;
                }
                let target = crate::call_plan::plan_native_java_static_owner_call(
                    owner,
                    name,
                    Some(args),
                    env,
                )?;
                let index = target.checked_identity_argument_index()?;
                Some(ResolvedCall {
                    type_args: Vec::new(),
                    declaring_type: crate::stdlib::canonical_external_receiver_type_key(owner),
                    rust_name: crate::parser::to_snake_case(name),
                    kind: CallKind::Static,
                    return_type: arg_types.get(index)?.clone(),
                    // This generic identity boundary preserves its argument's
                    // carrier until all arguments have been evaluated. A
                    // non-null formal ABI would unwrap it too early.
                    arg_target_types: Vec::new(),
                    arg_coercions: Vec::new(),
                })
            }) {
                *resolved = Some(identity_call);
            } else if let Some(enum_call) = receiver.as_deref().and_then(|receiver| {
                external_enum_synthetic_call(
                    receiver,
                    name,
                    arg_types.len(),
                    current_type,
                    symbols,
                    env,
                    external_facts,
                )
            }) {
                *resolved = Some(enum_call);
            } else {
                let receiver_type = receiver.as_deref().map(|receiver| {
                    unresolved_receiver_type(receiver, current_type, symbols, env, external_facts)
                });
                facts.unresolved_target_facts.push(UnresolvedTargetFact {
                    enclosing_type: current_type.to_string(),
                    enclosing_member: enclosing_member.map(str::to_string),
                    kind: UnresolvedTargetKind::MethodCall,
                    target_name: name.clone(),
                    receiver_type,
                    arg_types,
                    is_anonymous_class: false,
                    provenance: FactProvenance::ParsedSource,
                });
            }
        }
        Expr::New {
            ty,
            infer_type_arguments,
            qualifier,
            args,
            body,
            resolved,
        } => {
            if let Some(qualifier) = qualifier {
                resolve_expr_calls(
                    qualifier,
                    current_type,
                    symbols,
                    methods,
                    env,
                    diagnostics,
                    facts,
                    external_facts,
                    enclosing_member,
                );
            }
            for arg in args.iter_mut() {
                resolve_expr_calls(
                    arg,
                    current_type,
                    symbols,
                    methods,
                    env,
                    diagnostics,
                    facts,
                    external_facts,
                    enclosing_member,
                );
            }
            if let JavaType::Named { name, .. } | JavaType::Unknown(name) = &*ty {
                let arg_types = args
                    .iter()
                    .map(|arg| {
                        infer_resolver_expr_type_with_facts(
                            arg,
                            current_type,
                            symbols,
                            env,
                            external_facts,
                        )
                    })
                    .collect::<Vec<_>>();
                let constructor_candidates = symbols.constructors_for_new(name, current_type);
                let selection = if body.is_some() {
                    resolve_anonymous_class_constructor(
                        name,
                        &arg_types,
                        &constructor_candidates,
                        symbols,
                        external_facts,
                        Some(ty),
                        env,
                    )
                } else {
                    resolve_same_class_constructor(
                        &arg_types,
                        &constructor_candidates,
                        symbols,
                        external_facts,
                        Some(ty),
                        env,
                    )
                };
                match selection {
                    Ok(Some(mut constructor)) => {
                        constructor.constructed_type = if *infer_type_arguments {
                            diamond_constructed_type_from_arguments(
                                &constructor,
                                &arg_types,
                                symbols,
                                external_facts,
                            )
                        } else {
                            None
                        }
                        .unwrap_or_else(|| {
                            inferred_constructed_type(
                                ty,
                                None,
                                &constructor.declaring_type,
                                symbols,
                                external_facts,
                            )
                        });
                        refresh_constructor_derived_fields(
                            &mut constructor,
                            Some(&arg_types),
                            symbols,
                            external_facts,
                        );
                        apply_constructor_param_targets(
                            args,
                            &constructor,
                            symbols,
                            external_facts,
                        );
                        resolve_target_typed_lambda_args(
                            args,
                            &constructor.params,
                            current_type,
                            symbols,
                            methods,
                            env,
                            diagnostics,
                            facts,
                            external_facts,
                            enclosing_member,
                        );
                        *resolved = Some(constructor);
                    }
                    Ok(None) => facts.unresolved_target_facts.push(UnresolvedTargetFact {
                        enclosing_type: current_type.to_string(),
                        enclosing_member: enclosing_member.map(str::to_string),
                        kind: UnresolvedTargetKind::ConstructorCall,
                        target_name: name.clone(),
                        receiver_type: Some(ty.clone()),
                        arg_types,
                        is_anonymous_class: body.is_some(),
                        provenance: FactProvenance::ParsedSource,
                    }),
                    Err(message) => diagnostics.error(
                        "resolve_constructor_ambiguous",
                        format!("constructor {name}: {message}"),
                    ),
                }
            }
            if let Some(members) = body {
                let mut anonymous_env = env.clone();
                let mut fields = match &*ty {
                    JavaType::Named { name, .. } | JavaType::Unknown(name) => symbols
                        .fields_for_hierarchy(name)
                        .into_iter()
                        .map(|field| {
                            let ty = symbols.field_type_for_receiver(&field, ty);
                            (field.java_name, ty)
                        })
                        .collect::<Vec<_>>(),
                    _ => Vec::new(),
                };
                fields.extend(members.iter().filter_map(|member| match member {
                    Member::Field(field) => Some((field.name.clone(), field.ty.clone())),
                    _ => None,
                }));
                anonymous_env.enter_anonymous_field_scope(fields);
                for member in members {
                    match member {
                        Member::Field(field) => {
                            if let Some(initializer) = &mut field.initializer {
                                resolve_expr_calls(
                                    initializer,
                                    current_type,
                                    symbols,
                                    methods,
                                    &mut anonymous_env,
                                    diagnostics,
                                    facts,
                                    external_facts,
                                    Some(&field.name),
                                );
                                resolve_lambda_with_expected_type(
                                    initializer,
                                    &field.ty,
                                    current_type,
                                    symbols,
                                    methods,
                                    &anonymous_env,
                                    diagnostics,
                                    facts,
                                    external_facts,
                                    Some(&field.name),
                                );
                            }
                        }
                        Member::Method(method) => {
                            if let Some(body) = &mut method.body {
                                let mut body_env = anonymous_env.clone();
                                for param in &method.params {
                                    body_env.insert_local(param.name.clone(), param.ty.clone());
                                    body_env
                                        .insert_local(param.source_name.clone(), param.ty.clone());
                                }
                                resolve_block_calls(
                                    body,
                                    current_type,
                                    symbols,
                                    methods,
                                    &[],
                                    &mut body_env,
                                    diagnostics,
                                    facts,
                                    external_facts,
                                    Some(&method.name),
                                    Some(&method.return_type),
                                );
                            }
                        }
                        Member::Constructor(ctor) => {
                            if let Some(body) = &mut ctor.body {
                                let mut body_env = anonymous_env.clone();
                                for param in &ctor.params {
                                    body_env.insert_local(param.name.clone(), param.ty.clone());
                                    body_env
                                        .insert_local(param.source_name.clone(), param.ty.clone());
                                }
                                resolve_block_calls(
                                    body,
                                    current_type,
                                    symbols,
                                    methods,
                                    &[],
                                    &mut body_env,
                                    diagnostics,
                                    facts,
                                    external_facts,
                                    Some(&ctor.name),
                                    None,
                                );
                            }
                        }
                        Member::Type(ty) => {
                            // A local type declared inside a body: only its own
                            // members are in play, so its counts stand alone.
                            let mut local_counts = ProgramMethodCounts::new();
                            collect_program_method_counts(ty, &mut local_counts);
                            assign_type_rust_names(ty, diagnostics, &local_counts);
                        }
                        _ => {}
                    }
                }
            }
        }
        Expr::Cast { expr, .. } => resolve_expr_calls(
            expr,
            current_type,
            symbols,
            methods,
            env,
            diagnostics,
            facts,
            external_facts,
            enclosing_member,
        ),
        Expr::InstanceOf { expr, .. } => resolve_expr_calls(
            expr,
            current_type,
            symbols,
            methods,
            env,
            diagnostics,
            facts,
            external_facts,
            enclosing_member,
        ),
        Expr::Lambda { .. } => {}
        _ => {}
    }
}

fn should_defer_stream_lambda_arg_resolution(
    receiver_type: Option<&JavaType>,
    name: &str,
    args: &[Expr],
) -> bool {
    if !is_stream_lambda_arg_name(name) || args.len() != 1 {
        return false;
    }
    matches!(
        receiver_type,
        Some(JavaType::Named {
            name: receiver_name,
            ..
        }) if receiver_name == "Stream"
    ) && matches!(args.first(), Some(Expr::Lambda { .. }))
}

#[allow(clippy::too_many_arguments)]
fn resolve_stream_map_lambda_arg(
    receiver_type: Option<&JavaType>,
    name: &str,
    args: &mut [Expr],
    current_type: &str,
    symbols: &ProgramSymbols,
    methods: &[MethodSymbol],
    env: &TypeEnv,
    diagnostics: &mut DiagnosticBag,
    facts: &mut FactSet,
    external_facts: &ResolverExternalFacts<'_>,
    enclosing_member: Option<&str>,
) {
    if !is_stream_lambda_arg_name(name) || args.len() != 1 {
        return;
    }
    let Some(JavaType::Named {
        name: receiver_name,
        args: receiver_args,
    }) = receiver_type
    else {
        return;
    };
    if receiver_name != "Stream" {
        return;
    }
    let Some(stream_element) = receiver_args.first() else {
        return;
    };
    resolve_lambda_with_single_param_type(
        &mut args[0],
        stream_element,
        current_type,
        symbols,
        methods,
        env,
        diagnostics,
        facts,
        external_facts,
        enclosing_member,
    );
}

fn is_stream_lambda_mapper_name(name: &str) -> bool {
    matches!(name, "map" | "mapToInt" | "mapToLong" | "mapToDouble")
}

fn is_stream_lambda_arg_name(name: &str) -> bool {
    is_stream_lambda_mapper_name(name) || name == "flatMap"
}

#[allow(clippy::too_many_arguments)]
fn resolve_target_typed_lambda_args(
    args: &mut [Expr],
    params: &[Param],
    current_type: &str,
    symbols: &ProgramSymbols,
    methods: &[MethodSymbol],
    env: &TypeEnv,
    diagnostics: &mut DiagnosticBag,
    facts: &mut FactSet,
    external_facts: &ResolverExternalFacts<'_>,
    enclosing_member: Option<&str>,
) {
    for (arg, param) in args.iter_mut().zip(params) {
        resolve_lambda_with_expected_type(
            arg,
            &param.ty,
            current_type,
            symbols,
            methods,
            env,
            diagnostics,
            facts,
            external_facts,
            enclosing_member,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_lambda_with_single_param_type(
    expr: &mut Expr,
    expected_param_ty: &JavaType,
    current_type: &str,
    symbols: &ProgramSymbols,
    methods: &[MethodSymbol],
    env: &TypeEnv,
    diagnostics: &mut DiagnosticBag,
    facts: &mut FactSet,
    external_facts: &ResolverExternalFacts<'_>,
    enclosing_member: Option<&str>,
) {
    let Expr::Lambda { params, body } = expr else {
        return;
    };

    let mut lambda_env = env.clone();
    if let Some(lambda_param) = params.first() {
        let param_ty = lambda_param
            .ty
            .clone()
            .unwrap_or_else(|| expected_param_ty.clone());
        lambda_env.insert_local(lambda_param.name.clone(), param_ty);
    }

    match body {
        LambdaBody::Expr(expr) => resolve_expr_calls(
            expr,
            current_type,
            symbols,
            methods,
            &lambda_env,
            diagnostics,
            facts,
            external_facts,
            enclosing_member,
        ),
        LambdaBody::Block(block) => resolve_block_calls(
            block,
            current_type,
            symbols,
            methods,
            &[],
            &mut lambda_env,
            diagnostics,
            facts,
            external_facts,
            enclosing_member,
            None,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_lambda_with_expected_type(
    expr: &mut Expr,
    expected_ty: &JavaType,
    current_type: &str,
    symbols: &ProgramSymbols,
    methods: &[MethodSymbol],
    env: &TypeEnv,
    diagnostics: &mut DiagnosticBag,
    facts: &mut FactSet,
    external_facts: &ResolverExternalFacts<'_>,
    enclosing_member: Option<&str>,
) {
    let Expr::Lambda { params, body } = expr else {
        return;
    };
    let (expected_params, expected_return) = if let Some(functional_method) =
        symbols.source_functional_interface_method(expected_ty)
    {
        let substitutions = functional_interface_type_substitutions(expected_ty, symbols);
        (
            functional_method
                .params
                .iter()
                .map(|param| substitute_method_type_variables(&param.ty, &substitutions))
                .collect::<Vec<_>>(),
            substitute_method_type_variables(&functional_method.return_type, &substitutions),
        )
    } else {
        let JavaType::Named { name, args } = expected_ty else {
            return;
        };
        // A source declaration, including a non-SAM lookalike, always owns its
        // contract. Native SAM signatures reuse the same canonical model as
        // lowering rather than leaving their lambda bodies unresolved.
        if symbols.source_type_kind(name).is_some() {
            return;
        }
        let Some(sam) = crate::effective_facts::EffectiveFacts::new(external_facts)
            .native_functional_carrier(expected_ty)
        else {
            return;
        };
        let (Some(inputs), Some(result)) = (sam.input_types(args), sam.return_type(args)) else {
            return;
        };
        (inputs, result)
    };

    let mut lambda_env = env.clone();
    for (lambda_param, method_param_ty) in params.iter().zip(expected_params) {
        let param_ty = lambda_param.ty.clone().unwrap_or(method_param_ty);
        lambda_env.insert_local(lambda_param.name.clone(), param_ty);
    }

    match body {
        LambdaBody::Expr(expr) => {
            resolve_expr_calls(
                expr,
                current_type,
                symbols,
                methods,
                &lambda_env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
            apply_target_type_to_constructor_expr(expr, &expected_return, symbols, external_facts);
        }
        LambdaBody::Block(block) => resolve_block_calls(
            block,
            current_type,
            symbols,
            methods,
            &[],
            &mut lambda_env,
            diagnostics,
            facts,
            external_facts,
            enclosing_member,
            Some(&expected_return),
        ),
    }
}

fn record_unresolved_field_access(
    expr: &Expr,
    current_type: &str,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
    facts: &mut FactSet,
    external_facts: &ResolverExternalFacts<'_>,
    enclosing_member: Option<&str>,
) {
    let Expr::FieldAccess(receiver, field) = expr else {
        return;
    };
    if infer_resolver_member_access_type_with_facts(
        receiver,
        field,
        current_type,
        symbols,
        env,
        external_facts,
    )
    .is_some()
    {
        return;
    }
    if matches!(receiver.as_ref(), Expr::This) && env.field_type(field).is_some() {
        return;
    }
    if qualified_this_type_name(receiver, field, symbols).is_some() {
        return;
    }
    if let Some(receiver_type_name) = field_receiver_type_name(receiver, current_type, symbols, env)
        && symbols
            .field_for_from(&receiver_type_name, field, current_type)
            .is_some_and(|symbol| field_access_is_allowed(receiver, symbol, symbols, env))
    {
        return;
    }
    if let Some(receiver_type_name) = field_receiver_type_name(receiver, current_type, symbols, env)
        && symbols
            .external_super_field_type_for(&receiver_type_name, field)
            .is_some()
    {
        return;
    }
    if type_qualified_static_field(receiver, field, current_type, symbols, env).is_some() {
        return;
    }
    if is_package_qualified_type_fragment(expr, env) {
        return;
    }
    facts.unresolved_target_facts.push(UnresolvedTargetFact {
        enclosing_type: current_type.to_string(),
        enclosing_member: enclosing_member.map(str::to_string),
        kind: UnresolvedTargetKind::FieldAccess,
        target_name: field.clone(),
        receiver_type: Some(unresolved_receiver_type(
            receiver,
            current_type,
            symbols,
            env,
            external_facts,
        )),
        arg_types: Vec::new(),
        is_anonymous_class: false,
        provenance: FactProvenance::ParsedSource,
    });
}

fn is_package_qualified_type_fragment(expr: &Expr, env: &TypeEnv) -> bool {
    let Some(segments) = field_access_segments(expr) else {
        return false;
    };
    let [root, rest @ ..] = segments.as_slice() else {
        return false;
    };
    if rest.is_empty() || env.local_type(root).is_some() || env.field_type(root).is_some() {
        return false;
    }
    let Some(last) = segments.last() else {
        return false;
    };
    let prefix = &segments[..segments.len() - 1];
    if segments.iter().all(|segment| is_package_segment(segment)) {
        return true;
    }
    segments.len() >= 3
        && prefix.iter().all(|segment| is_package_segment(segment))
        && starts_like_type_name(last)
}

fn field_access_segments(expr: &Expr) -> Option<Vec<String>> {
    match expr {
        Expr::Var(name) => Some(vec![name.clone()]),
        Expr::FieldAccess(receiver, field) => {
            let mut segments = field_access_segments(receiver)?;
            segments.push(field.clone());
            Some(segments)
        }
        _ => None,
    }
}

fn is_package_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}

fn starts_like_type_name(segment: &str) -> bool {
    segment
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
}

fn field_receiver_type_name(
    receiver: &Expr,
    current_type: &str,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
) -> Option<String> {
    match receiver {
        Expr::This | Expr::Super => Some(current_type.to_string()),
        Expr::FieldAccess(inner_receiver, field_name)
            if qualified_this_type_name(inner_receiver, field_name, symbols).is_some() =>
        {
            qualified_this_type_name(inner_receiver, field_name, symbols)
        }
        Expr::Var(name) => env.local_type(name).map(java_type_simple_name).or_else(|| {
            env.field_type(name)
                .map(java_type_simple_name)
                .or_else(|| Some(name.clone()))
        }),
        Expr::FieldAccess(inner_receiver, field_name) => {
            if let Some(field) =
                type_qualified_static_field(inner_receiver, field_name, current_type, symbols, env)
            {
                return Some(java_type_simple_name(&field.ty));
            }
            let owner_type = field_receiver_type_name(inner_receiver, current_type, symbols, env)?;
            symbols
                .field_for_from(&owner_type, field_name, current_type)
                .map(|field| java_type_simple_name(&field.ty))
                .or_else(|| {
                    Some(java_type_simple_name(&infer_resolver_expr_type(
                        receiver,
                        current_type,
                        symbols,
                        env,
                    )))
                })
        }
        other => Some(java_type_simple_name(&infer_resolver_expr_type(
            other,
            current_type,
            symbols,
            env,
        ))),
    }
}

fn qualified_this_type_name(
    receiver: &Expr,
    field_name: &str,
    symbols: &ProgramSymbols,
) -> Option<String> {
    if field_name != "this" {
        return None;
    }
    let Expr::Var(name) = receiver else {
        return None;
    };
    let source_name = symbols.source_type_name(name);
    if source_name == name && !symbols.fields_by_type.contains_key(source_name) {
        None
    } else {
        Some(source_name.to_string())
    }
}

fn field_access_is_allowed(
    receiver: &Expr,
    symbol: &FieldSymbol,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
) -> bool {
    match receiver {
        Expr::Var(name)
            if env.local_type(name).is_none()
                && env.field_type(name).is_none()
                && symbols.source_type_name(name) == symbol.declaring_type =>
        {
            symbol.is_static
        }
        _ => true,
    }
}

fn type_qualified_static_field<'a>(
    receiver: &Expr,
    field_name: &str,
    current_type: &str,
    symbols: &'a ProgramSymbols,
    env: &TypeEnv,
) -> Option<&'a FieldSymbol> {
    let Expr::Var(name) = receiver else {
        return None;
    };
    if env.local_type(name).is_some() || env.field_type_exact(name).is_some() {
        return None;
    }
    let source_name = symbols
        .lexical_source_type_name(name, current_type)
        .unwrap_or_else(|| symbols.source_type_name(name).to_string());
    symbols.static_field_for_from(&source_name, field_name, current_type)
}

fn java_type_simple_name(java_ty: &JavaType) -> String {
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name.clone(),
        JavaType::String => "String".to_string(),
        JavaType::Array(_) => "Array".to_string(),
        JavaType::Primitive(primitive) => format!("{primitive:?}"),
        JavaType::Void => "void".to_string(),
        JavaType::Wildcard(_) => "wildcard".to_string(),
        JavaType::Intersection(bounds) => bounds
            .first()
            .map(java_type_simple_name)
            .unwrap_or_else(|| "Object".to_string()),
    }
}

fn nested_simple_type_name(type_name: &str) -> &str {
    type_name.rsplit('.').next().unwrap_or(type_name)
}

fn unresolved_receiver_type(
    receiver: &Expr,
    current_type: &str,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
    external_facts: &ResolverExternalFacts<'_>,
) -> JavaType {
    match receiver {
        Expr::This | Expr::Super => JavaType::Named {
            name: current_type.to_string(),
            args: Vec::new(),
        },
        Expr::FieldAccess(inner_receiver, field_name)
            if qualified_this_type_name(inner_receiver, field_name, symbols).is_some() =>
        {
            JavaType::Named {
                name: qualified_this_type_name(inner_receiver, field_name, symbols).unwrap(),
                args: Vec::new(),
            }
        }
        other => {
            infer_resolver_expr_type_with_facts(other, current_type, symbols, env, external_facts)
        }
    }
}

fn functional_interface_type_substitutions(
    expected_ty: &JavaType,
    symbols: &ProgramSymbols,
) -> HashMap<String, JavaType> {
    let JavaType::Named { name, args } = expected_ty else {
        return HashMap::new();
    };
    symbols
        .type_params_for(name)
        .unwrap_or(&[])
        .iter()
        .zip(args)
        .map(|(param, arg)| (param.clone(), arg.clone()))
        .collect()
}

/// Share the resolver's field/hierarchy/generic lookup with semantic consumers
/// that already own the immutable source symbol table. A local TypeEnv alone
/// does not know the fields of arbitrary source-typed receivers.
pub(crate) fn infer_source_expr_type_with_facts(
    expr: &Expr,
    current_type: &str,
    source_symbols: &SourceSymbols,
    env: &TypeEnv,
    facts: &FactSet,
) -> JavaType {
    infer_resolver_expr_type_with_facts(
        expr,
        current_type,
        &source_symbols.symbols,
        env,
        &ResolverExternalFacts::new(facts),
    )
}

fn infer_resolver_expr_type(
    expr: &Expr,
    current_type: &str,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
) -> JavaType {
    let facts = FactSet::default();
    let external_facts = ResolverExternalFacts::new(&facts);
    infer_resolver_expr_type_with_facts(expr, current_type, symbols, env, &external_facts)
}

fn infer_resolver_expr_type_with_facts(
    expr: &Expr,
    current_type: &str,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
    external_facts: &ResolverExternalFacts<'_>,
) -> JavaType {
    match expr {
        Expr::Call {
            resolved: Some(call),
            ..
        } => call.return_type.clone(),
        Expr::Call {
            receiver,
            name,
            args,
            resolved: None,
            ..
        } => {
            let receiver_type = receiver.as_deref().map(|receiver| {
                infer_resolver_expr_type_with_facts(
                    receiver,
                    current_type,
                    symbols,
                    env,
                    external_facts,
                )
            });
            let receiver_owner = receiver_type.as_ref().and_then(|ty| match ty {
                JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name.as_str()),
                _ => None,
            });
            if let Some(owner) = receiver_owner
                && symbols.source_type_kind(owner).is_none()
                && let Some(target) = crate::call_plan::plan_native_java_static_owner_call(
                    owner,
                    name,
                    Some(args),
                    env,
                )
            {
                if let Some(index) = target.checked_identity_argument_index()
                    && let Some(value) = args.get(index)
                {
                    // Java-level type inference preserves the entire declared
                    // type. Carrier nullability is projected later by shape
                    // inference; a real Optional<T> must not become T here.
                    return infer_resolver_expr_type_with_facts(
                        value,
                        current_type,
                        symbols,
                        env,
                        external_facts,
                    );
                }
                if let Some(result) = crate::call_plan::native_java_call_lowered_type(&target) {
                    return result.into_slot_shape().java_ty;
                }
            }
            if name == "nonNull"
                && !args.is_empty()
                && (receiver.is_none()
                    || receiver_owner.is_some_and(|owner| {
                        symbols.source_type_kind(owner).is_none()
                            && crate::javalibs::imported_static_method_returns_first_arg(
                                owner,
                                name,
                                args.len(),
                            )
                    }))
            {
                return infer_resolver_expr_type_with_facts(
                    &args[0],
                    current_type,
                    symbols,
                    env,
                    external_facts,
                );
            }
            if let Some(java_ty) = infer_stream_map_return_type(
                receiver_type.as_ref(),
                name,
                args,
                current_type,
                symbols,
                env,
                external_facts,
            ) {
                return java_ty;
            }
            if let Some(java_ty) = infer_stream_method_reference_map_return_type(
                receiver_type.as_ref(),
                name,
                args,
                current_type,
                symbols,
                env,
                external_facts,
            ) {
                return java_ty;
            }
            if let Some(java_ty) = infer_stream_lambda_flat_map_return_type(
                receiver_type.as_ref(),
                name,
                args,
                current_type,
                symbols,
                env,
                external_facts,
            ) {
                return java_ty;
            }
            if let Some(java_ty) = infer_stream_method_reference_flat_map_return_type(
                receiver_type.as_ref(),
                name,
                args,
            ) {
                return java_ty;
            }
            if let Some(java_ty) =
                infer_stream_to_array_generator_return_type(receiver_type.as_ref(), name, args)
            {
                return java_ty;
            }
            let arg_types = args
                .iter()
                .map(|arg| {
                    infer_resolver_expr_type_with_facts(
                        arg,
                        current_type,
                        symbols,
                        env,
                        external_facts,
                    )
                })
                .collect::<Vec<_>>();
            if receiver.is_none()
                && let Some(enum_call) =
                    source_enum_synthetic_call(current_type, name, arg_types.len(), symbols)
            {
                return enum_call.return_type;
            }
            if let Some(enum_call) = receiver.as_deref().and_then(|receiver| {
                external_enum_synthetic_call(
                    receiver,
                    name,
                    arg_types.len(),
                    current_type,
                    symbols,
                    env,
                    external_facts,
                )
            }) {
                return enum_call.return_type;
            }
            if receiver.is_none()
                || receiver
                    .as_deref()
                    .is_some_and(|expr| matches!(expr, Expr::This))
            {
                let methods = if receiver.is_none() {
                    symbols.methods_for_unqualified_call(current_type)
                } else {
                    symbols.methods_for_hierarchy(current_type)
                };
                if let Ok(Some(selection)) = resolve_same_class_call(
                    name,
                    &arg_types,
                    &methods,
                    symbols,
                    external_facts,
                    None,
                    env,
                ) {
                    return selection.call.return_type;
                }
            }
            if let Some((receiver_ty, receiver_actual_ty)) =
                receiver.as_deref().and_then(|receiver| {
                    source_receiver_type_for_method(
                        receiver,
                        name,
                        arg_types.len(),
                        current_type,
                        symbols,
                        env,
                    )
                })
            {
                let receiver_methods = symbols.methods_for_hierarchy(&receiver_ty);
                if let Ok(Some(selection)) = resolve_same_class_call(
                    name,
                    &arg_types,
                    &receiver_methods,
                    symbols,
                    external_facts,
                    Some(&receiver_actual_ty),
                    env,
                ) {
                    return selection.call.return_type;
                }
            }
            if let Some(java_ty) = known_resolver_external_method_return_type_for_call(
                receiver_type.as_ref(),
                name,
                &arg_types,
                symbols,
            ) {
                return java_ty;
            }
            if let Some(java_ty) = external_fact_method_return_type_for_call(
                receiver_type.as_ref(),
                name,
                &arg_types,
                external_facts,
            ) {
                return java_ty;
            }
            crate::stdlib::known_external_method_return_type_for_call(
                receiver_type.as_ref(),
                name,
                &arg_types,
            )
            .unwrap_or_else(|| JavaType::Unknown(name.clone()))
        }
        Expr::This | Expr::Super => JavaType::Named {
            name: current_type.to_string(),
            args: Vec::new(),
        },
        Expr::FieldAccess(receiver, field) => {
            if let Some(java_ty) = infer_resolver_member_access_type_with_facts(
                receiver,
                field,
                current_type,
                symbols,
                env,
                external_facts,
            ) {
                return java_ty;
            }
            if let Some(type_name) = qualified_this_type_name(receiver, field, symbols) {
                return JavaType::Named {
                    name: type_name,
                    args: Vec::new(),
                };
            }
            if let Some(symbol) =
                type_qualified_static_field(receiver, field, current_type, symbols, env)
            {
                return symbol.ty.clone();
            }
            if let Some(receiver_type_name) =
                field_receiver_type_name(receiver, current_type, symbols, env)
                && let Some(symbol) =
                    symbols.field_for_from(&receiver_type_name, field, current_type)
                && field_access_is_allowed(receiver, symbol, symbols, env)
            {
                let actual = infer_resolver_expr_type_with_facts(
                    receiver,
                    current_type,
                    symbols,
                    env,
                    external_facts,
                );
                return symbols.field_type_for_receiver(symbol, &actual);
            }
            if let Some(receiver_type_name) =
                field_receiver_type_name(receiver, current_type, symbols, env)
                && let Some(java_ty) =
                    symbols.external_super_field_type_for(&receiver_type_name, field)
            {
                return java_ty;
            }
            infer_expr_type(expr, env)
        }
        Expr::ArrayAccess(array, _) => crate::types::sequence_element_type(
            &infer_resolver_expr_type_with_facts(array, current_type, symbols, env, external_facts),
        )
        .unwrap_or_else(|| infer_expr_type(expr, env)),
        Expr::Ternary {
            then_expr,
            else_expr,
            ..
        } => resolver_common_type(
            &infer_resolver_expr_type_with_facts(
                then_expr,
                current_type,
                symbols,
                env,
                external_facts,
            ),
            &infer_resolver_expr_type_with_facts(
                else_expr,
                current_type,
                symbols,
                env,
                external_facts,
            ),
            symbols,
            external_facts,
        ),
        Expr::New {
            resolved: Some(resolved),
            ..
        } => resolved.constructed_type.clone(),
        Expr::New { ty, .. } | Expr::Cast { ty, .. } => ty.clone(),
        Expr::ClassLiteral(ty) => crate::types::class_literal_type(ty),
        other => infer_expr_type(other, env),
    }
}

fn source_enum_synthetic_call(
    current_type: &str,
    name: &str,
    arg_count: usize,
    symbols: &ProgramSymbols,
) -> Option<ResolvedCall> {
    if !symbols.is_source_enum_type(current_type) {
        return None;
    }
    let return_type = match (name, arg_count) {
        ("values", 0) => JavaType::Array(Box::new(JavaType::Named {
            name: current_type.to_string(),
            args: Vec::new(),
        })),
        _ => return None,
    };
    Some(ResolvedCall {
        type_args: Vec::new(),
        declaring_type: current_type.to_string(),
        rust_name: crate::parser::to_snake_case(name),
        kind: CallKind::Static,
        return_type,
        arg_target_types: Vec::new(),
        arg_coercions: Vec::new(),
    })
}

fn external_enum_synthetic_call(
    receiver: &Expr,
    name: &str,
    arg_count: usize,
    current_type: &str,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
    external_facts: &ResolverExternalFacts<'_>,
) -> Option<ResolvedCall> {
    let receiver_ty =
        infer_resolver_expr_type_with_facts(receiver, current_type, symbols, env, external_facts);
    let (JavaType::Named {
        name: receiver_name,
        ..
    }
    | JavaType::Unknown(receiver_name)) = receiver_ty
    else {
        return None;
    };
    let return_type = crate::stdlib::known_external_enum_synthetic_call_return_type(
        &receiver_name,
        name,
        arg_count,
    )?;
    Some(ResolvedCall {
        type_args: Vec::new(),
        declaring_type: crate::stdlib::canonical_external_receiver_type_key(&receiver_name),
        rust_name: crate::parser::to_snake_case(name),
        kind: CallKind::Static,
        return_type,
        arg_target_types: if name == "valueOf" && arg_count == 1 {
            vec![JavaType::String]
        } else {
            Vec::new()
        },
        arg_coercions: Vec::new(),
    })
}

fn source_receiver_type(
    receiver: &Expr,
    current_type: &str,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
) -> Option<(String, JavaType)> {
    if matches!(receiver, Expr::Super) {
        let current_source = symbols.source_type_name(current_type);
        let super_ty = symbols.extends_type_by_type.get(current_source)?.clone();
        let super_name = symbols
            .source_type_name(&java_type_simple_name(&super_ty))
            .to_string();
        return symbols
            .methods_by_type
            .contains_key(&super_name)
            .then_some((super_name, super_ty));
    }
    let receiver_actual_ty = infer_resolver_expr_type(receiver, current_type, symbols, env);
    if let JavaType::Named { name, args } = &receiver_actual_ty
        && args.is_empty()
        && let Some(bound) = env
            .type_param_bound(name)
            .cloned()
            .or_else(|| symbols.type_param_bound_for(current_type, name))
        && let Some(bound_name) = primary_named_intersection_bound(&bound)
    {
        return Some((symbols.source_type_name(bound_name).to_string(), bound));
    }
    source_receiver_type_from_actual(receiver_actual_ty, symbols)
}

fn source_receiver_type_for_method(
    receiver: &Expr,
    method_name: &str,
    arg_count: usize,
    current_type: &str,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
) -> Option<(String, JavaType)> {
    let receiver_actual_ty = infer_resolver_expr_type(receiver, current_type, symbols, env);
    let JavaType::Named { name, args } = &receiver_actual_ty else {
        return source_receiver_type(receiver, current_type, symbols, env);
    };
    if !args.is_empty() {
        return source_receiver_type(receiver, current_type, symbols, env);
    }
    let Some(bound) = env
        .type_param_bound(name)
        .cloned()
        .or_else(|| symbols.type_param_bound_for(current_type, name))
    else {
        return source_receiver_type(receiver, current_type, symbols, env);
    };
    let mut candidates = Vec::new();
    collect_intersection_bound_candidates(&bound, &mut candidates);
    candidates
        .into_iter()
        .filter_map(|candidate| {
            let bound_name = primary_named_intersection_bound(candidate)?;
            let owner = symbols.source_type_name(bound_name).to_string();
            symbols
                .methods_for_hierarchy(&owner)
                .iter()
                .any(|method| {
                    !method.is_static
                        && method.java_name == method_name
                        && method.params.len() == arg_count
                })
                .then(|| (owner, candidate.clone()))
        })
        .next()
        .or_else(|| source_receiver_type(receiver, current_type, symbols, env))
}

fn collect_intersection_bound_candidates<'a>(bound: &'a JavaType, out: &mut Vec<&'a JavaType>) {
    match bound {
        JavaType::Intersection(bounds) => {
            for bound in bounds {
                collect_intersection_bound_candidates(bound, out);
            }
        }
        _ => out.push(bound),
    }
}

fn primary_named_intersection_bound(bound: &JavaType) -> Option<&str> {
    match bound {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
        JavaType::Intersection(bounds) => bounds.first().and_then(primary_named_intersection_bound),
        _ => None,
    }
}

fn source_static_type_receiver(
    receiver: &Expr,
    current_type: &str,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
) -> Option<String> {
    let Expr::Var(name) = receiver else {
        return None;
    };
    // Java value bindings shadow type names at an expression receiver.  A
    // field such as `Parser.parser` is still an instance value even when its
    // normalized spelling collides with the source type `Parser`; treating it
    // as a type qualifier fabricates a static call on the enclosing class.
    if env.local_type(name).is_some() || env.field_type_exact(name).is_some() {
        return None;
    }
    let source_name = symbols
        .lexical_source_type_name(name, current_type)
        .unwrap_or_else(|| symbols.source_type_name(name).to_string());
    symbols
        .methods_by_type
        .contains_key(&source_name)
        .then_some(source_name)
}

fn source_receiver_type_from_actual(
    receiver_actual_ty: JavaType,
    symbols: &ProgramSymbols,
) -> Option<(String, JavaType)> {
    match &receiver_actual_ty {
        JavaType::Named { name, args } => {
            let source_name = symbols.source_type_name(name).to_string();
            if !args.is_empty() && !symbols.source_type_accepts_actual_type(&receiver_actual_ty) {
                return None;
            }
            symbols
                .methods_by_type
                .contains_key(&source_name)
                .then_some((source_name, receiver_actual_ty))
        }
        JavaType::Unknown(name) => {
            let source_name = symbols.source_type_name(name).to_string();
            symbols
                .methods_by_type
                .contains_key(&source_name)
                .then_some((source_name, receiver_actual_ty))
        }
        _ => None,
    }
}

fn infer_stream_map_return_type(
    receiver_type: Option<&JavaType>,
    name: &str,
    args: &[Expr],
    current_type: &str,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
    external_facts: &ResolverExternalFacts<'_>,
) -> Option<JavaType> {
    if !is_stream_lambda_mapper_name(name) || args.len() != 1 {
        return None;
    }
    let stream_element = match receiver_type {
        Some(JavaType::Named { name, args }) if name == "Stream" => args.first()?,
        _ => return None,
    };
    let Expr::Lambda { params, body } = &args[0] else {
        return None;
    };

    let mut lambda_env = env.clone();
    if let Some(lambda_param) = params.first() {
        let param_ty = lambda_param
            .ty
            .clone()
            .unwrap_or_else(|| stream_element.clone());
        lambda_env.insert_local(lambda_param.name.clone(), param_ty);
    }

    let mapped = match body {
        LambdaBody::Expr(expr) => infer_resolver_expr_type_with_facts(
            expr,
            current_type,
            symbols,
            &lambda_env,
            external_facts,
        ),
        LambdaBody::Block(_) => JavaType::Unknown("lambda".to_string()),
    };

    Some(match name {
        "mapToInt" => JavaType::Named {
            name: "IntStream".to_string(),
            args: Vec::new(),
        },
        "mapToLong" => JavaType::Named {
            name: "LongStream".to_string(),
            args: Vec::new(),
        },
        "mapToDouble" => JavaType::Named {
            name: "DoubleStream".to_string(),
            args: Vec::new(),
        },
        _ => JavaType::Named {
            name: "Stream".to_string(),
            args: vec![mapped],
        },
    })
}

fn known_resolver_external_method_return_type_for_call(
    receiver_type: Option<&JavaType>,
    name: &str,
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
) -> Option<JavaType> {
    if !matches!(
        receiver_type,
        Some(JavaType::Named { name, .. } | JavaType::Unknown(name))
            if crate::javalibs::is_broad_utils_type_name(name)
    ) || name != "stream"
        || arg_types.len() != 1
    {
        return None;
    }
    let element = symbols.source_single_arg_supertype_arg(&arg_types[0], "Iterable")?;
    Some(JavaType::Named {
        name: "Stream".to_string(),
        args: vec![element],
    })
}

fn infer_stream_method_reference_map_return_type(
    receiver_type: Option<&JavaType>,
    name: &str,
    args: &[Expr],
    current_type: &str,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
    external_facts: &FactSet,
) -> Option<JavaType> {
    if name != "map" || args.len() != 1 {
        return None;
    }
    let stream_element = match receiver_type {
        Some(JavaType::Named { name, args }) if name == "Stream" => args.first()?,
        _ => return None,
    };
    let Expr::MethodReference { target, method } = &args[0] else {
        return None;
    };

    let mapped = infer_method_reference_return_type(
        target,
        method,
        stream_element,
        current_type,
        symbols,
        env,
        external_facts,
    )?;
    Some(JavaType::Named {
        name: "Stream".to_string(),
        args: vec![mapped],
    })
}

fn infer_stream_lambda_flat_map_return_type(
    receiver_type: Option<&JavaType>,
    name: &str,
    args: &[Expr],
    current_type: &str,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
    external_facts: &ResolverExternalFacts<'_>,
) -> Option<JavaType> {
    if name != "flatMap" || args.len() != 1 {
        return None;
    }
    let stream_element = match receiver_type {
        Some(JavaType::Named { name, args }) if name == "Stream" => args.first()?,
        _ => return None,
    };
    let Expr::Lambda { params, body } = &args[0] else {
        return None;
    };

    let mut lambda_env = env.clone();
    if let Some(lambda_param) = params.first() {
        let param_ty = lambda_param
            .ty
            .clone()
            .unwrap_or_else(|| stream_element.clone());
        lambda_env.insert_local(lambda_param.name.clone(), param_ty);
    }

    let LambdaBody::Expr(expr) = body else {
        return None;
    };
    let JavaType::Named {
        name: body_name,
        args: body_args,
    } = infer_resolver_expr_type_with_facts(
        expr,
        current_type,
        symbols,
        &lambda_env,
        external_facts,
    )
    else {
        return None;
    };
    if body_name != "Stream" {
        return None;
    }
    Some(JavaType::Named {
        name: "Stream".to_string(),
        args: vec![body_args.first()?.clone()],
    })
}

fn infer_stream_method_reference_flat_map_return_type(
    receiver_type: Option<&JavaType>,
    name: &str,
    args: &[Expr],
) -> Option<JavaType> {
    if name != "flatMap" || args.len() != 1 {
        return None;
    }
    let stream_element = match receiver_type {
        Some(JavaType::Named { name, args }) if name == "Stream" => args.first()?,
        _ => return None,
    };
    let Expr::MethodReference { target, method } = &args[0] else {
        return None;
    };
    if method != "stream" || !matches!(target.as_str(), "Collection" | "Iterable") {
        return None;
    }
    let element = match stream_element {
        JavaType::Named { args, .. } => args.first()?.clone(),
        _ => return None,
    };
    Some(JavaType::Named {
        name: "Stream".to_string(),
        args: vec![element],
    })
}

fn infer_stream_to_array_generator_return_type(
    receiver_type: Option<&JavaType>,
    name: &str,
    args: &[Expr],
) -> Option<JavaType> {
    if name != "toArray" || args.len() != 1 {
        return None;
    }
    match receiver_type {
        Some(JavaType::Named { name, .. }) if name == "Stream" => {}
        _ => return None,
    }
    let Expr::MethodReference { target, method } = &args[0] else {
        return None;
    };
    if method != "new" {
        return None;
    }
    let java_ty = crate::parser::parse_type(target);
    matches!(java_ty, JavaType::Array(_)).then_some(java_ty)
}

fn infer_method_reference_return_type(
    target: &str,
    method: &str,
    stream_element: &JavaType,
    current_type: &str,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
    external_facts: &FactSet,
) -> Option<JavaType> {
    if target == "String" && method == "valueOf" {
        return Some(JavaType::String);
    }

    if matches!(target, "this" | "super") {
        let methods = symbols.methods_for_hierarchy(current_type);
        let selection = resolve_same_class_call(
            method,
            std::slice::from_ref(stream_element),
            &methods,
            symbols,
            external_facts,
            None,
            env,
        )
        .ok()
        .flatten()
        .or_else(|| {
            let snake_case = crate::parser::to_snake_case(method);
            (snake_case != method)
                .then(|| {
                    resolve_same_class_call(
                        &snake_case,
                        std::slice::from_ref(stream_element),
                        &methods,
                        symbols,
                        external_facts,
                        None,
                        env,
                    )
                    .ok()
                    .flatten()
                })
                .flatten()
        })?;
        return Some(selection.call.return_type);
    }

    if let Some(receiver_type) = method_reference_target_type(target, env, symbols) {
        let arg_types = std::slice::from_ref(stream_element);
        if let Some(java_ty) = crate::stdlib::known_external_method_return_type_for_call(
            Some(&receiver_type),
            method,
            arg_types,
        ) {
            return Some(java_ty);
        }

        let receiver_name = java_type_simple_name(&receiver_type);
        let receiver_methods = symbols.methods_for_hierarchy(&receiver_name);
        if let Ok(Some(selection)) = resolve_same_class_call(
            method,
            arg_types,
            &receiver_methods,
            symbols,
            external_facts,
            Some(&receiver_type),
            env,
        ) {
            return Some(selection.call.return_type);
        }
    }

    let receiver_type = if matches!(target, "this" | "super") {
        stream_element.clone()
    } else {
        JavaType::Named {
            name: target.to_string(),
            args: Vec::new(),
        }
    };

    if let Some(java_ty) = crate::stdlib::known_external_method_return_type_for_receiver(
        Some(&receiver_type),
        method,
        0,
    ) {
        return Some(java_ty);
    }

    let receiver_name = java_type_simple_name(&receiver_type);
    symbols
        .methods_for_hierarchy(&receiver_name)
        .into_iter()
        .find(|candidate| {
            candidate.java_name == crate::parser::to_snake_case(method)
                && candidate.params.is_empty()
        })
        .map(|candidate| candidate.return_type)
}

fn method_reference_target_type(
    target: &str,
    env: &TypeEnv,
    symbols: &ProgramSymbols,
) -> Option<JavaType> {
    env.local_type(target)
        .or_else(|| env.field_type(target))
        .or_else(|| {
            // An exact Java binding wins, but normalizing a type name must not
            // manufacture a binding to a different variable (Worker::same is
            // unbound even when a local named `worker` is in scope).
            if env.static_type_alias(target).is_some() || symbols.source_type_kind(target).is_some()
            {
                return None;
            }
            let normalized = crate::parser::to_snake_case(target);
            (normalized != target)
                .then(|| {
                    env.local_type(&normalized)
                        .or_else(|| env.field_type(&normalized))
                })
                .flatten()
        })
        .cloned()
}

/// Resolve a unary source callback with the same applicability, generic
/// substitution and overload ordering as an ordinary source invocation.
pub(crate) fn resolve_single_input_source_method_reference(
    target: &str,
    method: &str,
    input: &JavaType,
    current_type: &str,
    source_symbols: &SourceSymbols,
    env: &TypeEnv,
    facts: &FactSet,
) -> Option<ResolvedCall> {
    resolve_source_method_reference_with_input_types(
        target,
        method,
        std::slice::from_ref(input),
        current_type,
        source_symbols,
        env,
        facts,
    )
}

/// Give native method references the same typed invocation as a lambda.
/// This is shared by effect analysis and lowering; source overloads keep
/// their existing source resolver. Bound receivers must still be retained
/// and null-checked eagerly by the lowering capture adapter.
pub(crate) fn native_method_reference_lambda(
    target: &str,
    method: &str,
    inputs: &[JavaType],
    source_symbols: &SourceSymbols,
    env: &TypeEnv,
    facts: &FactSet,
    external_index: &ExternalFactLookupIndex<'_>,
) -> Option<Expr> {
    use crate::call_plan::CallTargetPlan;
    if method == "new" || matches!(target, "this" | "self") {
        return None;
    }
    let owner = env.static_type_alias(target).unwrap_or(target);
    let bound = method_reference_target_type(target, env, &source_symbols.symbols);
    let target_type = bound
        .clone()
        .unwrap_or_else(|| crate::parser::parse_hierarchy_type(owner));
    let target_type = crate::types::optional_element_type(&target_type).unwrap_or(target_type);
    if crate::effective_facts::EffectiveFacts::new(facts)
        .parsed_source_controls_native_carrier(&target_type)
        || source_symbols
            .symbols
            .source_type_kind(&java_type_simple_name(&target_type))
            .is_some()
    {
        return None;
    }
    let params = inputs
        .iter()
        .enumerate()
        .map(|(index, ty)| {
            let name = format!("__java_reference_arg_{index}");
            crate::java_ir::LambdaParam {
                source_name: name.clone(),
                name,
                ty: Some(ty.clone()),
            }
        })
        .collect::<Vec<_>>();
    let mut types = env.clone();
    for param in &params {
        types.insert_local(param.name.clone(), param.ty.clone().unwrap());
    }
    let arguments = params
        .iter()
        .map(|param| Expr::Var(param.name.clone()))
        .collect::<Vec<_>>();
    let lexical = crate::parser::parse_expr(target);
    let static_target = crate::call_plan::plan_call_target_with_external_index_and_args(
        Some(&lexical),
        method,
        &arguments,
        None,
        facts,
        external_index,
        &types,
    );
    let (receiver, args) = if bound.is_none()
        && matches!(static_target, CallTargetPlan::NativeJavaStatic { .. })
    {
        (lexical, arguments)
    } else {
        let (receiver, args) = if bound.is_some() {
            (lexical, arguments)
        } else {
            let (input, _) = inputs.split_first()?;
            let input = crate::types::optional_element_type(input).unwrap_or_else(|| input.clone());
            java_type_match_score(&input, &target_type, &source_symbols.symbols, facts)?;
            (arguments.first()?.clone(), arguments[1..].to_vec())
        };
        if !matches!(
            crate::call_plan::plan_call_target_with_external_index_and_args(
                Some(&receiver),
                method,
                &args,
                None,
                facts,
                external_index,
                &types,
            ),
            CallTargetPlan::NativeJavaInstance { .. }
                | CallTargetPlan::NativeCollectionMethod { .. }
                | CallTargetPlan::ExternalFacadeReceiver { .. }
        ) {
            return None;
        }
        (receiver, args)
    };
    Some(Expr::Lambda {
        params,
        body: LambdaBody::Expr(Box::new(Expr::Call {
            type_args: Vec::new(),
            receiver: Some(Box::new(receiver)),
            name: method.to_string(),
            args,
            resolved: None,
        })),
    })
}

/// Resolve the SAM's complete input list, including the receiver input of an
/// unbound reference. Selection uses the ordinary Java overload resolver.
pub(crate) fn resolve_source_method_reference_with_input_types(
    target: &str,
    method: &str,
    inputs: &[JavaType],
    current_type: &str,
    source_symbols: &SourceSymbols,
    env: &TypeEnv,
    facts: &FactSet,
) -> Option<ResolvedCall> {
    let symbols = &source_symbols.symbols;
    let bound_type = if matches!(target, "this" | "self") {
        Some(JavaType::Named {
            name: current_type.to_string(),
            args: Vec::new(),
        })
    } else {
        method_reference_target_type(target, env, symbols)
    };
    let owner = bound_type
        .as_ref()
        .map(java_type_simple_name)
        .unwrap_or_else(|| env.static_type_alias(target).unwrap_or(target).to_string());
    let methods = symbols.methods_for_hierarchy(&owner);
    let select = |static_call: bool, args: &[JavaType], receiver: Option<&JavaType>| {
        let candidates = methods
            .iter()
            .filter(|method| method.is_static == static_call)
            .cloned()
            .collect::<Vec<_>>();
        resolve_same_class_call(method, args, &candidates, symbols, facts, receiver, env)
            .ok()
            .flatten()
            .map(|selection| selection.call)
    };
    if let Some(bound_type) = bound_type {
        return select(false, inputs, Some(&bound_type));
    }
    let static_call = select(true, inputs, None);
    let receiver_type = JavaType::Named {
        name: owner,
        args: Vec::new(),
    };
    let unbound = inputs.split_first().and_then(|(receiver, args)| {
        java_type_match_score(receiver, &receiver_type, symbols, facts)
            .and_then(|_| select(false, args, Some(receiver)))
    });
    match (static_call, unbound) {
        (Some(call), None) | (None, Some(call)) => Some(call),
        _ => None,
    }
}

pub(crate) fn resolve_source_call_with_types(
    receiver: Option<&Expr>,
    name: &str,
    arg_types: &[JavaType],
    current_type: &str,
    source_symbols: &SourceSymbols,
    env: &TypeEnv,
    facts: &FactSet,
) -> Option<ResolvedCall> {
    let symbols = &source_symbols.symbols;
    let (methods, actual) = match receiver {
        None | Some(Expr::This) => (
            symbols.methods_for_unqualified_call(current_type),
            Some(JavaType::Named {
                name: current_type.to_string(),
                args: Vec::new(),
            }),
        ),
        Some(receiver) => {
            let actual = infer_expr_type(receiver, env);
            let actual = crate::types::optional_element_type(&actual).unwrap_or(actual);
            let owner = java_type_simple_name(&actual);
            let owner = env.static_type_alias(&owner).unwrap_or(&owner);
            let mut methods = symbols.methods_for_hierarchy(owner);
            if matches!(receiver, Expr::Var(name) if env.local_type(name).is_none() && env.field_type(name).is_none())
            {
                methods.retain(|method| method.is_static);
                (methods, None)
            } else {
                (methods, Some(actual))
            }
        }
    };
    resolve_same_class_call(
        name,
        arg_types,
        &methods,
        symbols,
        facts,
        actual.as_ref(),
        env,
    )
    .ok()
    .flatten()
    .or_else(|| {
        let normalized = crate::parser::to_snake_case(name);
        (normalized != name)
            .then(|| {
                resolve_same_class_call(
                    &normalized,
                    arg_types,
                    &methods,
                    symbols,
                    facts,
                    actual.as_ref(),
                    env,
                )
                .ok()
                .flatten()
            })
            .flatten()
    })
    .map(|selection| selection.call)
}

/// Select a declaration in an inline lexical method group using the ordinary
/// Java overload/substitution rules. Candidate labels are selection-only;
/// they never publish a fictitious named class or emitted callable identity.
pub(crate) fn select_scoped_instance_method<'a>(
    methods: &[&'a MethodDecl],
    name: &str,
    arg_types: &[JavaType],
    lexical_owner: &str,
    symbols: &SourceSymbols,
    env: &TypeEnv,
    facts: &FactSet,
) -> Option<&'a MethodDecl> {
    let candidates = methods
        .iter()
        .enumerate()
        .map(|(index, method)| MethodSymbol {
            declaring_type: lexical_owner.to_string(),
            owner_type_params: Vec::new(),
            owner_type_param_bounds: Vec::new(),
            method_type_params: method.type_params.clone(),
            method_type_param_bounds: method.type_param_bounds.clone(),
            java_name: method.name.clone(),
            rust_name: format!("__java_scoped_candidate_{index}"),
            params: method.params.clone(),
            return_type: method.return_type.clone(),
            is_static: method.modifiers.contains(&Modifier::Static),
        })
        .collect::<Vec<_>>();
    let selected = resolve_same_class_call(
        name,
        arg_types,
        &candidates,
        &symbols.symbols,
        facts,
        None,
        env,
    )
    .ok()
    .flatten()?;
    let index = candidates
        .iter()
        .position(|candidate| candidate.rust_name == selected.call.rust_name)?;
    Some(methods[index])
}

fn resolver_common_type(
    left: &JavaType,
    right: &JavaType,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> JavaType {
    if left == right {
        return left.clone();
    }
    if is_null_type(left) && !matches!(right, JavaType::Primitive(_)) {
        return right.clone();
    }
    if is_null_type(right) && !matches!(left, JavaType::Primitive(_)) {
        return left.clone();
    }
    if java_type_assignable(left, right, symbols, external_facts) {
        return right.clone();
    }
    if java_type_assignable(right, left, symbols, external_facts) {
        return left.clone();
    }
    if let Some(common) = known_family_common_type(left, right, symbols, external_facts) {
        return common;
    }
    JavaType::Unknown("common_type".to_string())
}

pub(crate) fn is_null_type(ty: &JavaType) -> bool {
    matches!(ty, JavaType::Unknown(name) if name == "null")
}

fn known_family_common_type(
    left: &JavaType,
    right: &JavaType,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Option<JavaType> {
    let common_families = ["List", "Set", "Collection", "Iterable", "Map", "Object"];
    common_families
        .iter()
        .find_map(|family| common_family_type(left, right, family, symbols, external_facts))
}

fn common_family_type(
    left: &JavaType,
    right: &JavaType,
    family: &str,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Option<JavaType> {
    let target = JavaType::Named {
        name: family.to_string(),
        args: common_family_args(left, right, family, symbols, external_facts)?,
    };
    java_type_assignable(left, &target, symbols, external_facts)
        .then_some(())
        .and_then(|_| {
            java_type_assignable(right, &target, symbols, external_facts).then_some(target)
        })
}

fn common_family_args(
    left: &JavaType,
    right: &JavaType,
    family: &str,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Option<Vec<JavaType>> {
    match family {
        "List" | "Set" | "Collection" | "Iterable" => {
            let left_element = single_generic_arg(left)?;
            let right_element = single_generic_arg(right)?;
            Some(vec![resolver_common_type(
                left_element,
                right_element,
                symbols,
                external_facts,
            )])
        }
        "Map" => {
            let (left_key, left_value) = two_generic_args(left)?;
            let (right_key, right_value) = two_generic_args(right)?;
            Some(vec![
                resolver_common_type(left_key, right_key, symbols, external_facts),
                resolver_common_type(left_value, right_value, symbols, external_facts),
            ])
        }
        "Object" => Some(Vec::new()),
        _ => None,
    }
}

fn single_generic_arg(ty: &JavaType) -> Option<&JavaType> {
    match ty {
        JavaType::Named { args, .. } => args.first(),
        _ => None,
    }
}

fn two_generic_args(ty: &JavaType) -> Option<(&JavaType, &JavaType)> {
    match ty {
        JavaType::Named { args, .. } => match args.as_slice() {
            [left, right] => Some((left, right)),
            _ => None,
        },
        _ => None,
    }
}

fn infer_resolver_member_access_type_with_facts(
    receiver: &Expr,
    member: &str,
    current_type: &str,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
    external_facts: &ResolverExternalFacts<'_>,
) -> Option<JavaType> {
    if let Some(java_ty) = crate::types::infer_member_access_type(receiver, member, env) {
        return Some(java_ty);
    }
    let receiver_ty =
        infer_resolver_expr_type_with_facts(receiver, current_type, symbols, env, external_facts);
    if member == "length" && matches!(receiver_ty, JavaType::Array(_) | JavaType::String) {
        return Some(JavaType::Primitive(crate::java_ir::PrimitiveType::Int));
    }
    if member == "length" && matches!(receiver_ty, JavaType::Unknown(_)) {
        return Some(JavaType::Primitive(crate::java_ir::PrimitiveType::Int));
    }
    if let Some((receiver_type_name, _)) =
        source_receiver_type(receiver, current_type, symbols, env)
        && let Some(symbol) = symbols.field_for_from(&receiver_type_name, member, current_type)
        && field_access_is_allowed(receiver, symbol, symbols, env)
    {
        return Some(symbols.field_type_for_receiver(symbol, &receiver_ty));
    }
    if let Some((receiver_type_name, _)) =
        source_receiver_type(receiver, current_type, symbols, env)
        && let Some(java_ty) = symbols.external_super_field_type_for(&receiver_type_name, member)
    {
        return Some(java_ty);
    }
    if let Some(java_ty) =
        collection_element_static_constant_type(&receiver_ty, member, external_facts)
    {
        return Some(java_ty);
    }
    if let Some(java_ty) =
        known_external_or_pack_field_type_for_receiver(&receiver_ty, member, external_facts)
    {
        return Some(java_ty);
    }
    if let Expr::Var(receiver_name) = receiver
        && env.local_type(receiver_name).is_none()
        && env.field_type(receiver_name).is_none()
    {
        return crate::stdlib::conservative_external_enum_field_type_for_type_qualifier(
            receiver_name,
            member,
        );
    }
    None
}

fn collection_element_static_constant_type(
    receiver_ty: &JavaType,
    member: &str,
    external_facts: &FactSet,
) -> Option<JavaType> {
    let JavaType::Named { name, args } = receiver_ty else {
        return None;
    };
    if !is_collection_like_type_name(name) {
        return None;
    }
    let element_ty = args.first()?;
    known_external_or_pack_field_type_for_receiver(element_ty, member, external_facts)
}

fn known_external_or_pack_field_type_for_receiver(
    receiver_ty: &JavaType,
    member: &str,
    external_facts: &FactSet,
) -> Option<JavaType> {
    if let Some(receiver_name) = crate::stdlib::external_named_receiver_owner_key(receiver_ty)
        && let Some(field) = external_facts.external_field_for(&receiver_name, member)
        && let Some(field_type) = field.field_type.clone()
    {
        return Some(field_type);
    }
    if let Some(receiver_name) = crate::stdlib::external_named_receiver_owner_key(receiver_ty)
        && let Some(field) = crate::external_packs::external_field_for(&receiver_name, member)
        && let Some(field_type) = field.field_type
    {
        return Some(field_type);
    }
    crate::stdlib::known_external_field_type_for_receiver(receiver_ty, member)
}

fn external_fact_method_return_type_for_call(
    receiver_type: Option<&JavaType>,
    name: &str,
    arg_types: &[JavaType],
    external_facts: &ResolverExternalFacts<'_>,
) -> Option<JavaType> {
    let receiver_name = receiver_type.and_then(crate::stdlib::external_named_receiver_owner_key);
    external_facts
        .lookup
        .external_api_effect_for_receiver_with_arity(
            receiver_name.as_deref(),
            name,
            Some(arg_types.len()),
        )
        .and_then(|effect| effect.return_type.clone())
}

fn is_collection_like_type_name(name: &str) -> bool {
    matches!(
        name.rsplit('.').next().unwrap_or(name),
        "Collection"
            | "List"
            | "Set"
            | "SortedSet"
            | "NavigableSet"
            | "EnumSet"
            | "ArrayList"
            | "HashSet"
            | "LinkedHashSet"
            | "TreeSet"
    )
}

fn resolve_same_class_call(
    name: &str,
    arg_types: &[JavaType],
    methods: &[MethodSymbol],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
    receiver_actual_ty: Option<&JavaType>,
    scope: &TypeEnv,
) -> Result<Option<MethodResolution>, String> {
    resolve_same_class_call_with_type_args(
        name,
        arg_types,
        methods,
        symbols,
        external_facts,
        receiver_actual_ty,
        scope,
        &[],
    )
}

fn resolve_same_class_call_with_type_args(
    name: &str,
    arg_types: &[JavaType],
    methods: &[MethodSymbol],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
    receiver_actual_ty: Option<&JavaType>,
    scope: &TypeEnv,
    explicit_type_args: &[JavaType],
) -> Result<Option<MethodResolution>, String> {
    let candidates = methods
        .iter()
        .filter(|method| {
            method.java_name == name
                && params_accept_arg_count(&method.params, arg_types.len())
                && (explicit_type_args.is_empty()
                    || method.method_type_params.is_empty()
                    || explicit_type_args.len() == method.method_type_params.len())
        })
        .collect::<Vec<_>>();
    let candidates = deduplicated_effective_method_candidates(
        candidates,
        receiver_actual_ty,
        symbols,
        external_facts,
    );
    if candidates.is_empty() {
        if !explicit_type_args.is_empty()
            && methods.iter().any(|method| {
                method.java_name == name && params_accept_arg_count(&method.params, arg_types.len())
            })
        {
            return Err(format!(
                "no overload of {name} accepts {} explicit method type arguments",
                explicit_type_args.len()
            ));
        }
        return Ok(None);
    }

    let exact = candidates
        .iter()
        .copied()
        .filter(|method| {
            if !method_type_arguments_satisfy_bounds(
                method,
                &method_call_type_substitutions_with_explicit(
                    method,
                    arg_types,
                    receiver_actual_ty,
                    symbols,
                    explicit_type_args,
                ),
                scope,
                symbols,
                external_facts,
            ) {
                return false;
            }
            let substitutions = receiver_type_substitutions(method, receiver_actual_ty, symbols);
            fixed_arity_exact_match(&method.params, arg_types, &substitutions)
        })
        .collect::<Vec<_>>();
    let assignable = candidates
        .iter()
        .copied()
        .filter(|method| {
            let substitutions = method_call_type_substitutions_with_explicit(
                method,
                arg_types,
                receiver_actual_ty,
                symbols,
                explicit_type_args,
            );
            if !method_type_arguments_satisfy_bounds(
                method,
                &substitutions,
                scope,
                symbols,
                external_facts,
            ) {
                return false;
            }
            callable_assignable_score(
                &method.params,
                arg_types,
                symbols,
                external_facts,
                Some(&substitutions),
            )
            .is_some()
        })
        .collect::<Vec<_>>();
    let best_assignable = best_method_match_with_type_args(
        &candidates,
        arg_types,
        symbols,
        external_facts,
        receiver_actual_ty,
        scope,
        explicit_type_args,
    );

    let selected = if let [one] = exact.as_slice() {
        *one
    } else if !exact.is_empty() {
        return Err(format!(
            "ambiguous overload of {name} for argument types {:?}",
            arg_types
        ));
    } else if let Some(one) = best_assignable {
        one
    } else if assignable.len() == 1 {
        assignable[0]
    } else if assignable.is_empty()
        && candidates.len() == 1
        && explicit_type_args.is_empty()
        && method_type_arguments_satisfy_bounds(
            candidates[0],
            &method_call_type_substitutions_with_explicit(
                candidates[0],
                arg_types,
                receiver_actual_ty,
                symbols,
                explicit_type_args,
            ),
            scope,
            symbols,
            external_facts,
        )
    {
        candidates[0]
    } else if assignable.is_empty() {
        return Err(format!(
            "no overload of {name} matches argument types {:?}",
            arg_types
        ));
    } else {
        return Err(format!(
            "ambiguous overload of {name} for argument types {:?}",
            arg_types
        ));
    };

    let substitutions = method_call_type_substitutions_with_explicit(
        selected,
        arg_types,
        receiver_actual_ty,
        symbols,
        explicit_type_args,
    );
    let substituted_params: Vec<Param> = selected
        .params
        .iter()
        .map(|param| Param {
            name: param.name.clone(),
            source_name: param.source_name.clone(),
            ty: substitute_method_type_variables(&param.ty, &substitutions),
            is_varargs: param.is_varargs,
            binding: param.binding.clone(),
        })
        .collect();
    let param_types = substituted_params
        .iter()
        .map(|param| param.ty.clone())
        .collect::<Vec<_>>();
    let arg_coercions = java_coercion_evidence_for_params(
        &selected.params,
        &param_types,
        arg_types,
        symbols,
        external_facts,
    );
    Ok(Some(MethodResolution {
        call: ResolvedCall {
            type_args: if selected.method_type_params.is_empty() {
                Vec::new()
            } else {
                explicit_type_args.to_vec()
            },
            declaring_type: selected.declaring_type.clone(),
            rust_name: selected.rust_name.clone(),
            kind: if selected.is_static {
                CallKind::Static
            } else {
                CallKind::Instance
            },
            return_type: substitute_method_type_variables(&selected.return_type, &substitutions),
            arg_target_types: param_types,
            arg_coercions,
        },
        params: substituted_params,
    }))
}

fn deduplicated_effective_method_candidates<'a>(
    candidates: Vec<&'a MethodSymbol>,
    receiver_actual_ty: Option<&JavaType>,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Vec<&'a MethodSymbol> {
    let mut selected = Vec::new();
    let mut indexes = HashMap::new();
    for method in candidates {
        let substitutions = receiver_type_substitutions(method, receiver_actual_ty, symbols);
        let substituted = substitute_method_symbol(method, &substitutions);
        let key = method_signature_key(&substituted);
        if let Some(&index) = indexes.get(&key) {
            let existing: &MethodSymbol = selected[index];
            let existing_substitutions =
                receiver_type_substitutions(existing, receiver_actual_ty, symbols);
            let existing_effective = substitute_method_symbol(existing, &existing_substitutions);
            // A Java interface inherits one contract for matching parameter
            // signatures, with the most specific covariant return regardless
            // of the parents' source order.
            if java_type_assignable(
                &substituted.return_type,
                &existing_effective.return_type,
                symbols,
                external_facts,
            ) && !java_type_assignable(
                &existing_effective.return_type,
                &substituted.return_type,
                symbols,
                external_facts,
            ) {
                selected[index] = method;
            }
        } else {
            indexes.insert(key, selected.len());
            selected.push(method);
        }
    }
    selected
}

fn method_type_substitutions(
    method: &MethodSymbol,
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
) -> HashMap<String, JavaType> {
    let mut substitutions = HashMap::new();
    if method.method_type_params.is_empty() {
        return substitutions;
    }
    for (param, arg_ty) in method.params.iter().zip(arg_types) {
        collect_method_type_substitutions(
            &param.ty,
            arg_ty,
            &mut substitutions,
            symbols,
            Some(&method.method_type_params),
        );
    }
    // A null argument supplies no concrete lower bound. If a binder has no
    // return-context constraint and every parameter mentioning it is null,
    // Java inference can use its declared upper bound (Object if unbounded).
    // Do not leak the callee binder into the caller or invent a facade named T.
    for (index, parameter) in method.method_type_params.iter().enumerate() {
        if substitutions.contains_key(parameter)
            || contains_type_parameter(&method.return_type, parameter)
        {
            continue;
        }
        let mut occurrences = method
            .params
            .iter()
            .zip(arg_types)
            .filter(|(param, _)| contains_type_parameter(&param.ty, parameter))
            .peekable();
        if occurrences.peek().is_none() || !occurrences.all(|(_, actual)| is_null_type(actual)) {
            continue;
        }
        let bound = method
            .method_type_param_bounds
            .get(index)
            .and_then(Option::as_ref)
            .cloned()
            .unwrap_or(JavaType::Named {
                name: "java.lang.Object".into(),
                args: Vec::new(),
            });
        let bound = substitute_method_type_variables(&bound, &substitutions);
        if method
            .method_type_params
            .iter()
            .any(|name| contains_type_parameter(&bound, name))
        {
            continue;
        }
        substitutions.insert(parameter.clone(), bound);
    }
    substitutions
}

fn receiver_type_substitutions(
    method: &MethodSymbol,
    receiver_actual_ty: Option<&JavaType>,
    symbols: &ProgramSymbols,
) -> HashMap<String, JavaType> {
    let Some(JavaType::Named { name, args }) = receiver_actual_ty else {
        return HashMap::new();
    };
    let receiver_bounds = symbols.type_param_bounds_for(name).unwrap_or(&[]);
    // Hierarchy method symbols are already specialized into the receiver's
    // lexical generic frame. Bind that frame to actual arguments, not the
    // original ancestor's (possibly differently named/reordered) parameters.
    let receiver_params = symbols
        .type_params_for(name)
        .unwrap_or(&method.owner_type_params);
    owner_type_argument_substitutions(
        receiver_params,
        &method.method_type_params,
        args,
        receiver_bounds,
    )
}

/// One owner/member binding rule for methods and constructors. A member's
/// own generic binder shadows an identically named enclosing-class binder.
pub(crate) fn owner_type_argument_substitutions(
    owner_params: &[String],
    member_params: &[String],
    args: &[JavaType],
    bounds: &[Option<JavaType>],
) -> HashMap<String, JavaType> {
    owner_params
        .iter()
        .zip(args.iter().enumerate())
        .filter(|(param_name, _)| !member_params.contains(param_name))
        .map(|(param_name, (index, arg))| {
            (
                param_name.clone(),
                wildcard_actual_or_declared_bound(arg, bounds.get(index)),
            )
        })
        .collect()
}

fn method_call_type_substitutions(
    method: &MethodSymbol,
    arg_types: &[JavaType],
    receiver_actual_ty: Option<&JavaType>,
    symbols: &ProgramSymbols,
) -> HashMap<String, JavaType> {
    let mut substitutions = receiver_type_substitutions(method, receiver_actual_ty, symbols);
    substitutions.extend(method_type_substitutions(method, arg_types, symbols));
    substitutions
}

fn method_call_type_substitutions_with_explicit(
    method: &MethodSymbol,
    arg_types: &[JavaType],
    receiver_actual_ty: Option<&JavaType>,
    symbols: &ProgramSymbols,
    explicit_type_args: &[JavaType],
) -> HashMap<String, JavaType> {
    let mut substitutions =
        method_call_type_substitutions(method, arg_types, receiver_actual_ty, symbols);
    substitutions.extend(
        method
            .method_type_params
            .iter()
            .cloned()
            .zip(explicit_type_args.iter().cloned()),
    );
    substitutions
}

fn method_type_arguments_satisfy_bounds(
    method: &MethodSymbol,
    substitutions: &HashMap<String, JavaType>,
    scope: &TypeEnv,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> bool {
    method
        .method_type_params
        .iter()
        .zip(&method.method_type_param_bounds)
        .all(|(param, bound)| {
            let (Some(actual), Some(bound)) = (substitutions.get(param), bound) else {
                return true;
            };
            let bound = substitute_method_type_variables(bound, substitutions);
            let accepts = |bound: &JavaType| {
                scoped_type_arguments_are_compatible(
                    actual,
                    bound,
                    scope,
                    symbols,
                    external_facts,
                    &mut HashSet::new(),
                ) && java_type_assignable(actual, bound, symbols, external_facts)
            };
            match &bound {
                JavaType::Intersection(bounds) => bounds.iter().all(accepts),
                bound => accepts(bound),
            }
        })
}

fn wildcard_actual_or_declared_bound(
    arg: &JavaType,
    declared_bound: Option<&Option<JavaType>>,
) -> JavaType {
    match arg {
        JavaType::Wildcard(None) => declared_bound
            .and_then(|bound| bound.clone())
            .unwrap_or_else(|| arg.clone()),
        JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Extends(bound))) => {
            bound.as_ref().clone()
        }
        _ => arg.clone(),
    }
}

fn collect_method_type_substitutions(
    param_ty: &JavaType,
    arg_ty: &JavaType,
    substitutions: &mut HashMap<String, JavaType>,
    symbols: &ProgramSymbols,
    method_type_params: Option<&[String]>,
) {
    collect_type_substitutions(
        param_ty,
        arg_ty,
        substitutions,
        Some(symbols),
        method_type_params,
    );
}

/// Recover callable-binder instantiations from the resolver's selected signature.
/// Argument targets have already been projected through source inheritance and
/// substituted, so downstream capability planning must not infer them again
/// from argument spelling or pair unrelated generic positions.
pub(crate) fn resolved_method_type_substitutions(
    owner_type_params: &[String],
    method: &MethodDecl,
    call: &ResolvedCall,
) -> HashMap<String, JavaType> {
    let type_params = owner_type_params
        .iter()
        .filter(|name| !method.type_params.contains(name))
        .chain(&method.type_params)
        .cloned()
        .collect::<Vec<_>>();
    let mut substitutions = method
        .type_params
        .iter()
        .cloned()
        .zip(call.type_args.iter().cloned())
        .collect::<HashMap<_, _>>();
    for (param, target) in method.params.iter().zip(&call.arg_target_types) {
        collect_type_substitutions(
            &param.ty,
            target,
            &mut substitutions,
            None,
            Some(&type_params),
        );
    }
    collect_type_substitutions(
        &method.return_type,
        &call.return_type,
        &mut substitutions,
        None,
        Some(&type_params),
    );
    substitutions
}

fn collect_type_substitutions(
    param_ty: &JavaType,
    arg_ty: &JavaType,
    substitutions: &mut HashMap<String, JavaType>,
    symbols: Option<&ProgramSymbols>,
    method_type_params: Option<&[String]>,
) {
    fn same_constructor_name(left: &str, right: &str) -> bool {
        left == right
            || ((!left.contains('.') || !right.contains('.'))
                && crate::facts::external_type_name_matches(left, right))
    }
    fn aligned_constructor(left: &JavaType, right: &JavaType) -> bool {
        match (left, right) {
            (
                JavaType::Named {
                    name: left,
                    args: left_args,
                },
                JavaType::Named {
                    name: right,
                    args: right_args,
                },
            ) => same_constructor_name(left, right) && left_args.len() == right_args.len(),
            (JavaType::Array(_), JavaType::Array(_)) => true,
            _ => false,
        }
    }
    let is_binder = matches!(param_ty, JavaType::Named { name, args }
        if args.is_empty() && is_method_type_variable(name, method_type_params));
    if symbols.is_none() && !is_binder && !aligned_constructor(param_ty, arg_ty) {
        // This path consumes an already-selected source signature, not fresh
        // Java overload evidence. Its ABI may retain an outer nullable
        // carrier. Align only when removing that carrier proves the same
        // constructor/arity; never infer a binder from an unrelated position.
        // A bare T still binds an entire Optional<X>, and Optional<T> matched
        // to Optional<X> keeps its genuine Java constructor intact.
        if let Some(inner) = crate::types::optional_element_type(arg_ty)
            && aligned_constructor(param_ty, &inner)
        {
            collect_type_substitutions(
                param_ty,
                &inner,
                substitutions,
                symbols,
                method_type_params,
            );
            return;
        }
        if let Some(inner) = crate::types::optional_element_type(param_ty)
            && aligned_constructor(&inner, arg_ty)
        {
            collect_type_substitutions(&inner, arg_ty, substitutions, symbols, method_type_params);
            return;
        }
    }
    match (param_ty, arg_ty) {
        (JavaType::Named { name, args }, actual)
            if args.is_empty() && is_method_type_variable(name, method_type_params) =>
        {
            // Null, unresolved calls, and un-targeted lambdas/references are
            // not concrete types from which to infer a fresh method binder.
            if !matches!(actual, JavaType::Unknown(_)) {
                substitutions
                    .entry(name.clone())
                    .or_insert_with(|| actual.clone());
            }
        }
        (JavaType::Array(param_inner), JavaType::Array(arg_inner)) => {
            collect_type_substitutions(
                param_inner,
                arg_inner,
                substitutions,
                symbols,
                method_type_params,
            );
        }
        (JavaType::Wildcard(Some(param_bound)), JavaType::Wildcard(Some(arg_bound))) => {
            let param_inner = match param_bound {
                crate::java_ir::WildcardBound::Extends(inner)
                | crate::java_ir::WildcardBound::Super(inner) => inner,
            };
            let arg_inner = match arg_bound {
                crate::java_ir::WildcardBound::Extends(inner)
                | crate::java_ir::WildcardBound::Super(inner) => inner,
            };
            collect_type_substitutions(
                param_inner,
                arg_inner,
                substitutions,
                symbols,
                method_type_params,
            );
        }
        (JavaType::Wildcard(Some(param_bound)), actual) => {
            let param_inner = match param_bound {
                crate::java_ir::WildcardBound::Extends(inner)
                | crate::java_ir::WildcardBound::Super(inner) => inner,
            };
            collect_type_substitutions(
                param_inner,
                actual,
                substitutions,
                symbols,
                method_type_params,
            );
        }
        (param, JavaType::Wildcard(Some(arg_bound))) => {
            let arg_inner = match arg_bound {
                crate::java_ir::WildcardBound::Extends(inner)
                | crate::java_ir::WildcardBound::Super(inner) => inner,
            };
            collect_type_substitutions(
                param,
                arg_inner,
                substitutions,
                symbols,
                method_type_params,
            );
        }
        (
            JavaType::Named {
                name: param_name,
                args: param_args,
                ..
            },
            JavaType::Named {
                name: arg_name,
                args: arg_args,
            },
        ) => {
            let projected_arg =
                symbols.and_then(|symbols| symbols.source_supertype_args(arg_ty, param_name));
            let effective_arg_args = if let Some(projected) = projected_arg.as_deref() {
                projected
            } else if same_constructor_name(param_name, arg_name) {
                arg_args
            } else if symbols.is_some_and(|symbols| {
                symbols.source_type_kind(param_name).is_none()
                    && symbols.source_type_kind(arg_name).is_none()
                    && crate::stdlib::is_known_java_platform_subtype(arg_name, param_name)
            }) {
                // JDK collection supertypes preserve their generic positions.
                // Source declarations instead require the inheritance
                // projection above, including any reordered parameters.
                arg_args
            } else {
                return;
            };
            if param_args.len() != effective_arg_args.len() {
                return;
            }
            for (param_arg, arg_arg) in param_args.iter().zip(effective_arg_args) {
                collect_type_substitutions(
                    param_arg,
                    arg_arg,
                    substitutions,
                    symbols,
                    method_type_params,
                );
            }
        }
        _ => {}
    }
}

pub(crate) fn substitute_method_type_variables(
    ty: &JavaType,
    substitutions: &HashMap<String, JavaType>,
) -> JavaType {
    match ty {
        JavaType::Named { name, args } if args.is_empty() && substitutions.contains_key(name) => {
            substitutions.get(name).cloned().unwrap()
        }
        JavaType::Unknown(name) if substitutions.contains_key(name) => {
            // Solved generic slots can retain a type parameter as Unknown;
            // an explicit binder substitution is equally authoritative there.
            substitutions.get(name).cloned().unwrap()
        }
        JavaType::Array(inner) => JavaType::Array(Box::new(substitute_method_type_variables(
            inner,
            substitutions,
        ))),
        JavaType::Named { name, args } => JavaType::Named {
            name: name.clone(),
            args: args
                .iter()
                .map(|arg| substitute_method_type_variables(arg, substitutions))
                .collect(),
        },
        JavaType::Wildcard(Some(bound)) => JavaType::Wildcard(Some(match bound {
            crate::java_ir::WildcardBound::Extends(inner) => {
                crate::java_ir::WildcardBound::Extends(Box::new(substitute_method_type_variables(
                    inner,
                    substitutions,
                )))
            }
            crate::java_ir::WildcardBound::Super(inner) => crate::java_ir::WildcardBound::Super(
                Box::new(substitute_method_type_variables(inner, substitutions)),
            ),
        })),
        other => other.clone(),
    }
}

fn resolve_same_class_constructor(
    arg_types: &[JavaType],
    constructors: &[ConstructorSymbol],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
    constructed_type: Option<&JavaType>,
    scope: &TypeEnv,
) -> Result<Option<ResolvedConstructor>, String> {
    let mut seen_candidate_signatures = HashSet::new();
    let candidates = constructors
        .iter()
        .filter(|constructor| params_accept_arg_count(&constructor.params, arg_types.len()))
        .filter(|constructor| {
            seen_candidate_signatures.insert(constructor_signature_key(constructor))
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Ok(None);
    }

    let exact = candidates
        .iter()
        .copied()
        .filter(|constructor| {
            let substitutions =
                constructor_type_substitutions(constructor, arg_types, symbols, constructed_type);
            fixed_arity_exact_match(&constructor.params, arg_types, &substitutions)
        })
        .collect::<Vec<_>>();
    let assignable = candidates
        .iter()
        .copied()
        .filter(|constructor| {
            let substitutions =
                constructor_type_substitutions(constructor, arg_types, symbols, constructed_type);
            if !constructor_arguments_respect_scope(
                constructor,
                arg_types,
                &substitutions,
                scope,
                symbols,
                external_facts,
            ) {
                return false;
            }
            callable_assignable_score(
                &constructor.params,
                arg_types,
                symbols,
                external_facts,
                Some(&substitutions),
            )
            .is_some()
        })
        .collect::<Vec<_>>();
    let best_assignable = best_constructor_match(
        &candidates,
        arg_types,
        symbols,
        external_facts,
        constructed_type,
        scope,
    );

    let selected = if let [one] = exact.as_slice() {
        *one
    } else if !exact.is_empty() {
        return Err(format!(
            "ambiguous constructor for argument types {:?}",
            arg_types
        ));
    } else if let Some(one) = best_assignable {
        one
    } else if assignable.len() == 1 {
        assignable[0]
    } else if assignable.is_empty()
        && candidates.len() == 1
        && constructor_arguments_respect_scope(
            candidates[0],
            arg_types,
            &constructor_type_substitutions(candidates[0], arg_types, symbols, constructed_type),
            scope,
            symbols,
            external_facts,
        )
    {
        candidates[0]
    } else if assignable.is_empty() {
        return Err(format!(
            "no constructor matches argument types {:?}",
            arg_types
        ));
    } else {
        return Err(format!(
            "ambiguous constructor for argument types {:?}",
            arg_types
        ));
    };

    let substitutions =
        constructor_type_substitutions(selected, arg_types, symbols, constructed_type);
    let params = selected
        .params
        .iter()
        .map(|param| Param {
            name: param.name.clone(),
            source_name: param.source_name.clone(),
            ty: substitute_method_type_variables(&param.ty, &substitutions),
            is_varargs: param.is_varargs,
            binding: param.binding.clone(),
        })
        .collect::<Vec<_>>();
    let param_types = params
        .iter()
        .map(|param| param.ty.clone())
        .collect::<Vec<_>>();
    let arg_coercions = java_coercion_evidence_for_params(
        &selected.params,
        &param_types,
        arg_types,
        symbols,
        external_facts,
    );

    Ok(Some(ResolvedConstructor {
        declaring_type: selected.declaring_type.clone(),
        rust_name: selected.rust_name.clone(),
        constructed_type: constructed_type
            .cloned()
            .unwrap_or_else(|| JavaType::Named {
                name: selected.declaring_type.clone(),
                args: Vec::new(),
            }),
        params: selected.params.clone(),
        arg_coercions,
    }))
}

fn resolve_anonymous_class_constructor(
    target_name: &str,
    arg_types: &[JavaType],
    constructors: &[ConstructorSymbol],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
    constructed_type: Option<&JavaType>,
    scope: &TypeEnv,
) -> Result<Option<ResolvedConstructor>, String> {
    match symbols.source_type_kind(target_name) {
        Some(TypeKind::Interface) if arg_types.is_empty() => Ok(Some(ResolvedConstructor {
            declaring_type: symbols.source_type_name(target_name).to_string(),
            rust_name: "anonymous".to_string(),
            constructed_type: JavaType::Named {
                name: symbols.source_type_name(target_name).to_string(),
                args: Vec::new(),
            },
            params: Vec::new(),
            arg_coercions: Vec::new(),
        })),
        Some(TypeKind::Class | TypeKind::Record | TypeKind::Enum) => {
            resolve_same_class_constructor(
                arg_types,
                constructors,
                symbols,
                external_facts,
                constructed_type,
                scope,
            )
        }
        None => Ok(None),
        Some(TypeKind::Interface) => Ok(None),
    }
}

fn constructor_signature_key(constructor: &ConstructorSymbol) -> String {
    let params = constructor
        .params
        .iter()
        .map(|param| format!("{:?}:{}", param.ty, param.is_varargs))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{}::{}({})",
        constructor.declaring_type, constructor.rust_name, params
    )
}

fn constructor_type_substitutions(
    constructor: &ConstructorSymbol,
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
    constructed_type: Option<&JavaType>,
) -> HashMap<String, JavaType> {
    let owner_params = symbols
        .type_params_for(&constructor.declaring_type)
        .unwrap_or(&[]);
    let type_params = owner_params
        .iter()
        .chain(&constructor.type_params)
        .cloned()
        .collect::<Vec<_>>();
    let mut substitutions = HashMap::new();
    if let Some(JavaType::Named { args, .. }) = constructed_type {
        // Class arguments (including captured outer arguments) are fixed by
        // the constructed receiver, not inference variables of each overload.
        // A constructor's own binder may shadow a same-named class binder.
        substitutions.extend(owner_type_argument_substitutions(
            owner_params,
            &constructor.type_params,
            args,
            symbols
                .type_param_bounds_for(&constructor.declaring_type)
                .unwrap_or(&[]),
        ));
    }
    for (param, arg_ty) in constructor.params.iter().zip(arg_types) {
        collect_method_type_substitutions(
            &param.ty,
            arg_ty,
            &mut substitutions,
            symbols,
            Some(&type_params),
        );
    }
    substitutions
}

/// Fixed type arguments may contain variables of the caller's scope. They
/// retain those identities; they are not fresh inference variables of this
/// constructor. Validate these constraints before applying conversion costs.
fn constructor_arguments_respect_scope(
    constructor: &ConstructorSymbol,
    arg_types: &[JavaType],
    substitutions: &HashMap<String, JavaType>,
    scope: &TypeEnv,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> bool {
    let param_types = constructor
        .params
        .iter()
        .map(|param| substitute_method_type_variables(&param.ty, substitutions))
        .collect::<Vec<_>>();
    let varargs = constructor
        .params
        .last()
        .is_some_and(|param| param.is_varargs);
    arg_types.iter().enumerate().all(|(index, actual)| {
        let target = if varargs && index >= param_types.len().saturating_sub(1) {
            let Some(JavaType::Array(element)) = param_types.last() else {
                return false;
            };
            if arg_types.len() == param_types.len()
                && (matches!(actual, JavaType::Array(_)) || is_null_type(actual))
            {
                param_types.last().unwrap()
            } else {
                element.as_ref()
            }
        } else {
            let Some(target) = param_types.get(index) else {
                return false;
            };
            target
        };
        scoped_type_arguments_are_compatible(
            actual,
            target,
            scope,
            symbols,
            external_facts,
            &mut HashSet::new(),
        )
    })
}

fn scoped_type_arguments_are_compatible(
    actual: &JavaType,
    target: &JavaType,
    scope: &TypeEnv,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
    visiting: &mut HashSet<String>,
) -> bool {
    if actual == target || is_null_type(actual) {
        return true;
    }
    match target {
        JavaType::Wildcard(None) => return true,
        JavaType::Wildcard(Some(WildcardBound::Extends(bound))) => {
            let actual = match actual {
                JavaType::Wildcard(Some(WildcardBound::Extends(inner))) => inner.as_ref(),
                actual => actual,
            };
            return scoped_type_arguments_are_compatible(
                actual,
                bound,
                scope,
                symbols,
                external_facts,
                visiting,
            );
        }
        JavaType::Wildcard(Some(WildcardBound::Super(bound))) => {
            let actual = match actual {
                JavaType::Wildcard(Some(WildcardBound::Super(inner))) => inner.as_ref(),
                actual => actual,
            };
            return scoped_type_arguments_are_compatible(
                bound,
                actual,
                scope,
                symbols,
                external_facts,
                visiting,
            );
        }
        _ => {}
    }
    if let JavaType::Named { name, args } = actual
        && args.is_empty()
        && let Some(bound) = scope.type_param_bound(name)
    {
        if !visiting.insert(name.clone()) {
            return false;
        }
        let accepts_bound = |bound: &JavaType, visiting: &mut HashSet<String>| {
            scoped_type_arguments_are_compatible(
                bound,
                target,
                scope,
                symbols,
                external_facts,
                visiting,
            ) && java_type_assignable(bound, target, symbols, external_facts)
        };
        let compatible = match bound {
            JavaType::Intersection(bounds) => {
                bounds.iter().any(|bound| accepts_bound(bound, visiting))
            }
            bound => accepts_bound(bound, visiting),
        };
        visiting.remove(name);
        return compatible;
    }
    if let JavaType::Named { name, args } = target
        && args.is_empty()
        && scope.type_param_bound(name).is_some()
    {
        // No concrete class can be assumed to be a particular unknown T.
        // The actual-variable case above proves valid U extends T flows.
        return false;
    }
    match (actual, target) {
        (JavaType::Array(actual), JavaType::Array(target)) => scoped_type_arguments_are_compatible(
            actual,
            target,
            scope,
            symbols,
            external_facts,
            visiting,
        ),
        (
            JavaType::Named {
                args: actual_args, ..
            },
            JavaType::Named { name, args },
        ) => {
            let projected = symbols.source_supertype_args(actual, name);
            let actual_args = projected.as_deref().unwrap_or(actual_args);
            // Scope validation also gates the single-candidate recovery path.
            // A concrete invariant mismatch must not become applicable merely
            // because neither child is a caller type variable. Reuse overload
            // matching's generic/wildcard rules, retaining raw-type recovery.
            generic_arguments_match_score(actual_args, args, symbols, external_facts).is_some()
                && actual_args.iter().zip(args).all(|(actual, target)| {
                    scoped_type_arguments_are_compatible(
                        actual,
                        target,
                        scope,
                        symbols,
                        external_facts,
                        visiting,
                    )
                })
        }
        _ => true,
    }
}

fn best_method_match_with_type_args<'a>(
    candidates: &[&'a MethodSymbol],
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
    receiver_actual_ty: Option<&JavaType>,
    scope: &TypeEnv,
    explicit_type_args: &[JavaType],
) -> Option<&'a MethodSymbol> {
    let scored = candidates
        .iter()
        .filter_map(|method| {
            let substitutions = method_call_type_substitutions_with_explicit(
                method,
                arg_types,
                receiver_actual_ty,
                symbols,
                explicit_type_args,
            );
            if !method_type_arguments_satisfy_bounds(
                method,
                &substitutions,
                scope,
                symbols,
                external_facts,
            ) {
                return None;
            }
            let param_types = method
                .params
                .iter()
                .map(|param| substitute_method_type_variables(&param.ty, &substitutions))
                .collect::<Vec<_>>();
            callable_match_score(
                &method.params,
                &param_types,
                arg_types,
                symbols,
                external_facts,
            )
            .map(|score| (*method, score, param_types))
        })
        .collect::<Vec<_>>();
    select_best_callable(scored, arg_types, symbols, external_facts)
}

fn best_constructor_match<'a>(
    candidates: &[&'a ConstructorSymbol],
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
    constructed_type: Option<&JavaType>,
    scope: &TypeEnv,
) -> Option<&'a ConstructorSymbol> {
    let scored = candidates
        .iter()
        .filter_map(|constructor| {
            let substitutions =
                constructor_type_substitutions(constructor, arg_types, symbols, constructed_type);
            if !constructor_arguments_respect_scope(
                constructor,
                arg_types,
                &substitutions,
                scope,
                symbols,
                external_facts,
            ) {
                return None;
            }
            let param_types = constructor
                .params
                .iter()
                .map(|param| substitute_method_type_variables(&param.ty, &substitutions))
                .collect::<Vec<_>>();
            callable_match_score(
                &constructor.params,
                &param_types,
                arg_types,
                symbols,
                external_facts,
            )
            .map(|score| (*constructor, score, param_types))
        })
        .collect::<Vec<_>>();
    select_best_callable(scored, arg_types, symbols, external_facts)
}

/// Share the final specificity comparison between method and constructor
/// overloads. Equal conversion costs do not imply ambiguity when exactly one
/// instantiated parameter list is strictly narrower than every other one.
fn select_best_callable<'a, T>(
    mut scored: Vec<(&'a T, u32, Vec<JavaType>)>,
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Option<&'a T> {
    let best_score = scored.iter().map(|(_, score, _)| *score).min()?;
    scored.retain(|(_, score, _)| *score == best_score);
    if scored.len() == 1 {
        return Some(scored[0].0);
    }
    // Unknown receiver/call results are not proof of applicability. Do not
    // manufacture a target merely because one overload has a narrower type.
    if arg_types
        .iter()
        .any(type_contains_unresolved_overload_evidence)
    {
        return None;
    }
    scored
        .iter()
        .enumerate()
        .find_map(|(index, (callable, _, params))| {
            scored
                .iter()
                .enumerate()
                .all(|(other_index, (_, _, other_params))| {
                    index == other_index
                        || (params.len() == other_params.len()
                            && params.iter().zip(other_params).all(|(param, other)| {
                                java_type_assignable(param, other, symbols, external_facts)
                            })
                            && params.iter().zip(other_params).any(|(param, other)| {
                                !java_type_assignable(other, param, symbols, external_facts)
                            }))
                })
                .then_some(*callable)
        })
}

fn type_contains_unresolved_overload_evidence(ty: &JavaType) -> bool {
    match ty {
        JavaType::Unknown(name) => name != "null",
        JavaType::Named { args, .. } | JavaType::Intersection(args) => {
            args.iter().any(type_contains_unresolved_overload_evidence)
        }
        JavaType::Array(inner) => type_contains_unresolved_overload_evidence(inner),
        JavaType::Wildcard(Some(bound)) => match bound {
            crate::java_ir::WildcardBound::Extends(inner)
            | crate::java_ir::WildcardBound::Super(inner) => {
                type_contains_unresolved_overload_evidence(inner)
            }
        },
        _ => false,
    }
}

fn params_accept_arg_count(params: &[Param], arg_count: usize) -> bool {
    if params.last().is_some_and(|param| param.is_varargs) {
        arg_count >= params.len().saturating_sub(1)
    } else {
        params.len() == arg_count
    }
}

fn fixed_arity_exact_match(
    params: &[Param],
    arg_types: &[JavaType],
    substitutions: &HashMap<String, JavaType>,
) -> bool {
    params.len() == arg_types.len()
        && params.iter().zip(arg_types).all(|(param, arg_ty)| {
            arg_ty == &substitute_method_type_variables(&param.ty, substitutions)
        })
}

fn callable_assignable_score(
    params: &[Param],
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
    substitutions: Option<&HashMap<String, JavaType>>,
) -> Option<u32> {
    let param_types = params
        .iter()
        .map(|param| {
            substitutions
                .map(|substitutions| substitute_method_type_variables(&param.ty, substitutions))
                .unwrap_or_else(|| param.ty.clone())
        })
        .collect::<Vec<_>>();
    callable_match_score(params, &param_types, arg_types, symbols, external_facts)
}

fn callable_match_score(
    params: &[Param],
    param_types: &[JavaType],
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Option<u32> {
    if !params.last().is_some_and(|param| param.is_varargs) {
        return overload_match_score(
            &param_types.iter().collect::<Vec<_>>(),
            arg_types,
            symbols,
            external_facts,
        );
    }

    if arg_types.len() < params.len().saturating_sub(1) {
        return None;
    }

    if params.len() == arg_types.len() {
        let fixed_score = overload_match_score(
            &param_types.iter().collect::<Vec<_>>(),
            arg_types,
            symbols,
            external_facts,
        );
        if fixed_score.is_some() {
            return fixed_score;
        }
    }

    let fixed_count = params.len().saturating_sub(1);
    let fixed_score = overload_match_score(
        &param_types[..fixed_count].iter().collect::<Vec<_>>(),
        &arg_types[..fixed_count],
        symbols,
        external_facts,
    )?;
    let JavaType::Array(vararg_element) = &param_types[fixed_count] else {
        return None;
    };
    let expanded_score = arg_types[fixed_count..]
        .iter()
        .map(|arg_ty| java_type_match_score(arg_ty, vararg_element, symbols, external_facts))
        .sum::<Option<u32>>()?;

    Some(fixed_score + expanded_score + 5)
}

pub fn overload_applicability_report(
    params: &[Param],
    arg_types: &[JavaType],
    source_symbols: &SourceSymbols,
) -> OverloadApplicabilityReport {
    let external_facts = FactSet::default();
    let param_types = params
        .iter()
        .map(|param| param.ty.clone())
        .collect::<Vec<_>>();
    callable_applicability_report(
        params,
        &param_types,
        arg_types,
        &source_symbols.symbols,
        &external_facts,
    )
}

fn callable_applicability_report(
    params: &[Param],
    param_types: &[JavaType],
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> OverloadApplicabilityReport {
    if !params.last().is_some_and(|param| param.is_varargs) {
        return fixed_arity_applicability_report(param_types, arg_types, symbols, external_facts);
    }

    if arg_types.len() < params.len().saturating_sub(1) {
        return OverloadApplicabilityReport {
            mode: None,
            score: None,
            arguments: Vec::new(),
            rejections: vec![OverloadRejectionReason::ArityMismatch {
                expected: params.len().saturating_sub(1),
                actual: arg_types.len(),
                varargs: true,
            }],
        };
    }

    if params.len() == arg_types.len() {
        let fixed =
            fixed_arity_applicability_report(param_types, arg_types, symbols, external_facts);
        if fixed.is_applicable() {
            return fixed;
        }
    }

    let fixed_count = params.len().saturating_sub(1);
    let JavaType::Array(vararg_element) = &param_types[fixed_count] else {
        return OverloadApplicabilityReport {
            mode: None,
            score: None,
            arguments: Vec::new(),
            rejections: vec![OverloadRejectionReason::MissingVarargsArrayParam],
        };
    };
    let mut report = fixed_arity_prefix_applicability_report(
        &param_types[..fixed_count],
        &arg_types[..fixed_count],
        symbols,
        external_facts,
        OverloadApplicabilityMode::VariableArity,
    );
    if !report.rejections.is_empty() {
        return report;
    }
    for (offset, arg_ty) in arg_types[fixed_count..].iter().enumerate() {
        let arg_index = fixed_count + offset;
        push_argument_applicability(
            &mut report,
            arg_index,
            arg_ty,
            vararg_element,
            symbols,
            external_facts,
        );
    }
    if report.rejections.is_empty() {
        let total = report.arguments.iter().map(|arg| arg.score).sum::<u32>() + 5;
        report.score = Some(total);
    }

    report
}

fn java_coercion_evidence_for_params(
    params: &[Param],
    param_types: &[JavaType],
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Vec<JavaCoercionEvidence> {
    callable_applicability_report(params, param_types, arg_types, symbols, external_facts)
        .arguments
        .into_iter()
        .map(|argument| JavaCoercionEvidence {
            arg_index: argument.arg_index,
            actual: argument.actual,
            target: argument.target,
            kind: java_coercion_kind_from_overload_conversion(argument.conversion),
        })
        .collect()
}

fn java_coercion_kind_from_overload_conversion(
    conversion: OverloadArgumentConversion,
) -> JavaCoercionKind {
    match conversion {
        OverloadArgumentConversion::Exact => JavaCoercionKind::Identity,
        OverloadArgumentConversion::PrimitiveWidening => JavaCoercionKind::PrimitiveWidening,
        OverloadArgumentConversion::Boxing => JavaCoercionKind::Boxing,
        OverloadArgumentConversion::Unboxing => JavaCoercionKind::Unboxing,
        OverloadArgumentConversion::BoxingWidening => JavaCoercionKind::BoxingWidening,
        OverloadArgumentConversion::ReferenceWidening => JavaCoercionKind::ReferenceWidening,
        OverloadArgumentConversion::GenericInference => JavaCoercionKind::GenericInference,
        OverloadArgumentConversion::NullToReference => JavaCoercionKind::NullToReference,
        OverloadArgumentConversion::LambdaToFunctionalInterface => {
            JavaCoercionKind::LambdaAdaptation
        }
        OverloadArgumentConversion::MethodReferenceToFunctionalInterface => {
            JavaCoercionKind::MethodReferenceAdaptation
        }
        OverloadArgumentConversion::UnknownFallback => JavaCoercionKind::UnknownFallback,
    }
}

fn overload_match_score(
    params: &[&JavaType],
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Option<u32> {
    if params.len() != arg_types.len() {
        return None;
    }
    params
        .iter()
        .zip(arg_types)
        .map(|(param, arg_ty)| java_type_match_score(arg_ty, param, symbols, external_facts))
        .sum()
}

fn java_type_applicability(
    actual: &JavaType,
    target: &JavaType,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Result<(OverloadArgumentConversion, u32), OverloadRejectionReason> {
    let actual_java = actual.clone();
    let target_java = target.clone();
    if actual == target {
        return Ok((OverloadArgumentConversion::Exact, 0));
    }
    if array_reference_widens_to(actual, target, symbols) {
        return Ok((OverloadArgumentConversion::ReferenceWidening, 60));
    }
    match (actual, target) {
        (
            JavaType::Named {
                name: actual_name,
                args: actual_args,
            },
            JavaType::Named { name: target, .. },
        ) if actual_args.is_empty() && is_method_type_variable(actual_name, None) => {
            let score = if crate::stdlib::is_known_structural_reference_target(target) {
                45
            } else {
                55
            };
            Ok((OverloadArgumentConversion::GenericInference, score))
        }
        (_, JavaType::Named { name, args })
            if args.is_empty() && is_method_type_variable(name, None) =>
        {
            Ok((
                OverloadArgumentConversion::GenericInference,
                if matches!(actual, JavaType::Primitive(_)) {
                    65
                } else {
                    40
                },
            ))
        }
        (JavaType::Primitive(actual), JavaType::Primitive(target)) => {
            primitive_widening_distance(*actual, *target)
                .map(|distance| {
                    (
                        OverloadArgumentConversion::PrimitiveWidening,
                        1 + u32::from(distance),
                    )
                })
                .ok_or_else(|| argument_type_mismatch(&actual_java, &target_java))
        }
        (JavaType::Primitive(actual), JavaType::Named { name: target, .. }) => {
            if target == "Object" {
                Ok((OverloadArgumentConversion::BoxingWidening, 70))
            } else if target == "Number" {
                primitive_can_box_to_number(*actual)
                    .then_some((OverloadArgumentConversion::BoxingWidening, 65))
                    .ok_or_else(|| argument_type_mismatch(&actual_java, &target_java))
            } else {
                crate::stdlib::boxed_primitive_type(target)
                    .and_then(|target| {
                        primitive_widening_distance(*actual, target).map(|distance| {
                            (
                                if distance == 0 {
                                    OverloadArgumentConversion::Boxing
                                } else {
                                    OverloadArgumentConversion::BoxingWidening
                                },
                                2 + u32::from(distance),
                            )
                        })
                    })
                    .ok_or_else(|| argument_type_mismatch(&actual_java, &target_java))
            }
        }
        (JavaType::Named { name: actual, .. }, JavaType::Primitive(target)) => {
            crate::stdlib::boxed_primitive_type(actual)
                .and_then(|actual| {
                    primitive_widening_distance(actual, *target).map(|distance| {
                        (
                            OverloadArgumentConversion::Unboxing,
                            2 + u32::from(distance),
                        )
                    })
                })
                .ok_or_else(|| argument_type_mismatch(&actual_java, &target_java))
        }
        (JavaType::Named { .. }, JavaType::Named { name, .. }) if name == "Object" => {
            Ok((OverloadArgumentConversion::ReferenceWidening, 60))
        }
        (JavaType::String, JavaType::Named { name, .. }) if name == "Object" => {
            Ok((OverloadArgumentConversion::ReferenceWidening, 60))
        }
        (JavaType::String, JavaType::Named { name, .. }) => {
            crate::stdlib::is_known_java_subtype("String", name)
                .then_some((OverloadArgumentConversion::ReferenceWidening, 10))
                .ok_or_else(|| argument_type_mismatch(&actual_java, &target_java))
        }
        (JavaType::Array(actual_inner), JavaType::Array(target_inner)) => {
            if !array_component_conversion_allowed(actual_inner, target_inner) {
                return Err(argument_type_mismatch(&actual_java, &target_java));
            }
            java_type_applicability(actual_inner, target_inner, symbols, external_facts)
                .map(|(conversion, score)| (conversion, score + 1))
                .map_err(|_| argument_type_mismatch(&actual_java, &target_java))
        }
        (
            JavaType::Named {
                name: actual,
                args: actual_args,
            },
            JavaType::Named {
                name: target,
                args: target_args,
            },
        ) => named_reference_match_score(
            actual,
            actual_args,
            target,
            target_args,
            symbols,
            external_facts,
        )
        .map(|score| (OverloadArgumentConversion::ReferenceWidening, score))
        .ok_or_else(|| argument_type_mismatch(&actual_java, &target_java)),
        (JavaType::Unknown(name), _) if name == "null" => {
            if matches!(target, JavaType::Primitive(_)) {
                Err(OverloadRejectionReason::NullToPrimitive {
                    arg_index: 0,
                    target: target_java.clone(),
                })
            } else {
                Ok((OverloadArgumentConversion::NullToReference, 30))
            }
        }
        (JavaType::Unknown(name), JavaType::Named { name: target, .. }) if name == "lambda" => {
            symbols
                .is_functional_interface_type(target)
                .then_some((OverloadArgumentConversion::LambdaToFunctionalInterface, 20))
                .ok_or_else(
                    || OverloadRejectionReason::LambdaTargetNotFunctionalInterface {
                        arg_index: 0,
                        target: target_java.clone(),
                    },
                )
        }
        (JavaType::Unknown(name), JavaType::Named { name: target, .. })
            if name == "method_reference" =>
        {
            symbols
                .is_functional_interface_type(target)
                .then_some((
                    OverloadArgumentConversion::MethodReferenceToFunctionalInterface,
                    20,
                ))
                .ok_or_else(|| {
                    OverloadRejectionReason::MethodReferenceTargetNotFunctionalInterface {
                        arg_index: 0,
                        target: target_java.clone(),
                    }
                })
        }
        (JavaType::Unknown(name), JavaType::Named { name: target, .. })
            if unknown_collection_factory_matches_target(name, target) =>
        {
            Ok((OverloadArgumentConversion::UnknownFallback, 25))
        }
        (JavaType::Unknown(_), JavaType::Void) => {
            Err(argument_type_mismatch(&actual_java, &target_java))
        }
        (JavaType::Unknown(_), JavaType::Primitive(_)) => {
            Ok((OverloadArgumentConversion::UnknownFallback, 95))
        }
        (JavaType::Unknown(_), _) => Ok((OverloadArgumentConversion::UnknownFallback, 80)),
        _ => Err(argument_type_mismatch(&actual_java, &target_java)),
    }
}

fn argument_type_mismatch(actual: &JavaType, target: &JavaType) -> OverloadRejectionReason {
    OverloadRejectionReason::ArgumentTypeMismatch {
        arg_index: 0,
        actual: actual.clone(),
        target: target.clone(),
    }
}

fn fixed_arity_applicability_report(
    param_types: &[JavaType],
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> OverloadApplicabilityReport {
    fixed_arity_prefix_applicability_report(
        param_types,
        arg_types,
        symbols,
        external_facts,
        OverloadApplicabilityMode::FixedArity,
    )
}

fn fixed_arity_prefix_applicability_report(
    param_types: &[JavaType],
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
    mode: OverloadApplicabilityMode,
) -> OverloadApplicabilityReport {
    if param_types.len() != arg_types.len() {
        return OverloadApplicabilityReport {
            mode: None,
            score: None,
            arguments: Vec::new(),
            rejections: vec![OverloadRejectionReason::ArityMismatch {
                expected: param_types.len(),
                actual: arg_types.len(),
                varargs: mode == OverloadApplicabilityMode::VariableArity,
            }],
        };
    }
    let mut report = OverloadApplicabilityReport {
        mode: Some(mode),
        score: None,
        arguments: Vec::new(),
        rejections: Vec::new(),
    };
    for (arg_index, (target, actual)) in param_types.iter().zip(arg_types).enumerate() {
        push_argument_applicability(
            &mut report,
            arg_index,
            actual,
            target,
            symbols,
            external_facts,
        );
    }
    if report.rejections.is_empty() {
        report.score = Some(report.arguments.iter().map(|arg| arg.score).sum());
    }
    report
}

fn push_argument_applicability(
    report: &mut OverloadApplicabilityReport,
    arg_index: usize,
    actual: &JavaType,
    target: &JavaType,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) {
    match java_type_applicability(actual, target, symbols, external_facts) {
        Ok((conversion, score)) => report.arguments.push(OverloadArgumentApplicability {
            arg_index,
            actual: actual.clone(),
            target: target.clone(),
            conversion,
            score,
        }),
        Err(reason) => report.rejections.push(reason.with_arg_index(arg_index)),
    }
}

impl OverloadRejectionReason {
    fn with_arg_index(self, arg_index: usize) -> Self {
        match self {
            OverloadRejectionReason::ArgumentTypeMismatch { actual, target, .. } => {
                OverloadRejectionReason::ArgumentTypeMismatch {
                    arg_index,
                    actual,
                    target,
                }
            }
            OverloadRejectionReason::NullToPrimitive { target, .. } => {
                OverloadRejectionReason::NullToPrimitive { arg_index, target }
            }
            OverloadRejectionReason::LambdaTargetNotFunctionalInterface { target, .. } => {
                OverloadRejectionReason::LambdaTargetNotFunctionalInterface { arg_index, target }
            }
            OverloadRejectionReason::MethodReferenceTargetNotFunctionalInterface {
                target, ..
            } => OverloadRejectionReason::MethodReferenceTargetNotFunctionalInterface {
                arg_index,
                target,
            },
            other => other,
        }
    }
}

// Array covariance is a reference conversion. It neither boxes nor widens
// primitive elements: int[] is an Object, but is not Object[] or long[].
fn array_component_conversion_allowed(actual: &JavaType, target: &JavaType) -> bool {
    actual == target
        || (!matches!(actual, JavaType::Primitive(_)) && !matches!(target, JavaType::Primitive(_)))
}

fn array_reference_widens_to(
    actual: &JavaType,
    target: &JavaType,
    symbols: &ProgramSymbols,
) -> bool {
    matches!(actual, JavaType::Array(_))
        && matches!(target, JavaType::Named { name, args } if args.is_empty()
            && matches!(name.as_str(), "Object" | "java.lang.Object" | "Cloneable" | "java.lang.Cloneable" | "Serializable" | "java.io.Serializable")
            && symbols.source_type_kind(name).is_none())
}

fn java_type_match_score(
    actual: &JavaType,
    target: &JavaType,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Option<u32> {
    if actual == target {
        return Some(0);
    }
    if array_reference_widens_to(actual, target, symbols) {
        return Some(60);
    }
    // Fixed JDK Comparable<Self> supertypes have no type arguments on the
    // concrete class itself. Project the self argument before checking the
    // interface's invariant/wildcard arguments; nominal acceptance alone
    // would incorrectly accept String as Comparable<Integer>.
    if let JavaType::Named { name, args } = target
        && matches!(name.as_str(), "Comparable" | "java.lang.Comparable")
        && symbols.source_type_kind(name).is_none()
        && match actual {
            JavaType::String => true,
            JavaType::Named { name, args } => {
                args.is_empty()
                    && symbols.source_type_kind(name).is_none()
                    && crate::stdlib::is_known_self_comparable_type(name)
            }
            _ => false,
        }
    {
        return generic_arguments_match_score(
            std::slice::from_ref(actual),
            args,
            symbols,
            external_facts,
        )
        .map(|score| 10 + score);
    }
    match (actual, target) {
        (
            JavaType::Named {
                name: actual_name,
                args: actual_args,
            },
            JavaType::Named { name: target, .. },
        ) if actual_args.is_empty() && is_method_type_variable(actual_name, None) => {
            if crate::stdlib::is_known_structural_reference_target(target) {
                Some(45)
            } else {
                Some(55)
            }
        }
        (_, JavaType::Named { name, args })
            if args.is_empty() && is_method_type_variable(name, None) =>
        {
            Some(if matches!(actual, JavaType::Primitive(_)) {
                65
            } else {
                40
            })
        }
        (JavaType::Primitive(actual), JavaType::Primitive(target)) => {
            primitive_widening_distance(*actual, *target).map(|distance| 1 + u32::from(distance))
        }
        (JavaType::Primitive(actual), JavaType::Named { name: target, .. }) => {
            if target == "Object" {
                Some(70)
            } else if target == "Number" {
                primitive_can_box_to_number(*actual).then_some(65)
            } else {
                crate::stdlib::boxed_primitive_type(target).and_then(|target| {
                    primitive_widening_distance(*actual, target)
                        .map(|distance| 2 + u32::from(distance))
                })
            }
        }
        (JavaType::Named { name: actual, .. }, JavaType::Primitive(target)) => {
            crate::stdlib::boxed_primitive_type(actual).and_then(|actual| {
                primitive_widening_distance(actual, *target).map(|distance| 2 + u32::from(distance))
            })
        }
        (JavaType::Named { .. }, JavaType::Named { name, .. }) if name == "Object" => Some(60),
        (JavaType::String, JavaType::Named { name, .. }) if name == "Object" => Some(60),
        (JavaType::String, JavaType::Named { name, .. }) => {
            crate::stdlib::is_known_java_subtype("String", name).then_some(10)
        }
        (JavaType::Array(actual_inner), JavaType::Array(target_inner)) => {
            if !array_component_conversion_allowed(actual_inner, target_inner) {
                return None;
            }
            java_type_match_score(actual_inner, target_inner, symbols, external_facts)
                .map(|score| score + 1)
        }
        (
            JavaType::Named {
                name: actual,
                args: actual_args,
            },
            JavaType::Named {
                name: target,
                args: target_args,
            },
        ) => named_reference_match_score(
            actual,
            actual_args,
            target,
            target_args,
            symbols,
            external_facts,
        ),
        (JavaType::Unknown(name), _) if name == "null" => {
            (!matches!(target, JavaType::Primitive(_))).then_some(30)
        }
        (JavaType::Unknown(name), JavaType::Named { name: target, .. }) if name == "lambda" => {
            symbols.is_functional_interface_type(target).then_some(20)
        }
        (JavaType::Unknown(name), JavaType::Named { name: target, .. })
            if name == "method_reference" =>
        {
            symbols.is_functional_interface_type(target).then_some(20)
        }
        (JavaType::Unknown(name), JavaType::Named { name: target, .. })
            if unknown_collection_factory_matches_target(name, target) =>
        {
            Some(25)
        }
        (JavaType::Unknown(_), JavaType::Void) => None,
        (JavaType::Unknown(_), JavaType::Primitive(_)) => Some(95),
        (JavaType::Unknown(_), _) => Some(80),
        _ => None,
    }
}

fn unknown_collection_factory_matches_target(name: &str, target: &str) -> bool {
    matches!(
        (name, target),
        ("emptyList", "List" | "Collection" | "Iterable")
            | ("emptySet", "Set" | "Collection" | "Iterable")
            | ("emptyMap", "Map")
    )
}

fn java_type_assignable(
    actual: &JavaType,
    target: &JavaType,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> bool {
    java_type_match_score(actual, target, symbols, external_facts).is_some()
}

fn named_reference_match_score(
    actual: &str,
    actual_args: &[JavaType],
    target: &str,
    target_args: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Option<u32> {
    if actual == target {
        return generic_arguments_match_score(actual_args, target_args, symbols, external_facts);
    }

    if crate::facts::external_type_name_matches(actual, target) {
        return generic_arguments_match_score(actual_args, target_args, symbols, external_facts)
            .map(|score| score + 1);
    }

    if target == "Enum" && symbols.is_source_enum_type(actual) {
        return generic_arguments_match_score(actual_args, target_args, symbols, external_facts)
            .or_else(|| target_args.iter().all(is_unbounded_wildcard).then_some(12));
    }

    if crate::stdlib::is_known_single_arg_generic_supertype(actual, target) {
        return generic_supertype_match_score(actual_args, target_args, symbols, external_facts)
            .map(|score| 10 + score);
    }

    if external_facts
        .external_conversion_adapter_for(actual, target)
        .is_some()
        || crate::external_packs::external_conversion_adapter_for(actual, target).is_some()
    {
        return generic_arguments_match_score(actual_args, target_args, symbols, external_facts)
            .map(|score| 20 + score);
    }

    if let Some(distance) = symbols.source_subtype_distance(actual, target, external_facts) {
        return Some(
            10 + distance
                + generic_arguments_match_score(actual_args, target_args, symbols, external_facts)?,
        );
    }

    crate::stdlib::is_known_java_subtype(actual, target).then(|| {
        generic_arguments_match_score(actual_args, target_args, symbols, external_facts)
            .unwrap_or(10)
    })
}

fn generic_supertype_match_score(
    actual_args: &[JavaType],
    target_args: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Option<u32> {
    if target_args.is_empty() {
        return Some(0);
    }
    if actual_args.is_empty() {
        return Some(50);
    }
    generic_arguments_match_score(actual_args, target_args, symbols, external_facts)
}

fn generic_arguments_match_score(
    actual_args: &[JavaType],
    target_args: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Option<u32> {
    match (actual_args, target_args) {
        (_, []) => Some(0),
        ([], targets) if targets.iter().all(is_unbounded_wildcard) => Some(5),
        ([], _) => Some(50),
        (actuals, targets) if actuals.len() == targets.len() => actuals
            .iter()
            .zip(targets)
            .map(|(actual, target)| {
                generic_argument_match_score(actual, target, symbols, external_facts)
            })
            .sum(),
        _ => None,
    }
}

fn generic_argument_match_score(
    actual: &JavaType,
    target: &JavaType,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Option<u32> {
    if actual == target {
        return Some(0);
    }
    match (actual, target) {
        (JavaType::Primitive(actual), JavaType::Named { name: target, .. }) => {
            if target == "Object" {
                Some(70)
            } else if target == "Number" {
                primitive_can_box_to_number(*actual).then_some(65)
            } else {
                crate::stdlib::boxed_primitive_type(target).and_then(|target| {
                    primitive_widening_distance(*actual, target)
                        .map(|distance| 2 + u32::from(distance))
                })
            }
        }
        (JavaType::Named { name: actual, .. }, JavaType::Primitive(target)) => {
            crate::stdlib::boxed_primitive_type(actual).and_then(|actual| {
                primitive_widening_distance(actual, *target).map(|distance| 2 + u32::from(distance))
            })
        }
        (JavaType::Array(actual_inner), JavaType::Array(target_inner)) => {
            if !array_component_conversion_allowed(actual_inner, target_inner) {
                return None;
            }
            generic_argument_match_score(actual_inner, target_inner, symbols, external_facts)
                .map(|score| score + 1)
        }
        (
            JavaType::Named {
                name: actual_name,
                args: actual_args,
            },
            JavaType::Named {
                name: target_name,
                args: target_args,
            },
        ) if actual_name == target_name => {
            generic_arguments_match_score(actual_args, target_args, symbols, external_facts)
                .map(|score| score + 1)
        }
        (
            JavaType::Named {
                name: actual_name,
                args: actual_args,
            },
            JavaType::Named {
                name: target_name,
                args: target_args,
            },
        ) if crate::facts::external_type_name_matches(actual_name, target_name) => {
            generic_arguments_match_score(actual_args, target_args, symbols, external_facts)
                .map(|score| score + 2)
        }
        (_, JavaType::Named { name, args })
            if args.is_empty() && is_method_type_variable(name, None) =>
        {
            Some(if matches!(actual, JavaType::Primitive(_)) {
                45
            } else {
                20
            })
        }
        (_, JavaType::Wildcard(None)) => Some(5),
        (_, JavaType::Wildcard(Some(WildcardBound::Extends(bound)))) => {
            java_type_match_score(actual, bound, symbols, external_facts).map(|score| 5 + score)
        }
        (_, JavaType::Wildcard(Some(WildcardBound::Super(bound)))) => {
            java_type_match_score(bound, actual, symbols, external_facts).map(|score| 5 + score)
        }
        _ => None,
    }
}

fn is_unbounded_wildcard(ty: &JavaType) -> bool {
    matches!(ty, JavaType::Wildcard(None))
}

fn is_method_type_variable(name: &str, declared_method_type_params: Option<&[String]>) -> bool {
    if let Some(declared) = declared_method_type_params {
        return declared.iter().any(|param| param == name);
    }
    let mut chars = name.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_uppercase())
        && chars.all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
}

/// How far `actual` must widen to reach `target`, for ranking overloads.
///
/// The rank-subtraction form this replaces put `char` beside `short` on one
/// chain, so `char`→`short` and `short`→`char` both scored **distance 0** —
/// an exact match — and `f(short)` tied with `f(char)` for a `char` argument.
/// Neither conversion exists in Java without a cast.
fn primitive_widening_distance(
    actual: crate::java_ir::PrimitiveType,
    target: crate::java_ir::PrimitiveType,
) -> Option<u8> {
    crate::types::primitive_widening_steps(actual, target)
}

fn primitive_can_box_to_number(primitive: crate::java_ir::PrimitiveType) -> bool {
    use crate::java_ir::PrimitiveType::*;
    matches!(primitive, Byte | Short | Int | Long | Float | Double)
}

fn collect_resolved_facts(
    program: &Program,
    symbols: &SourceSymbols,
    external_facts: &FactSet,
) -> FactSet {
    let mut facts = collect_capture_facts(program, symbols, external_facts);
    collect_type_capability_facts(program, &mut facts);
    collect_generic_parameter_variance_facts(program, &mut facts);
    facts
}

fn contains_type_parameter(ty: &JavaType, parameter: &str) -> bool {
    match ty {
        JavaType::Named { name, args } => {
            (name == parameter && args.is_empty())
                || args
                    .iter()
                    .any(|arg| contains_type_parameter(arg, parameter))
        }
        JavaType::Array(element) => contains_type_parameter(element, parameter),
        JavaType::Intersection(bounds) => bounds
            .iter()
            .any(|bound| contains_type_parameter(bound, parameter)),
        JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Extends(bound)))
        | JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Super(bound))) => {
            contains_type_parameter(bound, parameter)
        }
        JavaType::Void | JavaType::Primitive(_) | JavaType::String | JavaType::Wildcard(None) => {
            false
        }
        JavaType::Unknown(name) => name == parameter,
    }
}

fn collect_generic_parameter_variance_facts(program: &Program, facts: &mut FactSet) {
    fn visit(ty: &TypeDecl, facts: &mut FactSet) {
        for (parameter_index, parameter) in ty.type_params.iter().enumerate() {
            let mut consumed = false;
            let mut produced = false;
            for member in &ty.members {
                match member {
                    Member::Field(field) if contains_type_parameter(&field.ty, parameter) => {
                        consumed = true;
                        produced = true;
                    }
                    Member::Method(method) if !method.type_params.contains(parameter) => {
                        produced |= contains_type_parameter(&method.return_type, parameter);
                        consumed |= method
                            .params
                            .iter()
                            .any(|param| contains_type_parameter(&param.ty, parameter));
                    }
                    Member::Constructor(constructor)
                        if !constructor.type_params.contains(parameter) =>
                    {
                        consumed |= constructor
                            .params
                            .iter()
                            .any(|param| contains_type_parameter(&param.ty, parameter));
                    }
                    _ => {}
                }
            }
            facts.generic_parameter_variance_facts.push(
                crate::facts::GenericParameterVarianceFact {
                    type_name: ty.name.clone(),
                    parameter_index,
                    consumed,
                    produced,
                    provenance: crate::facts::FactProvenance::ParsedSource,
                },
            );
        }
        for member in &ty.members {
            if let Member::Type(nested) = member {
                visit(nested, facts);
            }
        }
    }

    for ty in &program.types {
        visit(ty, facts);
    }
}

fn collect_capture_facts(
    program: &Program,
    symbols: &SourceSymbols,
    external_facts: &FactSet,
) -> FactSet {
    let mut analyzer = CaptureAnalyzer {
        symbols: Some(symbols),
        external_facts: Some(external_facts),
        ..CaptureAnalyzer::default()
    };
    for ty in &program.types {
        analyzer.analyze_top_level_type(ty);
    }
    let mut captures = CaptureFacts {
        owners: analyzer.owners,
        ..CaptureFacts::default()
    };
    crate::outer_field_capture::analyze(program, &mut captures);
    FactSet {
        captures,
        ..FactSet::default()
    }
}

fn collect_type_capability_facts(program: &Program, facts: &mut FactSet) {
    for ty in &program.types {
        collect_type_capability_facts_for_type(ty, facts, &[]);
    }
}

fn collect_field_declaration_facts(program: &Program, facts: &mut FactSet) {
    for ty in &program.types {
        collect_field_declaration_facts_for_type(ty, facts, program.package.as_deref(), None);
    }
}

fn collect_field_declaration_facts_for_type(
    ty: &TypeDecl,
    facts: &mut FactSet,
    package: Option<&str>,
    outer_name: Option<&str>,
) {
    let type_name = package_qualified_source_type_name(ty, package, outer_name);
    let enum_variants = crate::object_model::java_enum_variant_names(ty)
        .into_iter()
        .collect::<HashSet<_>>();
    for member in &ty.members {
        match member {
            Member::Field(field) => {
                let is_static_final = field.modifiers.contains(&Modifier::Static)
                    && field.modifiers.contains(&Modifier::Final);
                let is_enum_variant = ty.kind == TypeKind::Enum
                    && enum_variants.contains(
                        &crate::object_model::rust_enum_variant_name_from_parsed_constant(
                            &field.name,
                        ),
                    );
                facts.external_field_facts.push(ExternalFieldFact {
                    field: FieldKey::named(type_name.clone(), field.name.clone()),
                    field_type: Some(field.ty.clone()),
                    field_kind: if is_enum_variant {
                        ExternalFieldKind::EnumVariant
                    } else if is_static_final {
                        ExternalFieldKind::StaticFinal
                    } else {
                        ExternalFieldKind::Unknown
                    },
                    provenance: FactProvenance::ParsedSource,
                });
            }
            Member::Type(nested) => {
                collect_field_declaration_facts_for_type(nested, facts, package, Some(&type_name))
            }
            _ => {}
        }
    }
}

/// The declared values of every `static final` field that is a Java constant
/// expression.
///
/// Without this the value is simply dropped and the facade emits a
/// `compile_error!` for a constant whose initialiser the translation just
/// parsed — group 3 of
/// [0031](../claude_audits/done/0031-stubbed-constant-census.md). Anything that does
/// not fold is left alone, so the diagnostic still stands wherever the value
/// really is unknown.
///
/// Run over the whole program rather than per type, because a constant may be
/// defined in terms of one owned by another class
/// (`VariantContext.NO_LOG10_PERROR = CommonInfo.NO_LOG10_PERROR`). That is a
/// dependency graph, so it is iterated to a fixpoint instead of assuming
/// declaration order resolves it.
fn collect_static_final_constant_value_facts(program: &Program, facts: &mut FactSet) {
    let declarations = static_final_constant_declarations(program);
    facts.extend_from(constant_value_facts_from_declarations(&declarations));
}

/// Every `static final` field of `program`, paired with its package-qualified
/// owner.
///
/// Exposed so a caller holding several parsed files can pool them: type names
/// are qualified with each file's *own* package, which a merged program no
/// longer knows.
pub(crate) fn static_final_constant_declarations(program: &Program) -> Vec<ConstantDeclaration> {
    let mut declarations = Vec::new();
    for ty in &program.types {
        collect_static_final_constant_declarations(
            ty,
            program.package.as_deref(),
            None,
            &mut declarations,
        );
    }
    declarations
}

/// One `static final` declaration, with what a fold needs to resolve it.
///
/// The package is carried separately rather than recovered from `owner`,
/// because a nested type's owner is `pkg.Outer.Inner` and splitting it would
/// give `pkg.Outer` as the package. Getting that wrong is how a simple-name
/// reference resolves into the wrong package
/// ([0031](../claude_audits/done/0031-stubbed-constant-census.md)).
#[derive(Debug, Clone)]
pub(crate) struct ConstantDeclaration {
    /// Package-qualified owner, e.g. `pkg1.Constants` or `pkg1.Outer.Inner`.
    owner: JavaOwnerKey,
    /// The declaring file's package, if any.
    package: Option<String>,
    field: crate::java_ir::FieldDecl,
}

/// Folds `declarations` to a fixpoint and reports the values that resolve.
pub(crate) fn constant_value_facts_from_declarations(
    declarations: &[ConstantDeclaration],
) -> FactSet {
    let (facts, _) = fold_constant_declarations(declarations);
    facts
}

/// Folds `declarations` and returns both the value facts and the scope they
/// were folded in.
///
/// The scope is what a *later* fold needs in order to resolve a constant owned
/// by another type — `static int[] a = new int[Other.N];`, which is ordinary
/// Java and which a per-owner scope cannot answer
/// ([0039](../claude_audits/done/0039-static-array-fields-lose-their-value-and-their-writes.md)).
pub(crate) fn fold_constant_declarations(
    declarations: &[ConstantDeclaration],
) -> (FactSet, crate::source_constant_values::ConstantScope) {
    let mut facts = FactSet::default();
    let mut scope = crate::source_constant_values::ConstantScope::new();
    let mut folded: std::collections::BTreeMap<
        usize,
        crate::source_constant_values::ConstantValue,
    > = std::collections::BTreeMap::new();
    // Each round can only resolve references to constants folded in an earlier
    // round, so the number of rounds is bounded by the longest chain. Stopping
    // when a round adds nothing is what makes that bound implicit.
    loop {
        let mut progressed = false;
        for (index, declaration) in declarations.iter().enumerate() {
            if folded.contains_key(&index) {
                continue;
            }
            let Some(initializer) = declaration.field.initializer.as_ref() else {
                continue;
            };
            // A type's own constants are visible to it as bare names, and must
            // not leak to any other owner — so they go into a copy of the scope
            // pointed at this declaration.
            let mut owner_scope = scope.clone();
            let mut own = std::collections::BTreeMap::new();
            for (other_index, other) in declarations.iter().enumerate() {
                if other.owner != declaration.owner {
                    continue;
                }
                let Some(value) = folded.get(&other_index) else {
                    continue;
                };
                for alias in [
                    other.field.name.clone(),
                    crate::parser::to_snake_case(&other.field.name),
                ] {
                    own.insert(alias, value.clone());
                }
            }
            owner_scope.focus_on_owner(declaration.package.as_deref(), own);

            let Some(value) =
                crate::source_constant_values::fold_constant_expr(initializer, &owner_scope)
            else {
                continue;
            };
            scope.record(
                declaration.owner.as_str(),
                &declaration.field.name,
                value.clone(),
            );
            folded.insert(index, value);
            progressed = true;
        }
        if !progressed {
            break;
        }
    }

    for (index, declaration) in declarations.iter().enumerate() {
        let Some(value) = folded.get(&index) else {
            continue;
        };
        if let Some(rust_value) = value.rust_value_for(&declaration.field.ty) {
            facts
                .external_field_value_facts
                .push(crate::facts::ExternalFieldValueFact {
                    field: FieldKey::named(
                        declaration.owner.as_str(),
                        declaration.field.name.clone(),
                    ),
                    rust_value,
                    provenance: FactProvenance::ParsedSource,
                });
        }
    }
    (facts, scope)
}

fn collect_static_final_constant_declarations(
    ty: &TypeDecl,
    package: Option<&str>,
    outer_name: Option<&str>,
    into: &mut Vec<ConstantDeclaration>,
) {
    let type_name = package_qualified_source_type_name(ty, package, outer_name);
    for member in &ty.members {
        match member {
            Member::Field(field)
                if field.modifiers.contains(&Modifier::Static)
                    && field.modifiers.contains(&Modifier::Final) =>
            {
                into.push(ConstantDeclaration {
                    owner: JavaOwnerKey::resolved_source(type_name.clone()),
                    package: package.map(str::to_string),
                    field: field.clone(),
                });
            }
            Member::Type(nested) => {
                collect_static_final_constant_declarations(nested, package, Some(&type_name), into)
            }
            _ => {}
        }
    }
}

fn package_qualified_source_type_name(
    ty: &TypeDecl,
    package: Option<&str>,
    outer_name: Option<&str>,
) -> String {
    if ty.name.contains('.') {
        return ty.name.clone();
    }
    if let Some(outer_name) = outer_name {
        return format!("{outer_name}.{}", ty.name);
    }
    match package {
        Some(package) if !package.is_empty() => format!("{package}.{}", ty.name),
        _ => ty.name.clone(),
    }
}

fn collect_type_capability_facts_for_type(
    ty: &TypeDecl,
    facts: &mut FactSet,
    inferred_final_type_names: &[String],
) {
    let object_safety = (ty.kind == TypeKind::Interface).then(|| interface_object_safety_fact(ty));
    let inferred_closed_nested_variants = inferred_closed_nested_variants_for_type(ty);
    let inferred_final = inferred_final_type_names
        .iter()
        .any(|type_name| type_name == &ty.name);
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: ty.name.clone(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: ty
                .implements
                .iter()
                .map(|name| crate::parser::parse_hierarchy_type(name))
                .collect(),
            extends: ty
                .extends
                .as_ref()
                .map(|name| crate::parser::parse_hierarchy_type(name)),
            final_type: Some(ty.modifiers.contains(&Modifier::Final) || inferred_final),
            object_safe_trait: object_safety.as_ref().map(|fact| fact.object_safe),
            provenance: FactProvenance::ParsedSource,
        });
    if let Some(object_safety) = object_safety {
        facts.object_safety_facts.push(object_safety);
    }
    if (ty.modifiers.contains(&Modifier::Sealed) && !ty.permits.is_empty())
        || !inferred_closed_nested_variants.is_empty()
    {
        facts.closed_hierarchy_facts.push(ClosedHierarchyFact {
            type_name: ty.name.clone(),
            closed: true,
            provenance: if inferred_closed_nested_variants.is_empty() {
                FactProvenance::ParsedSource
            } else {
                FactProvenance::CorpusAnalysis
            },
        });
    }
    if ty.modifiers.contains(&Modifier::Sealed) && !ty.permits.is_empty() {
        facts.sealed_permits_facts.push(SealedPermitsFact {
            type_name: ty.name.clone(),
            permitted_subtypes: ty.permits.clone(),
            provenance: FactProvenance::ParsedSource,
        });
    }
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_type_capability_facts_for_type(nested, facts, &inferred_closed_nested_variants);
        }
    }
}

fn inferred_closed_nested_variants_for_type(ty: &TypeDecl) -> Vec<String> {
    if ty.kind != TypeKind::Class || !ty.modifiers.contains(&Modifier::Abstract) {
        return Vec::new();
    }
    let variants = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Type(nested)
                if nested.kind == TypeKind::Class
                    && !nested.modifiers.contains(&Modifier::Abstract)
                    && nested_extends_type(nested, &ty.name)
                    && constructors_are_not_publicly_extensible(nested) =>
            {
                Some(nested.name.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if variants.is_empty() {
        return Vec::new();
    }
    let all_direct_nested_subclasses_are_variants = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Type(nested) if nested_extends_type(nested, &ty.name) => Some(nested),
            _ => None,
        })
        .all(|nested| variants.iter().any(|variant| variant == &nested.name));
    all_direct_nested_subclasses_are_variants
        .then_some(variants)
        .unwrap_or_default()
}

fn nested_extends_type(ty: &TypeDecl, super_name: &str) -> bool {
    ty.extends
        .as_ref()
        .map(|extends| crate::parser::parse_hierarchy_type(extends))
        .as_ref()
        .and_then(java_type_simple_name_option)
        == Some(super_name)
}

fn java_type_simple_name_option(java_ty: &JavaType) -> Option<&str> {
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            Some(name.rsplit('.').next().unwrap_or(name))
        }
        JavaType::String => Some("String"),
        _ => None,
    }
}

fn constructors_are_not_publicly_extensible(ty: &TypeDecl) -> bool {
    let constructors = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Constructor(constructor) => Some(constructor),
            _ => None,
        })
        .collect::<Vec<_>>();
    if constructors.is_empty() {
        return !ty.modifiers.contains(&Modifier::Public)
            && !ty.modifiers.contains(&Modifier::Protected);
    }
    constructors.iter().all(|constructor| {
        !constructor.modifiers.contains(&Modifier::Public)
            && !constructor.modifiers.contains(&Modifier::Protected)
    })
}

fn interface_object_safety_fact(ty: &TypeDecl) -> ObjectSafetyFact {
    let mut violations = Vec::new();
    collect_interface_object_safety_violations(ty, &mut violations);
    ObjectSafetyFact {
        type_name: ty.name.clone(),
        object_safe: violations.is_empty(),
        violations,
        provenance: FactProvenance::ParsedSource,
    }
}

fn collect_interface_object_safety_violations(
    ty: &TypeDecl,
    violations: &mut Vec<ObjectSafetyViolation>,
) {
    for member in &ty.members {
        match member {
            // A named Java interface is a reference value, including when it
            // names the enclosing interface. Its Rust carrier is a boxed or
            // shared trait object, never Rust's unsized `Self`. Generic
            // methods have their separate `Self: Sized` signature boundary.
            Member::Method(_) => {}
            Member::Type(_) => {}
            Member::Unsupported { .. } => violations.push(ObjectSafetyViolation::UnsupportedMember),
            Member::Field(_) => {}
            Member::Constructor(_) => violations.push(ObjectSafetyViolation::ConstructorMember),
        }
    }
}

/// Scope capture discovery to one callable, retaining its declaration IDs.
pub(crate) fn captures_in_callable(params: &[Param], body: &Block) -> Vec<Capture> {
    let mut analyzer = CaptureAnalyzer::default();
    let mut context = CaptureContext {
        type_name: String::new(),
        enclosing_receivers: Vec::new(),
        receiver_type: None,
        receiver_fields: HashSet::new(),
        capturing_receiver: false,
        method_name: None,
        return_type: None,
        fields: HashMap::new(),
        outer_field_owners: HashMap::new(),
        scopes: vec![method_param_frame(params, false)],
        mutated_names: mutated_names_in_block(body),
    };
    analyzer.analyze_block(body, &mut context, None);
    analyzer
        .owners
        .into_iter()
        .flat_map(|owner| owner.captures)
        .collect()
}

/// Reuse lexical capture analysis for lowering, including nested closures,
/// and retain only declarations visible at this closure's creation boundary.
pub(crate) fn captures_for_lambda<'a>(
    params: &[crate::java_ir::LambdaParam],
    body: &LambdaBody,
    bindings: impl Iterator<Item = (&'a str, &'a JavaType, crate::binding_id::BindingId)>,
) -> Vec<Capture> {
    let symbols = bindings
        .map(|(name, ty, binding)| {
            (
                name.to_string(),
                ScopeSymbol {
                    binding: Some(binding),
                    ty: ty.clone(),
                    source: CaptureSource::Local,
                },
            )
        })
        .collect::<HashMap<_, _>>();
    if symbols.is_empty() {
        return Vec::new();
    }
    let outer_ids = symbols
        .values()
        .filter_map(|symbol| symbol.binding.clone())
        .collect::<HashSet<_>>();
    let context = CaptureContext {
        type_name: String::new(),
        enclosing_receivers: Vec::new(),
        receiver_type: None,
        receiver_fields: HashSet::new(),
        capturing_receiver: false,
        method_name: None,
        return_type: None,
        fields: HashMap::new(),
        outer_field_owners: HashMap::new(),
        scopes: vec![ScopeFrame {
            symbols,
            capturable: true,
        }],
        mutated_names: HashSet::new(),
    };
    let mut analyzer = CaptureAnalyzer::default();
    analyzer.analyze_lambda(params, body, &context);
    let mut captures = analyzer
        .owners
        .into_iter()
        .flat_map(|owner| owner.captures)
        .filter(|capture| {
            capture
                .binding
                .as_ref()
                .is_some_and(|binding| outer_ids.contains(binding))
        })
        .collect::<Vec<_>>();
    let mut seen = HashSet::new();
    captures.retain(|capture| seen.insert(capture.binding.clone()));
    captures
}

#[derive(Debug, Default)]
struct CaptureAnalyzer<'a> {
    next_id: usize,
    owners: Vec<CaptureOwner>,
    symbols: Option<&'a SourceSymbols>,
    external_facts: Option<&'a FactSet>,
    anonymous_receivers: Vec<Option<(JavaType, Vec<MethodDecl>)>>,
}

#[derive(Debug, Clone)]
struct ScopeFrame {
    symbols: HashMap<String, ScopeSymbol>,
    capturable: bool,
}

#[derive(Debug, Clone)]
struct ScopeSymbol {
    binding: Option<crate::binding_id::BindingId>,
    ty: JavaType,
    source: CaptureSource,
}

#[derive(Debug, Clone)]
struct CaptureContext {
    type_name: String,
    enclosing_receivers: Vec<JavaType>,
    receiver_type: Option<JavaType>,
    receiver_fields: HashSet<String>,
    capturing_receiver: bool,
    method_name: Option<String>,
    return_type: Option<JavaType>,
    fields: HashMap<String, JavaType>,
    outer_field_owners: HashMap<String, String>,
    scopes: Vec<ScopeFrame>,
    mutated_names: HashSet<String>,
}

impl CaptureAnalyzer<'_> {
    fn analyze_top_level_type(&mut self, ty: &TypeDecl) {
        self.analyze_type_members(ty, &[]);
    }

    fn analyze_type_members(&mut self, ty: &TypeDecl, enclosing_types: &[&TypeDecl]) {
        let fields = field_types(ty);
        let outer_field_owners = enclosing_instance_field_owners(enclosing_types);

        for member in &ty.members {
            match member {
                Member::Method(method) => {
                    if let Some(body) = &method.body {
                        let mut ctx = CaptureContext {
                            type_name: ty.name.clone(),
                            enclosing_receivers: enclosing_types
                                .iter()
                                .map(|ty| capture_receiver_type(ty))
                                .collect(),
                            receiver_type: (!method.modifiers.contains(&Modifier::Static))
                                .then(|| capture_receiver_type(ty)),
                            receiver_fields: instance_field_types(ty).into_keys().collect(),
                            capturing_receiver: false,
                            method_name: Some(method.name.clone()),
                            return_type: Some(method.return_type.clone()),
                            fields: fields.clone(),
                            outer_field_owners: outer_field_owners.clone(),
                            scopes: vec![method_param_frame(&method.params, false)],
                            mutated_names: mutated_names_in_block(body),
                        };
                        self.analyze_block(body, &mut ctx, None);
                    }
                }
                Member::Constructor(ctor) => {
                    if let Some(body) = &ctor.body {
                        let mut ctx = CaptureContext {
                            type_name: ty.name.clone(),
                            enclosing_receivers: enclosing_types
                                .iter()
                                .map(|ty| capture_receiver_type(ty))
                                .collect(),
                            receiver_type: Some(capture_receiver_type(ty)),
                            receiver_fields: instance_field_types(ty).into_keys().collect(),
                            capturing_receiver: false,
                            method_name: None,
                            return_type: None,
                            fields: fields.clone(),
                            outer_field_owners: outer_field_owners.clone(),
                            scopes: vec![method_param_frame(&ctor.params, false)],
                            mutated_names: mutated_names_in_block(body),
                        };
                        self.analyze_block(body, &mut ctx, None);
                    }
                }
                // A `static` nested class has no enclosing instance, so nothing
                // in it can capture one. Analysing it as an inner class made an
                // unqualified reference to an outer *static* lower to
                // `self.__outer.borrow().field`
                // ([0037](../claude_audits/done/0037-nested-class-outer-static-read-is-capture.md)).
                // Member interfaces, enums, and records are implicitly static
                // in Java.  Only a non-static member class has an enclosing
                // instance to capture.
                Member::Type(inner) if inner.has_enclosing_instance_in(ty.kind) => {
                    let mut inner_enclosing_types = enclosing_types.to_vec();
                    inner_enclosing_types.push(ty);
                    self.analyze_inner_type(inner, ty, &inner_enclosing_types)
                }
                Member::Type(inner) => self.analyze_type_members(inner, &[]),
                _ => {}
            }
        }
    }

    fn analyze_inner_type(
        &mut self,
        inner: &TypeDecl,
        outer: &TypeDecl,
        enclosing_types: &[&TypeDecl],
    ) {
        let mut captures = Vec::new();
        self.analyze_type_members_for_owner(inner, enclosing_types, &mut captures);
        // Every non-static member class has an enclosing-instance association,
        // even if its own body does not presently read that instance.  Keeping
        // the handle is required when a nested member class later captures an
        // ancestor of this class.
        if !captures
            .iter()
            .any(|capture| capture.source == CaptureSource::OuterThis)
            && let Some(outer) = enclosing_types.last()
        {
            captures.push(Capture {
                binding: None,
                name: outer.name.clone(),
                source: CaptureSource::OuterThis,
                ty: JavaType::Named {
                    name: outer.name.clone(),
                    args: Vec::new(),
                },
                effectively_final: true,
                mutable_after_capture: false,
            });
        }
        let id = self.next_node_id();
        self.push_owner(CaptureOwner {
            id,
            kind: CaptureOwnerKind::InnerClass,
            name: Some(inner.name.clone()),
            target_type: Some(JavaType::Named {
                name: inner.name.clone(),
                args: Vec::new(),
            }),
            origin: CaptureOrigin::Type {
                type_name: outer.name.clone(),
            },
            captures,
        });
        self.analyze_type_members(inner, enclosing_types);
    }

    fn analyze_type_members_for_owner(
        &mut self,
        ty: &TypeDecl,
        enclosing_types: &[&TypeDecl],
        captures: &mut Vec<Capture>,
    ) {
        let fields = field_types(ty);
        let outer_field_owners = enclosing_instance_field_owners(enclosing_types);

        for member in &ty.members {
            match member {
                Member::Method(method) => {
                    if let Some(body) = &method.body {
                        let mut ctx = CaptureContext {
                            type_name: ty.name.clone(),
                            enclosing_receivers: enclosing_types
                                .iter()
                                .map(|ty| capture_receiver_type(ty))
                                .collect(),
                            receiver_type: (!method.modifiers.contains(&Modifier::Static))
                                .then(|| capture_receiver_type(ty)),
                            receiver_fields: instance_field_types(ty).into_keys().collect(),
                            capturing_receiver: false,
                            method_name: Some(method.name.clone()),
                            return_type: Some(method.return_type.clone()),
                            fields: fields.clone(),
                            outer_field_owners: outer_field_owners.clone(),
                            scopes: vec![method_param_frame(&method.params, false)],
                            mutated_names: mutated_names_in_block(body),
                        };
                        self.collect_block_captures(body, &mut ctx, captures);
                    }
                }
                Member::Constructor(ctor) => {
                    if let Some(body) = &ctor.body {
                        let mut ctx = CaptureContext {
                            type_name: ty.name.clone(),
                            enclosing_receivers: enclosing_types
                                .iter()
                                .map(|ty| capture_receiver_type(ty))
                                .collect(),
                            receiver_type: Some(capture_receiver_type(ty)),
                            receiver_fields: instance_field_types(ty).into_keys().collect(),
                            capturing_receiver: false,
                            method_name: None,
                            return_type: None,
                            fields: fields.clone(),
                            outer_field_owners: outer_field_owners.clone(),
                            scopes: vec![method_param_frame(&ctor.params, false)],
                            mutated_names: mutated_names_in_block(body),
                        };
                        self.collect_block_captures(body, &mut ctx, captures);
                    }
                }
                _ => {}
            }
        }
    }

    fn analyze_block(
        &mut self,
        block: &Block,
        ctx: &mut CaptureContext,
        owner_captures: Option<&mut Vec<Capture>>,
    ) {
        let mut owner_captures = owner_captures;
        for stmt in &block.statements {
            match &mut owner_captures {
                Some(captures) => self.collect_stmt_captures(stmt, ctx, captures),
                None => self.analyze_stmt(stmt, ctx),
            }
            if let Stmt::VarDecl {
                ty, name, binding, ..
            } = stmt
            {
                add_symbol(
                    ctx,
                    name,
                    ty.clone(),
                    CaptureSource::Local,
                    false,
                    binding.clone(),
                );
            }
        }
    }

    fn analyze_stmt(&mut self, stmt: &Stmt, ctx: &mut CaptureContext) {
        match stmt {
            Stmt::VarDecl {
                ty,
                initializer: Some(expr),
                ..
            } => {
                self.analyze_expr_with_target(expr, ctx, ty);
            }
            Stmt::VarDecl { .. } => {}
            Stmt::Assign { target, value } => {
                self.analyze_expr(target, ctx);
                self.analyze_expr(value, ctx);
            }
            Stmt::Return(Some(expr)) => {
                if let Some(target) = ctx.return_type.clone() {
                    self.analyze_expr_with_target(expr, ctx, &target);
                } else {
                    self.analyze_expr(expr, ctx);
                }
            }
            Stmt::Expr(expr) | Stmt::Throw(expr) => {
                self.analyze_expr(expr, ctx);
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
            } => {
                self.analyze_expr(condition, ctx);
                self.analyze_block(then_block, &mut ctx.clone(), None);
                if let Some(block) = else_block {
                    self.analyze_block(block, &mut ctx.clone(), None);
                }
            }
            Stmt::While {
                condition, body, ..
            }
            | Stmt::DoWhile {
                condition, body, ..
            } => {
                self.analyze_expr(condition, ctx);
                self.analyze_block(body, &mut ctx.clone(), None);
            }
            Stmt::For {
                init,
                condition,
                update,
                body,
                ..
            } => {
                let mut loop_ctx = ctx.clone();
                for stmt in init {
                    self.analyze_stmt(stmt, &mut loop_ctx);
                    if let Stmt::VarDecl {
                        ty, name, binding, ..
                    } = stmt
                    {
                        add_symbol(
                            &mut loop_ctx,
                            name,
                            ty.clone(),
                            CaptureSource::Local,
                            false,
                            binding.clone(),
                        );
                    }
                }
                if let Some(expr) = condition {
                    self.analyze_expr(expr, &mut loop_ctx);
                }
                for expr in update {
                    self.analyze_expr(expr, &mut loop_ctx);
                }
                self.analyze_block(body, &mut loop_ctx, None);
            }
            Stmt::EnhancedFor {
                var_type,
                var_name,
                iterable,
                body,
                binding,
                ..
            } => {
                self.analyze_expr(iterable, ctx);
                let mut body_ctx = ctx.clone();
                add_symbol(
                    &mut body_ctx,
                    var_name,
                    var_type.clone(),
                    CaptureSource::Local,
                    false,
                    binding.clone(),
                );
                self.analyze_block(body, &mut body_ctx, None);
            }
            Stmt::Switch { selector, cases } => {
                self.analyze_expr(selector, ctx);
                for case in cases {
                    for label in &case.labels {
                        if let crate::java_ir::SwitchLabel::Expr(expr) = label {
                            self.analyze_expr(expr, ctx);
                        }
                    }
                    let block = Block {
                        statements: case.statements.clone(),
                    };
                    self.analyze_block(&block, &mut ctx.clone(), None);
                }
            }
            Stmt::Try {
                resources,
                body,
                catches,
                finally_block,
            } => {
                for resource in resources {
                    self.analyze_stmt(resource, ctx);
                }
                self.analyze_block(body, &mut ctx.clone(), None);
                for catch in catches {
                    let mut catch_ctx = ctx.clone();
                    let ty = catch
                        .types
                        .first()
                        .cloned()
                        .unwrap_or_else(|| JavaType::Unknown(catch.source_name.clone()));
                    add_symbol(
                        &mut catch_ctx,
                        &catch.source_name,
                        ty,
                        CaptureSource::Local,
                        false,
                        None,
                    );
                    self.analyze_block(&catch.body, &mut catch_ctx, None);
                }
                if let Some(block) = finally_block {
                    self.analyze_block(block, &mut ctx.clone(), None);
                }
            }
            Stmt::LocalType(local) => self.analyze_local_type(local, ctx),
            Stmt::ConstructorInvocation { args, .. } => {
                for arg in args {
                    self.analyze_expr(arg, ctx);
                }
            }
            Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
                self.analyze_block(block, &mut ctx.clone(), None)
            }
            _ => {}
        }
    }

    fn analyze_local_type(&mut self, local: &TypeDecl, ctx: &CaptureContext) {
        let mut captures = Vec::new();
        self.anonymous_receivers.push(None);
        self.analyze_type_with_outer_scopes(local, ctx, &mut captures);
        self.anonymous_receivers.pop();
        let id = self.next_node_id();
        self.push_owner(CaptureOwner {
            id,
            kind: CaptureOwnerKind::LocalClass,
            name: Some(local.name.clone()),
            target_type: Some(JavaType::Named {
                name: local.name.clone(),
                args: Vec::new(),
            }),
            origin: current_origin(ctx),
            captures,
        });
    }

    fn analyze_type_with_outer_scopes(
        &mut self,
        ty: &TypeDecl,
        outer_ctx: &CaptureContext,
        captures: &mut Vec<Capture>,
    ) {
        let fields = field_types(ty);
        for member in &ty.members {
            match member {
                Member::Method(method) => {
                    if let Some(body) = &method.body {
                        let mut ctx = CaptureContext {
                            type_name: ty.name.clone(),
                            enclosing_receivers: outer_ctx
                                .enclosing_receivers
                                .iter()
                                .chain(outer_ctx.receiver_type.iter())
                                .cloned()
                                .collect(),
                            receiver_type: (!method.modifiers.contains(&Modifier::Static))
                                .then(|| capture_receiver_type(ty)),
                            receiver_fields: instance_field_types(ty).into_keys().collect(),
                            capturing_receiver: false,
                            method_name: Some(method.name.clone()),
                            return_type: Some(method.return_type.clone()),
                            fields: fields.clone(),
                            outer_field_owners: outer_ctx
                                .fields
                                .keys()
                                .map(|name| (name.clone(), outer_ctx.type_name.clone()))
                                .collect(),
                            scopes: capturable_outer_scopes(outer_ctx),
                            mutated_names: outer_ctx.mutated_names.clone(),
                        };
                        ctx.scopes.push(method_param_frame(&method.params, false));
                        self.collect_block_captures(body, &mut ctx, captures);
                    }
                }
                Member::Constructor(ctor) => {
                    if let Some(body) = &ctor.body {
                        let mut ctx = CaptureContext {
                            type_name: ty.name.clone(),
                            enclosing_receivers: outer_ctx
                                .enclosing_receivers
                                .iter()
                                .chain(outer_ctx.receiver_type.iter())
                                .cloned()
                                .collect(),
                            receiver_type: Some(capture_receiver_type(ty)),
                            receiver_fields: instance_field_types(ty).into_keys().collect(),
                            capturing_receiver: false,
                            method_name: None,
                            return_type: None,
                            fields: fields.clone(),
                            outer_field_owners: outer_ctx
                                .fields
                                .keys()
                                .map(|name| (name.clone(), outer_ctx.type_name.clone()))
                                .collect(),
                            scopes: capturable_outer_scopes(outer_ctx),
                            mutated_names: outer_ctx.mutated_names.clone(),
                        };
                        ctx.scopes.push(method_param_frame(&ctor.params, false));
                        self.collect_block_captures(body, &mut ctx, captures);
                    }
                }
                _ => {}
            }
        }
    }

    fn collect_block_captures(
        &mut self,
        block: &Block,
        ctx: &mut CaptureContext,
        captures: &mut Vec<Capture>,
    ) {
        self.analyze_block(block, ctx, Some(captures));
    }

    fn collect_stmt_captures(
        &mut self,
        stmt: &Stmt,
        ctx: &mut CaptureContext,
        captures: &mut Vec<Capture>,
    ) {
        match stmt {
            Stmt::VarDecl { initializer, .. } => {
                if let Some(expr) = initializer {
                    self.collect_expr_captures(expr, ctx, captures);
                }
            }
            Stmt::Assign { target, value } => {
                self.collect_expr_captures(target, ctx, captures);
                self.collect_expr_captures(value, ctx, captures);
            }
            Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
                self.collect_expr_captures(expr, ctx, captures);
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
            } => {
                self.collect_expr_captures(condition, ctx, captures);
                self.collect_block_captures(then_block, &mut ctx.clone(), captures);
                if let Some(block) = else_block {
                    self.collect_block_captures(block, &mut ctx.clone(), captures);
                }
            }
            Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
                self.collect_block_captures(block, &mut ctx.clone(), captures)
            }
            _ => self.analyze_stmt(stmt, ctx),
        }
    }

    fn analyze_expr(&mut self, expr: &Expr, ctx: &mut CaptureContext) {
        match expr {
            Expr::Lambda { params, body } => self.analyze_lambda(params, body, ctx),
            Expr::MethodReference { target, .. } => self.analyze_method_reference(target, ctx),
            Expr::New {
                qualifier,
                args,
                body,
                ty,
                ..
            } => {
                if let Some(qualifier) = qualifier {
                    self.analyze_expr(qualifier, ctx);
                }
                for arg in args {
                    self.analyze_expr(arg, ctx);
                }
                if let Some(members) = body {
                    self.analyze_anonymous_class(ty, members, ctx);
                }
            }
            Expr::FieldAccess(receiver, _) => self.analyze_expr(receiver, ctx),
            Expr::ArrayAccess(array, index) => {
                self.analyze_expr(array, ctx);
                self.analyze_expr(index, ctx);
            }
            Expr::Binary { left, right, .. } => {
                self.analyze_expr(left, ctx);
                self.analyze_expr(right, ctx);
            }
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
                self.analyze_expr(expr, ctx);
            }
            Expr::Ternary {
                condition,
                then_expr,
                else_expr,
            } => {
                self.analyze_expr(condition, ctx);
                self.analyze_expr(then_expr, ctx);
                self.analyze_expr(else_expr, ctx);
            }
            Expr::Call { receiver, args, .. } => {
                if let Some(receiver) = receiver {
                    self.analyze_expr(receiver, ctx);
                }
                for arg in args {
                    self.analyze_expr(arg, ctx);
                }
            }
            _ => {}
        }
    }

    fn analyze_expr_with_target(
        &mut self,
        expr: &Expr,
        ctx: &mut CaptureContext,
        target: &JavaType,
    ) {
        match expr {
            Expr::Lambda { params, body } => {
                self.analyze_lambda_with_target(params, body, ctx, Some(target))
            }
            Expr::MethodReference {
                target: receiver, ..
            } => {
                let previous_owners = self.owners.len();
                self.analyze_method_reference(receiver, ctx);
                if self.owners.len() > previous_owners
                    && let Some(owner) = self.owners.last_mut()
                    && owner.kind == CaptureOwnerKind::MethodReference
                {
                    owner.target_type = Some(target.clone());
                }
            }
            Expr::Ternary {
                condition,
                then_expr,
                else_expr,
            } => {
                self.analyze_expr(condition, ctx);
                self.analyze_expr_with_target(then_expr, ctx, target);
                self.analyze_expr_with_target(else_expr, ctx, target);
            }
            Expr::Cast { ty, expr } => self.analyze_expr_with_target(expr, ctx, ty),
            _ => self.analyze_expr(expr, ctx),
        }
    }

    fn analyze_method_reference(&mut self, target: &str, ctx: &CaptureContext) {
        // The IR retains reference targets as source expressions. Analyze
        // that expression through the same lexical capture walk as a lambda;
        // unbound/static type names do not resolve to capturable bindings.
        let target = crate::parser::parse_expr(target);
        let mut reference_ctx = ctx.clone();
        // A bound reference evaluates its target now and retains that value.
        // Only this::method/super::method retain the lexical receiver itself;
        // field::method retains the field value, unlike a deferred lambda
        // which reads the field on each invocation. The enclosing lambda's
        // separate walk still captures this when it evaluates this target.
        reference_ctx.capturing_receiver = matches!(target, Expr::This | Expr::Super);
        reference_ctx.scopes = capturable_outer_scopes(ctx);
        let mut captures = Vec::new();
        self.collect_expr_captures(&target, &mut reference_ctx, &mut captures);
        let field = match &target {
            Expr::Var(name) if captures.is_empty() => Some(name),
            Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
                Some(field)
            }
            _ => None,
        };
        if let Some(field) = field
            && let Some(ty) = ctx.fields.get(field)
        {
            captures.push(Capture {
                binding: None,
                name: field.clone(),
                source: CaptureSource::ThisField,
                ty: ty.clone(),
                effectively_final: true,
                mutable_after_capture: false,
            });
        }
        if captures.is_empty() {
            return;
        }
        // A bound reference captures the receiver value at creation, not a
        // mutable variable cell. Java allows reassignment of that variable.
        for capture in &mut captures {
            capture.effectively_final = true;
            capture.mutable_after_capture = false;
        }
        let id = self.next_node_id();
        self.push_owner(CaptureOwner {
            id,
            kind: CaptureOwnerKind::MethodReference,
            name: None,
            target_type: None,
            origin: current_origin(ctx),
            captures,
        });
    }

    fn analyze_lambda(
        &mut self,
        params: &[crate::java_ir::LambdaParam],
        body: &LambdaBody,
        ctx: &CaptureContext,
    ) {
        self.analyze_lambda_with_target(params, body, ctx, None);
    }

    fn analyze_lambda_with_target(
        &mut self,
        params: &[crate::java_ir::LambdaParam],
        body: &LambdaBody,
        ctx: &CaptureContext,
        target: Option<&JavaType>,
    ) {
        let mut lambda_ctx = ctx.clone();
        lambda_ctx.return_type = target.and_then(|ty| {
            let JavaType::Named { name, args } = ty else {
                return None;
            };
            crate::stdlib::functional_interface_shape(name)?.return_type(args)
        });
        lambda_ctx.capturing_receiver = true;
        lambda_ctx.scopes = capturable_outer_scopes(ctx);
        lambda_ctx.scopes.push(lambda_param_frame(params, false));
        let mut captures = Vec::new();
        match body {
            LambdaBody::Expr(expr) => {
                self.collect_expr_captures(expr, &mut lambda_ctx, &mut captures);
            }
            LambdaBody::Block(block) => {
                self.collect_block_captures(block, &mut lambda_ctx, &mut captures);
            }
        }
        let id = self.next_node_id();
        self.push_owner(CaptureOwner {
            id,
            kind: CaptureOwnerKind::Lambda,
            name: None,
            target_type: target.cloned(),
            origin: current_origin(ctx),
            captures,
        });
    }

    fn analyze_anonymous_class(&mut self, ty: &JavaType, members: &[Member], ctx: &CaptureContext) {
        let synthetic = TypeDecl {
            source_names: None,
            static_init_blocks: Vec::new(),
            kind: crate::java_ir::TypeKind::Class,
            name: "Anonymous".to_string(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            modifiers: Vec::new(),
            extends: None,
            implements: Vec::new(),
            permits: Vec::new(),
            members: members.to_vec(),
            span: Default::default(),
        };
        let mut captures = Vec::new();
        // Selection needs only declarations, not another copy of all method
        // bodies. Nested anonymous classes push their own receiver scope.
        let methods = members
            .iter()
            .filter_map(|member| match member {
                Member::Method(method) => {
                    let mut method = method.clone();
                    method.body = None;
                    Some(method)
                }
                _ => None,
            })
            .collect();
        self.anonymous_receivers.push(Some((ty.clone(), methods)));
        self.analyze_type_with_outer_scopes(&synthetic, ctx, &mut captures);
        self.anonymous_receivers.pop();
        let id = self.next_node_id();
        self.push_owner(CaptureOwner {
            id,
            kind: CaptureOwnerKind::AnonymousClass,
            name: None,
            target_type: Some(ty.clone()),
            origin: current_origin(ctx),
            captures,
        });
    }

    fn collect_expr_captures(
        &mut self,
        expr: &Expr,
        ctx: &mut CaptureContext,
        captures: &mut Vec<Capture>,
    ) {
        match expr {
            Expr::FieldAccess(receiver, field) if field == "this" => {
                if let Expr::Var(requested) = receiver.as_ref()
                    && let Some(ty) = ctx.enclosing_receivers.iter().rev().find(|ty| {
                        let name = java_type_simple_name(ty);
                        name == *requested
                            || name.rsplit(['.', '$']).next() == Some(requested.as_str())
                    })
                {
                    captures.push(Capture {
                        binding: None,
                        name: java_type_simple_name(ty),
                        source: CaptureSource::OuterThis,
                        ty: ty.clone(),
                        effectively_final: true,
                        mutable_after_capture: false,
                    });
                }
            }
            Expr::Var(name) => capture_var(name, ctx, captures),
            Expr::This | Expr::Super => capture_this(ctx, captures),
            Expr::Lambda { params, body } => self.analyze_lambda(params, body, ctx),
            Expr::MethodReference { target, .. } => {
                self.analyze_method_reference(target, ctx);
                // Creating a bound reference inside another closure also
                // requires that enclosing closure to retain the receiver.
                self.collect_expr_captures(&crate::parser::parse_expr(target), ctx, captures);
            }
            Expr::New {
                ty,
                qualifier,
                args,
                body,
                ..
            } => {
                if let Some(qualifier) = qualifier {
                    self.collect_expr_captures(qualifier, ctx, captures);
                }
                for arg in args {
                    self.collect_expr_captures(arg, ctx, captures);
                }
                if let Some(members) = body {
                    self.analyze_anonymous_class(ty, members, ctx);
                }
            }
            Expr::FieldAccess(receiver, _) => self.collect_expr_captures(receiver, ctx, captures),
            Expr::ArrayAccess(array, index) => {
                self.collect_expr_captures(array, ctx, captures);
                self.collect_expr_captures(index, ctx, captures);
            }
            Expr::Binary { left, right, .. } => {
                self.collect_expr_captures(left, ctx, captures);
                self.collect_expr_captures(right, ctx, captures);
            }
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
                self.collect_expr_captures(expr, ctx, captures);
            }
            Expr::Ternary {
                condition,
                then_expr,
                else_expr,
            } => {
                self.collect_expr_captures(condition, ctx, captures);
                self.collect_expr_captures(then_expr, ctx, captures);
                self.collect_expr_captures(else_expr, ctx, captures);
            }
            Expr::Call {
                receiver,
                args,
                resolved,
                ..
            } => {
                if receiver.is_none()
                    && resolved
                        .as_ref()
                        .is_some_and(|call| call.kind == CallKind::Instance)
                {
                    let enclosing =
                        self.symbols
                            .zip(self.external_facts)
                            .and_then(|(symbols, facts)| {
                                let (base, methods) = self.anonymous_receivers.last()?.as_ref()?;
                                let mut env = TypeEnv::new();
                                for (name, ty) in &ctx.fields {
                                    env.insert_field(name.clone(), ty.clone());
                                }
                                for scope in &ctx.scopes {
                                    for (name, symbol) in &scope.symbols {
                                        env.insert_local(name.clone(), symbol.ty.clone());
                                    }
                                }
                                symbols.anonymous_enclosing_call_receiver(
                                    expr,
                                    base,
                                    &methods.iter().collect::<Vec<_>>(),
                                    &ctx.enclosing_receivers,
                                    &env,
                                    facts,
                                )
                            });
                    if let Some(ty) = enclosing {
                        captures.push(Capture {
                            binding: None,
                            name: java_type_simple_name(&ty),
                            source: CaptureSource::OuterThis,
                            ty,
                            effectively_final: true,
                            mutable_after_capture: false,
                        });
                    } else {
                        capture_this(ctx, captures);
                    }
                }
                if let Some(receiver) = receiver {
                    self.collect_expr_captures(receiver, ctx, captures);
                }
                for arg in args {
                    self.collect_expr_captures(arg, ctx, captures);
                }
            }
            _ => {}
        }
    }

    fn next_node_id(&mut self) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id += 1;
        id
    }

    fn push_owner(&mut self, mut owner: CaptureOwner) {
        dedup_captures(&mut owner.captures);
        self.owners.push(owner);
    }
}

fn field_types(ty: &TypeDecl) -> HashMap<String, JavaType> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) => Some((field.name.clone(), field.ty.clone())),
            _ => None,
        })
        .collect()
}

/// Instance fields only.
///
/// Used for the *enclosing* type when classifying outer-`this` captures: a
/// `static` member is reached through the class, not through an instance, so
/// naming one from a nested class is not a capture at all. Including statics
/// here produced `self.__outer.borrow().counter` for a field that is not even a
/// struct member ([0037](../claude_audits/done/0037-nested-class-outer-static-read-is-capture.md)).
fn instance_field_types(ty: &TypeDecl) -> HashMap<String, JavaType> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) if !field.modifiers.contains(&Modifier::Static) => {
                Some((field.name.clone(), field.ty.clone()))
            }
            _ => None,
        })
        .collect()
}

/// Resolve unqualified fields through the lexical enclosing-instance chain.
/// The nearest declaration wins, exactly as Java's nested-class lookup does.
fn enclosing_instance_field_owners(enclosing_types: &[&TypeDecl]) -> HashMap<String, String> {
    let mut owners = HashMap::new();
    for ty in enclosing_types.iter().rev() {
        for (name, _) in instance_field_types(ty) {
            if let std::collections::hash_map::Entry::Vacant(entry) = owners.entry(name) {
                entry.insert(ty.name.clone());
            }
        }
    }
    owners
}

fn method_param_frame(params: &[Param], capturable: bool) -> ScopeFrame {
    ScopeFrame {
        symbols: params
            .iter()
            .map(|param| {
                (
                    param.name.clone(),
                    ScopeSymbol {
                        binding: param.binding.clone(),
                        ty: param.ty.clone(),
                        source: CaptureSource::Parameter,
                    },
                )
            })
            .collect(),
        capturable,
    }
}

fn lambda_param_frame(params: &[crate::java_ir::LambdaParam], capturable: bool) -> ScopeFrame {
    ScopeFrame {
        symbols: params
            .iter()
            .map(|param| {
                (
                    param.name.clone(),
                    ScopeSymbol {
                        binding: None,
                        ty: param
                            .ty
                            .clone()
                            .unwrap_or_else(|| JavaType::Unknown(param.name.clone())),
                        source: CaptureSource::Parameter,
                    },
                )
            })
            .collect(),
        capturable,
    }
}

fn capturable_outer_scopes(ctx: &CaptureContext) -> Vec<ScopeFrame> {
    ctx.scopes
        .iter()
        .map(|frame| ScopeFrame {
            symbols: frame.symbols.clone(),
            capturable: true,
        })
        .collect()
}

fn add_symbol(
    ctx: &mut CaptureContext,
    name: &str,
    ty: JavaType,
    source: CaptureSource,
    capturable: bool,
    binding: Option<crate::binding_id::BindingId>,
) {
    if ctx.scopes.is_empty() {
        ctx.scopes.push(ScopeFrame {
            symbols: HashMap::new(),
            capturable,
        });
    }
    let idx = ctx.scopes.len() - 1;
    ctx.scopes[idx].symbols.insert(
        name.to_string(),
        ScopeSymbol {
            ty,
            source,
            binding,
        },
    );
}

fn capture_receiver_type(ty: &TypeDecl) -> JavaType {
    JavaType::Named {
        name: ty.name.clone(),
        args: ty
            .type_params
            .iter()
            .map(|name| JavaType::Named {
                name: name.clone(),
                args: Vec::new(),
            })
            .collect(),
    }
}

fn capture_this(ctx: &CaptureContext, captures: &mut Vec<Capture>) {
    if ctx.capturing_receiver
        && let Some(ty) = &ctx.receiver_type
    {
        captures.push(Capture {
            binding: None,
            name: "this".into(),
            source: CaptureSource::This,
            ty: ty.clone(),
            effectively_final: true,
            mutable_after_capture: false,
        });
    }
}

fn capture_var(name: &str, ctx: &CaptureContext, captures: &mut Vec<Capture>) {
    for frame in ctx.scopes.iter().rev() {
        if let Some(symbol) = frame.symbols.get(name) {
            if frame.capturable {
                captures.push(Capture {
                    binding: symbol.binding.clone(),
                    name: name.to_string(),
                    source: symbol.source,
                    ty: symbol.ty.clone(),
                    effectively_final: !ctx.mutated_names.contains(name),
                    mutable_after_capture: ctx.mutated_names.contains(name),
                });
            }
            return;
        }
    }

    if ctx.fields.contains_key(name) {
        if ctx.receiver_fields.contains(name) {
            capture_this(ctx, captures);
        }
        return;
    }

    if let Some(outer_name) = ctx.outer_field_owners.get(name) {
        captures.push(Capture {
            binding: None,
            name: outer_name.clone(),
            source: CaptureSource::OuterThis,
            ty: JavaType::Named {
                name: outer_name.clone(),
                args: Vec::new(),
            },
            effectively_final: true,
            mutable_after_capture: false,
        });
    }
}

fn current_origin(ctx: &CaptureContext) -> CaptureOrigin {
    if let Some(method_name) = &ctx.method_name {
        CaptureOrigin::Method {
            type_name: ctx.type_name.clone(),
            method_name: method_name.clone(),
        }
    } else {
        CaptureOrigin::Constructor {
            type_name: ctx.type_name.clone(),
        }
    }
}

fn dedup_captures(captures: &mut Vec<Capture>) {
    let mut seen = HashSet::new();
    captures.retain(|capture| seen.insert((capture.name.clone(), capture.source)));
}

fn mutated_names_in_block(block: &Block) -> HashSet<String> {
    let mut out = HashSet::new();
    for stmt in &block.statements {
        collect_mutated_names_stmt(stmt, &mut out);
    }
    out
}

fn collect_mutated_names_stmt(stmt: &Stmt, out: &mut HashSet<String>) {
    match stmt {
        Stmt::Assign { target, value } => {
            collect_assignment_target_name(target, out);
            collect_mutated_names_expr(value, out);
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            collect_mutated_names_expr(expr, out);
        }
        Stmt::VarDecl {
            initializer: Some(expr),
            ..
        } => {
            collect_mutated_names_expr(expr, out);
        }
        Stmt::VarDecl { .. } => {}
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_mutated_names_expr(condition, out);
            for stmt in &then_block.statements {
                collect_mutated_names_stmt(stmt, out);
            }
            if let Some(block) = else_block {
                for stmt in &block.statements {
                    collect_mutated_names_stmt(stmt, out);
                }
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            collect_mutated_names_expr(condition, out);
            for stmt in &body.statements {
                collect_mutated_names_stmt(stmt, out);
            }
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                collect_mutated_names_stmt(stmt, out);
            }
            if let Some(expr) = condition {
                collect_mutated_names_expr(expr, out);
            }
            for expr in update {
                collect_mutated_names_expr(expr, out);
            }
            for stmt in &body.statements {
                collect_mutated_names_stmt(stmt, out);
            }
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            for stmt in &block.statements {
                collect_mutated_names_stmt(stmt, out);
            }
        }
        _ => {}
    }
}

fn collect_mutated_names_expr(expr: &Expr, out: &mut HashSet<String>) {
    match expr {
        Expr::Unary {
            op:
                crate::java_ir::UnaryOp::PreInc
                | crate::java_ir::UnaryOp::PreDec
                | crate::java_ir::UnaryOp::PostInc
                | crate::java_ir::UnaryOp::PostDec,
            expr,
        } => collect_assignment_target_name(expr, out),
        Expr::FieldAccess(receiver, _) => collect_mutated_names_expr(receiver, out),
        Expr::ArrayAccess(array, index) => {
            collect_mutated_names_expr(array, out);
            collect_mutated_names_expr(index, out);
        }
        Expr::Binary { left, right, .. } => {
            collect_mutated_names_expr(left, out);
            collect_mutated_names_expr(right, out);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_mutated_names_expr(condition, out);
            collect_mutated_names_expr(then_expr, out);
            collect_mutated_names_expr(else_expr, out);
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_mutated_names_expr(receiver, out);
            }
            for arg in args {
                collect_mutated_names_expr(arg, out);
            }
        }
        Expr::New {
            qualifier, args, ..
        } => {
            if let Some(qualifier) = qualifier {
                collect_mutated_names_expr(qualifier, out);
            }
            for arg in args {
                collect_mutated_names_expr(arg, out);
            }
        }
        Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_mutated_names_expr(expr, out);
        }
        _ => {}
    }
}

fn collect_assignment_target_name(expr: &Expr, out: &mut HashSet<String>) {
    match expr {
        Expr::Var(name) => {
            out.insert(name.clone());
        }
        Expr::FieldAccess(receiver, _) | Expr::ArrayAccess(receiver, _) => {
            collect_assignment_target_name(receiver, out);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn primitive_arrays_are_objects_without_element_boxing_or_widening() {
        use super::*;
        let parsed = crate::parser::parse_java_source("class Empty {}");
        let source = source_symbols_from_program(&parsed.program);
        let facts = FactSet::default();
        let int_array = crate::parser::parse_type("int[]");
        for target in ["Object[]", "Integer[]", "Number[]", "long[]", "double[]"] {
            let target = crate::parser::parse_type(target);
            assert!(java_type_applicability(&int_array, &target, &source.symbols, &facts).is_err());
            assert!(java_type_match_score(&int_array, &target, &source.symbols, &facts).is_none());
            assert!(
                generic_argument_match_score(&int_array, &target, &source.symbols, &facts)
                    .is_none()
            );
        }
        for target in ["Object", "Cloneable", "java.io.Serializable", "int[]"] {
            let target = crate::parser::parse_type(target);
            assert!(java_type_applicability(&int_array, &target, &source.symbols, &facts).is_ok());
            assert!(java_type_match_score(&int_array, &target, &source.symbols, &facts).is_some());
        }
        let object_array = crate::parser::parse_type("Object[]");
        let param = Param {
            name: "values".into(),
            source_name: "values".into(),
            ty: object_array.clone(),
            is_varargs: true,
            binding: None,
        };
        assert_eq!(
            overload_applicability_report(&[param.clone()], &[int_array], &source).mode(),
            Some(OverloadApplicabilityMode::VariableArity)
        );
        assert_eq!(
            overload_applicability_report(&[param], &[object_array], &source).mode(),
            Some(OverloadApplicabilityMode::FixedArity)
        );
        // Nested primitive arrays are reference-valued components.
        assert!(
            java_type_applicability(
                &crate::parser::parse_type("int[][]"),
                &crate::parser::parse_type("Object[]"),
                &source.symbols,
                &facts
            )
            .is_ok()
        );
    }

    #[test]
    fn source_symbol_import_aliases_retain_original_canonical_and_binary_owners() {
        let parsed = crate::parser::parse_java_source(
            "package origin.names; class Owner { static int pick(int x) { return x; } static int pick(int x, int y) { return x+y; } static class Nested {} }",
        );
        let mut merged = parsed.program.clone();
        merged.package = None;
        let symbols = super::source_symbols_from_program(&merged);
        assert_eq!(symbols.source_type_name("origin.names.Owner"), "Owner");
        assert_eq!(
            symbols.source_type_name("origin.names.Owner.Nested"),
            "Owner.Nested"
        );
        assert_eq!(
            symbols.source_type_name("origin.names.Owner$Nested"),
            "Owner.Nested"
        );
        let mut env = crate::types::TypeEnv::new();
        env.insert_static_type_alias("Owner", "origin.names.Owner");
        let int = crate::java_ir::JavaType::Primitive(crate::java_ir::PrimitiveType::Int);
        for inputs in [vec![int.clone()], vec![int.clone(), int]] {
            let selected = super::resolve_source_method_reference_with_input_types(
                "Owner",
                "pick",
                &inputs,
                "Owner",
                &symbols,
                &env,
                &crate::facts::FactSet::default(),
            )
            .expect("import must select the original source overload");
            assert_eq!(selected.arg_target_types.len(), inputs.len());
            assert!(selected.rust_name.starts_with("pick_int"));
        }
    }

    #[test]
    fn number_platform_defaults_are_idempotent_source_methods_and_resolved_edges() {
        let parsed = crate::parser::parse_java_source(include_str!(
            "../tests/corpus/numbers/InheritedNumberConversionsProbe.java"
        ));
        let resolved = super::resolve_program(&parsed.program, parsed.diagnostics);
        assert!(
            !resolved.diagnostics.has_errors(),
            "{:?}",
            resolved.diagnostics
        );
        let root = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "SmallNumber")
            .unwrap();
        for name in ["byteValue", "shortValue"] {
            let method = root
                .members
                .iter()
                .find_map(|member| match member {
                    crate::java_ir::Member::Method(method)
                        if method.name == name && method.params.is_empty() =>
                    {
                        Some(method)
                    }
                    _ => None,
                })
                .unwrap();
            let crate::java_ir::Stmt::Return(Some(crate::java_ir::Expr::Cast { expr, .. })) =
                &method.body.as_ref().unwrap().statements[0]
            else {
                panic!("missing narrowing body");
            };
            let crate::java_ir::Expr::Call {
                resolved: Some(call),
                ..
            } = expr.as_ref()
            else {
                panic!("unresolved delegation");
            };
            assert_eq!(call.declaring_type, "SmallNumber");
            assert_eq!(call.rust_name, "int_value");
        }
        let again =
            super::resolve_program(&resolved.program, crate::diagnostics::DiagnosticBag::new());
        let root_again = again
            .program
            .types
            .iter()
            .find(|ty| ty.name == "SmallNumber")
            .unwrap();
        assert_eq!(root.members.len(), root_again.members.len());
        let abstract_root = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "AbstractNumber")
            .unwrap();
        assert_eq!(abstract_root.members.iter().filter(|member| matches!(member,
            crate::java_ir::Member::Method(method) if method.modifiers.contains(&crate::java_ir::Modifier::Abstract) && method.body.is_none()
        )).count(), 4);
    }

    #[test]
    fn number_platform_defaults_do_not_replace_source_or_imported_lookalikes() {
        for source in [
            "class Number { int intValue() { return 0; } } class Child extends Number {}",
            "class Outer { static class Number {} static class Child extends Number {} }",
            "import vendor.Number; class Child extends Number {}",
        ] {
            let parsed = crate::parser::parse_java_source(source);
            let mut program = parsed.program;
            super::qualify_nested_type_names(&mut program);
            let symbols = super::ProgramSymbols::from_program(&program);
            let before = program.clone();
            super::materialize_platform_number_defaults(&mut program, &symbols);
            assert_eq!(program, before, "{source}");
        }
    }

    #[test]
    fn number_defaults_resolve_in_declaration_only_support_programs() {
        let parsed = crate::parser::parse_java_source(
            "class Numeric extends java.lang.Number { public int intValue(){return 255;} public long longValue(){return 255;} public float floatValue(){return 255;} public double doubleValue(){return 255;} }",
        );
        let symbols = super::source_symbols_from_program(&parsed.program);
        let mut summary = parsed.program;
        for member in &mut summary.types[0].members {
            if let crate::java_ir::Member::Method(method) = member {
                method.body = None;
            }
        }
        super::assign_rust_names_to_context_program(&mut summary);
        let resolved =
            super::resolve_program_with_source_symbols(&summary, parsed.diagnostics, &symbols);
        assert!(!resolved.diagnostics.has_errors());
        let methods = resolved.program.types[0]
            .members
            .iter()
            .filter_map(|member| match member {
                crate::java_ir::Member::Method(method)
                    if matches!(method.name.as_str(), "byteValue" | "shortValue") =>
                {
                    Some(method)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(methods.len(), 2);
        assert!(methods.iter().all(|method| method.body.is_some()));
    }

    #[test]
    fn anonymous_implicit_enclosing_captures_respect_scoped_and_inherited_bindings() {
        let parsed = crate::parser::parse_java_source(
            r#"
class Parent { int read(int value) { return value; } }
class Holder<T> extends Parent {
    interface Action { int get(); }
    public int hashCode() { return 7; }
    Action outer() { return new Action() { public int get() { return read(2); } }; }
    Action helper() { return new Action() {
        int read(double value) { return (int) value; }
        public int get() { return read(2); }
    }; }
    Parent inherited() { return new Parent() { int other() { return read(2); } }; }
    Action object() { return new Action() { public int get() { return hashCode(); } }; }
}
interface NestedAction { int get(); }
class ZOuter extends Parent {
    class AInner extends Parent {
        NestedAction nearest() { return new NestedAction() {
            public int get() { return ZOuter.this.read(1) + read(2); }
        }; }
    }
}
"#,
        );
        let resolved = super::resolve_program(&parsed.program, parsed.diagnostics);
        assert!(
            !resolved.diagnostics.has_errors(),
            "{:?}",
            resolved.diagnostics
        );
        let mut observed = std::collections::BTreeSet::new();
        for owner in resolved
            .facts
            .captures
            .owners
            .iter()
            .filter(|owner| owner.kind == crate::facts::CaptureOwnerKind::AnonymousClass)
        {
            let crate::facts::CaptureOrigin::Method { method_name, .. } = &owner.origin else {
                panic!("{owner:?}");
            };
            observed.insert(method_name.as_str());
            let captures = owner
                .captures
                .iter()
                .filter(|capture| capture.source == crate::facts::CaptureSource::OuterThis)
                .collect::<Vec<_>>();
            if method_name == "outer" {
                assert_eq!(captures.len(), 1, "{owner:?}");
                assert_eq!(
                    captures[0].ty,
                    crate::parser::parse_hierarchy_type("Holder<T>")
                );
            } else if method_name == "nearest" {
                assert_eq!(captures.len(), 2, "{owner:?}");
                let types = captures
                    .iter()
                    .map(|capture| capture.ty.clone())
                    .collect::<Vec<_>>();
                assert!(
                    types.contains(&crate::parser::parse_hierarchy_type("ZOuter")),
                    "{owner:?}"
                );
                assert!(
                    types.contains(&crate::parser::parse_hierarchy_type("ZOuter.AInner")),
                    "{owner:?}"
                );
            } else {
                assert!(captures.is_empty(), "{owner:?}");
            }
        }
        assert_eq!(
            observed,
            std::collections::BTreeSet::from(["outer", "helper", "inherited", "object", "nearest"])
        );
    }

    #[test]
    fn qualified_enclosing_capture_retains_generic_receiver_without_field_evidence() {
        let parsed = crate::parser::parse_java_source(
            r#"
class Holder<T> {
    interface Action { void run(); }
    Action make() { return new Action() {
        public void run() { Holder<T> owner = Holder.this; }
    }; }
}
"#,
        );
        let resolved = super::resolve_program(&parsed.program, parsed.diagnostics);
        assert!(
            !resolved.diagnostics.has_errors(),
            "{:?}",
            resolved.diagnostics
        );
        let captures = resolved
            .facts
            .captures
            .owners
            .iter()
            .filter(|owner| owner.kind == crate::facts::CaptureOwnerKind::AnonymousClass)
            .flat_map(|owner| &owner.captures)
            .filter(|capture| capture.source == crate::facts::CaptureSource::OuterThis)
            .collect::<Vec<_>>();
        assert_eq!(captures.len(), 1, "{captures:?}");
        assert_eq!(
            captures[0].ty,
            crate::parser::parse_hierarchy_type("Holder<T>")
        );
        assert!(captures[0].effectively_final);
    }

    #[test]
    fn anonymous_inherited_generic_fields_keep_specialized_types() {
        let parsed = crate::parser::parse_java_source(
            r#"
class Base<T> { T inherited; }
class Probe {
    static int tag(int value) { return 1; }
    static int tag(String value) { return 2; }
    static Base<String> make(int inherited) {
        return new Base<String>() {
            int score() { return tag(inherited) + tag(this.inherited); }
        };
    }
}
"#,
        );
        let resolved = super::resolve_program(&parsed.program, parsed.diagnostics);
        assert!(
            !resolved.diagnostics.has_errors(),
            "{:?}",
            resolved.diagnostics
        );
        let mut targets = Vec::new();
        crate::semantic_analysis::walk_program_exprs(&resolved.program, &mut |expr| {
            if let crate::java_ir::Expr::Call { name, resolved, .. } = expr
                && name == "tag"
            {
                targets.push(
                    resolved
                        .as_ref()
                        .expect("overload must resolve")
                        .rust_name
                        .clone(),
                );
            }
        });
        assert_eq!(targets, ["tag_string", "tag_string"]);
    }

    #[test]
    fn anonymous_fields_resolve_in_their_own_lexical_scope() {
        let parsed = crate::parser::parse_java_source(include_str!(
            "../tests/corpus/numbers/AnonymousFieldScopeProbe.java"
        ));
        let resolved = super::resolve_program(&parsed.program, parsed.diagnostics);
        assert!(
            !resolved.diagnostics.has_errors(),
            "{:?}",
            resolved.diagnostics
        );
        let mut targets = Vec::new();
        crate::semantic_analysis::walk_program_exprs(&resolved.program, &mut |expr| {
            if let crate::java_ir::Expr::Call { name, resolved, .. } = expr
                && name == "tag"
            {
                let target = resolved
                    .as_ref()
                    .expect("overload must resolve, not fall back");
                targets.push(target.rust_name.clone());
            }
        });
        assert_eq!(
            targets,
            [
                "tag_double",
                "tag_double",
                "tag_double",
                "tag_int",
                "tag_int",
                "tag_int",
                "tag_double",
                "tag_int",
                "tag_double",
            ]
        );
    }

    #[test]
    fn explicit_imported_value_fields_bind_before_receiver_inference() {
        let parsed = crate::parser::parse_java_source(
            "import static dep.Constants.LIMIT; import static dep.Constants.VALUE; class Probe { static int value() { return LIMIT; } static int parameter(int LIMIT) { return LIMIT; } static int local() { int LIMIT = 3; return LIMIT; } static String text() { return VALUE.toString(); } } class Shadow { static int LIMIT = 4; static int value() { return LIMIT; } }",
        );
        let pack = crate::external_packs::parse_external_pack_manifest(
            "external-pack dep.constants dep.Constants rust-crate-mapping mapped target=dep_runtime\nexternal-pack-field dep.constants dep.Constants LIMIT mapped type=int value=dep_runtime::limit()\nexternal-pack-field dep.constants dep.Constants VALUE mapped type=dep.Widget value=dep_runtime::value()\n",
        );
        assert!(!pack.diagnostics().has_errors());
        let resolved = super::resolve_program_with_source_program_and_facts(
            &parsed.program,
            parsed.diagnostics,
            &parsed.program,
            pack.facts(),
        );
        assert!(
            !resolved.diagnostics.has_errors(),
            "{:?}",
            resolved.diagnostics
        );
        let mut checked = 0;
        for ty in &resolved.program.types {
            for member in &ty.members {
                let crate::java_ir::Member::Method(method) = member else {
                    continue;
                };
                let Some(crate::java_ir::Stmt::Return(Some(value))) =
                    method.body.as_ref().and_then(|body| body.statements.last())
                else {
                    continue;
                };
                if ty.name == "Probe" && method.name == "value" {
                    assert!(
                        matches!(value, crate::java_ir::Expr::FieldAccess(owner, field) if matches!(owner.as_ref(), crate::java_ir::Expr::Var(name) if name == "dep.Constants") && field == "LIMIT")
                    );
                } else if method.name == "text" {
                    assert!(
                        matches!(value, crate::java_ir::Expr::Call { receiver: Some(receiver), .. } if matches!(receiver.as_ref(), crate::java_ir::Expr::FieldAccess(_, field) if field == "VALUE"))
                    );
                } else {
                    assert!(
                        matches!(value, crate::java_ir::Expr::Var(name) if name == "LIMIT"),
                        "{}::{}: {value:?}",
                        ty.name,
                        method.name
                    );
                }
                checked += 1;
            }
        }
        assert_eq!(checked, 5);
        assert!(resolved.facts.unresolved_target_facts.iter().any(|fact| {
            fact.target_name == "toString"
                && matches!(&fact.receiver_type, Some(crate::java_ir::JavaType::Named { name, .. }) if name == "Widget" || name == "dep.Widget")
        }), "{:?}", resolved.facts.unresolved_target_facts);
    }

    #[test]
    fn explicit_generic_substitution_handles_solved_unknown_binders() {
        use super::substitute_method_type_variables;
        use crate::java_ir::JavaType;
        let substitutions = std::collections::HashMap::from([("T".to_string(), JavaType::String)]);
        for binder in [
            JavaType::Unknown("T".into()),
            JavaType::Named {
                name: "T".into(),
                args: vec![],
            },
        ] {
            let nullable = crate::types::optional_java_type(binder);
            assert_eq!(
                substitute_method_type_variables(&nullable, &substitutions),
                crate::types::optional_java_type(JavaType::String),
            );
        }
        assert_eq!(
            substitute_method_type_variables(&JavaType::Unknown("Other".into()), &substitutions),
            JavaType::Unknown("Other".into()),
        );
    }

    #[test]
    fn lambda_receiver_captures_keep_owner_binders_and_respect_shadowing() {
        let parsed = crate::parser::parse_java_source(
            r#"
            import java.util.function.*;
            class Captures<T> {
                T field;
                static int count;
                T read() { return field; }
                Supplier<T> implicit() { return () -> read(); }
                Supplier<T> explicit() { return () -> this.field; }
                Supplier<T> fieldOnly() { return () -> field; }
                Supplier<T> shadow(T field) { return () -> field; }
                static Supplier<Integer> staticOnly() { return () -> count; }
            }
        "#,
        );
        let resolved = super::resolve_program(&parsed.program, parsed.diagnostics);
        for (method, expected) in [
            ("implicit", true),
            ("explicit", true),
            ("fieldOnly", true),
            ("shadow", false),
            ("staticOnly", false),
        ] {
            let captures = resolved.facts.captures.owners.iter().filter(|owner| {
                matches!(&owner.origin, crate::facts::CaptureOrigin::Method { method_name, .. } if method_name == method)
            }).flat_map(|owner| &owner.captures).filter(|capture| capture.source == crate::facts::CaptureSource::This).collect::<Vec<_>>();
            assert_eq!(!captures.is_empty(), expected, "{method}: {captures:?}");
            for capture in captures {
                assert_eq!(
                    capture.ty,
                    crate::java_ir::JavaType::Named {
                        name: "Captures".into(),
                        args: vec![crate::java_ir::JavaType::Named {
                            name: "T".into(),
                            args: Vec::new()
                        }],
                    }
                );
            }
        }
    }

    #[test]
    fn jdk_self_comparable_bounds_keep_invariant_arguments() {
        for (bound, succeeds) in [("String", true), ("Integer", false)] {
            let source = format!(
                "class Calls {{ static <T extends Comparable<{bound}>> int compare(T value) {{ return 0; }} static int run() {{ return compare(\"a\"); }} }}"
            );
            let parsed = crate::parser::parse_java_source(&source);
            let resolved = resolve_program(&parsed.program, parsed.diagnostics);
            assert_eq!(
                !resolved.diagnostics.has_errors(),
                succeeds,
                "{bound}: {:?}",
                resolved.diagnostics
            );
        }
    }

    #[test]
    fn selected_signature_substitution_keeps_native_subtype_inference() {
        let parsed = crate::parser::parse_java_source(
            r#"
import java.util.*;
class Calls {
    static <T> T first(List<T> values) { return values.get(0); }
    static <K,V> V value(Map<K,V> values) { return values.get(null); }
    static void run(ArrayList<String> list, HashMap<Integer, String> map) {
        first(list); value(map);
    }

}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        assert!(
            !resolved.diagnostics.has_errors(),
            "{:?}",
            resolved.diagnostics
        );
        let method = resolved.program.types[0]
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "run" => Some(method),
                _ => None,
            })
            .unwrap();
        let mut calls = 0;
        for stmt in &method.body.as_ref().unwrap().statements {
            if let Stmt::Expr(Expr::Call {
                resolved: Some(call),
                ..
            }) = stmt
            {
                assert_eq!(call.return_type, JavaType::String, "{call:?}");
                calls += 1;
            }
        }
        assert_eq!(calls, 2);
    }

    #[test]
    fn selected_signature_substitution_aligns_nullable_constructors_not_argument_positions() {
        let plain = crate::parser::parse_type("Pair<A, B>");
        let formal = crate::parser::parse_type("Pair<? super V, ? super E>");
        for (formal, actual) in [
            (
                formal.clone(),
                crate::types::optional_java_type(plain.clone()),
            ),
            (crate::types::optional_java_type(formal), plain),
        ] {
            let mut substitutions = std::collections::HashMap::new();
            super::collect_type_substitutions(
                &formal,
                &actual,
                &mut substitutions,
                None,
                Some(&["V".into(), "E".into()]),
            );
            assert_eq!(
                substitutions.get("V"),
                Some(&crate::parser::parse_type("A"))
            );
            assert_eq!(
                substitutions.get("E"),
                Some(&crate::parser::parse_type("B"))
            );
        }
    }

    #[test]
    fn selected_signature_substitution_rejects_unrelated_or_partial_generic_positions() {
        for actual in ["Other<A, B>", "Pair<A>", "right.Pair<A, B>"] {
            let mut substitutions = std::collections::HashMap::new();
            super::collect_type_substitutions(
                &crate::parser::parse_hierarchy_type("left.Pair<V, E>"),
                &crate::parser::parse_hierarchy_type(actual),
                &mut substitutions,
                None,
                Some(&["V".into(), "E".into()]),
            );
            assert!(substitutions.is_empty(), "{actual}: {substitutions:?}");
        }
    }

    #[test]
    fn selected_signature_substitution_keeps_real_optional_and_whole_binder_arguments() {
        for (formal, actual, expected) in [
            (
                "T",
                "java.util.Optional<String>",
                "java.util.Optional<String>",
            ),
            (
                "java.util.Optional<T>",
                "java.util.Optional<String>",
                "String",
            ),
        ] {
            let mut substitutions = std::collections::HashMap::new();
            super::collect_type_substitutions(
                &crate::parser::parse_hierarchy_type(formal),
                &crate::parser::parse_hierarchy_type(actual),
                &mut substitutions,
                None,
                Some(&["T".into()]),
            );
            assert_eq!(
                substitutions.get("T"),
                Some(&crate::parser::parse_hierarchy_type(expected))
            );
        }
    }

    #[test]
    fn native_collection_diamond_keeps_context_for_capability_inference() {
        let parsed = crate::parser::parse_java_source(
            r#"
import java.util.*;
class Factory {
    static <T> Set<T> set() { return new LinkedHashSet<>(); }
    static <T> List<T> list() { return new ArrayList<>(); }
    static <K,V> Map<K,V> map() { return new HashMap<>(); }
    static Set raw() { return new LinkedHashSet(); }
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        assert!(
            !resolved.diagnostics.has_errors(),
            "{:?}",
            resolved.diagnostics
        );
        for member in &resolved.program.types[0].members {
            let Member::Method(method) = member else {
                continue;
            };
            let Stmt::Return(Some(expr)) = &method.body.as_ref().unwrap().statements[0] else {
                panic!("expected factory return");
            };
            let actual = infer_expr_type(expr, &TypeEnv::default());
            let JavaType::Named { args, .. } = actual else {
                panic!("{actual:?}")
            };
            let expected = match method.name.as_str() {
                "set" | "list" => vec!["T"],
                "map" => vec!["K", "V"],
                "raw" => vec![],
                other => panic!("unexpected {other}"),
            };
            assert_eq!(
                args,
                expected
                    .into_iter()
                    .map(|name| JavaType::Named {
                        name: name.into(),
                        args: vec![],
                    })
                    .collect::<Vec<_>>(),
                "{}",
                method.name
            );
        }
    }
    use super::*;

    #[test]
    fn nested_parent_edges_keep_lexical_identity_and_generic_arguments() {
        let parsed = crate::parser::parse_java_source(
            r#"
class First {
    interface Child<X> extends Parent<X> {}
    interface Parent<T> { T value(); }
}
class Second {
    interface Parent<T> { int other(); }
    interface Child<X> extends Parent<X> {}
}
"#,
        );
        let mut program = parsed.program;
        qualify_nested_type_names(&mut program);
        let symbols = ProgramSymbols::from_program(&program);
        let methods = symbols.methods_for_hierarchy("First.Child");
        let inherited = methods
            .iter()
            .find(|method| method.java_name == "value")
            .unwrap();
        assert_eq!(inherited.declaring_type, "First.Parent");
        assert_eq!(
            inherited.return_type,
            JavaType::Named {
                name: "X".into(),
                args: vec![]
            }
        );
        assert!(!methods.iter().any(|method| method.java_name == "other"));
        assert!(
            symbols
                .methods_for_hierarchy("Second.Child")
                .iter()
                .any(|method| method.java_name == "other")
        );
        assert_eq!(
            symbols.source_supertype_args(
                &JavaType::Named {
                    name: "First.Child".into(),
                    args: vec![JavaType::String]
                },
                "First.Parent",
            ),
            Some(vec![JavaType::String])
        );
    }

    #[test]
    fn explicit_nested_type_import_retains_its_source_owner_identity() {
        let parsed = crate::parser::parse_java_source(
            r#"
import be.acme.CommandLineArguments.Arguments;
class Use {
    Arguments field;
    void consume(Arguments value) {}
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let use_ty = &resolved.program.types[0];
        let field_ty = use_ty.members.iter().find_map(|member| match member {
            Member::Field(field) => Some(&field.ty),
            _ => None,
        });
        let param_ty = use_ty.members.iter().find_map(|member| match member {
            Member::Method(method) => method.params.first().map(|param| &param.ty),
            _ => None,
        });
        let expected = JavaType::Named {
            name: "CommandLineArguments.Arguments".to_string(),
            args: Vec::new(),
        };
        assert_eq!(field_ty, Some(&expected));
        assert_eq!(param_ty, Some(&expected));
    }

    #[test]
    fn inherited_nested_type_uses_its_declaring_owner_identity() {
        let parsed = crate::parser::parse_java_source(
            r#"
class Base {
    static enum Mode { FAST, SAFE }
}
class Child extends Base {
    void consume(Mode mode) {}
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let child = &resolved.program.types[1];
        let param_ty = child.members.iter().find_map(|member| match member {
            Member::Method(method) => method.params.first().map(|param| &param.ty),
            _ => None,
        });
        assert_eq!(
            param_ty,
            Some(&JavaType::Named {
                name: "Base.Mode".to_string(),
                args: Vec::new(),
            })
        );
    }

    #[test]
    fn imported_jdk_map_entry_implementation_is_qualified() {
        let parsed = crate::parser::parse_java_source(
            "import java.util.Map; class Holder { static class Entry implements Map.Entry<String, Integer> { public String getKey() { return null; } public Integer getValue() { return null; } public Integer setValue(Integer value) { return value; } } }",
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let entry = resolved.program.types[0]
            .members
            .iter()
            .find_map(|member| match member {
                Member::Type(nested) => Some(nested),
                _ => None,
            })
            .expect("nested Entry");
        assert!(entry.implements[0].starts_with("java.util.Map.Entry<"));
    }

    #[test]
    fn source_map_shadows_imported_jdk_map_entry_implementation() {
        let parsed = crate::parser::parse_java_source(
            "import java.util.Map; class Holder { static class Map { interface Entry {} } static class Use implements Map.Entry {} }",
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let implementation = resolved.program.types[0]
            .members
            .iter()
            .find_map(|member| match member {
                Member::Type(nested) if nested.name.ends_with(".Use") => Some(nested),
                _ => None,
            })
            .expect("nested implementation");
        assert_eq!(implementation.implements, vec!["Holder.Map.Entry"]);
    }

    #[test]
    fn sibling_qualified_nested_types_stay_concrete_in_local_declarations() {
        let parsed = crate::parser::parse_java_source(
            r#"
                class Use {
                    void use(VidMapping mapping) {
                        VidMapping.Builder builder = mapping.toBuilder();
                        FieldInfo.Builder field = builder.getFieldsBuilder();
                    }
                }
                class VidMapping {
                    Builder toBuilder() { return new Builder(); }
                    static class Builder {
                        FieldInfo.Builder getFieldsBuilder() { return new FieldInfo.Builder(); }
                    }
                }
                class FieldInfo { static class Builder {} }
            "#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let use_type = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Use")
            .expect("Use type");
        let method = use_type
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "use" => Some(method),
                _ => None,
            })
            .expect("use method");
        let body = method.body.as_ref().expect("use body");
        let declared = body
            .statements
            .iter()
            .filter_map(|stmt| match stmt {
                Stmt::VarDecl { ty, .. } => Some(ty),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            declared,
            vec![
                &JavaType::Named {
                    name: "VidMapping.Builder".to_string(),
                    args: Vec::new(),
                },
                &JavaType::Named {
                    name: "FieldInfo.Builder".to_string(),
                    args: Vec::new(),
                },
            ]
        );
        let resolved_calls = body
            .statements
            .iter()
            .filter_map(|stmt| match stmt {
                Stmt::VarDecl {
                    initializer: Some(Expr::Call { resolved, .. }),
                    ..
                } => resolved.as_ref(),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            resolved_calls
                .iter()
                .map(|call| call.return_type.clone())
                .collect::<Vec<_>>(),
            vec![
                JavaType::Named {
                    name: "VidMapping.Builder".to_string(),
                    args: Vec::new(),
                },
                JavaType::Named {
                    name: "FieldInfo.Builder".to_string(),
                    args: Vec::new(),
                },
            ]
        );

        let semantic = crate::semantic_analysis::analyze_program(
            &resolved.program,
            &resolved.facts,
            &FactSet::default(),
        );
        let representations =
            crate::representation::plan_representations(&resolved.program, &semantic.facts);
        let signatures = crate::signature::solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &semantic.usage,
            &semantic.facts,
        );
        let local = signatures
            .required_method_signature(use_type, method)
            .locals()
            .iter()
            .find(|local| local.name() == "builder")
            .expect("builder local signature");
        assert_eq!(
            local.shape().java_ty(),
            JavaType::Named {
                name: "VidMapping.Builder".to_string(),
                args: Vec::new(),
            }
        );
        assert_eq!(
            local.shape().dispatch,
            crate::slot_shape::DispatchShape::Concrete,
            "dispatch requirements: {:?}\nrepresentation: {:?}",
            semantic
                .facts
                .dispatch_requirements_for("VidMapping.Builder"),
            representations.type_representation("VidMapping.Builder")
        );
    }

    #[test]
    fn source_class_named_path_wins_over_platform_facade_in_static_call_result() {
        let parsed = crate::parser::parse_java_source(
            r#"
class Caller {
    void run(String raw) {
        Path path = raw == null ? null : IO.getPath(raw);
    }
}
class IO {
    static Path getPath(String raw) { return new Path(); }
}
class Path {}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let caller = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Caller")
            .expect("Caller type");
        let method = caller
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "run" => Some(method),
                _ => None,
            })
            .expect("run method");
        let body = method.body.as_ref().expect("run body");
        let Stmt::VarDecl {
            ty,
            initializer: Some(Expr::Ternary { else_expr, .. }),
            ..
        } = &body.statements[0]
        else {
            panic!("expected nullable Path declaration");
        };
        assert_eq!(
            ty,
            &JavaType::Named {
                name: "Path".to_string(),
                args: Vec::new(),
            }
        );
        let Expr::Call {
            resolved: Some(call),
            ..
        } = else_expr.as_ref()
        else {
            panic!("expected resolved IO.getPath call");
        };
        assert_eq!(
            call.return_type,
            JavaType::Named {
                name: "Path".to_string(),
                args: Vec::new(),
            }
        );
    }

    #[test]
    fn mutated_name_scan_sees_qualified_new_receiver() {
        let expr = Expr::New {
            infer_type_arguments: false,
            ty: JavaType::Unknown("Inner".to_string()),
            qualifier: Some(Box::new(Expr::Unary {
                op: crate::java_ir::UnaryOp::PostInc,
                expr: Box::new(Expr::Var("value".to_string())),
            })),
            args: Vec::new(),
            body: None,
            resolved: None,
        };
        let mut mutated = HashSet::new();

        collect_mutated_names_expr(&expr, &mut mutated);

        assert_eq!(mutated, HashSet::from(["value".to_string()]));
    }

    #[test]
    fn bound_references_capture_values_without_becoming_lambda_owners() {
        let parsed = crate::parser::parse_java_source(
            r#"
import java.util.function.ToIntFunction;
class KeySource { int key(int value) { return value; } }
class References {
    static int key(int value) { return value; }
    static void capture(KeySource source, KeySource other) {
        ToIntFunction<Integer> bound = source::key;
        source = new KeySource();
        ToIntFunction<Integer> unbound = References::key;
        ToIntFunction<Integer> lambda = value -> other.key(value);
    }
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let references = resolved
            .facts
            .captures
            .owners
            .iter()
            .filter(|owner| owner.kind == CaptureOwnerKind::MethodReference)
            .collect::<Vec<_>>();
        assert_eq!(references.len(), 1);
        assert_eq!(references[0].captures.len(), 1);
        let capture = &references[0].captures[0];
        assert_eq!(capture.name, "source");
        assert!(capture.effectively_final);
        assert!(!capture.mutable_after_capture);
        assert_eq!(
            resolved
                .facts
                .captures
                .owners
                .iter()
                .filter(|owner| owner.kind == CaptureOwnerKind::Lambda)
                .count(),
            1
        );
    }

    #[test]
    fn bound_field_reference_values_are_not_lexical_receiver_captures() {
        let parsed = crate::parser::parse_java_source(
            r#"
import java.util.function.*;
class Counter { int read() { return 1; } }
class References {
    Counter field;
    IntSupplier bound() { return field::read; }
    IntSupplier explicit() { return this.field::read; }
    IntSupplier shadow(Counter field) { return field::read; }
    IntSupplier self() { return this::read; }
    int read() { return 2; }
    Supplier<IntSupplier> nested() { return () -> field::read; }
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        for owner in &resolved.facts.captures.owners {
            let CaptureOrigin::Method { method_name, .. } = &owner.origin else {
                continue;
            };
            let expected = match (method_name.as_str(), owner.kind) {
                ("bound" | "explicit" | "nested", CaptureOwnerKind::MethodReference) => {
                    CaptureSource::ThisField
                }
                ("shadow", CaptureOwnerKind::MethodReference) => CaptureSource::Parameter,
                ("self", CaptureOwnerKind::MethodReference)
                | ("nested", CaptureOwnerKind::Lambda) => CaptureSource::This,
                _ => continue,
            };
            assert_eq!(owner.captures.len(), 1, "{owner:?}");
            assert_eq!(owner.captures[0].source, expected, "{owner:?}");
        }
        assert_eq!(resolved.facts.captures.owners.len(), 6);
    }

    #[test]
    fn enclosing_lambda_retains_a_nested_bound_reference_receiver() {
        let parsed = crate::parser::parse_java_source(
            r#"
import java.util.function.Supplier;
import java.util.function.ToIntFunction;
class KeySource { int key(int value) { return value; } }
class References {
    static Supplier<ToIntFunction<Integer>> nested(KeySource source) {
        return () -> source::key;
    }
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        for kind in [CaptureOwnerKind::Lambda, CaptureOwnerKind::MethodReference] {
            let owners = resolved
                .facts
                .captures
                .owners
                .iter()
                .filter(|owner| owner.kind == kind)
                .collect::<Vec<_>>();
            assert_eq!(owners.len(), 1);
            assert_eq!(owners[0].captures.len(), 1);
            assert_eq!(owners[0].captures[0].name, "source");
        }
    }

    #[test]
    fn nested_inner_capture_uses_the_lexical_owner_of_an_ancestor_field() {
        let parsed = crate::parser::parse_java_source(
            r#"
                class Outer {
                    int value;
                    class Middle {
                        class Deep { int read() { return value; } }
                    }
                }
            "#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);

        assert_eq!(
            resolved
                .facts
                .captures
                .outer_this_type_for_type("Outer.Middle"),
            Some("Outer")
        );
        assert_eq!(
            resolved
                .facts
                .captures
                .outer_this_type_for_type("Outer.Middle.Deep"),
            Some("Outer")
        );
    }

    #[test]
    fn nested_record_is_implicitly_static_and_does_not_capture_outer_this() {
        let parsed = crate::parser::parse_java_source(
            r#"
                class Outer {
                    record Pair(int value) {}
                }
            "#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);

        assert!(
            resolved
                .facts
                .captures
                .outer_this_type_for_type("Outer.Pair")
                .is_none()
        );
    }

    #[test]
    fn interface_member_class_has_no_enclosing_instance_or_outer_type_parameters() {
        let parsed = crate::parser::parse_java_source(
            "interface Factory<T> { class Value<U> { U item; Value(U item) { this.item = item; } } }",
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        assert!(
            resolved
                .facts
                .captures
                .outer_this_type_for_type("Factory.Value")
                .is_none()
        );
        let nested = resolved.program.types[0]
            .members
            .iter()
            .find_map(|member| {
                if let Member::Type(ty) = member {
                    Some(ty)
                } else {
                    None
                }
            })
            .expect("member class");
        assert_eq!(nested.type_params, vec!["U"]);
    }

    #[test]
    fn native_reference_invocations_reuse_typed_calls_without_stealing_source_owners() {
        let parsed = crate::parser::parse_java_source("class Host {}");
        let symbols = source_symbols_from_program(&parsed.program);
        let facts = FactSet::default();
        let index = facts.external_lookup_index();
        let mut env = TypeEnv::default();
        let unbound = native_method_reference_lambda(
            "String",
            "length",
            &[JavaType::String],
            &symbols,
            &env,
            &facts,
            &index,
        )
        .unwrap();
        let Expr::Lambda {
            params,
            body: LambdaBody::Expr(body),
        } = unbound
        else {
            panic!("typed lambda");
        };
        assert_eq!(params.len(), 1);
        assert!(
            matches!(*body, Expr::Call { receiver: Some(ref receiver), ref args, .. }
            if matches!(receiver.as_ref(), Expr::Var(name) if name == &params[0].name) && args.is_empty())
        );
        assert!(
            native_method_reference_lambda(
                "String",
                "length",
                &[JavaType::Primitive(PrimitiveType::Int)],
                &symbols,
                &env,
                &facts,
                &index
            )
            .is_none()
        );
        env.insert_local("text", JavaType::String);
        assert!(
            native_method_reference_lambda("text", "length", &[], &symbols, &env, &facts, &index)
                .is_some()
        );
        assert!(
            native_method_reference_lambda(
                "Integer",
                "parseInt",
                &[JavaType::String],
                &symbols,
                &env,
                &facts,
                &index
            )
            .is_some()
        );
        let source =
            crate::parser::parse_java_source("class String { int length() { return 17; } }");
        let source = source_symbols_from_program(&source.program);
        assert!(
            native_method_reference_lambda(
                "String",
                "length",
                &[JavaType::String],
                &source,
                &env,
                &facts,
                &index
            )
            .is_none()
        );
    }
}
