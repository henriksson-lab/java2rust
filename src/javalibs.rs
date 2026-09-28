use crate::java_ir::{Expr, JavaType, Member, Program, Stmt, TypeDecl, WildcardBound};

pub(crate) fn domain_external_receiver_type_alias(name: &str) -> Option<&'static str> {
    match name {
        "HtsjdkTribbleAbstractFeatureReader" => Some("htsjdk.tribble.AbstractFeatureReader"),
        "HtsjdkTribbleUtilParsingUtils" => Some("htsjdk.tribble.util.ParsingUtils"),
        _ => None,
    }
}

pub(crate) fn facade_sequence_element_type(java_ty: &JavaType) -> Option<JavaType> {
    let JavaType::Named { name, args } = java_ty else {
        return None;
    };
    let canonical = crate::stdlib::canonical_external_receiver_type_key(name);
    let simple = canonical.rsplit('.').next().unwrap_or(&canonical);
    match simple {
        "JavaRDD" if args.len() == 1 => Some(args[0].clone()),
        "JavaPairRDD" if args.len() == 2 => Some(JavaType::Named {
            name: "Tuple2".to_string(),
            args: args.to_vec(),
        }),
        "JavaPairRDD" if args.is_empty() => Some(JavaType::Named {
            name: "Tuple2".to_string(),
            args: Vec::new(),
        }),
        _ => None,
    }
}

pub(crate) fn generic_facade_interface_erases_to_named_rust_type(java_ty: &JavaType) -> bool {
    let JavaType::Named { name, args } = java_ty else {
        return false;
    };
    if args.is_empty() {
        return false;
    }
    if crate::stdlib::external_facade_generic_carrier_arity(name) == Some(args.len()) {
        return true;
    }
    matches!(
        crate::stdlib::canonical_external_receiver_type_key(name).as_str(),
        "Comparator" | "java.util.Comparator" | "Comparable" | "java.lang.Comparable"
    )
}

pub(crate) fn broad_ioutils_support_item() -> crate::rust_ir::RustItem {
    crate::rust_ir::RustItem::Raw(
        r#"pub struct IOUtils;

impl IOUtils {
    pub fn get_path<P: JavaPathArg>(path: P) -> Path {
        Path::new(path.into_path_value())
    }

    pub fn append_path_to_dir<D: JavaPathArg, P: JavaPathArg>(dir: D, path: P) -> String {
        let dir = dir.into_path_value();
        let path = path.into_path_value();
        if dir.is_empty() {
            path
        } else if dir.ends_with('/') {
            format!("{dir}{path}")
        } else {
            format!("{dir}/{path}")
        }
    }

    pub fn assert_file_is_readable<P: JavaPathArg>(_path: P) {}

    pub fn assert_paths_are_readable<P: JavaPathArg>(_path: P) {}

    pub fn create_temp_file<P: JavaPathArg, S: JavaPathArg>(prefix: P, suffix: S) -> File {
        File::new(format!("{}{}", prefix.into_path_value(), suffix.into_path_value()))
    }
}"#
        .to_string(),
    )
}

pub(crate) fn program_uses_broad_ioutils_facade(program: &Program) -> bool {
    program.imports.iter().any(|import| {
        matches!(
            import.path.as_str(),
            "org.broadinstitute.hellbender.utils.io.IOUtils"
        )
    }) || program.types.iter().any(type_uses_broad_ioutils_facade)
}

pub(crate) fn is_builtin_external_facade_type(name: &str) -> bool {
    crate::stdlib::canonical_external_receiver_type_key(name) == "IOUtils"
}

pub(crate) fn is_known_java_subtype(actual: &str, target: &str) -> bool {
    let actual = crate::stdlib::canonical_external_receiver_type_key(actual);
    let target = crate::stdlib::canonical_external_receiver_type_key(target);
    if actual == target {
        return true;
    }
    matches!(
        (actual.as_str(), target.as_str()),
        ("SimpleInterval", "Locatable")
            | (
                "org.broadinstitute.hellbender.utils.SimpleInterval",
                "Locatable"
            )
            | (
                "org.broadinstitute.hellbender.utils.SimpleInterval",
                "htsjdk.samtools.util.Locatable"
            )
    )
}

pub(crate) fn needs_conversion_constructor_for_facade(name: &str) -> bool {
    matches!(
        crate::stdlib::canonical_external_receiver_type_key(name).as_str(),
        "Locatable" | "htsjdk.samtools.util.Locatable"
    )
}

pub(crate) fn is_static_facade_method(receiver_type: &str, method: &str, arg_count: usize) -> bool {
    let receiver = crate::stdlib::canonical_external_receiver_type_key(receiver_type);
    if imported_static_method_returns_broad_utils_first_arg(&receiver, method, arg_count) {
        return true;
    }
    if matches!(method, "valueOf" | "value_of") && arg_count == 1 {
        let simple = receiver
            .rsplit(['.', '$'])
            .next()
            .unwrap_or(receiver.as_str());
        if simple != "String" && crate::stdlib::boxed_primitive_type(simple).is_none() {
            return true;
        }
    }
    matches!(
        (receiver.as_str(), method, arg_count),
        ("IOUtils", "getPath", 1)
            | (
                "org.broadinstitute.hellbender.utils.io.IOUtils",
                "getPath",
                1
            )
            | ("IOUtils", "appendPathToDir", 2)
            | (
                "org.broadinstitute.hellbender.utils.io.IOUtils",
                "appendPathToDir",
                2
            )
            | ("IOUtils", "assertFileIsReadable", 1)
            | (
                "org.broadinstitute.hellbender.utils.io.IOUtils",
                "assertFileIsReadable",
                1
            )
            | ("IOUtils", "assertPathsAreReadable", 1)
            | (
                "org.broadinstitute.hellbender.utils.io.IOUtils",
                "assertPathsAreReadable",
                1
            )
            | ("IOUtils", "createTempFile", 2)
            | (
                "org.broadinstitute.hellbender.utils.io.IOUtils",
                "createTempFile",
                2
            )
            | (
                "ClassLoader",
                "getSystemResource" | "get_system_resource",
                1
            )
            | (
                "java.lang.ClassLoader",
                "getSystemResource" | "get_system_resource",
                1
            )
            | ("HtsjdkReadsRddStorage", "makeDefault", 1)
            | ("org.disq_bio.disq.HtsjdkReadsRddStorage", "makeDefault", 1)
            | ("HtsjdkVariantsRddStorage", "makeDefault", 1)
            | (
                "org.disq_bio.disq.HtsjdkVariantsRddStorage",
                "makeDefault",
                1
            )
            | ("BaiWriteOption", "fromBoolean", 1)
            | ("org.disq_bio.disq.BaiWriteOption", "fromBoolean", 1)
            | ("SbiWriteOption", "fromBoolean", 1)
            | ("org.disq_bio.disq.SbiWriteOption", "fromBoolean", 1)
            | ("TabixIndexWriteOption", "fromBoolean", 1)
            | ("org.disq_bio.disq.TabixIndexWriteOption", "fromBoolean", 1)
            | (
                "JavaxSwingRepaintManager",
                "currentManager" | "current_manager",
                1
            )
            | ("RepaintManager", "currentManager" | "current_manager", 1)
            | (
                "javax.swing.RepaintManager",
                "currentManager" | "current_manager",
                1
            )
            | ("Base64", "getEncoder" | "get_encoder", 0)
            | ("JavaUtilBase64", "getEncoder" | "get_encoder", 0)
            | ("java.util.Base64", "getEncoder" | "get_encoder", 0)
            | ("JavaxImageioImageIO", "read", 1)
            | ("ImageIO", "read", 1)
            | ("javax.imageio.ImageIO", "read", 1)
            | ("JavaxImageioImageIO", "write", 3)
            | ("ImageIO", "write", 3)
            | ("javax.imageio.ImageIO", "write", 3)
            | (
                "htsjdk.variant.variantcontext.GenotypeLikelihoods",
                "fromGLField" | "from_g_l_field",
                1,
            )
            | ("GenotypeLikelihoods", "fromGLField" | "from_g_l_field", 1)
            | (
                "htsjdk.variant.vcf.VCFHeaderLineTranslator" | "VCFHeaderLineTranslator",
                "parseLine" | "parse_line",
                4,
            )
            | (
                "htsjdk.tribble.AbstractFeatureReader" | "AbstractFeatureReader",
                "getFeatureReader" | "get_feature_reader",
                3..=4
            )
            | (
                "htsjdk.tribble.util.ParsingUtils" | "ParsingUtils",
                "split",
                2..=4
            )
    )
}

pub(crate) fn native_java_static_owner_alias(receiver_type: &str) -> Option<&'static str> {
    let receiver = crate::stdlib::canonical_external_receiver_type_key(receiver_type);
    match receiver.as_str() {
        "FastMath"
        | "fast_math"
        | "OrgApacheCommonsMath3UtilFastMath"
        | "org.apache.commons.math3.util.FastMath"
        | "StrictMath"
        | "strict_math"
        | "java.lang.StrictMath" => Some("Math"),
        _ => None,
    }
}

pub(crate) fn preferred_static_owner_alias(
    owner: &str,
    method: &str,
    arg_count: usize,
) -> Option<&'static str> {
    let owner = crate::stdlib::canonical_external_receiver_type_key(owner);
    match (owner.as_str(), method, arg_count) {
        (
            "class_loader" | "ClassLoader" | "java.lang.ClassLoader",
            "get_system_resource"
            | "getSystemResource"
            | "get_system_resource_as_stream"
            | "getSystemResourceAsStream",
            1,
        ) => Some("ClassLoader"),
        ("runtime" | "Runtime" | "java.lang.Runtime", "get_runtime" | "getRuntime", 0) => {
            Some("Runtime")
        }
        (
            "org.disq_bio.disq.TabixIndexWriteOption" | "OrgDisqBioDisqTabixIndexWriteOption",
            "from_boolean" | "fromBoolean",
            1,
        ) => Some("TabixIndexWriteOption"),
        (
            "org.broadinstitute.hellbender.utils.read.ReadUtils"
            | "OrgBroadinstituteHellbenderUtilsReadReadUtils",
            "is_read_name_grouped_bam" | "isReadNameGroupedBam",
            1,
        ) => Some("ReadUtils"),
        (
            "org.broadinstitute.hellbender.utils.read.SAMRecordToGATKReadAdapter"
            | "OrgBroadinstituteHellbenderUtilsReadSAMRecordToGATKReadAdapter",
            "headerless_read_adapter" | "headerlessReadAdapter",
            1,
        ) => Some("SAMRecordToGATKReadAdapter"),
        (
            "org.broadinstitute.hellbender.utils.variant.VariantContextVariantAdapter"
            | "OrgBroadinstituteHellbenderUtilsVariantVariantContextVariantAdapter",
            "spark_variant_adapter" | "sparkVariantAdapter",
            1,
        ) => Some("VariantContextVariantAdapter"),
        (
            "org.broadinstitute.hellbender.utils.spark.SparkUtils"
            | "OrgBroadinstituteHellbenderUtilsSparkSparkUtils",
            "hadoop_path_exists" | "hadoopPathExists",
            2,
        ) => Some("SparkUtils"),
        (
            "htsjdk.variant.variantcontext.GenotypeLikelihoods"
            | "HtsjdkVariantVariantcontextGenotypeLikelihoods",
            "from_g_l_field" | "fromGLField",
            1,
        ) => Some("GenotypeLikelihoods"),
        _ => None,
    }
}

pub(crate) fn wildcard_import_static_type_aliases(
    package: &str,
) -> &'static [(&'static str, &'static str)] {
    match package {
        "org.broadinstitute.hellbender.utils.read" => &[
            (
                "ReadUtils",
                "org.broadinstitute.hellbender.utils.read.ReadUtils",
            ),
            (
                "SAMRecordToGATKReadAdapter",
                "org.broadinstitute.hellbender.utils.read.SAMRecordToGATKReadAdapter",
            ),
        ],
        "org.broadinstitute.hellbender.utils.variant" => &[(
            "VariantContextVariantAdapter",
            "org.broadinstitute.hellbender.utils.variant.VariantContextVariantAdapter",
        )],
        _ => &[],
    }
}

pub(crate) fn broad_utils_dynamic_return_type(
    receiver_type: Option<&str>,
    name: &str,
    arg_types: &[JavaType],
) -> Option<JavaType> {
    let receiver_type = receiver_type?;
    if !is_broad_utils_type_name(receiver_type) {
        return None;
    }
    match (name, arg_types) {
        ("stream", [source]) => {
            collection_element_type(source).map(|element| stream_return_type(element.clone()))
        }
        _ => None,
    }
}

pub(crate) fn broad_static_utility_return_type(
    receiver_type: Option<&str>,
    name: &str,
    arg_count: usize,
) -> Option<JavaType> {
    let receiver_type = receiver_type?;
    if is_broad_utils_type_name(receiver_type) {
        return match (name, arg_count) {
            ("join", 2) => Some(JavaType::String),
            ("split", 2) => Some(JavaType::Named {
                name: "List".to_string(),
                args: vec![JavaType::String],
            }),
            _ => None,
        };
    }
    if is_broad_quality_utils_type_name(receiver_type) {
        return match (name, arg_count) {
            ("qualToProb" | "qualToProbLog10", 1) => {
                Some(JavaType::Primitive(crate::java_ir::PrimitiveType::Double))
            }
            _ => None,
        };
    }
    if is_broad_ioutils_type_name(receiver_type) {
        return match (name, arg_count) {
            ("replaceExtension", 2) => Some(JavaType::String),
            _ => None,
        };
    }
    None
}

pub(crate) fn imported_static_method_returns_broad_utils_first_arg(
    owner_key: &str,
    name: &str,
    arg_count: usize,
) -> bool {
    arg_count >= 1
        && ((matches!(name, "non_null" | "nonNull") && is_broad_utils_type_name(owner_key))
            || (matches!(name, "flatten") && is_arg_utils_type_name(owner_key)))
}

pub(crate) fn is_broad_utils_type_name(name: &str) -> bool {
    matches!(
        name,
        "Utils" | "utils" | "org.broadinstitute.hellbender.utils.Utils"
    )
}

fn is_arg_utils_type_name(name: &str) -> bool {
    matches!(name, "ArgUtils" | "arg_utils")
}

fn is_broad_quality_utils_type_name(name: &str) -> bool {
    matches!(
        name,
        "QualityUtils" | "quality_utils" | "org.broadinstitute.hellbender.utils.QualityUtils"
    )
}

fn stream_return_type(element: JavaType) -> JavaType {
    JavaType::Named {
        name: "Stream".to_string(),
        args: vec![element],
    }
}

fn collection_element_type(java_ty: &JavaType) -> Option<&JavaType> {
    match java_ty {
        JavaType::Array(element) => Some(element.as_ref()),
        JavaType::Named { name, args }
            if matches!(
                name.as_str(),
                "Iterable"
                    | "Iterator"
                    | "Collection"
                    | "List"
                    | "Set"
                    | "SortedSet"
                    | "Queue"
                    | "Deque"
                    | "Stream"
                    | "Option"
            ) =>
        {
            args.first()
        }
        JavaType::Named { name, args } if name == "Map" => args.get(1),
        _ => None,
    }
}

fn type_uses_broad_ioutils_facade(ty: &TypeDecl) -> bool {
    ty.extends
        .as_deref()
        .is_some_and(is_broad_ioutils_type_name)
        || ty
            .implements
            .iter()
            .any(|name| is_broad_ioutils_type_name(name))
        || ty
            .type_param_bounds
            .iter()
            .flatten()
            .any(java_type_uses_broad_ioutils_facade)
        || ty.members.iter().any(member_uses_broad_ioutils_facade)
}

fn member_uses_broad_ioutils_facade(member: &Member) -> bool {
    match member {
        Member::Field(field) => {
            java_type_uses_broad_ioutils_facade(&field.ty)
                || field
                    .initializer
                    .as_ref()
                    .is_some_and(expr_uses_broad_ioutils_facade)
        }
        Member::Method(method) => {
            java_type_uses_broad_ioutils_facade(&method.return_type)
                || method
                    .throws
                    .iter()
                    .any(java_type_uses_broad_ioutils_facade)
                || method
                    .params
                    .iter()
                    .any(|param| java_type_uses_broad_ioutils_facade(&param.ty))
                || method
                    .type_param_bounds
                    .iter()
                    .flatten()
                    .any(java_type_uses_broad_ioutils_facade)
                || method
                    .body
                    .as_ref()
                    .is_some_and(block_uses_broad_ioutils_facade)
        }
        Member::Constructor(constructor) => {
            constructor
                .throws
                .iter()
                .any(java_type_uses_broad_ioutils_facade)
                || constructor
                    .params
                    .iter()
                    .any(|param| java_type_uses_broad_ioutils_facade(&param.ty))
                || constructor
                    .type_param_bounds
                    .iter()
                    .flatten()
                    .any(java_type_uses_broad_ioutils_facade)
                || constructor
                    .body
                    .as_ref()
                    .is_some_and(block_uses_broad_ioutils_facade)
        }
        Member::Type(nested) => type_uses_broad_ioutils_facade(nested),
        Member::Unsupported { .. } => false,
    }
}

fn block_uses_broad_ioutils_facade(block: &crate::java_ir::Block) -> bool {
    block.statements.iter().any(stmt_uses_broad_ioutils_facade)
}

fn stmt_uses_broad_ioutils_facade(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::VarDecl {
            ty, initializer, ..
        } => {
            java_type_uses_broad_ioutils_facade(ty)
                || initializer
                    .as_ref()
                    .is_some_and(expr_uses_broad_ioutils_facade)
        }
        Stmt::Return(initializer) => initializer
            .as_ref()
            .is_some_and(expr_uses_broad_ioutils_facade),
        Stmt::Expr(expr) | Stmt::Throw(expr) => expr_uses_broad_ioutils_facade(expr),
        Stmt::Assign { target, value } => {
            expr_uses_broad_ioutils_facade(target) || expr_uses_broad_ioutils_facade(value)
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_uses_broad_ioutils_facade(condition)
                || block_uses_broad_ioutils_facade(then_block)
                || else_block
                    .as_ref()
                    .is_some_and(block_uses_broad_ioutils_facade)
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => expr_uses_broad_ioutils_facade(condition) || block_uses_broad_ioutils_facade(body),
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter().any(stmt_uses_broad_ioutils_facade)
                || condition
                    .as_ref()
                    .is_some_and(expr_uses_broad_ioutils_facade)
                || update.iter().any(expr_uses_broad_ioutils_facade)
                || block_uses_broad_ioutils_facade(body)
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            expr_uses_broad_ioutils_facade(iterable) || block_uses_broad_ioutils_facade(body)
        }
        Stmt::Switch { selector, cases } => {
            expr_uses_broad_ioutils_facade(selector)
                || cases
                    .iter()
                    .any(|case| case.statements.iter().any(stmt_uses_broad_ioutils_facade))
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources.iter().any(stmt_uses_broad_ioutils_facade)
                || block_uses_broad_ioutils_facade(body)
                || catches
                    .iter()
                    .any(|catch| block_uses_broad_ioutils_facade(&catch.body))
                || finally_block
                    .as_ref()
                    .is_some_and(block_uses_broad_ioutils_facade)
        }
        Stmt::ConstructorInvocation { args, .. } => args.iter().any(expr_uses_broad_ioutils_facade),
        Stmt::LocalType(ty) => type_uses_broad_ioutils_facade(ty),
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_uses_broad_ioutils_facade(block)
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => false,
    }
}

fn expr_uses_broad_ioutils_facade(expr: &Expr) -> bool {
    match expr {
        Expr::Call { receiver, args, .. } => {
            receiver
                .as_deref()
                .is_some_and(expr_is_broad_ioutils_receiver)
                || receiver
                    .as_deref()
                    .is_some_and(expr_uses_broad_ioutils_facade)
                || args.iter().any(expr_uses_broad_ioutils_facade)
        }
        Expr::FieldAccess(receiver, _) => expr_uses_broad_ioutils_facade(receiver),
        Expr::ArrayAccess(array, index) => {
            expr_uses_broad_ioutils_facade(array) || expr_uses_broad_ioutils_facade(index)
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions.iter().any(expr_uses_broad_ioutils_facade)
                || initializer
                    .iter()
                    .flatten()
                    .any(expr_uses_broad_ioutils_facade)
        }
        Expr::ArrayInitializer(values) => values.iter().any(expr_uses_broad_ioutils_facade),
        Expr::Cast { expr, .. } | Expr::Unary { expr, .. } => expr_uses_broad_ioutils_facade(expr),
        Expr::Binary { left, right, .. } => {
            expr_uses_broad_ioutils_facade(left) || expr_uses_broad_ioutils_facade(right)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_uses_broad_ioutils_facade(condition)
                || expr_uses_broad_ioutils_facade(then_expr)
                || expr_uses_broad_ioutils_facade(else_expr)
        }
        Expr::Switch { selector, cases } => {
            expr_uses_broad_ioutils_facade(selector)
                || cases.iter().any(|case| {
                    case.labels.iter().any(|label| {
                        matches!(
                            label,
                            crate::java_ir::SwitchLabel::Expr(expr)
                                if expr_uses_broad_ioutils_facade(expr)
                        )
                    }) || expr_uses_broad_ioutils_facade(&case.value)
                })
        }
        Expr::InstanceOf { expr, .. } => expr_uses_broad_ioutils_facade(expr),
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => expr_uses_broad_ioutils_facade(expr),
            crate::java_ir::LambdaBody::Block(block) => block_uses_broad_ioutils_facade(block),
        },
        Expr::New { args, body, .. } => {
            args.iter().any(expr_uses_broad_ioutils_facade)
                || body.iter().flatten().any(member_uses_broad_ioutils_facade)
        }
        Expr::ClassLiteral(ty) => java_type_uses_broad_ioutils_facade(ty),
        Expr::MethodReference { target, .. } => is_broad_ioutils_type_name(target),
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => false,
    }
}

fn expr_is_broad_ioutils_receiver(expr: &Expr) -> bool {
    matches!(expr, Expr::Var(name) if is_broad_ioutils_type_name(name))
}

fn java_type_uses_broad_ioutils_facade(ty: &JavaType) -> bool {
    match ty {
        JavaType::Named { name, args } => {
            is_broad_ioutils_type_name(name) || args.iter().any(java_type_uses_broad_ioutils_facade)
        }
        JavaType::Array(inner) => java_type_uses_broad_ioutils_facade(inner),
        JavaType::Wildcard(Some(WildcardBound::Extends(bound)))
        | JavaType::Wildcard(Some(WildcardBound::Super(bound))) => {
            java_type_uses_broad_ioutils_facade(bound)
        }
        JavaType::Unknown(name) => is_broad_ioutils_type_name(name),
        JavaType::Void | JavaType::Primitive(_) | JavaType::String | JavaType::Wildcard(None) => {
            false
        }
    }
}

fn is_broad_ioutils_type_name(name: &str) -> bool {
    matches!(
        name,
        "IOUtils" | "io_utils" | "i_o_utils" | "org.broadinstitute.hellbender.utils.io.IOUtils"
    )
}

/// JDK class names the translator has hardcoded knowledge of.
///
/// These sets used to live inline in `src/lower.rs`, as `matches!` arms in
/// one-off `-> bool` predicates. That is the "hardcoded library knowledge"
/// cluster [0008](../claude_audits/done/0008-inference-machinery-does-not-reach-lowering.md)
/// step 4 is about: facts about a *library* compiled into the translator's
/// lowering rules, where they cannot be inspected, tested, or overridden.
///
/// Moving them here does not make them data-driven — `javalibs` is Rust, not a
/// manifest — but it does put library knowledge in the library module and give
/// each set one named query and one test. That is the honest half of step 4;
/// see the issue for why the other half is not a mechanical move.
/// Exception classes broad enough to be worth treating as a supertype when
/// matching a `catch`.
const KNOWN_EXCEPTION_SUPERTYPES: &[&str] = &[
    "Throwable",
    "Exception",
    "RuntimeException",
    "IllegalArgumentException",
    "IllegalStateException",
];

/// `java.io` stream classes a source type can extend and still be lowered
/// against the stream facade.
const JDK_INPUT_STREAM_FACADE_TYPES: &[&str] = &[
    "InputStream",
    "FilterInputStream",
    "GZIPInputStream",
    "java.io.InputStream",
    "java.io.FilterInputStream",
    "java.util.zip.GZIPInputStream",
];

/// Whether a simple class name is one of the broad JDK exception supertypes.
pub(crate) fn is_known_exception_supertype(simple_name: &str) -> bool {
    KNOWN_EXCEPTION_SUPERTYPES.contains(&simple_name)
}

/// Whether a canonical external type key names a JDK input-stream facade type.
pub(crate) fn is_jdk_input_stream_facade_type(canonical_key: &str) -> bool {
    JDK_INPUT_STREAM_FACADE_TYPES.contains(&canonical_key)
}

#[cfg(test)]
mod jdk_name_set_tests {
    use super::*;
    use crate::parser::parse_java_source;
    use crate::rust_ir::RustItem;

    #[test]
    fn exception_supertypes_cover_the_hierarchy_roots_and_nothing_else() {
        assert!(is_known_exception_supertype("Throwable"));
        assert!(is_known_exception_supertype("RuntimeException"));
        // A specific exception is not a supertype for this purpose: treating it
        // as one would make a narrow `catch` swallow more than Java does.
        assert!(!is_known_exception_supertype("IOException"));
        assert!(!is_known_exception_supertype("NumberFormatException"));
    }

    #[test]
    fn input_stream_facade_types_match_both_spellings() {
        assert!(is_jdk_input_stream_facade_type("InputStream"));
        assert!(is_jdk_input_stream_facade_type("java.io.InputStream"));
        assert!(is_jdk_input_stream_facade_type(
            "java.util.zip.GZIPInputStream"
        ));
        assert!(!is_jdk_input_stream_facade_type("OutputStream"));
        assert!(!is_jdk_input_stream_facade_type("Reader"));
    }

    #[test]
    fn broad_ioutils_support_item_contains_path_file_boundaries() {
        let RustItem::Raw(support) = broad_ioutils_support_item() else {
            panic!("Broad IOUtils support should remain a single runtime support item");
        };

        assert!(support.contains("pub struct IOUtils;"));
        assert!(support.contains("pub fn get_path<P: JavaPathArg>(path: P) -> Path"));
        assert!(support.contains("pub fn append_path_to_dir<D: JavaPathArg, P: JavaPathArg>"));
        assert!(support.contains("pub fn assert_file_is_readable<P: JavaPathArg>"));
        assert!(support.contains("pub fn assert_paths_are_readable<P: JavaPathArg>"));
        assert!(support.contains("pub fn create_temp_file<P: JavaPathArg, S: JavaPathArg>"));
    }

    #[test]
    fn broad_ioutils_facade_detector_matches_imports() {
        let imported = parse_java_source(
            r#"
import org.broadinstitute.hellbender.utils.io.IOUtils;

class UseImportedBroadIoUtils {
    String fromString(String value) {
        return IOUtils.getPath(value).toString();
    }
}
"#,
        );
        assert!(program_uses_broad_ioutils_facade(&imported.program));
    }
}
