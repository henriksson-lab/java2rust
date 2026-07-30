use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::effective_facts::EffectiveFacts;
use crate::facts::FactSet;
use crate::facts::{
    CallableKey, ExternalApiEffectFact, ExternalApiRole, ExternalArgumentAliasPolicy,
    ExternalConversionAdapterKind, ExternalFieldFact, ExternalFieldKind, FacadeAbiFact, FacadeSlot,
    FactProvenance, FieldKey, HierarchyStrategy, NullabilityState, ReceiverAbi,
    ReceiverMutabilityFact, ResultAbi, Staticness, TypeKey, TypePattern,
};
use crate::java_ir::{
    BinaryOp, Block, Expr, FieldDecl, JavaType, Literal, Member, MethodDecl, Modifier,
    PrimitiveType, Program, Stmt, TypeDecl, TypeKind, WildcardBound,
};
use crate::shape_projection::{
    apply_borrowed_sequence_content_projection, apply_borrowed_string_content_projection,
};
use crate::slot_render::{
    dyn_trait_rust_path, rust_name_for_java_named_type, rust_type_for_java_type_with_facts,
    rust_type_for_slot_with_facts,
};
use crate::slot_shape::{MutabilityShape, OwnershipShape, RustSlotShape};
use crate::types::{
    raw_generic_type_head, raw_rust_option_inner, raw_rust_result_args, raw_rust_type_is_option,
    raw_rust_type_is_result,
};

pub fn is_known_jdk_shadow_sensitive_owner(name: &str) -> bool {
    matches!(name, "Objects" | "Collections" | "Integer" | "Paths")
}

pub fn is_content_read_method(name: &str) -> bool {
    matches!(
        name,
        "length"
            | "isEmpty"
            | "is_empty"
            | "trim"
            | "contains"
            | "startsWith"
            | "starts_with"
            | "endsWith"
            | "ends_with"
            | "replace"
            | "replaceFirst"
            | "replace_first"
            | "replaceAll"
            | "replace_all"
            | "split"
    )
}

pub fn is_borrowed_string_view_method(name: &str, arg_count: usize) -> bool {
    name == "trim" && arg_count == 0
}

pub fn is_owned_string_result_method(name: &str, arg_count: usize) -> bool {
    matches!(name, "toLowerCase" | "toUpperCase" | "toString") && arg_count == 0
}

pub fn is_known_receiverless_static_utility_method(name: &str, arg_count: usize) -> bool {
    matches!(name, "nonNull" | "isNull") && arg_count == 1
}

pub fn has_receiver_agnostic_external_return_type(name: &str, arg_count: usize) -> bool {
    known_external_method_return_type(None, name, arg_count).is_some()
}

pub fn is_known_object_inherited_method(name: &str, arg_count: usize) -> bool {
    matches!(
        (name, arg_count),
        ("getClass", 0) | ("hashCode", 0) | ("toString", 0) | ("equals", 1)
    )
}

pub fn is_known_functional_interface(name: &str) -> bool {
    matches!(
        name,
        "Supplier"
            | "Consumer"
            | "Function"
            | "Predicate"
            | "BiFunction"
            | "BiConsumer"
            | "UnaryOperator"
            | "BinaryOperator"
            | "ToDoubleFunction"
            | "ToIntFunction"
            | "ToLongFunction"
            | "DoublePredicate"
            | "DoubleUnaryOperator"
            | "DoubleFunction"
            | "DoubleConsumer"
            | "DoubleSupplier"
            | "DoubleToIntFunction"
            | "DoubleToLongFunction"
            | "IntPredicate"
            | "IntUnaryOperator"
            | "IntFunction"
            | "IntConsumer"
            | "IntSupplier"
            | "IntToDoubleFunction"
            | "IntToLongFunction"
            | "LongPredicate"
            | "LongUnaryOperator"
            | "LongFunction"
            | "LongConsumer"
            | "LongSupplier"
            | "LongToDoubleFunction"
            | "LongToIntFunction"
            | "Runnable"
            | "Callable"
            | "Comparator"
            | "Iterable"
            | "FlatMapFunction"
            | "MapFunction"
            | "PairFunction"
            | "VoidFunction"
    )
}

pub fn boxed_primitive_type(name: &str) -> Option<PrimitiveType> {
    match name {
        "Boolean" => Some(PrimitiveType::Boolean),
        "Byte" => Some(PrimitiveType::Byte),
        "Short" => Some(PrimitiveType::Short),
        "Character" => Some(PrimitiveType::Char),
        "Integer" => Some(PrimitiveType::Int),
        "Long" => Some(PrimitiveType::Long),
        "Float" => Some(PrimitiveType::Float),
        "Double" => Some(PrimitiveType::Double),
        _ => None,
    }
}

fn number_value_method_return_type(name: &str, arg_count: usize) -> Option<PrimitiveType> {
    if arg_count != 0 {
        return None;
    }
    match name {
        "byteValue" => Some(PrimitiveType::Byte),
        "shortValue" => Some(PrimitiveType::Short),
        "intValue" => Some(PrimitiveType::Int),
        "longValue" => Some(PrimitiveType::Long),
        "floatValue" => Some(PrimitiveType::Float),
        "doubleValue" => Some(PrimitiveType::Double),
        _ => None,
    }
}

fn boxed_number_value_method_return_type(
    receiver: &str,
    name: &str,
    arg_count: usize,
) -> Option<PrimitiveType> {
    is_known_boxed_number_receiver(receiver)
        .then(|| number_value_method_return_type(name, arg_count))
        .flatten()
}

fn is_known_boxed_number_receiver(receiver: &str) -> bool {
    matches!(
        receiver,
        "Number" | "Byte" | "Short" | "Integer" | "Long" | "Float" | "Double"
    )
}

pub fn is_known_java_subtype(actual: &str, target: &str) -> bool {
    let actual = strip_java_type_modifiers(actual);
    let target = strip_java_type_modifiers(target);
    if actual == target {
        return true;
    }
    match target {
        "Object" => is_known_reference_type(actual),
        "Throwable" => is_known_throwable_type(actual),
        "Exception" => is_known_exception_type(actual),
        "Error" => is_known_error_type(actual),
        "IOException" => matches!(
            actual,
            "IOException" | "FileNotFoundException" | "EOFException"
        ),
        "RuntimeException" => matches!(
            actual,
            "RuntimeException"
                | "IllegalArgumentException"
                | "IllegalStateException"
                | "NoSuchElementException"
                | "NumberFormatException"
                | "ArrayIndexOutOfBoundsException"
                | "IndexOutOfBoundsException"
                | "ConcurrentModificationException"
                | "UnsupportedOperationException"
                | "UncheckedIOException"
        ),
        "Number" => matches!(
            actual,
            "Byte" | "Short" | "Integer" | "Long" | "Float" | "Double"
        ),
        "CharSequence" => matches!(actual, "String" | "StringBuilder" | "StringBuffer"),
        "Appendable" => matches!(actual, "StringBuilder" | "StringBuffer" | "PrintStream"),
        "Iterable" => is_known_iterable_type(actual),
        "Collection" => is_known_collection_type(actual),
        "List" => is_known_list_type(actual),
        "Set" => is_known_set_type(actual),
        "SortedSet" => matches!(actual, "SortedSet" | "NavigableSet" | "TreeSet"),
        "NavigableSet" => matches!(actual, "NavigableSet" | "TreeSet"),
        "Queue" => matches!(
            actual,
            "Queue" | "Deque" | "PriorityQueue" | "ArrayDeque" | "ArrayBlockingQueue"
        ),
        "Deque" => matches!(actual, "Deque" | "ArrayDeque" | "LinkedList"),
        "Map" => is_known_map_type(actual),
        "SortedMap" => matches!(
            actual,
            "SortedMap" | "NavigableMap" | "TreeMap" | "ImmutableSortedMap"
        ),
        "NavigableMap" => matches!(actual, "NavigableMap" | "TreeMap"),
        "InputStream" => is_known_input_stream_type(actual),
        "OutputStream" => is_known_output_stream_type(actual),
        "Reader" => is_known_reader_type(actual),
        "Writer" => is_known_writer_type(actual),
        "WriteOption" => is_known_write_option_type(actual),
        _ => false,
    }
}

pub fn is_known_single_arg_generic_supertype(actual: &str, target: &str) -> bool {
    matches!(
        target,
        "Iterable"
            | "Collection"
            | "List"
            | "Set"
            | "SortedSet"
            | "NavigableSet"
            | "Queue"
            | "Deque"
    ) && is_known_java_subtype(actual, target)
}

pub fn is_known_structural_reference_target(name: &str) -> bool {
    matches!(name, "Iterable" | "Comparable" | "Serializable")
}

fn is_known_iterable_type(name: &str) -> bool {
    is_known_collection_type(name)
        || matches!(
            name,
            "Iterable" | "Iterator" | "Spliterator" | "Enumeration" | "DirectoryStream"
        )
}

fn is_known_iterator_type(name: &str) -> bool {
    matches!(name, "Iterator" | "ListIterator" | "Enumeration")
}

fn is_known_collection_type(name: &str) -> bool {
    is_known_list_type(name)
        || is_known_set_type(name)
        || is_known_queue_type(name)
        || matches!(name, "Collection" | "IntervalList")
}

fn is_known_list_type(name: &str) -> bool {
    matches!(
        name,
        "List" | "AbstractList" | "ArrayList" | "LinkedList" | "Vector" | "Stack"
    )
}

fn is_known_set_type(name: &str) -> bool {
    matches!(
        name,
        "Set"
            | "AbstractSet"
            | "HashSet"
            | "LinkedHashSet"
            | "SortedSet"
            | "NavigableSet"
            | "TreeSet"
            | "EnumSet"
    )
}

fn is_known_queue_type(name: &str) -> bool {
    matches!(
        name,
        "Queue" | "Deque" | "PriorityQueue" | "ArrayDeque" | "ArrayBlockingQueue" | "LinkedList"
    )
}

fn is_known_map_type(name: &str) -> bool {
    matches!(
        name,
        "Map"
            | "HashMap"
            | "Hashtable"
            | "LinkedHashMap"
            | "TreeMap"
            | "ConcurrentHashMap"
            | "SortedMap"
            | "NavigableMap"
            | "EnumMap"
            | "ImmutableMap"
            | "ImmutableSortedMap"
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JavaCollectionKind {
    List,
    Set,
    Map,
}

pub fn java_collection_kind_for_name(name: &str) -> Option<JavaCollectionKind> {
    let name = canonical_external_receiver_type_key(name);
    let name = name.rsplit('.').next().unwrap_or(&name);
    if is_known_map_type(name) {
        Some(JavaCollectionKind::Map)
    } else if is_known_set_type(name) {
        Some(JavaCollectionKind::Set)
    } else if is_known_list_type(name) || is_known_collection_type(name) {
        Some(JavaCollectionKind::List)
    } else {
        None
    }
}

pub fn java_collection_kind_for_type(ty: &JavaType) -> Option<JavaCollectionKind> {
    match ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            java_collection_kind_for_name(name)
        }
        _ => None,
    }
}

pub fn native_collection_constructor_kind_for_type(ty: &JavaType) -> Option<JavaCollectionKind> {
    let kind = java_collection_kind_for_type(ty)?;
    match ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            let simple = canonical_external_receiver_type_key(name);
            let simple = simple.rsplit('.').next().unwrap_or(simple.as_str());
            if matches!(simple, "IntervalList") {
                None
            } else {
                Some(kind)
            }
        }
        _ => None,
    }
}

pub fn rust_stdlib_signature_facts() -> FactSet {
    let mut facts = FactSet::default();
    facts.facade_abi_facts.extend([
        rust_stdlib_method_abi("String", "length", 0, "str::len", vec![], FacadeSlot::Owned),
        rust_stdlib_method_abi(
            "String",
            "isEmpty",
            0,
            "str::is_empty",
            vec![],
            FacadeSlot::Owned,
        ),
        rust_stdlib_method_abi(
            "String",
            "trim",
            0,
            "str::trim",
            vec![],
            FacadeSlot::Borrowed,
        ),
        rust_stdlib_method_abi(
            "String",
            "toLowerCase",
            0,
            "str::to_lowercase",
            vec![],
            FacadeSlot::Owned,
        ),
        rust_stdlib_method_abi(
            "String",
            "toUpperCase",
            0,
            "str::to_uppercase",
            vec![],
            FacadeSlot::Owned,
        ),
        rust_stdlib_method_abi(
            "String",
            "toString",
            0,
            "ToString::to_string",
            vec![],
            FacadeSlot::Owned,
        ),
        rust_stdlib_method_abi(
            "String",
            "contains",
            1,
            "str::contains",
            vec![FacadeSlot::Borrowed],
            FacadeSlot::Owned,
        ),
        rust_stdlib_method_abi(
            "String",
            "startsWith",
            1,
            "str::starts_with",
            vec![FacadeSlot::Borrowed],
            FacadeSlot::Owned,
        ),
        rust_stdlib_method_abi(
            "String",
            "endsWith",
            1,
            "str::ends_with",
            vec![FacadeSlot::Borrowed],
            FacadeSlot::Owned,
        ),
        rust_stdlib_method_abi(
            "String",
            "replace",
            2,
            "str::replace",
            vec![FacadeSlot::Borrowed, FacadeSlot::Borrowed],
            FacadeSlot::Owned,
        ),
        rust_stdlib_method_abi(
            "String",
            "replaceFirst",
            2,
            "java_string_replace_first",
            vec![FacadeSlot::Borrowed, FacadeSlot::Borrowed],
            FacadeSlot::Owned,
        ),
        rust_stdlib_method_abi(
            "String",
            "replaceAll",
            2,
            "java_string_replace_all",
            vec![FacadeSlot::Borrowed, FacadeSlot::Borrowed],
            FacadeSlot::Owned,
        ),
        rust_stdlib_method_abi(
            "String",
            "matches",
            1,
            "java_string_matches",
            vec![FacadeSlot::Borrowed],
            FacadeSlot::Owned,
        ),
        rust_stdlib_method_abi(
            "String",
            "split",
            1,
            "java_string_split",
            vec![FacadeSlot::Borrowed],
            FacadeSlot::Owned,
        ),
        rust_stdlib_method_abi(
            "String",
            "split",
            2,
            "java_string_split",
            vec![FacadeSlot::Borrowed, FacadeSlot::Owned],
            FacadeSlot::Owned,
        ),
        rust_stdlib_method_abi(
            "String",
            "charAt",
            1,
            "str::chars::nth",
            vec![FacadeSlot::Index],
            FacadeSlot::Owned,
        ),
        rust_stdlib_method_abi(
            "String",
            "char_at",
            1,
            "str::chars::nth",
            vec![FacadeSlot::Index],
            FacadeSlot::Owned,
        ),
        rust_stdlib_method_abi(
            "String",
            "compareTo",
            1,
            "java_string_compare_to",
            vec![FacadeSlot::Borrowed],
            FacadeSlot::Owned,
        ),
        rust_stdlib_method_abi(
            "String",
            "compareToIgnoreCase",
            1,
            "java_string_compare_to_ignore_case",
            vec![FacadeSlot::Borrowed],
            FacadeSlot::Owned,
        ),
        rust_stdlib_method_abi(
            "String",
            "equalsIgnoreCase",
            1,
            "java_string_equals_ignore_case",
            vec![FacadeSlot::Borrowed],
            FacadeSlot::Owned,
        ),
        rust_stdlib_method_abi(
            "String",
            "indexOf",
            1,
            "str::find",
            vec![FacadeSlot::Borrowed],
            FacadeSlot::Owned,
        ),
        rust_stdlib_method_abi(
            "String",
            "indexOf",
            2,
            "java_string_index_of_from",
            vec![FacadeSlot::Borrowed, FacadeSlot::Owned],
            FacadeSlot::Owned,
        ),
        rust_stdlib_method_abi(
            "String",
            "lastIndexOf",
            1,
            "str::rfind",
            vec![FacadeSlot::Borrowed],
            FacadeSlot::Owned,
        ),
        rust_stdlib_method_abi(
            "String",
            "lastIndexOf",
            2,
            "java_string_last_index_of_from",
            vec![FacadeSlot::Borrowed, FacadeSlot::Owned],
            FacadeSlot::Owned,
        ),
        rust_stdlib_method_abi(
            "String",
            "getBytes",
            1,
            "str::as_bytes",
            vec![FacadeSlot::Borrowed],
            FacadeSlot::Owned,
        ),
        rust_stdlib_method_abi(
            "String",
            "get_bytes",
            1,
            "str::as_bytes",
            vec![FacadeSlot::Borrowed],
            FacadeSlot::Owned,
        ),
        rust_stdlib_static_abi(
            "Integer",
            "parseInt",
            1,
            "str::parse::<i32>",
            vec![FacadeSlot::Borrowed],
            FacadeSlot::Owned,
        ),
        rust_stdlib_static_abi(
            "Double",
            "parseDouble",
            1,
            "str::parse::<f64>",
            vec![FacadeSlot::Borrowed],
            FacadeSlot::Owned,
        ),
        rust_stdlib_static_abi(
            "Float",
            "parseFloat",
            1,
            "str::parse::<f32>",
            vec![FacadeSlot::Borrowed],
            FacadeSlot::Owned,
        ),
        rust_stdlib_static_abi(
            "Float",
            "parse_float",
            1,
            "str::parse::<f32>",
            vec![FacadeSlot::Borrowed],
            FacadeSlot::Owned,
        ),
        rust_stdlib_static_abi(
            "Double",
            "isNaN",
            1,
            "f64::is_nan",
            vec![FacadeSlot::Owned],
            FacadeSlot::Owned,
        ),
        rust_stdlib_static_abi(
            "Float",
            "isNaN",
            1,
            "f32::is_nan",
            vec![FacadeSlot::Owned],
            FacadeSlot::Owned,
        ),
    ]);
    for owner in ["Math", "java.lang.Math"] {
        facts.facade_abi_facts.extend([
            rust_stdlib_static_abi(
                owner,
                "max",
                2,
                "std::cmp::max",
                vec![FacadeSlot::Owned, FacadeSlot::Owned],
                FacadeSlot::Owned,
            ),
            rust_stdlib_static_abi(
                owner,
                "min",
                2,
                "std::cmp::min",
                vec![FacadeSlot::Owned, FacadeSlot::Owned],
                FacadeSlot::Owned,
            ),
            rust_stdlib_static_abi(
                owner,
                "log",
                1,
                "f64::ln",
                vec![FacadeSlot::Owned],
                FacadeSlot::Owned,
            ),
            rust_stdlib_static_abi(
                owner,
                "exp",
                1,
                "f64::exp",
                vec![FacadeSlot::Owned],
                FacadeSlot::Owned,
            ),
            rust_stdlib_static_abi(
                owner,
                "toIntExact",
                1,
                "i32::try_from",
                vec![FacadeSlot::Owned],
                FacadeSlot::Owned,
            ),
        ]);
    }
    for owner in ["System", "java.lang.System"] {
        facts.facade_abi_facts.push(rust_stdlib_static_abi(
            owner,
            "gc",
            0,
            "System::gc",
            Vec::new(),
            FacadeSlot::Owned,
        ));
        facts.facade_abi_facts.push(rust_stdlib_static_abi(
            owner,
            "exit",
            1,
            "std::process::exit",
            vec![FacadeSlot::Owned],
            FacadeSlot::Owned,
        ));
        facts.facade_abi_facts.push(rust_stdlib_static_abi(
            owner,
            "arraycopy",
            5,
            "System::arraycopy",
            vec![
                FacadeSlot::Slice(Box::new(FacadeSlot::Borrowed)),
                FacadeSlot::Index,
                FacadeSlot::Slice(Box::new(FacadeSlot::MutBorrowed)),
                FacadeSlot::Index,
                FacadeSlot::Index,
            ],
            FacadeSlot::Owned,
        ));
    }
    for owner in ["Thread", "java.lang.Thread"] {
        facts.facade_abi_facts.push(rust_stdlib_static_abi(
            owner,
            "sleep",
            1,
            "std::thread::sleep",
            vec![FacadeSlot::Owned],
            FacadeSlot::Owned,
        ));
    }
    add_jdk_numeric_facade_facts(&mut facts);
    add_jdk_collection_abi_facts(&mut facts);
    add_jdk_iterator_abi_facts(&mut facts);
    add_jdk_optional_abi_facts(&mut facts);
    add_jdk_objects_abi_facts(&mut facts);
    add_jdk_string_tokenizer_abi_facts(&mut facts);
    add_jdk_platform_stream_facts(&mut facts);
    add_jdk_platform_io_abi_facts(&mut facts);
    facts
}

fn add_jdk_iterator_abi_facts(facts: &mut FactSet) {
    for owner in ["Iterator", "java.util.Iterator", "JavaIterator"] {
        facts.facade_abi_facts.push(rust_stdlib_method_abi(
            owner,
            "hasNext",
            0,
            "JavaIterator::has_next",
            Vec::new(),
            FacadeSlot::Owned,
        ));
        facts.facade_abi_facts.push(rust_stdlib_method_abi(
            owner,
            "has_next",
            0,
            "JavaIterator::has_next",
            Vec::new(),
            FacadeSlot::Owned,
        ));
        for method in ["next", "next_element"] {
            let mut abi = rust_stdlib_method_abi(
                owner,
                method,
                0,
                "JavaIterator::next",
                Vec::new(),
                FacadeSlot::Owned,
            );
            abi.receiver = ReceiverAbi::MutBorrowedSelf;
            facts.facade_abi_facts.push(abi);
        }
        for method in ["forEachRemaining", "for_each_remaining"] {
            let mut abi = rust_stdlib_method_abi(
                owner,
                method,
                1,
                "JavaIterator::for_each_remaining",
                vec![FacadeSlot::Owned],
                FacadeSlot::Owned,
            );
            abi.receiver = ReceiverAbi::MutBorrowedSelf;
            facts.facade_abi_facts.push(abi);
        }
    }
}

fn add_jdk_string_tokenizer_abi_facts(facts: &mut FactSet) {
    for owner in ["StringTokenizer", "java.util.StringTokenizer"] {
        facts.external_api_effect_facts.extend([
            jdk_instance_api_fact(
                owner,
                "countTokens",
                0,
                JavaType::Primitive(PrimitiveType::Int),
            ),
            jdk_instance_api_fact(
                owner,
                "count_tokens",
                0,
                JavaType::Primitive(PrimitiveType::Int),
            ),
            jdk_instance_api_fact(
                owner,
                "hasMoreTokens",
                0,
                JavaType::Primitive(PrimitiveType::Boolean),
            ),
            jdk_instance_api_fact(
                owner,
                "has_more_tokens",
                0,
                JavaType::Primitive(PrimitiveType::Boolean),
            ),
        ]);
        facts.external_api_effect_facts.push(jdk_instance_api_fact(
            owner,
            "nextToken",
            0,
            JavaType::String,
        ));
        facts.external_api_effect_facts.push(jdk_instance_api_fact(
            owner,
            "next_token",
            0,
            JavaType::String,
        ));
        for method in ["countTokens", "count_tokens"] {
            facts.facade_abi_facts.push(rust_stdlib_method_abi(
                owner,
                method,
                0,
                "StringTokenizer::count_tokens",
                Vec::new(),
                FacadeSlot::Owned,
            ));
        }
        for method in ["hasMoreTokens", "has_more_tokens"] {
            facts.facade_abi_facts.push(rust_stdlib_method_abi(
                owner,
                method,
                0,
                "StringTokenizer::has_more_tokens",
                Vec::new(),
                FacadeSlot::Owned,
            ));
        }
        for method in ["nextToken", "next_token"] {
            let mut abi = rust_stdlib_method_abi(
                owner,
                method,
                0,
                "StringTokenizer::next_token",
                Vec::new(),
                FacadeSlot::Owned,
            );
            abi.receiver = ReceiverAbi::MutBorrowedSelf;
            facts.facade_abi_facts.push(abi);
        }
    }
}

fn add_jdk_objects_abi_facts(facts: &mut FactSet) {
    for owner in ["Objects", "java.util.Objects"] {
        facts.facade_abi_facts.extend([
            rust_stdlib_static_abi(
                owner,
                "equals",
                2,
                "Objects::equals",
                vec![
                    FacadeSlot::Optional(Box::new(FacadeSlot::Borrowed)),
                    FacadeSlot::Optional(Box::new(FacadeSlot::Borrowed)),
                ],
                FacadeSlot::Owned,
            ),
            rust_stdlib_static_abi(
                owner,
                "nonNull",
                1,
                "Option::is_some",
                vec![FacadeSlot::Optional(Box::new(FacadeSlot::Borrowed))],
                FacadeSlot::Owned,
            ),
            rust_stdlib_static_abi(
                owner,
                "isNull",
                1,
                "Option::is_none",
                vec![FacadeSlot::Optional(Box::new(FacadeSlot::Borrowed))],
                FacadeSlot::Owned,
            ),
        ]);
    }
}

fn add_jdk_optional_abi_facts(facts: &mut FactSet) {
    for owner in ["Optional", "java.util.Optional"] {
        facts.facade_abi_facts.extend([
            rust_stdlib_method_abi(
                owner,
                "isPresent",
                0,
                "Option::is_some",
                Vec::new(),
                FacadeSlot::Owned,
            ),
            rust_stdlib_method_abi(
                owner,
                "get",
                0,
                "Option::as_ref",
                Vec::new(),
                FacadeSlot::Borrowed,
            ),
            rust_stdlib_method_abi(
                owner,
                "orElse",
                1,
                "Option::unwrap_or",
                vec![FacadeSlot::Owned],
                FacadeSlot::Owned,
            ),
        ]);
    }
}

fn add_jdk_collection_abi_facts(facts: &mut FactSet) {
    for owner in [
        "List",
        "java.util.List",
        "ArrayList",
        "java.util.ArrayList",
        "LinkedList",
        "java.util.LinkedList",
    ] {
        facts.facade_abi_facts.extend([
            rust_stdlib_method_abi(owner, "size", 0, "Vec::len", Vec::new(), FacadeSlot::Owned),
            rust_stdlib_method_abi(
                owner,
                "isEmpty",
                0,
                "Vec::is_empty",
                Vec::new(),
                FacadeSlot::Owned,
            ),
            rust_stdlib_method_abi(
                owner,
                "add",
                1,
                "Vec::push",
                vec![FacadeSlot::Owned],
                FacadeSlot::Owned,
            ),
            rust_stdlib_method_abi(
                owner,
                "addAll",
                1,
                "Vec::extend_from_slice",
                vec![FacadeSlot::Slice(Box::new(FacadeSlot::Borrowed))],
                FacadeSlot::Owned,
            ),
            rust_stdlib_method_abi(
                owner,
                "get",
                1,
                "slice::get",
                vec![FacadeSlot::Index],
                FacadeSlot::Borrowed,
            ),
            rust_stdlib_method_abi(
                owner,
                "toArray",
                0,
                "slice::to_vec",
                Vec::new(),
                FacadeSlot::Owned,
            ),
        ]);
    }
    for owner in ["Set", "java.util.Set", "HashSet", "java.util.HashSet"] {
        facts.facade_abi_facts.extend([
            rust_stdlib_method_abi(
                owner,
                "size",
                0,
                "HashSet::len",
                Vec::new(),
                FacadeSlot::Owned,
            ),
            rust_stdlib_method_abi(
                owner,
                "isEmpty",
                0,
                "HashSet::is_empty",
                Vec::new(),
                FacadeSlot::Owned,
            ),
            rust_stdlib_method_abi(
                owner,
                "add",
                1,
                "HashSet::insert",
                vec![FacadeSlot::Owned],
                FacadeSlot::Owned,
            ),
            rust_stdlib_method_abi(
                owner,
                "toArray",
                0,
                "HashSet::iter",
                Vec::new(),
                FacadeSlot::Owned,
            ),
        ]);
    }
    for owner in [
        "Map",
        "java.util.Map",
        "HashMap",
        "java.util.HashMap",
        "LinkedHashMap",
        "java.util.LinkedHashMap",
    ] {
        facts.facade_abi_facts.extend([
            rust_stdlib_method_abi(
                owner,
                "size",
                0,
                "HashMap::len",
                Vec::new(),
                FacadeSlot::Owned,
            ),
            rust_stdlib_method_abi(
                owner,
                "isEmpty",
                0,
                "HashMap::is_empty",
                Vec::new(),
                FacadeSlot::Owned,
            ),
            rust_stdlib_method_abi(
                owner,
                "put",
                2,
                "HashMap::insert",
                vec![FacadeSlot::Owned, FacadeSlot::Owned],
                FacadeSlot::Owned,
            ),
            rust_stdlib_method_abi(
                owner,
                "putAll",
                1,
                "HashMap::extend",
                vec![FacadeSlot::Owned],
                FacadeSlot::Owned,
            ),
            rust_stdlib_method_abi(
                owner,
                "get",
                1,
                "HashMap::get",
                vec![FacadeSlot::Borrowed],
                FacadeSlot::Borrowed,
            ),
            rust_stdlib_method_abi(
                owner,
                "containsKey",
                1,
                "HashMap::contains_key",
                vec![FacadeSlot::Borrowed],
                FacadeSlot::Owned,
            ),
            rust_stdlib_method_abi(
                owner,
                "keySet",
                0,
                "HashMap::keys",
                Vec::new(),
                FacadeSlot::Owned,
            ),
            rust_stdlib_method_abi(
                owner,
                "entrySet",
                0,
                "HashMap::iter",
                Vec::new(),
                FacadeSlot::Borrowed,
            ),
        ]);
    }
}

fn add_jdk_platform_io_abi_facts(facts: &mut FactSet) {
    for owner in ["Class", "JavaClass", "java.lang.Class"] {
        for method in ["getResourceAsStream", "get_resource_as_stream"] {
            facts.facade_abi_facts.push(jdk_option_method_abi(
                owner,
                method,
                1,
                "JavaClass::get_resource_as_stream",
                vec![FacadeSlot::Owned],
                FacadeSlot::Owned,
            ));
        }
    }
    for owner in ["ClassLoader", "java.lang.ClassLoader"] {
        for method in ["getResource", "get_resource"] {
            facts.facade_abi_facts.push(jdk_option_method_abi(
                owner,
                method,
                1,
                "ClassLoader::get_resource",
                vec![FacadeSlot::Owned],
                FacadeSlot::Owned,
            ));
        }
        for method in ["getResourceAsStream", "get_resource_as_stream"] {
            facts.facade_abi_facts.push(jdk_option_method_abi(
                owner,
                method,
                1,
                "ClassLoader::get_resource_as_stream",
                vec![FacadeSlot::Owned],
                FacadeSlot::Owned,
            ));
        }
    }
    for owner in ["Files", "java.nio.file.Files", "JavaNioFileFiles"] {
        facts.facade_abi_facts.push(rust_stdlib_static_abi(
            owner,
            "exists",
            1,
            "Path::exists",
            vec![FacadeSlot::Borrowed],
            FacadeSlot::Owned,
        ));
        for method in ["newInputStream", "new_input_stream"] {
            facts.facade_abi_facts.push(jdk_result_static_abi(
                owner,
                method,
                1,
                "JavaNioFileFiles::new_input_stream",
                vec![FacadeSlot::Borrowed],
                FacadeSlot::Owned,
            ));
        }
        for method in ["newOutputStream", "new_output_stream"] {
            facts.facade_abi_facts.push(jdk_result_static_abi(
                owner,
                method,
                1,
                "JavaNioFileFiles::new_output_stream",
                vec![FacadeSlot::Borrowed],
                FacadeSlot::Owned,
            ));
        }
        for method in ["newBufferedReader", "new_buffered_reader"] {
            facts.facade_abi_facts.push(jdk_result_static_abi(
                owner,
                method,
                1,
                "JavaNioFileFiles::new_buffered_reader",
                vec![FacadeSlot::Borrowed],
                FacadeSlot::Owned,
            ));
        }
        for method in ["readAllLines", "read_all_lines"] {
            facts.facade_abi_facts.push(jdk_result_static_abi(
                owner,
                method,
                1,
                "JavaNioFileFiles::read_all_lines",
                vec![FacadeSlot::Borrowed],
                FacadeSlot::Owned,
            ));
        }
    }
    for owner in ["File", "java.io.File"] {
        facts.facade_abi_facts.push(rust_stdlib_method_abi(
            owner,
            "exists",
            0,
            "Path::exists",
            Vec::new(),
            FacadeSlot::Owned,
        ));
        for method in ["list", "isFile", "is_file", "canRead", "can_read"] {
            let rust_path = match method {
                "list" => "File::list",
                "isFile" | "is_file" => "File::is_file",
                "canRead" | "can_read" => "File::can_read",
                _ => unreachable!("checked File facade method"),
            };
            facts.facade_abi_facts.push(rust_stdlib_method_abi(
                owner,
                method,
                0,
                rust_path,
                Vec::new(),
                FacadeSlot::Owned,
            ));
        }
        for method in ["deleteOnExit", "delete_on_exit"] {
            facts.facade_abi_facts.push(rust_stdlib_method_abi(
                owner,
                method,
                0,
                "File::delete_on_exit",
                Vec::new(),
                FacadeSlot::Owned,
            ));
        }
    }
    for owner in ["URI", "java.net.URI"] {
        for method in ["getRawPath", "get_raw_path"] {
            facts.facade_abi_facts.push(rust_stdlib_method_abi(
                owner,
                method,
                0,
                "URI::get_raw_path",
                Vec::new(),
                FacadeSlot::Owned,
            ));
        }
    }
    for owner in ["URL", "java.net.URL"] {
        facts.facade_abi_facts.push(rust_stdlib_method_abi(
            owner,
            "getFile",
            0,
            "URL::get_file",
            Vec::new(),
            FacadeSlot::Owned,
        ));
        facts.facade_abi_facts.push(rust_stdlib_method_abi(
            owner,
            "get_file",
            0,
            "URL::get_file",
            Vec::new(),
            FacadeSlot::Owned,
        ));
        facts.facade_abi_facts.push(jdk_result_method_abi(
            owner,
            "openConnection",
            0,
            "URL::open_connection",
            Vec::new(),
            FacadeSlot::Owned,
        ));
        facts.facade_abi_facts.push(jdk_result_method_abi(
            owner,
            "open_connection",
            0,
            "URL::open_connection",
            Vec::new(),
            FacadeSlot::Owned,
        ));
    }
    for owner in ["JarURLConnection", "java.net.JarURLConnection"] {
        facts.facade_abi_facts.push(jdk_result_method_abi(
            owner,
            "getJarFile",
            0,
            "JarURLConnection::get_jar_file",
            Vec::new(),
            FacadeSlot::Owned,
        ));
        facts.facade_abi_facts.push(jdk_result_method_abi(
            owner,
            "get_jar_file",
            0,
            "JarURLConnection::get_jar_file",
            Vec::new(),
            FacadeSlot::Owned,
        ));
    }
    for owner in ["JarFile", "java.util.jar.JarFile"] {
        facts.facade_abi_facts.push(jdk_result_method_abi(
            owner,
            "entries",
            0,
            "JarFile::entries",
            Vec::new(),
            FacadeSlot::Owned,
        ));
        facts.facade_abi_facts.push(jdk_result_method_abi(
            owner,
            "getManifest",
            0,
            "JarFile::get_manifest",
            Vec::new(),
            FacadeSlot::Owned,
        ));
        facts.facade_abi_facts.push(jdk_result_method_abi(
            owner,
            "get_manifest",
            0,
            "JarFile::get_manifest",
            Vec::new(),
            FacadeSlot::Owned,
        ));
    }
    for owner in ["Manifest", "java.util.jar.Manifest"] {
        facts.facade_abi_facts.push(rust_stdlib_method_abi(
            owner,
            "getMainAttributes",
            0,
            "Manifest::get_main_attributes",
            Vec::new(),
            FacadeSlot::Owned,
        ));
        facts.facade_abi_facts.push(rust_stdlib_method_abi(
            owner,
            "get_main_attributes",
            0,
            "Manifest::get_main_attributes",
            Vec::new(),
            FacadeSlot::Owned,
        ));
    }
    for owner in ["Attributes", "java.util.jar.Attributes"] {
        facts.facade_abi_facts.push(rust_stdlib_method_abi(
            owner,
            "getValue",
            1,
            "Attributes::get_value",
            vec![FacadeSlot::Owned],
            FacadeSlot::Owned,
        ));
        facts.facade_abi_facts.push(rust_stdlib_method_abi(
            owner,
            "get_value",
            1,
            "Attributes::get_value",
            vec![FacadeSlot::Owned],
            FacadeSlot::Owned,
        ));
    }
    for owner in [
        "InputStream",
        "java.io.InputStream",
        "GZIPInputStream",
        "java.util.zip.GZIPInputStream",
    ] {
        facts.facade_abi_facts.push(jdk_result_method_abi(
            owner,
            "read",
            0,
            "InputStream::read_0",
            Vec::new(),
            FacadeSlot::Owned,
        ));
        facts.facade_abi_facts.push(jdk_result_method_abi(
            owner,
            "read",
            1,
            "InputStream::read",
            vec![FacadeSlot::Slice(Box::new(FacadeSlot::MutBorrowed))],
            FacadeSlot::Owned,
        ));
        facts.facade_abi_facts.push(jdk_result_method_abi(
            owner,
            "read",
            3,
            "InputStream::read_3",
            vec![
                FacadeSlot::Slice(Box::new(FacadeSlot::MutBorrowed)),
                FacadeSlot::Owned,
                FacadeSlot::Owned,
            ],
            FacadeSlot::Owned,
        ));
        facts.facade_abi_facts.push(jdk_result_method_abi(
            owner,
            "read_3",
            3,
            "InputStream::read_3",
            vec![
                FacadeSlot::Slice(Box::new(FacadeSlot::MutBorrowed)),
                FacadeSlot::Owned,
                FacadeSlot::Owned,
            ],
            FacadeSlot::Owned,
        ));
    }
    for owner in [
        "StringBuffer",
        "StringBuilder",
        "java.lang.StringBuffer",
        "java.lang.StringBuilder",
    ] {
        facts.facade_abi_facts.push(rust_stdlib_method_abi(
            owner,
            "length",
            0,
            "StringBuffer::length",
            Vec::new(),
            FacadeSlot::Owned,
        ));
    }
    for owner in ["Random", "java.util.Random"] {
        facts.facade_abi_facts.push(rust_stdlib_method_abi(
            owner,
            "nextInt",
            1,
            "Random::next_int",
            vec![FacadeSlot::Owned],
            FacadeSlot::Owned,
        ));
        facts.facade_abi_facts.push(rust_stdlib_method_abi(
            owner,
            "next_int",
            1,
            "Random::next_int",
            vec![FacadeSlot::Owned],
            FacadeSlot::Owned,
        ));
    }
    for (owner, rust_owner) in [
        ("ByteBuffer", "ByteBuffer"),
        ("java.nio.ByteBuffer", "ByteBuffer"),
        ("CharBuffer", "CharBuffer"),
        ("java.nio.CharBuffer", "CharBuffer"),
    ] {
        facts.facade_abi_facts.push(rust_stdlib_method_abi(
            owner,
            "hasRemaining",
            0,
            &format!("{rust_owner}::has_remaining"),
            Vec::new(),
            FacadeSlot::Owned,
        ));
        facts.facade_abi_facts.push(rust_stdlib_method_abi(
            owner,
            "has_remaining",
            0,
            &format!("{rust_owner}::has_remaining"),
            Vec::new(),
            FacadeSlot::Owned,
        ));
        facts.facade_abi_facts.push(rust_stdlib_method_abi(
            owner,
            "remaining",
            0,
            &format!("{rust_owner}::remaining"),
            Vec::new(),
            FacadeSlot::Owned,
        ));
    }
    for owner in [
        "OutputStream",
        "java.io.OutputStream",
        "JavaIoOutputStream",
        "FileOutputStream",
        "java.io.FileOutputStream",
        "JavaIoFileOutputStream",
        "ByteArrayOutputStream",
        "java.io.ByteArrayOutputStream",
        "JavaIoByteArrayOutputStream",
        "BufferedOutputStream",
        "java.io.BufferedOutputStream",
        "JavaIoBufferedOutputStream",
        "DataOutputStream",
        "java.io.DataOutputStream",
        "JavaIoDataOutputStream",
        "GZIPOutputStream",
        "java.util.zip.GZIPOutputStream",
        "JavaUtilZipGZIPOutputStream",
        "ZipOutputStream",
        "java.util.zip.ZipOutputStream",
        "JavaUtilZipZipOutputStream",
        "ObjectOutputStream",
        "java.io.ObjectOutputStream",
        "JavaIoObjectOutputStream",
        "FilterOutputStream",
        "java.io.FilterOutputStream",
        "JavaIoFilterOutputStream",
    ] {
        facts
            .facade_abi_facts
            .push(jdk_result_method_abi_with_params(
                owner,
                "write",
                "OutputStream::write_1",
                vec![TypePattern::Exact(TypeKey {
                    type_name: "byte[]".to_string(),
                })],
                vec![FacadeSlot::Slice(Box::new(FacadeSlot::Borrowed))],
                FacadeSlot::Owned,
            ));
        facts
            .facade_abi_facts
            .push(jdk_result_method_abi_with_params(
                owner,
                "write",
                "OutputStream::write_int",
                vec![TypePattern::Exact(TypeKey {
                    type_name: "int".to_string(),
                })],
                vec![FacadeSlot::Owned],
                FacadeSlot::Owned,
            ));
        for primitive in ["byte", "short", "char"] {
            facts
                .facade_abi_facts
                .push(jdk_result_method_abi_with_params(
                    owner,
                    "write",
                    "OutputStream::write_int",
                    vec![TypePattern::Exact(TypeKey {
                        type_name: primitive.to_string(),
                    })],
                    vec![FacadeSlot::Owned],
                    FacadeSlot::Owned,
                ));
        }
        facts.facade_abi_facts.push(jdk_result_method_abi(
            owner,
            "write",
            3,
            "OutputStream::write_3",
            vec![
                FacadeSlot::Slice(Box::new(FacadeSlot::Borrowed)),
                FacadeSlot::Owned,
                FacadeSlot::Owned,
            ],
            FacadeSlot::Owned,
        ));
        facts.facade_abi_facts.push(jdk_result_method_abi(
            owner,
            "write_3",
            3,
            "OutputStream::write_3",
            vec![
                FacadeSlot::Slice(Box::new(FacadeSlot::Borrowed)),
                FacadeSlot::Owned,
                FacadeSlot::Owned,
            ],
            FacadeSlot::Owned,
        ));
    }
    for owner in ["PushbackInputStream", "java.io.PushbackInputStream"] {
        facts.facade_abi_facts.push(rust_stdlib_method_abi(
            owner,
            "unread",
            3,
            "PushbackInputStream::unread",
            vec![FacadeSlot::Borrowed, FacadeSlot::Owned, FacadeSlot::Owned],
            FacadeSlot::Owned,
        ));
    }
}

fn jdk_result_method_abi(
    owner: &str,
    method: &str,
    arity: usize,
    rust_path: &str,
    args: Vec<FacadeSlot>,
    return_slot: FacadeSlot,
) -> FacadeAbiFact {
    let mut fact = rust_stdlib_method_abi(owner, method, arity, rust_path, args, return_slot);
    fact.result = ResultAbi::ResultWrapped;
    fact
}

fn jdk_result_method_abi_with_params(
    owner: &str,
    method: &str,
    rust_path: &str,
    params: Vec<TypePattern>,
    args: Vec<FacadeSlot>,
    return_slot: FacadeSlot,
) -> FacadeAbiFact {
    let mut fact = rust_stdlib_method_abi(owner, method, args.len(), rust_path, args, return_slot);
    fact.key = CallableKey::method_with_params(
        owner.to_string(),
        method.to_string(),
        Staticness::Unknown,
        params,
    );
    fact.result = ResultAbi::ResultWrapped;
    fact
}

fn jdk_option_method_abi(
    owner: &str,
    method: &str,
    arity: usize,
    rust_path: &str,
    args: Vec<FacadeSlot>,
    return_slot: FacadeSlot,
) -> FacadeAbiFact {
    let mut fact = rust_stdlib_method_abi(owner, method, arity, rust_path, args, return_slot);
    fact.result = ResultAbi::OptionWrapped;
    fact
}

fn jdk_result_static_abi(
    owner: &str,
    method: &str,
    arity: usize,
    rust_path: &str,
    args: Vec<FacadeSlot>,
    return_slot: FacadeSlot,
) -> FacadeAbiFact {
    let mut fact = rust_stdlib_static_abi(owner, method, arity, rust_path, args, return_slot);
    fact.result = ResultAbi::ResultWrapped;
    fact
}

fn add_jdk_platform_stream_facts(facts: &mut FactSet) {
    for owner in ["java.util.zip.GZIPInputStream", "GZIPInputStream"] {
        facts.external_field_facts.push(jdk_static_final_field_fact(
            owner,
            "GZIP_MAGIC",
            JavaType::Primitive(PrimitiveType::Int),
        ));
    }
    for owner in [
        "java.io.FilterInputStream",
        "java.util.zip.InflaterInputStream",
        "java.util.zip.GZIPInputStream",
        "GZIPInputStream",
    ] {
        facts.external_field_facts.push(jdk_instance_field_fact(
            owner,
            "in",
            JavaType::Named {
                name: "InputStream".to_string(),
                args: Vec::new(),
            },
        ));
    }
    for owner in [
        "java.util.zip.InflaterInputStream",
        "java.util.zip.GZIPInputStream",
        "GZIPInputStream",
    ] {
        facts.external_field_facts.push(jdk_instance_field_fact(
            owner,
            "inf",
            JavaType::Named {
                name: "Inflater".to_string(),
                args: Vec::new(),
            },
        ));
        facts.external_field_facts.push(jdk_instance_field_fact(
            owner,
            "buf",
            JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Byte))),
        ));
        facts.external_field_facts.push(jdk_instance_field_fact(
            owner,
            "len",
            JavaType::Primitive(PrimitiveType::Int),
        ));
    }
}

fn add_jdk_numeric_facade_facts(facts: &mut FactSet) {
    for field_name in [
        "ROUND_UP",
        "ROUND_DOWN",
        "ROUND_CEILING",
        "ROUND_FLOOR",
        "ROUND_HALF_UP",
        "ROUND_HALF_DOWN",
        "ROUND_HALF_EVEN",
        "ROUND_UNNECESSARY",
    ] {
        facts.external_field_facts.push(jdk_static_final_field_fact(
            "BigDecimal",
            field_name,
            JavaType::Primitive(PrimitiveType::Int),
        ));
    }
    for field_name in ["ZERO", "ONE"] {
        facts.external_field_facts.push(jdk_static_final_field_fact(
            "BigInteger",
            field_name,
            JavaType::Named {
                name: "BigInteger".to_string(),
                args: Vec::new(),
            },
        ));
    }
    facts.external_api_effect_facts.extend([
        jdk_instance_api_fact(
            "StrictMath",
            "IEEEremainder",
            2,
            JavaType::Primitive(PrimitiveType::Double),
        ),
        jdk_instance_api_fact(
            "BigDecimal",
            "setScale",
            2,
            JavaType::Named {
                name: "BigDecimal".to_string(),
                args: Vec::new(),
            },
        ),
        jdk_instance_api_fact(
            "BigDecimal",
            "doubleValue",
            0,
            JavaType::Primitive(PrimitiveType::Double),
        ),
        jdk_instance_api_fact(
            "BigDecimal",
            "floatValue",
            0,
            JavaType::Primitive(PrimitiveType::Float),
        ),
        jdk_instance_api_fact(
            "BigInteger",
            "compareTo",
            1,
            JavaType::Primitive(PrimitiveType::Int),
        ),
        jdk_instance_api_fact(
            "BigInteger",
            "multiply",
            1,
            JavaType::Named {
                name: "BigInteger".to_string(),
                args: Vec::new(),
            },
        ),
        jdk_instance_api_fact(
            "BigInteger",
            "pow",
            1,
            JavaType::Named {
                name: "BigInteger".to_string(),
                args: Vec::new(),
            },
        ),
        jdk_instance_api_fact(
            "BigInteger",
            "shiftRight",
            1,
            JavaType::Named {
                name: "BigInteger".to_string(),
                args: Vec::new(),
            },
        ),
        jdk_instance_api_fact(
            "BigInteger",
            "testBit",
            1,
            JavaType::Primitive(PrimitiveType::Boolean),
        ),
    ]);
}

fn jdk_static_final_field_fact(
    owner: &str,
    field_name: &str,
    field_type: JavaType,
) -> ExternalFieldFact {
    ExternalFieldFact {
        field: FieldKey {
            type_name: owner.to_string(),
            field_name: field_name.to_string(),
        },
        field_type: Some(field_type),
        field_kind: ExternalFieldKind::StaticFinal,
        provenance: FactProvenance::BuiltinExternalModel,
    }
}

fn jdk_instance_field_fact(
    owner: &str,
    field_name: &str,
    field_type: JavaType,
) -> ExternalFieldFact {
    ExternalFieldFact {
        field: FieldKey {
            type_name: owner.to_string(),
            field_name: field_name.to_string(),
        },
        field_type: Some(field_type),
        field_kind: ExternalFieldKind::Unknown,
        provenance: FactProvenance::BuiltinExternalModel,
    }
}

fn jdk_instance_api_fact(
    owner: &str,
    method_name: &str,
    arg_count: usize,
    return_type: JavaType,
) -> crate::facts::ExternalApiEffectFact {
    crate::facts::ExternalApiEffectFact {
        method_name: method_name.to_string(),
        receiver_type: Some(owner.to_string()),
        arg_count: Some(arg_count),
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(return_type),
        return_nullability: NullabilityState::DefinitelyNonNull,
        thrown: Vec::new(),
        provenance: FactProvenance::BuiltinExternalModel,
    }
}

fn rust_stdlib_method_abi(
    owner: &str,
    method: &str,
    arity: usize,
    rust_path: &str,
    args: Vec<FacadeSlot>,
    return_slot: FacadeSlot,
) -> FacadeAbiFact {
    let borrow_source =
        matches!(return_slot, FacadeSlot::Borrowed).then_some(crate::facts::BorrowSource::Receiver);
    FacadeAbiFact {
        key: CallableKey::method(
            owner.to_string(),
            method.to_string(),
            Staticness::Unknown,
            arity,
        ),
        rust_path: rust_path.to_string(),
        receiver: ReceiverAbi::BorrowedSelf,
        args,
        return_slot,
        result: ResultAbi::Plain,
        borrow_source,
        borrowed_return_recovery: None,
        provenance: FactProvenance::BuiltinExternalModel,
    }
}

fn rust_stdlib_static_abi(
    owner: &str,
    method: &str,
    arity: usize,
    rust_path: &str,
    args: Vec<FacadeSlot>,
    return_slot: FacadeSlot,
) -> FacadeAbiFact {
    FacadeAbiFact {
        key: CallableKey::method(
            owner.to_string(),
            method.to_string(),
            Staticness::Unknown,
            arity,
        ),
        rust_path: rust_path.to_string(),
        receiver: ReceiverAbi::AssociatedFunction,
        args,
        return_slot,
        result: ResultAbi::Plain,
        borrow_source: None,
        borrowed_return_recovery: None,
        provenance: FactProvenance::BuiltinExternalModel,
    }
}

pub fn native_collection_receiver_kind_for_type(ty: &JavaType) -> Option<JavaCollectionKind> {
    let kind = java_collection_kind_for_type(ty)?;
    match ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            let simple = canonical_external_receiver_type_key(name);
            let simple = simple.rsplit('.').next().unwrap_or(simple.as_str());
            if matches!(simple, "IntervalList") {
                None
            } else {
                Some(kind)
            }
        }
        _ => None,
    }
}

pub fn native_collection_receiver_mutates(ty: &JavaType, method_name: &str) -> bool {
    let Some(kind) = native_collection_receiver_kind_for_type(ty) else {
        return false;
    };
    match kind {
        JavaCollectionKind::List | JavaCollectionKind::Set => matches!(
            method_name,
            "add"
                | "addAll"
                | "add_all"
                | "remove"
                | "removeAll"
                | "remove_all"
                | "removeIf"
                | "remove_if"
                | "clear"
                | "insert"
                | "extend"
                | "push"
                | "retain"
        ),
        JavaCollectionKind::Map => matches!(
            method_name,
            "put"
                | "putAll"
                | "put_all"
                | "remove"
                | "removeAll"
                | "remove_all"
                | "clear"
                | "insert"
                | "extend"
        ),
    }
}

pub fn java_map_type_args(ty: &JavaType) -> Option<(&JavaType, &JavaType)> {
    let JavaType::Named { name, args } = ty else {
        return None;
    };
    let name = canonical_external_receiver_type_key(name);
    let name = name.rsplit('.').next().unwrap_or(&name);
    if !is_known_map_type(name) {
        return None;
    }
    match args.as_slice() {
        [key, value] => Some((key, value)),
        _ => None,
    }
}

fn is_known_input_stream_type(name: &str) -> bool {
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
}

fn is_known_output_stream_type(name: &str) -> bool {
    matches!(
        name,
        "OutputStream"
            | "FileOutputStream"
            | "ByteArrayOutputStream"
            | "BufferedOutputStream"
            | "DataOutputStream"
            | "GZIPOutputStream"
            | "ZipOutputStream"
            | "ObjectOutputStream"
            | "FilterOutputStream"
            | "PrintStream"
    )
}

fn is_known_reader_type(name: &str) -> bool {
    matches!(
        name,
        "Reader"
            | "LineNumberReader"
            | "FileReader"
            | "BufferedReader"
            | "StringReader"
            | "InputStreamReader"
            | "CharArrayReader"
            | "FilterReader"
            | "PushbackReader"
            | "PipedReader"
    )
}

fn is_known_writer_type(name: &str) -> bool {
    matches!(
        name,
        "Writer"
            | "FileWriter"
            | "BufferedWriter"
            | "StringWriter"
            | "OutputStreamWriter"
            | "CharArrayWriter"
            | "FilterWriter"
            | "PrintWriter"
            | "PipedWriter"
    )
}

fn is_known_write_option_type(name: &str) -> bool {
    matches!(
        name.rsplit('.').next().unwrap_or(name),
        "WriteOption" | "BaiWriteOption" | "SbiWriteOption"
    )
}

fn is_known_reference_type(name: &str) -> bool {
    !matches!(
        name,
        "boolean" | "byte" | "short" | "char" | "int" | "long" | "float" | "double" | "void"
    )
}

fn is_known_throwable_type(name: &str) -> bool {
    name.rsplit('.').next().unwrap_or(name) == "Throwable"
        || is_known_exception_type(name)
        || is_known_error_type(name)
}

fn is_known_exception_type(name: &str) -> bool {
    let simple = name.rsplit('.').next().unwrap_or(name);
    simple == "Exception" || simple.ends_with("Exception")
}

fn is_known_error_type(name: &str) -> bool {
    let simple = name.rsplit('.').next().unwrap_or(name);
    if simple == "AssertionError" {
        return true;
    }
    matches!(simple, "Error" | "OutOfMemoryError" | "StackOverflowError")
        || simple.ends_with("Error")
}

fn is_optional_like_receiver(name: &str) -> bool {
    matches!(
        name.rsplit('.').next().unwrap_or(name),
        "Optional" | "OptionalInt" | "OptionalLong" | "OptionalDouble"
    )
}

fn external_builder_product_type(receiver: &str) -> Option<String> {
    let simple = receiver.rsplit('.').next().unwrap_or(receiver);
    let product = simple.strip_suffix("Builder")?;
    (!product.is_empty()).then(|| product.to_string())
}

fn is_external_builder_type(receiver: &str) -> bool {
    external_builder_product_type(receiver).is_some()
}

fn is_external_builder_fluent_method(receiver: &str, name: &str, arg_count: usize) -> bool {
    is_external_builder_type(receiver)
        && arg_count > 0
        && !is_known_output_side_effect_method(name)
        && !matches!(
            name,
            "make" | "toString" | "equals" | "hashCode" | "getClass"
        )
        && !name.starts_with("get")
        && !is_java_bean_boolean_getter(name, arg_count)
}

pub fn known_external_method_return_type(
    receiver_type: Option<&str>,
    name: &str,
    arg_count: usize,
) -> Option<JavaType> {
    if let Some(return_type) = crate::external_packs::external_api_effect_for_receiver_with_arity(
        receiver_type,
        name,
        Some(arg_count),
    )
    .and_then(|effect| effect.return_type)
    {
        return Some(return_type);
    }
    if let Some(return_type) =
        crate::javalibs::broad_utils_dynamic_return_type(receiver_type, name, &[])
    {
        return Some(return_type);
    }
    if let Some(return_type) =
        crate::javalibs::broad_static_utility_return_type(receiver_type, name, arg_count)
    {
        return Some(return_type);
    }
    if arg_count == 1
        && name == "submit"
        && receiver_type
            .map(canonical_external_receiver_type_key)
            .is_some_and(|receiver| {
                receiver.rsplit('.').next().unwrap_or(&receiver) == "ExecutorService"
            })
    {
        return Some(JavaType::Named {
            name: "Future".to_string(),
            args: Vec::new(),
        });
    }
    if arg_count == 0 {
        if is_string_domain_getter(name) {
            return Some(JavaType::String);
        }
        if is_zero_arg_string_named_getter(name) {
            return Some(JavaType::String);
        }
        if is_integral_domain_getter(name) {
            return Some(JavaType::Primitive(PrimitiveType::Int));
        }
        if is_long_domain_getter(name) {
            return Some(JavaType::Primitive(PrimitiveType::Long));
        }
        if is_byte_array_domain_getter(name) {
            return Some(JavaType::Array(Box::new(JavaType::Primitive(
                PrimitiveType::Byte,
            ))));
        }
        if is_int_array_domain_getter(name) {
            return Some(JavaType::Array(Box::new(JavaType::Primitive(
                PrimitiveType::Int,
            ))));
        }
        if is_allele_domain_getter(name) {
            return Some(JavaType::Named {
                name: "Allele".to_string(),
                args: Vec::new(),
            });
        }
        if is_allele_list_domain_getter(name) {
            return Some(JavaType::Named {
                name: "List".to_string(),
                args: vec![JavaType::Named {
                    name: "Allele".to_string(),
                    args: Vec::new(),
                }],
            });
        }
        if is_string_list_domain_getter(name) {
            return Some(JavaType::Named {
                name: "List".to_string(),
                args: vec![JavaType::String],
            });
        }
        if is_object_list_domain_getter(name) {
            return Some(JavaType::Named {
                name: "List".to_string(),
                args: vec![JavaType::Named {
                    name: "Object".to_string(),
                    args: Vec::new(),
                }],
            });
        }
        if is_sequence_dictionary_getter(name) {
            return Some(JavaType::Named {
                name: "SAMSequenceDictionary".to_string(),
                args: Vec::new(),
            });
        }
        if is_path_getter(name) {
            return Some(JavaType::Named {
                name: "Path".to_string(),
                args: Vec::new(),
            });
        }
    }
    if is_boolean_external_method(name, arg_count) {
        return Some(JavaType::Primitive(PrimitiveType::Boolean));
    }
    match (receiver_type, name, arg_count) {
        (_, "toString", 0) => Some(JavaType::String),
        (_, "hashCode", 0) => Some(JavaType::Primitive(PrimitiveType::Int)),
        (_, "equals", 1) => Some(JavaType::Primitive(PrimitiveType::Boolean)),
        (Some(owner), "length", 0)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "File") =>
        {
            Some(JavaType::Primitive(PrimitiveType::Long))
        }
        (_, "length", 0) => Some(JavaType::Primitive(PrimitiveType::Int)),
        (_, "compareTo", 0 | 1) => Some(JavaType::Primitive(PrimitiveType::Int)),
        (_, "ordinal", 0) => Some(JavaType::Primitive(PrimitiveType::Int)),
        (Some(owner), "setScale" | "set_scale", 2)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "BigDecimal") =>
        {
            Some(JavaType::Named {
                name: canonical_external_receiver_type_key(owner),
                args: Vec::new(),
            })
        }
        (Some(owner), "doubleValue" | "double_value", 0)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "BigDecimal") =>
        {
            Some(JavaType::Primitive(PrimitiveType::Double))
        }
        (Some(owner), "floatValue" | "float_value", 0)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "BigDecimal") =>
        {
            Some(JavaType::Primitive(PrimitiveType::Float))
        }
        (_, "logBeta", 2) => Some(JavaType::Primitive(PrimitiveType::Double)),
        (
            Some(owner),
            "abs" | "ceil" | "floor" | "log" | "log10" | "exp" | "sqrt" | "random",
            0 | 1,
        ) if canonical_external_receiver_type_key(owner)
            .rsplit('.')
            .next()
            .is_some_and(|simple| simple == "Math") =>
        {
            Some(JavaType::Primitive(PrimitiveType::Double))
        }
        (Some(owner), "pow", 2)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "Math") =>
        {
            Some(JavaType::Primitive(PrimitiveType::Double))
        }
        (Some(owner), "round", 1)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "Math") =>
        {
            Some(JavaType::Primitive(PrimitiveType::Long))
        }
        (Some(owner), "isNaN" | "is_na_n", 1)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "Double") =>
        {
            Some(JavaType::Primitive(PrimitiveType::Boolean))
        }
        (Some(owner), "isInfinite" | "is_infinite", 1)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| matches!(simple, "Double" | "Float")) =>
        {
            Some(JavaType::Primitive(PrimitiveType::Boolean))
        }
        (Some(owner), "isNaN" | "is_na_n", 1)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "Float") =>
        {
            Some(JavaType::Primitive(PrimitiveType::Boolean))
        }
        (Some(owner), "parseFloat" | "parse_float", 1)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "Float") =>
        {
            Some(JavaType::Primitive(PrimitiveType::Float))
        }
        (Some(owner), "doubleToLongBits" | "double_to_long_bits", 1)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "Double") =>
        {
            Some(JavaType::Primitive(PrimitiveType::Long))
        }
        (Some(owner), "doubleToRawLongBits" | "double_to_raw_long_bits", 1)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "Double") =>
        {
            Some(JavaType::Primitive(PrimitiveType::Long))
        }
        (Some(owner), "longBitsToDouble" | "long_bits_to_double", 1)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "Double") =>
        {
            Some(JavaType::Primitive(PrimitiveType::Double))
        }
        (Some(owner), "floatToIntBits" | "float_to_int_bits", 1)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "Float") =>
        {
            Some(JavaType::Primitive(PrimitiveType::Int))
        }
        (Some(owner), "floatToRawIntBits" | "float_to_raw_int_bits", 1)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "Float") =>
        {
            Some(JavaType::Primitive(PrimitiveType::Int))
        }
        (Some(owner), "intBitsToFloat" | "int_bits_to_float", 1)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "Float") =>
        {
            Some(JavaType::Primitive(PrimitiveType::Float))
        }
        (Some(owner), "compare", 2)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| matches!(simple, "Double" | "Float" | "Integer")) =>
        {
            Some(JavaType::Primitive(PrimitiveType::Int))
        }
        (Some(owner), "toString" | "to_string", 1)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| matches!(simple, "Double" | "Float" | "Integer")) =>
        {
            Some(JavaType::String)
        }
        (Some(owner), "valueOf" | "value_of", 1)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "String") =>
        {
            Some(JavaType::String)
        }
        (Some(owner), "valueOf" | "value_of", 1)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "Integer") =>
        {
            Some(JavaType::Primitive(PrimitiveType::Int))
        }
        (Some(owner), "toHexString" | "to_hex_string", 1)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "Integer") =>
        {
            Some(JavaType::String)
        }
        (Some(owner), "exists", 0)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "File") =>
        {
            Some(JavaType::Primitive(PrimitiveType::Boolean))
        }
        (Some(owner), "isFile" | "is_file" | "canRead" | "can_read", 0)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "File") =>
        {
            Some(JavaType::Primitive(PrimitiveType::Boolean))
        }
        (Some(owner), "list", 0)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "File") =>
        {
            Some(JavaType::Array(Box::new(JavaType::String)))
        }
        (Some(owner), "get", 1)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "BitSet") =>
        {
            Some(JavaType::Primitive(PrimitiveType::Boolean))
        }
        (Some(owner), "ready", 0)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| {
                    matches!(simple, "Reader" | "BufferedReader" | "InputStreamReader")
                }) =>
        {
            Some(JavaType::Primitive(PrimitiveType::Boolean))
        }
        (Some(owner), "getProperty" | "get_property", 1 | 2)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "System") =>
        {
            Some(JavaType::String)
        }
        (Some(owner), "lineSeparator" | "line_separator", 0)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "System") =>
        {
            Some(JavaType::String)
        }
        (Some(owner), "currentTimeMillis" | "current_time_millis", 0)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "System") =>
        {
            Some(JavaType::Primitive(PrimitiveType::Long))
        }
        (Some(owner), "identityHashCode" | "identity_hash_code", 1)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "System") =>
        {
            Some(JavaType::Primitive(PrimitiveType::Int))
        }
        (Some(owner), "exit", 1)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "System") =>
        {
            Some(JavaType::Void)
        }
        (Some(owner), "gc", 0)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "System") =>
        {
            Some(JavaType::Void)
        }
        (Some(owner), "sleep", 1)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "Thread") =>
        {
            Some(JavaType::Void)
        }
        (Some(owner), "readInt" | "read_int", 0)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "ObjectInputStream") =>
        {
            Some(JavaType::Primitive(PrimitiveType::Int))
        }
        (Some(owner), "readObject" | "read_object", 0)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "ObjectInputStream") =>
        {
            Some(JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            })
        }
        (Some(owner), "writeInt" | "write_int" | "writeObject" | "write_object", 1)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "ObjectOutputStream") =>
        {
            Some(JavaType::Void)
        }
        (_, name, arg_count) if number_value_method_return_type(name, arg_count).is_some() => {
            number_value_method_return_type(name, arg_count).map(JavaType::Primitive)
        }
        (Some(owner), name, arg_count)
            if boxed_number_value_method_return_type(owner, name, arg_count).is_some() =>
        {
            boxed_number_value_method_return_type(owner, name, arg_count).map(JavaType::Primitive)
        }
        (Some(owner), "decode", _) => known_external_decoder_return_type(owner),
        (_, "getClass", 0) => Some(JavaType::Named {
            name: "Class".to_string(),
            args: Vec::new(),
        }),
        (Some("Class" | "JavaClass"), "getClassLoader" | "get_class_loader", 0) => {
            Some(JavaType::Named {
                name: "ClassLoader".to_string(),
                args: Vec::new(),
            })
        }
        (Some("Class" | "JavaClass"), "isArray" | "is_array", 0) => {
            Some(JavaType::Primitive(PrimitiveType::Boolean))
        }
        (Some("Class" | "JavaClass"), "getResourceAsStream" | "get_resource_as_stream", 1) => {
            Some(JavaType::Named {
                name: "InputStream".to_string(),
                args: Vec::new(),
            })
        }
        (Some("ClassLoader"), "getResource" | "get_resource", 1) => Some(JavaType::Named {
            name: "URL".to_string(),
            args: Vec::new(),
        }),
        (Some("ClassLoader"), "getResourceAsStream" | "get_resource_as_stream", 1) => {
            Some(JavaType::Named {
                name: "InputStream".to_string(),
                args: Vec::new(),
            })
        }
        (Some("URL"), "getFile" | "get_file", 0) => Some(JavaType::String),
        (Some("URL"), "openConnection" | "open_connection", 0) => Some(JavaType::Named {
            name: "URLConnection".to_string(),
            args: Vec::new(),
        }),
        (Some("JarURLConnection"), "getJarFile" | "get_jar_file", 0) => Some(JavaType::Named {
            name: "JarFile".to_string(),
            args: Vec::new(),
        }),
        (Some("JarFile"), "entries", 0) => Some(JavaType::Named {
            name: "Enumeration".to_string(),
            args: Vec::new(),
        }),
        (Some("JarFile"), "getManifest" | "get_manifest", 0) => Some(JavaType::Named {
            name: "Manifest".to_string(),
            args: Vec::new(),
        }),
        (Some("Manifest"), "getMainAttributes" | "get_main_attributes", 0) => {
            Some(JavaType::Named {
                name: "Attributes".to_string(),
                args: Vec::new(),
            })
        }
        (Some("Attributes"), "getValue" | "get_value", 1) => Some(JavaType::String),
        (Some("BufferedImage"), "createGraphics" | "create_graphics", 0) => Some(JavaType::Named {
            name: "Graphics2D".to_string(),
            args: Vec::new(),
        }),
        (Some("BufferedImage"), "getGraphics" | "get_graphics", 0) => Some(JavaType::Named {
            name: "Graphics".to_string(),
            args: Vec::new(),
        }),
        (Some("Component"), "getGraphics" | "get_graphics", 0) => Some(JavaType::Named {
            name: "Graphics".to_string(),
            args: Vec::new(),
        }),
        (Some("Component"), "getWidth" | "get_width" | "getHeight" | "get_height", 0) => {
            Some(JavaType::Primitive(PrimitiveType::Int))
        }
        (
            Some("Color"),
            "getRed" | "get_red" | "getGreen" | "get_green" | "getBlue" | "get_blue",
            0,
        ) => Some(JavaType::Primitive(PrimitiveType::Int)),
        (Some("Font"), "getSize" | "get_size", 0) => Some(JavaType::Primitive(PrimitiveType::Int)),
        (Some("Font"), "isBold" | "is_bold", 0) => {
            Some(JavaType::Primitive(PrimitiveType::Boolean))
        }
        (Some("Graphics"), "dispose", 0) => Some(JavaType::Void),
        (Some("Graphics"), "getClip" | "get_clip", 0) => Some(JavaType::Named {
            name: "Shape".to_string(),
            args: Vec::new(),
        }),
        (Some("Graphics"), "getClipBounds" | "get_clip_bounds", 0) => Some(JavaType::Named {
            name: "Rectangle".to_string(),
            args: Vec::new(),
        }),
        (Some("Graphics"), "getFontMetrics" | "get_font_metrics", 1) => Some(JavaType::Named {
            name: "FontMetrics".to_string(),
            args: Vec::new(),
        }),
        (Some("Graphics"), "drawImage" | "draw_image", 4) => {
            Some(JavaType::Primitive(PrimitiveType::Boolean))
        }
        (Some("JPanel" | "Component"), "print", 1) => Some(JavaType::Void),
        (Some("TableModel"), "getRowCount" | "get_row_count", 0) => {
            Some(JavaType::Primitive(PrimitiveType::Int))
        }
        (Some("TableModel"), "getColumnCount" | "get_column_count", 0) => {
            Some(JavaType::Primitive(PrimitiveType::Int))
        }
        (Some("TableModel"), "getColumnName" | "get_column_name", 1) => Some(JavaType::String),
        (Some("TableModel"), "getValueAt" | "get_value_at", 2) => Some(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        }),
        (Some(owner), "isPresent", 0) if is_optional_like_receiver(owner) => {
            Some(JavaType::Primitive(PrimitiveType::Boolean))
        }
        (Some(owner), "get", 0) if is_optional_like_receiver(owner) => Some(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        }),
        (Some(owner), "name", 0) if is_known_external_enum_type(owner) => Some(JavaType::String),
        (Some(owner), "values", 0) if is_known_external_enum_type(owner) => {
            Some(JavaType::Array(Box::new(JavaType::Named {
                name: owner.to_string(),
                args: Vec::new(),
            })))
        }
        (Some(owner), "valueOf", 1) if is_known_external_enum_type(owner) => {
            Some(JavaType::Named {
                name: owner.to_string(),
                args: Vec::new(),
            })
        }
        (
            Some(owner),
            "getMessage" | "get_message" | "getLocalizedMessage" | "get_localized_message",
            0,
        ) if is_known_throwable_type(owner) => Some(JavaType::String),
        (Some(owner), "make", 0) => {
            external_builder_product_type(owner).map(|name| JavaType::Named {
                name,
                args: Vec::new(),
            })
        }
        (Some(_), "getMaxPloidy", 1) => Some(JavaType::Primitive(PrimitiveType::Int)),
        (Some(_), "toPath", 0) => Some(JavaType::Named {
            name: "Path".to_string(),
            args: Vec::new(),
        }),
        (Some(owner), "toFile", 0) if is_path_like_receiver(owner) => Some(JavaType::Named {
            name: "File".to_string(),
            args: Vec::new(),
        }),
        (_, "getCanonicalPath", 0) => Some(JavaType::String),
        (Some(_), "getString", _) => Some(JavaType::String),
        (Some(_), "getAttributeAsString", 1 | 2) => Some(JavaType::String),
        (Some("String"), "split", 1 | 2) => Some(JavaType::Array(Box::new(JavaType::String))),
        (Some("String"), "substring", 1 | 2) => Some(JavaType::String),
        (Some(owner), "toString", 1) if boxed_primitive_type(owner).is_some() => {
            Some(JavaType::String)
        }
        (Some("String"), "format", _) => Some(JavaType::String),
        (Some(owner), "format", _) if is_string_formatting_receiver(owner) => {
            Some(JavaType::String)
        }
        (Some(owner), "join", _) if is_string_join_receiver(owner) => Some(JavaType::String),
        (Some(owner), "parser", 0)
            if canonical_external_receiver_type_key(owner)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "JsonFormat") =>
        {
            Some(JavaType::Named {
                name: "Parser".to_string(),
                args: Vec::new(),
            })
        }
        (Some("StringUtils"), "isEmpty", 1) => Some(JavaType::Primitive(PrimitiveType::Boolean)),
        (Some("StringUtils"), "split", 2) => Some(JavaType::Array(Box::new(JavaType::String))),
        (Some(owner), "writeString", 2 | 3) if is_java_nio_files_receiver(owner) => {
            Some(JavaType::Named {
                name: "Path".to_string(),
                args: Vec::new(),
            })
        }
        (Some("Integer"), "parseInt", 1 | 2) => Some(JavaType::Primitive(PrimitiveType::Int)),
        (_, "getReadGroup", 0) => Some(JavaType::Named {
            name: "SAMReadGroupRecord".to_string(),
            args: Vec::new(),
        }),
        (_, "getIntervals" | "getIntervalsForTraversal", 0 | 1) => Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::Named {
                name: "SimpleInterval".to_string(),
                args: Vec::new(),
            }],
        }),
        (_, "retrieveAllHeaderInfo", 0) => Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::Named {
                name: "HeaderLine".to_string(),
                args: Vec::new(),
            }],
        }),
        (_, "getTraversalParameters", 1) => Some(JavaType::Named {
            name: "TraversalParameters".to_string(),
            args: Vec::new(),
        }),
        (_, "getCigarElements", 0) => Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::Named {
                name: "CigarElement".to_string(),
                args: Vec::new(),
            }],
        }),
        (None, "format", _) => Some(JavaType::String),
        (Some("String"), "valueOf", 1) => Some(JavaType::String),
        (Some("Objects"), "toString", 1 | 2) => Some(JavaType::String),
        (Some("Long"), "toBinaryString", 1) => Some(JavaType::String),
        (_, "charAt", 1) => Some(JavaType::Primitive(PrimitiveType::Char)),
        (_, "getBytes", 0 | 1) | (_, "get_bytes", 0 | 1) | (_, "toByteArray", 0) => Some(
            JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Byte))),
        ),
        _ => None,
    }
}

pub fn known_external_method_return_type_for_receiver(
    receiver_type: Option<&JavaType>,
    name: &str,
    arg_count: usize,
) -> Option<JavaType> {
    if let Some(receiver_type) = receiver_type
        && let Some(java_ty) =
            known_generic_external_method_return_type(receiver_type, name, arg_count)
    {
        return Some(java_ty);
    }

    let receiver_key = receiver_type.and_then(external_receiver_type_key);
    known_external_method_return_type(receiver_key.as_deref(), name, arg_count)
}

pub fn known_external_method_return_type_for_call(
    receiver_type: Option<&JavaType>,
    name: &str,
    arg_types: &[JavaType],
) -> Option<JavaType> {
    if let Some(java_ty) =
        known_java_string_receiver_method_return_type(receiver_type, name, arg_types.len())
    {
        return Some(java_ty);
    }
    if let Some(java_ty) =
        known_argument_sensitive_external_method_return_type(receiver_type, name, arg_types)
    {
        return Some(java_ty);
    }
    known_external_method_return_type_for_receiver(receiver_type, name, arg_types.len())
}

fn known_java_string_receiver_method_return_type(
    receiver_type: Option<&JavaType>,
    name: &str,
    arg_count: usize,
) -> Option<JavaType> {
    let receiver_type = receiver_type?;
    let effective_receiver_type =
        crate::types::optional_element_type(receiver_type).unwrap_or_else(|| receiver_type.clone());
    if !matches!(effective_receiver_type, JavaType::String) {
        return None;
    }
    java_string_runtime_method_return_type(name, arg_count)
}

pub fn java_string_runtime_method_return_type(name: &str, arg_count: usize) -> Option<JavaType> {
    match (name, arg_count) {
        ("toString" | "toLowerCase" | "toUpperCase" | "trim", 0) => Some(JavaType::String),
        ("replace" | "replaceFirst" | "replace_first" | "replaceAll" | "replace_all", 2) => {
            Some(JavaType::String)
        }
        ("substring", 1 | 2) => Some(JavaType::String),
        ("split", 1 | 2) => Some(JavaType::Array(Box::new(JavaType::String))),
        ("isEmpty", 0) => Some(JavaType::Primitive(PrimitiveType::Boolean)),
        ("contains" | "startsWith" | "endsWith" | "equals" | "matches", 1) => {
            Some(JavaType::Primitive(PrimitiveType::Boolean))
        }
        ("length" | "indexOf" | "lastIndexOf", 0 | 1 | 2) => {
            Some(JavaType::Primitive(PrimitiveType::Int))
        }
        _ => None,
    }
}

pub fn known_external_field_type_for_receiver(
    receiver_type: &JavaType,
    field_name: &str,
) -> Option<JavaType> {
    if let Some(receiver) = external_receiver_type_key(receiver_type)
        && let Some(java_ty) = known_numeric_constant_field_type(&receiver, field_name)
    {
        return Some(java_ty);
    }

    if let Some(receiver) = external_receiver_type_key(receiver_type)
        && let Some(java_ty) = known_boolean_constant_field_type(&receiver, field_name)
    {
        return Some(java_ty);
    }

    if let Some(receiver) = external_receiver_type_key(receiver_type)
        && is_known_string_constant_field(&receiver, field_name)
    {
        return Some(JavaType::String);
    }

    if let Some(receiver) = external_receiver_type_key(receiver_type)
        && let Some(java_ty) = known_typed_constant_field_type(&receiver, field_name)
    {
        return Some(java_ty);
    }

    if let Some(receiver) = external_receiver_type_key(receiver_type)
        && let Some(java_ty) = known_external_public_data_field_type(&receiver, field_name)
    {
        return Some(java_ty);
    }

    if let Some(receiver) = external_receiver_type_key(receiver_type)
        && is_enum_like_external_constant(&receiver, field_name)
    {
        return Some(JavaType::Named {
            name: receiver,
            args: Vec::new(),
        });
    }
    match receiver_type {
        JavaType::Named { name, args } if is_pair_like_receiver(name) => {
            match (field_name, args.as_slice()) {
                ("_1" | "a" | "left", [left, _]) => Some(left.clone()),
                ("_2" | "b" | "right", [_, right]) => Some(right.clone()),
                _ => None,
            }
        }
        _ => None,
    }
}

pub fn conservative_external_enum_field_type_for_type_qualifier(
    receiver: &str,
    field_name: &str,
) -> Option<JavaType> {
    let receiver = canonical_external_receiver_type_key(receiver);
    let receiver = conservative_external_enum_receiver_type_name(&receiver, field_name)?;
    Some(JavaType::Named {
        name: receiver,
        args: Vec::new(),
    })
}

fn known_boolean_constant_field_type(receiver: &str, field_name: &str) -> Option<JavaType> {
    let field_name = canonical_external_field_name(field_name);
    match (
        receiver.rsplit('.').next().unwrap_or(receiver),
        field_name.as_str(),
    ) {
        ("Boolean", "TRUE" | "FALSE") => Some(JavaType::Primitive(PrimitiveType::Boolean)),
        _ => None,
    }
}

pub fn canonical_external_field_name(field_name: &str) -> String {
    if field_name.contains('_') || field_name.chars().all(|ch| ch.is_ascii_lowercase()) {
        field_name.to_ascii_uppercase()
    } else {
        field_name.to_string()
    }
}

fn known_numeric_constant_field_type(receiver: &str, field_name: &str) -> Option<JavaType> {
    let field_name = canonical_external_field_name(field_name);
    if is_wrapper_size_constant(receiver, &field_name) {
        return Some(JavaType::Primitive(PrimitiveType::Int));
    }

    let primitive = match (
        receiver.rsplit('.').next().unwrap_or(receiver),
        field_name.as_str(),
    ) {
        ("Byte", "MAX_VALUE" | "MIN_VALUE") => PrimitiveType::Byte,
        ("Short", "MAX_VALUE" | "MIN_VALUE") => PrimitiveType::Short,
        ("Integer", "MAX_VALUE" | "MIN_VALUE") => PrimitiveType::Int,
        ("Long", "MAX_VALUE" | "MIN_VALUE") => PrimitiveType::Long,
        ("Character", "MAX_VALUE" | "MIN_VALUE") => PrimitiveType::Char,
        (
            "Float",
            "MAX_VALUE" | "MIN_VALUE" | "NaN" | "POSITIVE_INFINITY" | "NEGATIVE_INFINITY",
        ) => PrimitiveType::Float,
        (
            "Double",
            "MAX_VALUE" | "MIN_VALUE" | "NaN" | "POSITIVE_INFINITY" | "NEGATIVE_INFINITY",
        ) => PrimitiveType::Double,
        ("Math", "PI" | "E") => PrimitiveType::Double,
        (
            "Pattern",
            "UNIX_LINES"
            | "CASE_INSENSITIVE"
            | "COMMENTS"
            | "MULTILINE"
            | "LITERAL"
            | "DOTALL"
            | "UNICODE_CASE"
            | "CANON_EQ"
            | "UNICODE_CHARACTER_CLASS",
        ) => PrimitiveType::Int,
        (
            "BigDecimal",
            "ROUND_UP" | "ROUND_DOWN" | "ROUND_CEILING" | "ROUND_FLOOR" | "ROUND_HALF_UP"
            | "ROUND_HALF_DOWN" | "ROUND_HALF_EVEN" | "ROUND_UNNECESSARY",
        ) => PrimitiveType::Int,
        ("BufferedImage", "TYPE_INT_RGB" | "TYPE_INT_ARGB") => PrimitiveType::Int,
        _ => return None,
    };
    Some(JavaType::Primitive(primitive))
}

fn is_wrapper_size_constant(receiver: &str, field_name: &str) -> bool {
    matches!(field_name, "SIZE" | "BYTES")
        && matches!(
            receiver.rsplit('.').next().unwrap_or(receiver),
            "Byte" | "Short" | "Character" | "Integer" | "Long" | "Float" | "Double"
        )
}

fn known_typed_constant_field_type(receiver: &str, field_name: &str) -> Option<JavaType> {
    let field_name = canonical_external_field_name(field_name);
    let qualified_receiver = receiver;
    let receiver = receiver.rsplit('.').next().unwrap_or(receiver);
    if receiver == "HelpConstants" && field_name.starts_with("DOC_CAT_") {
        return Some(JavaType::String);
    }
    if receiver == "ArrayUtils"
        && let Some(primitive) = array_utils_empty_array_element_type(&field_name)
    {
        return Some(JavaType::Array(Box::new(JavaType::Primitive(primitive))));
    }
    if receiver == "BigInteger" && matches!(field_name.as_str(), "ZERO" | "ONE") {
        return Some(JavaType::Named {
            name: qualified_receiver.to_string(),
            args: Vec::new(),
        });
    }
    if receiver == "Collections"
        && let Some(java_ty) = collections_empty_field_type(&field_name)
    {
        return Some(java_ty);
    }
    if receiver == "Constants"
        && matches!(
            field_name.as_str(),
            "CHROMOSOME_FOLDER_DELIMITER_SYMBOL_REGEX"
        )
    {
        return Some(JavaType::String);
    }
    if receiver == "File" {
        match field_name.as_str() {
            "SEPARATOR" => return Some(JavaType::String),
            "separatorChar" => return Some(JavaType::Primitive(PrimitiveType::Char)),
            _ => {}
        }
    }
    if receiver == "Math" && matches!(field_name.as_str(), "PI" | "E") {
        return Some(JavaType::Primitive(PrimitiveType::Double));
    }
    if receiver == "Font" && matches!(field_name.as_str(), "PLAIN" | "BOLD" | "ITALIC") {
        return Some(JavaType::Primitive(PrimitiveType::Int));
    }
    if receiver == "Spliterator"
        && matches!(
            field_name.as_str(),
            "IMMUTABLE" | "NONNULL" | "SIZED" | "SUBSIZED" | "ORDERED" | "SORTED" | "DISTINCT"
        )
    {
        return Some(JavaType::Primitive(PrimitiveType::Int));
    }
    let named = match (receiver, field_name.as_str()) {
        ("System", "OUT" | "ERR") => "PrintStream",
        ("System", "IN") => "InputStream",
        (
            "Color",
            "BLACK" | "BLUE" | "CYAN" | "DARK_GRAY" | "GRAY" | "GREEN" | "LIGHT_GRAY" | "MAGENTA"
            | "ORANGE" | "PINK" | "RED" | "WHITE" | "YELLOW",
        ) => "Color",
        ("ByteOrder", "BIG_ENDIAN" | "LITTLE_ENDIAN") => "ByteOrder",
        (
            "StandardCharsets",
            "US_ASCII" | "ISO_8859_1" | "UTF_8" | "UTF_16BE" | "UTF_16LE" | "UTF_16",
        ) => "Charset",
        (
            "RoundingMode",
            "UP" | "DOWN" | "CEILING" | "FLOOR" | "HALF_UP" | "HALF_DOWN" | "HALF_EVEN"
            | "UNNECESSARY",
        ) => "RoundingMode",
        ("Option", "CACHE_FILE_BASED_INDEXES" | "INCLUDE_SOURCE_IN_RECORDS")
            if qualified_receiver == "Option" =>
        {
            "Option"
        }
        ("Level", "DEBUG" | "INFO" | "ERROR") => "Level",
        ("Locale", "ENGLISH" | "US") => "Locale",
        ("ElementKind", "FIELD") => "ElementKind",
        ("StandardOpenOption", "CREATE_NEW" | "CREATE" | "WRITE" | "READ" | "APPEND") => {
            "StandardOpenOption"
        }
        ("AccessMode", "READ" | "WRITE" | "EXECUTE") => "AccessMode",
        (
            "PosixFilePermission",
            "GROUP_EXECUTE" | "GROUP_READ" | "GROUP_WRITE" | "OWNER_EXECUTE" | "OWNER_READ"
            | "OWNER_WRITE" | "OTHERS_EXECUTE" | "OTHERS_READ" | "OTHERS_WRITE",
        ) => "PosixFilePermission",
        ("HttpMethod", "GET" | "POST" | "PUT" | "DELETE" | "HEAD" | "PATCH") => "HttpMethod",
        ("TimeUnit", "SECONDS" | "MILLISECONDS" | "MICROSECONDS" | "NANOSECONDS") => "TimeUnit",
        _ => return None,
    };
    Some(JavaType::Named {
        name: named.to_string(),
        args: Vec::new(),
    })
}

fn collections_empty_field_type(field_name: &str) -> Option<JavaType> {
    match field_name {
        "EMPTY_LIST" => Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            }],
        }),
        "EMPTY_SET" => Some(JavaType::Named {
            name: "Set".to_string(),
            args: vec![JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            }],
        }),
        "EMPTY_MAP" => Some(JavaType::Named {
            name: "Map".to_string(),
            args: vec![
                JavaType::Named {
                    name: "Object".to_string(),
                    args: Vec::new(),
                },
                JavaType::Named {
                    name: "Object".to_string(),
                    args: Vec::new(),
                },
            ],
        }),
        _ => None,
    }
}

fn array_utils_empty_array_element_type(field_name: &str) -> Option<PrimitiveType> {
    let primitive_name = field_name.strip_prefix("EMPTY_")?.strip_suffix("_ARRAY")?;
    match primitive_name {
        "BOOLEAN" => Some(PrimitiveType::Boolean),
        "BYTE" => Some(PrimitiveType::Byte),
        "SHORT" => Some(PrimitiveType::Short),
        "CHAR" | "CHARACTER" => Some(PrimitiveType::Char),
        "INT" | "INTEGER" => Some(PrimitiveType::Int),
        "LONG" => Some(PrimitiveType::Long),
        "FLOAT" => Some(PrimitiveType::Float),
        "DOUBLE" => Some(PrimitiveType::Double),
        _ => None,
    }
}

fn known_external_public_data_field_type(receiver: &str, field_name: &str) -> Option<JavaType> {
    let receiver = receiver.rsplit('.').next().unwrap_or(receiver);
    match (receiver, field_name) {
        ("scala", "Option") => Some(JavaType::Named {
            name: "Option".to_string(),
            args: Vec::new(),
        }),
        ("ClassTag$", "MODULE$") | ("class_tag$", "MODULE$") => Some(JavaType::Named {
            name: "ClassTag$".to_string(),
            args: Vec::new(),
        }),
        _ => None,
    }
}

fn is_known_string_constant_field(receiver: &str, field_name: &str) -> bool {
    let field_name = canonical_external_field_name(field_name);
    let receiver = receiver.rsplit('.').next().unwrap_or(receiver);
    let is_constant_name = !field_name.is_empty()
        && field_name
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_');
    if !is_constant_name {
        return false;
    }

    match receiver {
        "FileExtensions" => true,
        _ => is_conventionally_string_constant_field(&field_name),
    }
}

fn is_conventionally_string_constant_field(field_name: &str) -> bool {
    field_name.ends_with("_KEY")
        || field_name.ends_with("_FIELD")
        || field_name.ends_with("_NAME")
        || field_name.ends_with("_LONG_NAME")
        || field_name.ends_with("_SHORT_NAME")
        || field_name.ends_with("_DELIM")
        || field_name.ends_with("_DELIMITER")
        || field_name.ends_with("_SEPARATOR")
        || field_name.ends_with("_PREFIX")
        || field_name.ends_with("_SUFFIX")
        || field_name.ends_with("_REGEX")
        || field_name.ends_with("_PATTERN")
        || field_name.ends_with("_EXTENSION")
        || field_name.ends_with("_PATH")
        || field_name.ends_with("_URI")
        || field_name.ends_with("_URL")
        || matches!(
            field_name,
            "NAME" | "DESCRIPTION" | "DOC" | "USAGE" | "VERSION"
        )
}

fn known_argument_sensitive_external_method_return_type(
    receiver_type: Option<&JavaType>,
    name: &str,
    arg_types: &[JavaType],
) -> Option<JavaType> {
    let receiver_key = receiver_type.and_then(external_receiver_type_key);
    if let Some(return_type) =
        crate::javalibs::broad_utils_dynamic_return_type(receiver_key.as_deref(), name, arg_types)
    {
        return Some(return_type);
    }
    if let Some(return_type) = crate::javalibs::broad_static_utility_return_type(
        receiver_key.as_deref(),
        name,
        arg_types.len(),
    ) {
        return Some(return_type);
    }
    if name == "toArray" {
        if let Some(JavaType::Array(element)) = arg_types.first() {
            return Some(JavaType::Array(element.clone()));
        }
        if arg_types.is_empty()
            && receiver_type
                .and_then(external_receiver_type_key)
                .as_deref()
                .is_some_and(is_known_collection_type)
        {
            return Some(JavaType::Array(Box::new(JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            })));
        }
    }

    match (receiver_key.as_deref(), name, arg_types) {
        (Some(owner), "get", []) if is_optional_like_receiver(owner) => receiver_type
            .and_then(crate::types::optional_element_type)
            .or_else(|| {
                if let Some(JavaType::Named { args, .. }) = receiver_type {
                    args.first().cloned()
                } else {
                    None
                }
            }),
        (Some(owner), "orElse", [fallback]) if is_optional_like_receiver(owner) => receiver_type
            .and_then(crate::types::optional_element_type)
            .or_else(|| {
                if let Some(JavaType::Named { args, .. }) = receiver_type {
                    args.first().cloned()
                } else {
                    None
                }
            })
            .or_else(|| Some(generic_read_type(fallback))),
        (Some("Option"), "apply", [value]) => Some(JavaType::Named {
            name: "Option".to_string(),
            args: vec![generic_read_type(value)],
        }),
        (Some("ClassTag$"), "apply", [_]) => Some(JavaType::Named {
            name: "ClassTag".to_string(),
            args: Vec::new(),
        }),
        (Some(_), "fastqToPhred", [arg]) if is_char_like_type(arg) => {
            Some(JavaType::Primitive(PrimitiveType::Int))
        }
        (Some(_), "fastqToPhred", [arg]) if is_string_like_type(arg) => Some(JavaType::Array(
            Box::new(JavaType::Primitive(PrimitiveType::Byte)),
        )),
        (Some(_), "phredToFastq", [arg]) if is_integral_like_type(arg) => Some(JavaType::String),
        (Some(_), "phredToFastq", [arg]) if is_byte_array_type(arg) => Some(JavaType::String),
        (Some(_), "getBasesReverseComplement", [arg]) if is_byte_array_type(arg) => Some(
            JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Byte))),
        ),
        (Some(owner), "toArray", [_]) => primitive_array_utility_element_type(owner)
            .map(|primitive| JavaType::Array(Box::new(JavaType::Primitive(primitive)))),
        (Some("Collections"), "singletonList", [element]) => Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![element.clone()],
        }),
        (Some("Collections"), "singleton", [element]) => Some(JavaType::Named {
            name: "Set".to_string(),
            args: vec![element.clone()],
        }),
        (Some("Collections"), "singletonMap", [key, value]) => Some(JavaType::Named {
            name: "Map".to_string(),
            args: vec![key.clone(), value.clone()],
        }),
        (Some("ImmutableMap"), "of", args) => immutable_map_factory_return_type("Map", args),
        (Some("ImmutableSortedMap"), "of", args) => {
            immutable_map_factory_return_type("SortedMap", args)
        }
        (Some("EnumSet"), "of", args) if !args.is_empty() => Some(JavaType::Named {
            name: "EnumSet".to_string(),
            args: vec![common_enum_set_element_type(args)],
        }),
        (Some("Iterators"), "forEnumeration", [enumeration]) => {
            collection_element_type(enumeration).map(|element| JavaType::Named {
                name: "Iterator".to_string(),
                args: vec![element.clone()],
            })
        }
        (Some("JavaSparkContext"), "parallelize", [collection]) => {
            collection_element_type(collection).map(|element| JavaType::Named {
                name: "JavaRDD".to_string(),
                args: vec![element.clone()],
            })
        }
        (Some("Files"), "newDirectoryStream", [_] | [_, _]) => Some(JavaType::Named {
            name: "DirectoryStream".to_string(),
            args: vec![JavaType::Named {
                name: "Path".to_string(),
                args: Vec::new(),
            }],
        }),
        (Some("System"), "getProperty", [_] | [_, _]) => Some(JavaType::String),
        (Some("ImmutablePair" | "Pair"), "of", [left, right]) => Some(JavaType::Named {
            name: "Pair".to_string(),
            args: vec![left.clone(), right.clone()],
        }),
        (Some("ListUtils"), "union", [left, right]) => {
            Some(collection_union_return_type("List", left, right))
        }
        (Some("Sets"), "union", [left, right]) => {
            Some(collection_union_return_type("Set", left, right))
        }
        (Some("Collectors"), "toList", []) => Some(collector_return_type("List")),
        (Some("Collectors"), "toSet", []) => Some(collector_return_type("Set")),
        (Some("Collectors"), "joining", [] | [_] | [_, _, _]) => {
            Some(collector_scalar_return_type(JavaType::String))
        }
        (Some("IntStream"), "range" | "range_closed", [_, _]) => Some(JavaType::Named {
            name: "IntStream".to_string(),
            args: Vec::new(),
        }),
        (Some("Stream"), "collect", [collector]) => {
            stream_collect_return_type(receiver_type, collector)
        }
        (Some("Stream"), "of", args) if !args.is_empty() => {
            Some(stream_return_type(common_stream_of_element_type(args)))
        }
        (Some("Arrays"), "stream", [JavaType::Array(element)]) => {
            Some(stream_return_type(element.as_ref().clone()))
        }
        (Some("Arrays"), "asList", args) if !args.is_empty() => Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![arrays_as_list_element_type(args)],
        }),
        (Some("Arrays"), "equals", [JavaType::Array(_), JavaType::Array(_)]) => {
            Some(JavaType::Primitive(PrimitiveType::Boolean))
        }
        (Some("Arrays"), "copyOf", [JavaType::Array(element), _]) => {
            Some(JavaType::Array(element.clone()))
        }
        (Some("Arrays"), "copyOfRange", [JavaType::Array(element), _, _]) => {
            Some(JavaType::Array(element.clone()))
        }
        (Some("ArrayUtils"), "addAll", [left, right]) => matching_array_type(left, right),
        (Some("MathArrays"), "ebeMultiply", [left, right])
            if matching_primitive_array_type(left, right).is_some() =>
        {
            matching_primitive_array_type(left, right)
        }
        _ => None,
    }
}

fn matching_array_type(left: &JavaType, right: &JavaType) -> Option<JavaType> {
    match (left, right) {
        (JavaType::Array(left), JavaType::Array(right)) if left == right => {
            Some(JavaType::Array(left.clone()))
        }
        _ => None,
    }
}

fn matching_primitive_array_type(left: &JavaType, right: &JavaType) -> Option<JavaType> {
    match (left, right) {
        (JavaType::Array(left), JavaType::Array(right))
            if matches!(
                (left.as_ref(), right.as_ref()),
                (JavaType::Primitive(left), JavaType::Primitive(right)) if left == right
            ) =>
        {
            Some(JavaType::Array(left.clone()))
        }
        _ => None,
    }
}

fn known_generic_iterator_return_type(
    receiver: &str,
    args: &[JavaType],
    name: &str,
    arg_count: usize,
) -> Option<JavaType> {
    if !is_known_iterator_type(receiver) {
        return None;
    }
    match (name, arg_count, args) {
        ("hasNext", 0, _) => Some(JavaType::Primitive(PrimitiveType::Boolean)),
        ("next", 0, [element]) => Some(generic_read_type(element)),
        _ => None,
    }
}

fn known_generic_queue_return_type(
    receiver: &str,
    args: &[JavaType],
    name: &str,
    arg_count: usize,
) -> Option<JavaType> {
    if !is_known_queue_type(receiver) {
        return None;
    }
    match (name, arg_count, args) {
        ("peek" | "poll" | "remove" | "element", 0, [element]) => Some(generic_read_type(element)),
        ("offer" | "add", 1, _) => Some(JavaType::Primitive(PrimitiveType::Boolean)),
        _ => None,
    }
}

fn iterator_return_type(element: JavaType) -> JavaType {
    JavaType::Named {
        name: "Iterator".to_string(),
        args: vec![element],
    }
}

fn generic_read_type(ty: &JavaType) -> JavaType {
    match ty {
        JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Extends(bound))) => {
            bound.as_ref().clone()
        }
        _ => ty.clone(),
    }
}

fn immutable_map_factory_return_type(map_name: &str, args: &[JavaType]) -> Option<JavaType> {
    if args.len() % 2 != 0 {
        return None;
    }
    let (key, value) = match args {
        [] => (
            JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            },
            JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            },
        ),
        [key, value, ..] => (key.clone(), value.clone()),
        _ => return None,
    };
    Some(JavaType::Named {
        name: map_name.to_string(),
        args: vec![key, value],
    })
}

fn common_enum_set_element_type(args: &[JavaType]) -> JavaType {
    let Some(first) = args.first() else {
        return JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        };
    };
    if args.iter().all(|arg| arg == first) {
        first.clone()
    } else {
        JavaType::Named {
            name: "Enum".to_string(),
            args: Vec::new(),
        }
    }
}

fn known_generic_external_method_return_type(
    receiver_type: &JavaType,
    name: &str,
    arg_count: usize,
) -> Option<JavaType> {
    if name == "clone" && arg_count == 0 && matches!(receiver_type, JavaType::Array(_)) {
        return Some(receiver_type.clone());
    }

    if let JavaType::Named {
        name: receiver,
        args,
    } = receiver_type
    {
        if let Some(java_ty) = known_generic_iterator_return_type(receiver, args, name, arg_count) {
            return Some(java_ty);
        }
        if let Some(java_ty) = known_generic_queue_return_type(receiver, args, name, arg_count) {
            return Some(java_ty);
        }
        if receiver == "ThreadLocal"
            && name == "get"
            && arg_count == 0
            && let [value] = args.as_slice()
        {
            return Some(generic_read_type(value));
        }
    }

    match receiver_type {
        JavaType::Named {
            name: receiver,
            args,
        } if is_known_list_type(receiver) => match (name, arg_count, args.as_slice()) {
            ("size", 0, _) => Some(JavaType::Primitive(PrimitiveType::Int)),
            ("isEmpty", 0, _) => Some(JavaType::Primitive(PrimitiveType::Boolean)),
            ("stream", 0, [element]) => Some(stream_return_type(generic_read_type(element))),
            ("iterator", 0, [element]) => Some(iterator_return_type(generic_read_type(element))),
            ("get", 1, [element]) => Some(generic_read_type(element)),
            _ => None,
        },
        JavaType::Named {
            name: receiver,
            args,
        } if is_known_set_type(receiver) => match (name, arg_count, args.as_slice()) {
            ("size", 0, _) => Some(JavaType::Primitive(PrimitiveType::Int)),
            ("isEmpty", 0, _) => Some(JavaType::Primitive(PrimitiveType::Boolean)),
            ("stream", 0, [element]) => Some(stream_return_type(generic_read_type(element))),
            ("iterator", 0, [element]) => Some(iterator_return_type(generic_read_type(element))),
            _ => None,
        },
        JavaType::Named {
            name: receiver,
            args,
        } if is_known_collection_type(receiver) => match (name, arg_count, args.as_slice()) {
            ("size", 0, _) => Some(JavaType::Primitive(PrimitiveType::Int)),
            ("isEmpty", 0, _) => Some(JavaType::Primitive(PrimitiveType::Boolean)),
            ("stream", 0, [element]) => Some(stream_return_type(generic_read_type(element))),
            ("iterator", 0, [element]) => Some(iterator_return_type(generic_read_type(element))),
            _ => None,
        },
        JavaType::Named {
            name: receiver,
            args,
        } if is_known_iterable_type(receiver) => match (name, arg_count, args.as_slice()) {
            ("iterator", 0, [element]) => Some(iterator_return_type(generic_read_type(element))),
            _ => None,
        },
        JavaType::Named {
            name: receiver,
            args,
        } if receiver == "Stream" => match (name, arg_count, args.as_slice()) {
            ("filter", 1, [element]) | ("distinct", 0, [element]) | ("sorted", 0, [element]) => {
                Some(stream_return_type(element.clone()))
            }
            ("mapToInt", 1, [_]) => Some(JavaType::Named {
                name: "IntStream".to_string(),
                args: Vec::new(),
            }),
            ("mapToLong", 1, [_]) => Some(JavaType::Named {
                name: "LongStream".to_string(),
                args: Vec::new(),
            }),
            ("mapToDouble", 1, [_]) => Some(JavaType::Named {
                name: "DoubleStream".to_string(),
                args: Vec::new(),
            }),
            ("iterator", 0, [element]) => Some(JavaType::Named {
                name: "Iterator".to_string(),
                args: vec![element.clone()],
            }),
            _ => None,
        },
        JavaType::Named { name: receiver, .. } if receiver == "IntStream" => {
            match (name, arg_count) {
                ("sum", 0) => Some(JavaType::Primitive(PrimitiveType::Int)),
                _ => None,
            }
        }
        JavaType::Named { name: receiver, .. } if receiver == "LongStream" => {
            match (name, arg_count) {
                ("sum", 0) => Some(JavaType::Primitive(PrimitiveType::Long)),
                _ => None,
            }
        }
        JavaType::Named { name: receiver, .. } if receiver == "DoubleStream" => {
            match (name, arg_count) {
                ("sum", 0) => Some(JavaType::Primitive(PrimitiveType::Double)),
                _ => None,
            }
        }
        JavaType::Named {
            name: receiver,
            args,
        } if is_known_map_type(receiver) => match (name, arg_count, args.as_slice()) {
            ("get", 1, [_, value]) | ("getOrDefault", 2, [_, value]) => {
                Some(generic_read_type(value))
            }
            ("entrySet", 0, [key, value]) => Some(JavaType::Named {
                name: "Set".to_string(),
                args: vec![JavaType::Named {
                    name: "Entry".to_string(),
                    args: vec![generic_read_type(key), generic_read_type(value)],
                }],
            }),
            _ => None,
        },
        JavaType::Named {
            name: receiver,
            args,
        } if is_pair_like_receiver(receiver) => match (name, args.as_slice()) {
            ("getLeft" | "getKey" | "_1", [left, _]) if arg_count == 0 => Some(left.clone()),
            ("getRight" | "getValue" | "_2", [_, right]) if arg_count == 0 => Some(right.clone()),
            _ => None,
        },
        JavaType::Named { name: receiver, .. }
            if is_external_builder_fluent_method(receiver, name, arg_count) =>
        {
            Some(receiver_type.clone())
        }
        JavaType::Named { name: receiver, .. }
            if receiver == "File"
                && matches!(name, "getPath" | "getAbsolutePath")
                && arg_count == 0 =>
        {
            Some(JavaType::String)
        }
        JavaType::Named { name: receiver, .. }
            if matches!(receiver.as_str(), "Path" | "URI" | "URL")
                && name == "toString"
                && arg_count == 0 =>
        {
            Some(JavaType::String)
        }
        _ => None,
    }
}

fn is_char_like_type(java_ty: &JavaType) -> bool {
    matches!(java_ty, JavaType::Primitive(PrimitiveType::Char))
        || matches!(java_ty, JavaType::Named { name, .. } if name == "Character")
}

fn is_string_like_type(java_ty: &JavaType) -> bool {
    matches!(java_ty, JavaType::String)
        || matches!(java_ty, JavaType::Named { name, .. } if name == "String")
}

fn is_byte_array_type(java_ty: &JavaType) -> bool {
    matches!(
        java_ty,
        JavaType::Array(inner) if matches!(inner.as_ref(), JavaType::Primitive(PrimitiveType::Byte))
    )
}

fn is_integral_like_type(java_ty: &JavaType) -> bool {
    matches!(
        java_ty,
        JavaType::Primitive(
            PrimitiveType::Byte
                | PrimitiveType::Short
                | PrimitiveType::Char
                | PrimitiveType::Int
                | PrimitiveType::Long
        )
    ) || matches!(java_ty, JavaType::Named { name, .. } if matches!(name.as_str(), "Byte" | "Short" | "Character" | "Integer" | "Long"))
}

fn arrays_as_list_element_type(args: &[JavaType]) -> JavaType {
    if let [JavaType::Array(inner)] = args {
        if matches!(inner.as_ref(), JavaType::Primitive(_)) {
            return args[0].clone();
        }
        return inner.as_ref().clone();
    }
    let Some(first) = args.first() else {
        return JavaType::Unknown("asList".to_string());
    };
    if args.iter().all(|arg| arg == first) {
        return first.clone();
    }
    JavaType::Named {
        name: "Object".to_string(),
        args: Vec::new(),
    }
}

fn common_stream_of_element_type(args: &[JavaType]) -> JavaType {
    let Some(first) = args.first() else {
        return JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        };
    };
    if let [JavaType::Array(element)] = args {
        return element.as_ref().clone();
    }
    if args.iter().all(|arg| arg == first) {
        return first.clone();
    }
    JavaType::Named {
        name: "Object".to_string(),
        args: Vec::new(),
    }
}

fn stream_return_type(element: JavaType) -> JavaType {
    JavaType::Named {
        name: "Stream".to_string(),
        args: vec![element],
    }
}

fn collector_return_type(collection_kind: &str) -> JavaType {
    let element = JavaType::Unknown("collector_element".to_string());
    collector_type(
        element.clone(),
        JavaType::Named {
            name: collection_kind.to_string(),
            args: vec![element],
        },
    )
}

fn collector_scalar_return_type(result_type: JavaType) -> JavaType {
    collector_type(
        JavaType::Unknown("collector_element".to_string()),
        result_type,
    )
}

fn collector_type(input_type: JavaType, result_type: JavaType) -> JavaType {
    JavaType::Named {
        name: "Collector".to_string(),
        args: vec![
            input_type,
            JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            },
            result_type,
        ],
    }
}

fn stream_collect_return_type(
    receiver_type: Option<&JavaType>,
    collector: &JavaType,
) -> Option<JavaType> {
    let stream_element = match receiver_type {
        Some(JavaType::Named { name, args }) if name == "Stream" => args.first(),
        _ => None,
    }?;
    let collector_result = match collector {
        JavaType::Named { name, args } if name == "Collector" => args.get(2),
        _ => None,
    }?;
    Some(substitute_collector_element_type(
        collector_result,
        stream_element,
    ))
}

fn substitute_collector_element_type(java_ty: &JavaType, stream_element: &JavaType) -> JavaType {
    match java_ty {
        JavaType::Unknown(name) if name == "collector_element" => stream_element.clone(),
        JavaType::Array(element) => JavaType::Array(Box::new(substitute_collector_element_type(
            element,
            stream_element,
        ))),
        JavaType::Named { name, args } => JavaType::Named {
            name: name.clone(),
            args: args
                .iter()
                .map(|arg| substitute_collector_element_type(arg, stream_element))
                .collect(),
        },
        JavaType::Wildcard(Some(bound)) => JavaType::Wildcard(Some(match bound {
            crate::java_ir::WildcardBound::Extends(inner) => {
                crate::java_ir::WildcardBound::Extends(Box::new(substitute_collector_element_type(
                    inner,
                    stream_element,
                )))
            }
            crate::java_ir::WildcardBound::Super(inner) => crate::java_ir::WildcardBound::Super(
                Box::new(substitute_collector_element_type(inner, stream_element)),
            ),
        })),
        other => other.clone(),
    }
}

fn collection_union_return_type(kind: &str, left: &JavaType, right: &JavaType) -> JavaType {
    JavaType::Named {
        name: kind.to_string(),
        args: vec![
            common_collection_element_type(left, right).unwrap_or_else(|| JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            }),
        ],
    }
}

fn common_collection_element_type(left: &JavaType, right: &JavaType) -> Option<JavaType> {
    let left_element = collection_element_type(left)?;
    let right_element = collection_element_type(right)?;
    if left_element == right_element {
        Some(left_element.clone())
    } else {
        Some(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        })
    }
}

fn collection_element_type(java_ty: &JavaType) -> Option<&JavaType> {
    match java_ty {
        JavaType::Named { args, .. } => args.first(),
        _ => None,
    }
}

fn primitive_array_utility_element_type(owner: &str) -> Option<PrimitiveType> {
    match owner.rsplit('.').next().unwrap_or(owner) {
        "Booleans" => Some(PrimitiveType::Boolean),
        "Bytes" => Some(PrimitiveType::Byte),
        "Shorts" => Some(PrimitiveType::Short),
        "Chars" => Some(PrimitiveType::Char),
        "Ints" => Some(PrimitiveType::Int),
        "Longs" => Some(PrimitiveType::Long),
        "Floats" => Some(PrimitiveType::Float),
        "Doubles" => Some(PrimitiveType::Double),
        _ => None,
    }
}

fn is_pair_like_receiver(receiver: &str) -> bool {
    let receiver = receiver.rsplit('.').next().unwrap_or(receiver);
    matches!(
        receiver,
        "Pair"
            | "ImmutablePair"
            | "MutablePair"
            | "Tuple"
            | "Map.Entry"
            | "Entry"
            | "Tuple2"
            | "Tuple3"
            | "Tuple4"
    )
}

fn known_external_decoder_return_type(owner: &str) -> Option<JavaType> {
    match owner.rsplit('.').next().unwrap_or(owner) {
        "TextCigarCodec" => Some(JavaType::Named {
            name: "Cigar".to_string(),
            args: Vec::new(),
        }),
        _ => None,
    }
}

fn is_sequence_dictionary_getter(name: &str) -> bool {
    matches!(
        name,
        "getSequenceDictionary" | "getSequenceDictionaryForDrivingVariants"
    )
}

fn is_path_getter(name: &str) -> bool {
    matches!(name, "getReferencePath")
}

fn is_path_like_receiver(receiver: &str) -> bool {
    matches!(receiver.rsplit('.').next().unwrap_or(receiver), "Path")
}

fn is_java_nio_files_receiver(receiver: &str) -> bool {
    matches!(
        canonical_external_receiver_type_key(receiver).as_str(),
        "Files" | "java.nio.file.Files" | "JavaNioFileFiles"
    )
}

fn is_enum_like_external_constant(receiver: &str, field_name: &str) -> bool {
    let original_field_name = field_name;
    let field_name = canonical_external_field_name(field_name);
    if matches!(receiver, "Attribute" | "Priority") {
        return is_enum_constant_name(&field_name);
    }
    match receiver.rsplit('.').next().unwrap_or(receiver) {
        "SAMTag" => is_sam_tag_constant(&field_name),
        "VCFHeaderLineCount"
        | "GenotypeType"
        | "Strand"
        | "SAMFlag"
        | "VCFHeaderVersion"
        | "Options"
        | "SWOverhangStrategy"
        | "StructuralVariantType"
        | "Characteristics"
        | "HEADER_FIELDS"
        | "TimeUnit"
        | "OutputType"
        | "Level"
        | "Locale"
        | "ElementKind" => is_enum_constant_name(&field_name),
        "VCFHeaderLineType" => is_vcf_header_line_type_constant(original_field_name),
        _ => false,
    }
}

fn is_self_typed_external_enum_constant_field(
    receiver: &str,
    field_name: &str,
    field_type: &JavaType,
) -> bool {
    let JavaType::Named { name, args } = field_type else {
        return false;
    };
    args.is_empty()
        && canonical_external_receiver_type_key(name)
            == canonical_external_receiver_type_key(receiver)
        && is_enum_constant_name(&canonical_external_field_name(field_name))
}

fn external_enum_constant_field_type_matches_owner(receiver: &str, field_type: &JavaType) -> bool {
    let JavaType::Named { name, args } = field_type else {
        return false;
    };
    if !args.is_empty() {
        return false;
    }
    let field_type_key = canonical_external_receiver_type_key(name);
    let receiver_key = canonical_external_receiver_type_key(receiver);
    field_type_key == receiver_key
        || field_type_key.rsplit('.').next() == receiver_key.rsplit('.').next()
}

fn conservative_external_enum_receiver_type_name(
    receiver: &str,
    field_name: &str,
) -> Option<String> {
    let field_name = canonical_external_field_name(field_name);
    if !is_enum_constant_name(&field_name) || is_conservative_static_data_constant(&field_name) {
        return None;
    }
    let simple_receiver = receiver.rsplit('.').next().unwrap_or(receiver);
    if is_conservative_static_data_receiver(simple_receiver) {
        return None;
    }
    let type_name = if simple_receiver
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
    {
        simple_receiver.to_string()
    } else if simple_receiver
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
    {
        upper_camel_from_snake(simple_receiver)
    } else {
        return None;
    };
    type_name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        .then_some(type_name)
}

fn is_conservative_static_data_receiver(receiver: &str) -> bool {
    matches!(
        receiver,
        "Constants" | "Defaults" | "Config" | "Configuration"
    ) || receiver.ends_with("Constants")
        || receiver.ends_with("_constants")
        || receiver.ends_with("Config")
        || receiver.ends_with("_config")
        || receiver.ends_with("Configuration")
        || receiver.ends_with("_configuration")
}

fn is_conservative_static_data_constant(field_name: &str) -> bool {
    field_name.starts_with("MAX_")
        || field_name.starts_with("MIN_")
        || field_name.starts_with("DEFAULT_")
        || field_name.starts_with("UNKNOWN_")
        || field_name.starts_with("NO_")
        || field_name.ends_with("_LENGTH")
        || field_name.ends_with("_COUNT")
        || field_name.ends_with("_SIZE")
}

fn upper_camel_from_snake(name: &str) -> String {
    let mut out = String::new();
    let mut uppercase_next = true;
    for ch in name.chars() {
        if ch == '_' {
            uppercase_next = true;
        } else if uppercase_next {
            out.push(ch.to_ascii_uppercase());
            uppercase_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn lower_camel_from_snake(name: &str) -> String {
    let mut out = String::new();
    let mut uppercase_next = false;
    for ch in name.chars() {
        if ch == '_' {
            uppercase_next = true;
        } else if uppercase_next {
            out.push(ch.to_ascii_uppercase());
            uppercase_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn is_enum_constant_name(field_name: &str) -> bool {
    !is_string_like_constant_name(field_name)
        && field_name.chars().any(|ch| ch.is_ascii_uppercase())
        && field_name
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
}

fn is_string_like_constant_name(field_name: &str) -> bool {
    field_name.ends_with("_STRING")
        || field_name.ends_with("_KEY")
        || field_name.ends_with("_FIELD")
}

fn is_known_external_enum_type(receiver: &str) -> bool {
    if matches!(receiver, "Attribute" | "Priority") {
        return true;
    }
    matches!(
        receiver.rsplit('.').next().unwrap_or(receiver),
        "SAMTag"
            | "VCFHeaderLineType"
            | "VCFHeaderLineCount"
            | "GenotypeType"
            | "Strand"
            | "Characteristics"
            | "HEADER_FIELDS"
            | "SAMFlag"
            | "VCFHeaderVersion"
            | "Options"
            | "SWOverhangStrategy"
            | "StructuralVariantType"
            | "TimeUnit"
            | "OutputType"
    )
}

fn is_vcf_header_line_type_constant(field_name: &str) -> bool {
    matches!(
        field_name,
        "Integer" | "Float" | "Flag" | "Character" | "String"
    )
}

fn is_sam_tag_constant(field_name: &str) -> bool {
    !field_name.is_empty()
        && field_name
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
}

fn is_string_formatting_receiver(receiver: &str) -> bool {
    matches!(
        receiver.rsplit('.').next().unwrap_or(receiver),
        "DecimalFormat" | "NumberFormat" | "DateFormat" | "MessageFormat" | "Format"
    )
}

fn is_string_join_receiver(receiver: &str) -> bool {
    matches!(
        receiver.rsplit('.').next().unwrap_or(receiver),
        "String" | "StringUtils" | "StringUtil"
    )
}

fn external_receiver_type_key(receiver_type: &JavaType) -> Option<String> {
    match receiver_type {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            Some(canonical_external_receiver_type_key(name))
        }
        JavaType::String => Some("String".to_string()),
        JavaType::Array(_) => Some("Array".to_string()),
        JavaType::Primitive(primitive) => Some(format!("{primitive:?}")),
        JavaType::Void | JavaType::Wildcard(_) => None,
    }
}

fn is_boolean_external_method(name: &str, arg_count: usize) -> bool {
    is_java_bean_boolean_getter(name, arg_count)
        || is_contains_predicate(name, arg_count)
        || (arg_count == 0 && name.ends_with("Specified"))
        || matches!(
            (name, arg_count),
            ("isEmpty", 0)
                | ("hasNext", 0)
                | ("contains", 1)
                | ("containsAll", 1)
                | ("containsKey", 1)
                | ("containsValue", 1)
                | ("hasAttribute", 1)
                | ("startsWith", 1)
                | ("endsWith", 1)
                | ("matches", 1)
                | ("allMatch", 1)
                | ("anyMatch", 1)
                | ("noneMatch", 1)
                | ("isSet", 1)
                | ("isUnset", 1)
        )
}

fn is_java_bean_boolean_getter(name: &str, arg_count: usize) -> bool {
    arg_count == 0 && (has_camel_suffix(name, "is") || has_camel_suffix(name, "has"))
}

fn is_contains_predicate(name: &str, arg_count: usize) -> bool {
    arg_count == 1 && (name == "contains" || has_camel_suffix(name, "contains"))
}

fn has_camel_suffix(name: &str, prefix: &str) -> bool {
    let Some(suffix) = name.strip_prefix(prefix) else {
        return false;
    };
    suffix
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
}

fn is_string_domain_getter(name: &str) -> bool {
    matches!(
        name,
        "getContig"
            | "getChromosome"
            | "getSequenceName"
            | "getName"
            | "getID"
            | "getReadName"
            | "getSampleName"
            | "getScheme"
    )
}

fn is_zero_arg_string_named_getter(name: &str) -> bool {
    has_camel_suffix(name, "get") && name.ends_with("String")
}

fn is_integral_domain_getter(name: &str) -> bool {
    matches!(
        name,
        "getStart"
            | "getEnd"
            | "getPosition"
            | "getLength"
            | "getSequenceIndex"
            | "getReferenceIndex"
            | "getGQ"
            | "getLineNumber"
            | "getChromosomeIndex"
            | "getPeriod"
            | "getSequenceLength"
    )
}

fn is_long_domain_getter(name: &str) -> bool {
    matches!(name, "getMask")
}

fn is_byte_array_domain_getter(name: &str) -> bool {
    matches!(name, "getBases" | "getReadBases" | "getBaseQualities")
}

fn is_known_zero_arg_domain_return_getter(name: &str) -> bool {
    is_string_domain_getter(name)
        || is_integral_domain_getter(name)
        || is_long_domain_getter(name)
        || is_byte_array_domain_getter(name)
        || is_int_array_domain_getter(name)
        || is_allele_domain_getter(name)
        || is_allele_list_domain_getter(name)
        || is_string_list_domain_getter(name)
        || is_object_list_domain_getter(name)
        || is_sequence_dictionary_getter(name)
        || is_path_getter(name)
}

fn is_int_array_domain_getter(name: &str) -> bool {
    matches!(name, "getAD" | "getPL")
}

fn is_allele_domain_getter(name: &str) -> bool {
    matches!(name, "getReference")
}

fn is_allele_list_domain_getter(name: &str) -> bool {
    matches!(name, "getAlleles" | "getAlternateAlleles")
}

fn is_string_list_domain_getter(name: &str) -> bool {
    matches!(name, "getGenotypeSamples")
}

fn is_object_list_domain_getter(name: &str) -> bool {
    matches!(name, "getRecords")
}

pub fn canonical_external_receiver_type_key(name: &str) -> String {
    let name = name.trim();
    let name = strip_java_type_modifiers(name);
    match name {
        "string" => "String",
        "class" => "Class",
        "boolean" => "Boolean",
        "java.lang.Boolean" | "JavaLangBoolean" | "java_lang_boolean" => "Boolean",
        "byte" => "Byte",
        "java.lang.Byte" | "JavaLangByte" | "java_lang_byte" => "Byte",
        "short" => "Short",
        "java.lang.Short" | "JavaLangShort" | "java_lang_short" => "Short",
        "character" => "Character",
        "java.lang.Character" | "JavaLangCharacter" | "java_lang_character" => "Character",
        "integer" => "Integer",
        "java.lang.Integer" | "JavaLangInteger" | "java_lang_integer" => "Integer",
        "long" => "Long",
        "java.lang.Long" | "JavaLangLong" | "java_lang_long" => "Long",
        "float" => "Float",
        "java.lang.Float" | "JavaLangFloat" | "java_lang_float" => "Float",
        "double" => "Double",
        "java.lang.Double" | "JavaLangDouble" | "java_lang_double" => "Double",
        "string_utils" => "StringUtils",
        "string_util" => "StringUtil",
        "stream" => "Stream",
        "int_stream" => "IntStream",
        "long_stream" => "LongStream",
        "double_stream" => "DoubleStream",
        "list_utils" => "ListUtils",
        "listutils" => "ListUtils",
        "objects" => "Objects",
        "collections" => "Collections",
        "collectors" => "Collectors",
        "enum_set" => "EnumSet",
        "iterators" => "Iterators",
        "file" => "File",
        "files" => "Files",
        "math" => "Math",
        "strict_math" => "StrictMath",
        "system_utils" => "SystemUtils",
        "help_constants" => "HelpConstants",
        "defaults" => "Defaults",
        "retry_handler" => "RetryHandler",
        "sam_reader_factory" => "SamReaderFactory",
        "v_c_f_compound_header_line" => "VCFCompoundHeaderLine",
        "standard_open_option" => "StandardOpenOption",
        "access_mode" => "AccessMode",
        "posix_file_permission" => "PosixFilePermission",
        "arrays" => "Arrays",
        "math_arrays" => "MathArrays",
        "array_utils" => "ArrayUtils",
        "beta" => "Beta",
        "sets" => "Sets",
        "booleans" => "Booleans",
        "bytes" => "Bytes",
        "shorts" => "Shorts",
        "chars" => "Chars",
        "ints" => "Ints",
        "longs" => "Longs",
        "floats" => "Floats",
        "doubles" => "Doubles",
        "allele" => "Allele",
        "pair" => "Pair",
        "immutable_pair" => "ImmutablePair",
        "immutable_map" => "ImmutableMap",
        "immutable_sorted_map" => "ImmutableSortedMap",
        "text_cigar_codec" => "TextCigarCodec",
        "v_c_f_constants" => "VCFConstants",
        "constants" => "Constants",
        "file_extensions" => "FileExtensions",
        "s_a_m_file_header" => "SAMFileHeader",
        "field_value" => "FieldValue",
        "query_job_configuration" => "QueryJobConfiguration",
        "block_compressed_input_stream" => "BlockCompressedInputStream",
        "tar_archive_output_stream" => "TarArchiveOutputStream",
        "s_b_i_index_writer" => "SBIIndexWriter",
        "bwa_mem_aligner" => "BwaMemAligner",
        "spliterator" => "Spliterator",
        "percentile" => "Percentile",
        "log" => "Log",
        "byte_order" => "ByteOrder",
        "open_mode" => "OpenMode",
        "log_level" => "LogLevel",
        "sort_order" => "SortOrder",
        "group_order" => "GroupOrder",
        "attribute" => "Attribute",
        "priority" => "Priority",
        "file_termination" => "FileTermination",
        "structural_variant_type" => "StructuralVariantType",
        "goal_type" => "GoalType",
        "estimation_type" => "EstimationType",
        "read_ends" => "ReadEnds",
        "cigar_operator" => "CigarOperator",
        "s_a_m_tag" => "SAMTag",
        "s_a_m_record" => "SAMRecord",
        "type" => "Type",
        "s_a_m_flag" => "SAMFlag",
        "v_c_f_header_version" => "VCFHeaderVersion",
        "mark_duplicates" => "MarkDuplicates",
        "duplicate_tagging_policy" => "DuplicateTaggingPolicy",
        "sam_pair_util" => "SamPairUtil",
        "pair_orientation" => "PairOrientation",
        "genotype_likelihoods" => "GenotypeLikelihoods",
        "common_info" => "CommonInfo",
        "read_utils" => "ReadUtils",
        "block_compressed_stream_constants" => "BlockCompressedStreamConstants",
        "coordinates" => "Coordinates",
        "contig_position" => "ContigPosition",
        "jexl_v_c_match_exp" => "JexlVCMatchExp",
        "s_a_m_tag_and_value" => "SAMTagAndValue",
        "batch_completion_callback_function_argument" => "BatchCompletionCallbackFunctionArgument",
        "v_c_f_header_line_type" => "VCFHeaderLineType",
        "v_c_f_header_line_count" => "VCFHeaderLineCount",
        "genotype_type" => "GenotypeType",
        "strand" => "Strand",
        "characteristics" => "Characteristics",
        "options" => "Options",
        "s_w_overhang_strategy" => "SWOverhangStrategy",
        "genotype_concordance_states" => "GenotypeConcordanceStates",
        "call_state" => "CallState",
        "truth_state" => "TruthState",
        "system" => "System",
        "thread" => "Thread",
        "standard_charsets" => "StandardCharsets",
        "pattern" => "Pattern",
        "rounding_mode" => "RoundingMode",
        "duplication_metrics" => "DuplicationMetrics",
        "template_properties" => "TemplateProperties",
        "reads_format_write_option" => "ReadsFormatWriteOption",
        "s_a_m_utils" => "SAMUtils",
        "s_a_m_sequence_record" => "SAMSequenceRecord",
        "genotype_concordance_summary_metrics" => "GenotypeConcordanceSummaryMetrics",
        "v_c_f_header" => "VCFHeader",
        "time_unit" => "TimeUnit",
        "output_type" => "OutputType",
        "reserved_tag_constants" => "ReservedTagConstants",
        "sequence_util" => "SequenceUtil",
        "b_a_m_file_constants" => "BAMFileConstants",
        "g_a_t_k_config" => "GATKConfig",
        "cloud_storage_file_system" => "CloudStorageFileSystem",
        "reference_context" => "ReferenceContext",
        "http_file_system_provider" => "HttpFileSystemProvider",
        "https_file_system_provider" => "HttpsFileSystemProvider",
        "slice" => "Slice",
        "tabix_format" => "TabixFormat",
        "option" => "Option",
        "supported_header_line_type" => "SupportedHeaderLineType",
        "genotype_concordance" => "GenotypeConcordance",
        "http_file_system_provider_settings" => "HttpFileSystemProviderSettings",
        "level" => "Level",
        "java.util.logging.Level" | "JavaUtilLoggingLevel" | "java_util_logging_level" => "Level",
        "locale" => "Locale",
        "java.util.Locale" | "JavaUtilLocale" | "java_util_locale" => "Locale",
        "fastq_constants" => "FastqConstants",
        "element_kind" => "ElementKind",
        "export_configuration" => "ExportConfiguration",
        "read_likelihood_calculation_engine" => "ReadLikelihoodCalculationEngine",
        "special_arguments_collection" => "SpecialArgumentsCollection",
        "http_method" => "HttpMethod",
        "data_format" => "DataFormat",
        "data_series_type" => "DataSeriesType",
        "cloud_storage_configuration" => "CloudStorageConfiguration",
        "null_output_stream" => "NullOutputStream",
        "null_writer" => "NullWriter",
        "validation_stringency" => "ValidationStringency",
        "index_factory" => "IndexFactory",
        "file_cardinality_write_option" => "FileCardinalityWriteOption",
        "genotypes_context" => "GenotypesContext",
        "message_digest_algorithms" => "MessageDigestAlgorithms",
        "s_a_m_read_group_record" => "SAMReadGroupRecord",
        "deserialization_feature" => "DeserializationFeature",
        "jexl_missing_value_treatment" => "JexlMissingValueTreatment",
        "w_d_l_work_unit_handler" => "WDLWorkUnitHandler",
        "genotype" => "Genotype",
        "vid_mapping_p_b" => "VidMappingPB",
        "field_length_descriptor_component_p_b" => "FieldLengthDescriptorComponentPB",
        "read_session" => "ReadSession",
        "table_read_options" => "TableReadOptions",
        "import_configuration" => "ImportConfiguration",
        "partition" => "Partition",
        "storage" => "Storage",
        "sign_url_option" => "SignUrlOption",
        "index_type" => "IndexType",
        "index_balance_approach" => "IndexBalanceApproach",
        "s_a_m_binary_tag_and_value" => "SAMBinaryTagAndValue",
        "read_data_holder" => "ReadDataHolder",
        "haplotype_data_holder" => "HaplotypeDataHolder",
        "genotype_likelihoods_allele_pair" => "GenotypeLikelihoodsAllelePair",
        "s_w_native_aligner_result" => "SWNativeAlignerResult",
        "p_d_h_m_m_native_arguments" => "PDHMMNativeArguments",
        "pair_h_m_m_native_arguments" => "PairHMMNativeArguments",
        "a_v_x_level" => "AVXLevel",
        "open_m_p_setting" => "OpenMPSetting",
        other => other,
    }
    .to_string()
}

fn is_known_java_platform_output_stream_receiver(receiver: &str) -> bool {
    let canonical = canonical_external_receiver_type_key(receiver);
    let simple = canonical.rsplit('.').next().unwrap_or(&canonical);
    matches!(
        simple,
        "OutputStream"
            | "FileOutputStream"
            | "ByteArrayOutputStream"
            | "BufferedOutputStream"
            | "DataOutputStream"
            | "GZIPOutputStream"
            | "ZipOutputStream"
            | "ObjectOutputStream"
            | "FilterOutputStream"
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassLiteralTokenPlan {
    pub java_name: String,
    pub rust_type: String,
}

pub fn class_literal_token_plan(ty: &JavaType, facts: &FactSet) -> ClassLiteralTokenPlan {
    let shape = RustSlotShape::conservative(ty.clone());
    ClassLiteralTokenPlan {
        java_name: class_literal_java_name(ty),
        rust_type: crate::emit::emit_type(&rust_type_for_slot_with_facts(&shape, facts)),
    }
}

pub fn class_literal_java_name(ty: &JavaType) -> String {
    match ty {
        JavaType::Void => "void".to_string(),
        JavaType::String => "String".to_string(),
        JavaType::Primitive(primitive) => match primitive {
            PrimitiveType::Boolean => "boolean",
            PrimitiveType::Byte => "byte",
            PrimitiveType::Short => "short",
            PrimitiveType::Char => "char",
            PrimitiveType::Int => "int",
            PrimitiveType::Long => "long",
            PrimitiveType::Float => "float",
            PrimitiveType::Double => "double",
        }
        .to_string(),
        JavaType::Array(element) => format!("{}[]", class_literal_java_name(element)),
        JavaType::Named { name, args } if args.is_empty() => name.clone(),
        JavaType::Named { name, args } => {
            let args = args
                .iter()
                .map(class_literal_java_name)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{name}<{args}>")
        }
        JavaType::Wildcard(Some(WildcardBound::Extends(bound)))
        | JavaType::Wildcard(Some(WildcardBound::Super(bound))) => class_literal_java_name(bound),
        JavaType::Wildcard(None) => "?".to_string(),
        JavaType::Unknown(name) => name.clone(),
    }
}

pub fn java_class_support_item() -> crate::rust_ir::RustItem {
    java_class_support_item_with_classloader_shim(false)
}

pub fn java_class_support_item_with_classloader_shim(
    include_classloader_shim: bool,
) -> crate::rust_ir::RustItem {
    let classloader_shim = if include_classloader_shim {
        r#"

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct URL {
    value: String,
}

impl URL {
    pub fn new<S: Into<String>>(value: S) -> URL {
        URL { value: value.into() }
    }

    pub fn get_file(&self) -> String {
        self.value.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ClassLoader;

impl ClassLoader {
    pub fn get_system_resource<S: Into<String>>(path: S) -> URL {
        URL::new(path)
    }

    pub fn get_resource<S: Into<String>>(&self, path: S) -> URL {
        URL::new(path)
    }

    pub fn get_resource_as_stream<S: Into<String>>(&self, _path: S) -> Option<InputStream> {
        Some(InputStream)
    }
}"#
    } else {
        ""
    };
    let mut support = r#"#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaClass<T> {
    name: String,
    _marker: std::marker::PhantomData<T>,
}

impl<T> JavaClass<T> {
    pub fn literal(name: &'static str) -> JavaClass<T> {
        JavaClass { name: name.to_string(), _marker: std::marker::PhantomData }
    }

    pub fn for_name<S: Into<String>>(name: S) -> JavaClass<T> {
        JavaClass { name: name.into(), _marker: std::marker::PhantomData }
    }

    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    pub fn get_simple_name(&self) -> String {
        self.name.rsplit('.').next().unwrap_or(&self.name).to_string()
    }

    pub fn is_array(&self) -> bool {
        false
    }

    pub fn get_resource_as_stream<S: Into<String>>(&self, _path: S) -> Option<InputStream> {
        Some(InputStream)
    }

    pub fn get_class_loader(&self) -> ClassLoader {
        ClassLoader
    }

    pub fn equals<U>(&self, other: JavaClass<U>) -> bool {
        self.name == other.name
    }
}"#
    .to_string();
    support.push_str(classloader_shim);
    crate::rust_ir::RustItem::Raw(support)
}

pub fn java_platform_facade_support_item() -> crate::rust_ir::RustItem {
    crate::rust_ir::RustItem::Raw(
        r#"#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct URL {
    value: String,
}

impl URL {
    pub fn new<S: Into<String>>(value: S) -> URL {
        URL { value: value.into() }
    }

    pub fn get_file(&self) -> String {
        self.value.clone()
    }

    pub fn open_connection<T: Default>(&self) -> Result<T, JavaException> {
        Ok(T::default())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct URLConnection;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct JarURLConnection;

impl JarURLConnection {
    pub fn get_jar_file(&self) -> Result<JarFile, JavaException> {
        Ok(JarFile)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct JarFile;

impl JarFile {
    pub fn entries(&self) -> Result<Enumeration, JavaException> {
        Ok(Enumeration)
    }

    pub fn get_manifest(&self) -> Result<Manifest, JavaException> {
        Ok(Manifest)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct JarEntry;

impl JarEntry {
    pub fn is_directory(&self) -> bool {
        false
    }

    pub fn get_name(&self) -> String {
        String::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Manifest;

impl Manifest {
    pub fn get_main_attributes(&self) -> Attributes {
        Attributes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Attributes;

impl Attributes {
    pub fn get_value<T>(&self, _name: T) -> String {
        String::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct AttributesName {
    value: String,
}

impl AttributesName {
    pub fn new<S: Into<String>>(value: S) -> AttributesName {
        AttributesName { value: value.into() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Enumeration;

impl Enumeration {
    pub fn has_more_elements(&self) -> Result<bool, JavaException> {
        Ok(false)
    }

    pub fn next_element<T: Default>(&self) -> Result<T, JavaException> {
        Ok(T::default())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct JavaIterator<T> {
    items: Vec<T>,
    index: usize,
}

impl<T: Clone> JavaIterator<T> {
    pub fn new<I: IntoIterator<Item = T>>(items: I) -> JavaIterator<T> {
        JavaIterator { items: items.into_iter().collect(), index: 0 }
    }

    pub fn has_next(&self) -> bool {
        self.index < self.items.len()
    }

    pub fn next(&mut self) -> T {
        let value = self.items.get(self.index).expect("Java iterator exhausted").clone();
        self.index += 1;
        value
    }

    pub fn for_each_remaining<F>(&mut self, mut f: F)
    where
        F: FnMut(T),
    {
        while self.has_next() {
            f(self.next());
        }
    }
}

#[derive(Debug, Default)]
pub struct AtomicInteger {
    value: std::cell::Cell<i32>,
}

impl AtomicInteger {
    pub fn new(value: i32) -> AtomicInteger {
        AtomicInteger { value: std::cell::Cell::new(value) }
    }

    pub fn get(&self) -> i32 {
        self.value.get()
    }

    pub fn set(&self, value: i32) {
        self.value.set(value);
    }

    pub fn increment_and_get(&self) -> i32 {
        let value = self.value.get().wrapping_add(1);
        self.value.set(value);
        value
    }

    pub fn get_and_increment(&self) -> i32 {
        let value = self.value.get();
        self.value.set(value.wrapping_add(1));
        value
    }
}

impl Clone for AtomicInteger {
    fn clone(&self) -> Self {
        AtomicInteger::new(self.value.get())
    }
}

impl PartialEq for AtomicInteger {
    fn eq(&self, other: &Self) -> bool {
        self.value.get() == other.value.get()
    }
}

impl Eq for AtomicInteger {}

impl std::hash::Hash for AtomicInteger {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.value.get(), state);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ClassLoader;

impl ClassLoader {
    pub fn get_system_resource<S: Into<String>>(path: S) -> URL {
        URL::new(path)
    }

    pub fn get_resource<S: Into<String>>(&self, path: S) -> URL {
        URL::new(path)
    }

    pub fn get_resource_as_stream<S: Into<String>>(&self, _path: S) -> Option<InputStream> {
        Some(InputStream)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct FileSystem {}

impl FileSystem {
    pub fn delete<T0>(&self, _path: T0, _recursive: bool) -> bool {
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Graphics;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Graphics2D;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct JavaAwtGraphics;

impl Graphics {
    pub fn dispose(&self) {}
    pub fn draw_image<T0, T1, T2, T3>(&self, _image: T0, _x: T1, _y: T2, _observer: T3) -> bool {
        true
    }
    pub fn get_clip<R: Default>(&self) -> R {
        Default::default()
    }
    pub fn get_clip_bounds<R: Default>(&self) -> R {
        Default::default()
    }
    pub fn get_font_metrics<R: Default, T0>(&self, _font: T0) -> R {
        Default::default()
    }
}

impl Graphics2D {
    pub fn dispose(&self) {}
    pub fn draw_image<T0, T1, T2, T3>(&self, _image: T0, _x: T1, _y: T2, _observer: T3) -> bool {
        true
    }
    pub fn set_stroke<T0>(&self, _stroke: T0) {}
    pub fn set_rendering_hint<T0, T1>(&self, _key: T0, _value: T1) {}
}

impl From<Graphics2D> for Graphics {
    fn from(_value: Graphics2D) -> Self {
        Default::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct BufferedImage;

impl BufferedImage {
    pub const TYPE_INT_RGB: i32 = 1;
    pub const TYPE_INT_ARGB: i32 = 2;

    pub fn new<T0, T1, T2>(_width: T0, _height: T1, _image_type: T2) -> Self {
        Self
    }

    pub fn get_graphics(&self) -> Graphics {
        Graphics
    }

    pub fn create_graphics(&self) -> Graphics2D {
        Graphics2D
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Path {
    value: String,
}

impl Path {
    pub fn new<P: JavaPathArg>(value: P) -> Path {
        Path { value: value.into_path_value() }
    }

    pub fn to_uri(&self) -> URI {
        URI::new(self.value.clone())
    }

    pub fn to_absolute_path(&self) -> Path {
        self.clone()
    }

    pub fn to_file(&self) -> File {
        File::new(self.value.clone())
    }

    pub fn resolve_sibling<P: JavaPathArg>(&self, sibling: P) -> Path {
        Path::new(sibling)
    }

    pub fn get_file_name(&self) -> String {
        self.value.rsplit('/').next().unwrap_or(&self.value).to_string()
    }

    pub fn get_file_system<T0>(&self, _arg0: T0) -> FileSystem {
        FileSystem {}
    }
}

impl std::fmt::Display for Path {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl AsRef<std::path::Path> for Path {
    fn as_ref(&self) -> &std::path::Path {
        std::path::Path::new(&self.value)
    }
}

impl From<Path> for std::path::PathBuf {
    fn from(path: Path) -> std::path::PathBuf {
        std::path::PathBuf::from(path.value)
    }
}

impl From<&Path> for std::path::PathBuf {
    fn from(path: &Path) -> std::path::PathBuf {
        std::path::PathBuf::from(path.value.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct URI {
    value: String,
}

impl URI {
    pub fn new<P: JavaPathArg>(value: P) -> URI {
        URI { value: value.into_path_value() }
    }

    pub fn parse<P: JavaPathArg>(value: P) -> Result<URI, JavaException> {
        let value = value.into_path_value();
        if value.trim().is_empty() {
            Err(JavaException::new("URISyntaxException", value))
        } else {
            Ok(URI { value })
        }
    }

    pub fn to_string(&self) -> String {
        self.value.clone()
    }

    pub fn get_raw_path(&self) -> String {
        self.value.clone()
    }
}

impl std::fmt::Display for URI {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct File {
    value: String,
}

impl File {
    pub fn new<P: JavaPathArg>(value: P) -> File {
        File { value: value.into_path_value() }
    }

    pub fn new_1<P: JavaPathArg>(value: P) -> File {
        Self::new(value)
    }

    pub fn new_2<P: JavaPathArg, C: JavaPathArg>(parent: P, child: C) -> File {
        let parent = parent.into_path_value();
        let child = child.into_path_value();
        if parent.is_empty() {
            Self::new(child)
        } else if parent.ends_with('/') {
            Self::new(format!("{parent}{child}"))
        } else {
            Self::new(format!("{parent}/{child}"))
        }
    }

    pub fn to_path(&self) -> Path {
        Path::new(self.value.clone())
    }

    pub fn get_path(&self) -> String {
        self.value.clone()
    }

    pub fn get_absolute_path(&self) -> String {
        self.value.clone()
    }

    pub fn get_name(&self) -> String {
        self.value
            .rsplit('/')
            .next()
            .unwrap_or(&self.value)
            .to_string()
    }

    pub fn get_parent(&self) -> String {
        self.value
            .rsplit_once('/')
            .map(|(parent, _)| parent.to_string())
            .unwrap_or_default()
    }

    pub fn get_absolute_file(&self) -> File {
        self.clone()
    }

    pub fn delete_on_exit(&self) {}

    pub fn is_directory(&self) -> bool {
        std::path::Path::new(&self.value).is_dir()
    }

    pub fn is_file(&self) -> bool {
        std::path::Path::new(&self.value).is_file()
    }

    pub fn can_read(&self) -> bool {
        std::fs::File::open(std::path::Path::new(&self.value)).is_ok()
    }

    pub fn length(&self) -> i64 {
        std::fs::metadata(std::path::Path::new(&self.value))
            .map(|metadata| metadata.len() as i64)
            .unwrap_or(0)
    }

    pub fn list(&self) -> Vec<String> {
        std::fs::read_dir(std::path::Path::new(&self.value))
            .map(|entries| {
                entries
                    .filter_map(|entry| {
                        entry
                            .ok()
                            .and_then(|entry| entry.file_name().into_string().ok())
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn mkdir(&self) -> bool {
        std::fs::create_dir(std::path::Path::new(&self.value)).is_ok()
    }
}

impl std::fmt::Display for File {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl AsRef<std::path::Path> for File {
    fn as_ref(&self) -> &std::path::Path {
        std::path::Path::new(&self.value)
    }
}

impl From<File> for std::path::PathBuf {
    fn from(file: File) -> std::path::PathBuf {
        std::path::PathBuf::from(file.value)
    }
}

impl From<&File> for std::path::PathBuf {
    fn from(file: &File) -> std::path::PathBuf {
        std::path::PathBuf::from(file.value.clone())
    }
}

pub trait JavaPathArg {
    fn into_path_value(self) -> String;
}

impl JavaPathArg for String {
    fn into_path_value(self) -> String {
        self
    }
}

impl JavaPathArg for &String {
    fn into_path_value(self) -> String {
        self.clone()
    }
}

impl JavaPathArg for &str {
    fn into_path_value(self) -> String {
        self.to_string()
    }
}

impl JavaPathArg for Option<String> {
    fn into_path_value(self) -> String {
        self.unwrap_or_default()
    }
}

impl JavaPathArg for &Option<String> {
    fn into_path_value(self) -> String {
        self.clone().unwrap_or_default()
    }
}

impl JavaPathArg for Path {
    fn into_path_value(self) -> String {
        self.value
    }
}

impl JavaPathArg for &Path {
    fn into_path_value(self) -> String {
        self.value.clone()
    }
}

impl JavaPathArg for Option<Path> {
    fn into_path_value(self) -> String {
        self.map(|path| path.value).unwrap_or_default()
    }
}

impl JavaPathArg for &Option<Path> {
    fn into_path_value(self) -> String {
        self.as_ref().map(|path| path.value.clone()).unwrap_or_default()
    }
}

impl JavaPathArg for URI {
    fn into_path_value(self) -> String {
        self.value
    }
}

impl JavaPathArg for &URI {
    fn into_path_value(self) -> String {
        self.value.clone()
    }
}

impl JavaPathArg for File {
    fn into_path_value(self) -> String {
        self.value
    }
}

impl JavaPathArg for &File {
    fn into_path_value(self) -> String {
        self.value.clone()
    }
}

pub mod logger {
    pub fn info<M: std::fmt::Display>(_message: M) {}
    pub fn warn<M: std::fmt::Display>(_message: M) {}
    pub fn debug<M: std::fmt::Display>(_message: M) {}
    pub fn error<M: std::fmt::Display>(_message: M) {}
    pub fn trace<M: std::fmt::Display>(_message: M) {}
}"#
        .to_string(),
    )
}

pub fn java_erased_object_runtime_support_item() -> crate::rust_ir::RustItem {
    crate::rust_ir::RustItem::Raw(
        r#"pub fn java_is_serializable<T: ?Sized>(_value: &T) -> bool {
    true
}

pub fn java_clone_erased_object_ref(_value: &Box<dyn std::any::Any>) -> Box<dyn std::any::Any> {
    Box::new(())
}

pub fn java_clone_erased_object_vec_ref(
    _value: &Vec<Box<dyn std::any::Any>>,
) -> Vec<Box<dyn std::any::Any>> {
    Vec::new()
}

pub fn java_erased_object_to_string<T: ?Sized>(_value: &T) -> String {
    Default::default()
}"#
        .to_string(),
    )
}

pub fn java_string_split_support_item() -> crate::rust_ir::RustItem {
    crate::rust_ir::RustItem::Raw(
        r#"pub fn java_string_substring<V: AsRef<str>>(value: V, begin: i32, end: i32) -> String {
    let value = value.as_ref();
    let begin = begin.max(0) as usize;
    let end = end.max(begin as i32) as usize;
    value
        .chars()
        .skip(begin)
        .take(end.saturating_sub(begin))
        .collect()
}

pub fn java_string_hash_code(value: &str) -> i32 {
    value
        .encode_utf16()
        .fold(0i32, |hash, unit| hash.wrapping_mul(31).wrapping_add(unit as i32))
}

pub fn java_list_hash_code<T>(value: &[T]) -> i32 {
    value.len() as i32
}

pub fn java_set_hash_code<T, S>(value: &std::collections::HashSet<T, S>) -> i32 {
    value.len() as i32
}

pub fn java_map_hash_code<K, V, S>(value: &std::collections::HashMap<K, V, S>) -> i32 {
    value.len() as i32
}

pub fn java_value_hash_code<T: std::hash::Hash + ?Sized>(value: &T) -> i32 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish() as i32
}

pub fn java_string_index_of<V: AsRef<str>, N: AsRef<str>>(value: V, needle: N) -> i32 {
    value
        .as_ref()
        .find(needle.as_ref())
        .map(|index| index as i32)
        .unwrap_or(-1)
}

pub fn java_string_index_of_from<V: AsRef<str>, N: AsRef<str>>(
    value: V,
    needle: N,
    from_index: i32,
) -> i32 {
    let value = value.as_ref();
    let needle = needle.as_ref();
    let from_index = from_index.max(0) as usize;
    if from_index >= value.len() {
        return if needle.is_empty() { value.len() as i32 } else { -1 };
    }
    value[from_index..]
        .find(needle)
        .map(|index| (from_index + index) as i32)
        .unwrap_or(-1)
}

pub fn java_string_last_index_of<V: AsRef<str>, N: AsRef<str>>(value: V, needle: N) -> i32 {
    value
        .as_ref()
        .rfind(needle.as_ref())
        .map(|index| index as i32)
        .unwrap_or(-1)
}

pub fn java_string_last_index_of_from<V: AsRef<str>, N: AsRef<str>>(
    value: V,
    needle: N,
    from_index: i32,
) -> i32 {
    let value = value.as_ref();
    let needle = needle.as_ref();
    if from_index < 0 {
        return -1;
    }
    if needle.is_empty() {
        return std::cmp::min(from_index as usize, value.len()) as i32;
    }
    let end = std::cmp::min((from_index as usize).saturating_add(needle.len()), value.len());
    value[..end]
        .rfind(needle)
        .map(|index| index as i32)
        .unwrap_or(-1)
}

pub fn java_string_compare_to<L: AsRef<str>, R: AsRef<str>>(left: L, right: R) -> i32 {
    match left.as_ref().cmp(right.as_ref()) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

pub fn java_string_compare_to_ignore_case<L: AsRef<str>, R: AsRef<str>>(
    left: L,
    right: R,
) -> i32 {
    java_string_compare_to(left.as_ref().to_lowercase(), right.as_ref().to_lowercase())
}

pub fn java_string_equals_ignore_case<L: AsRef<str>, R: AsRef<str>>(left: L, right: R) -> bool {
    left.as_ref().eq_ignore_ascii_case(right.as_ref())
}

pub fn java_primitive_compare_to<T: PartialOrd>(left: T, right: T) -> i32 {
    if left < right {
        -1
    } else if left > right {
        1
    } else {
        0
    }
}

pub fn java_math_floor_div_i32(left: i32, right: i32) -> i32 {
    let quotient = left / right;
    let remainder = left % right;
    if remainder != 0 && ((left < 0) != (right < 0)) {
        quotient - 1
    } else {
        quotient
    }
}

pub fn java_math_floor_mod_i32(left: i32, right: i32) -> i32 {
    left - java_math_floor_div_i32(left, right) * right
}

pub fn java_math_round_f32(value: f32) -> i32 {
    if value.is_nan() {
        0
    } else if value >= i32::MAX as f32 {
        i32::MAX
    } else if value <= i32::MIN as f32 {
        i32::MIN
    } else {
        (value + 0.5).floor() as i32
    }
}

pub fn java_math_round_f64(value: f64) -> i64 {
    if value.is_nan() {
        0
    } else if value >= i64::MAX as f64 {
        i64::MAX
    } else if value <= i64::MIN as f64 {
        i64::MIN
    } else {
        (value + 0.5).floor() as i64
    }
}

pub fn java_math_max_f32(left: f32, right: f32) -> f32 {
    if left.is_nan() || right.is_nan() {
        f32::NAN
    } else if left == 0.0 && right == 0.0 {
        if left.is_sign_positive() || right.is_sign_positive() {
            0.0
        } else {
            -0.0
        }
    } else {
        left.max(right)
    }
}

pub fn java_math_max_f64(left: f64, right: f64) -> f64 {
    if left.is_nan() || right.is_nan() {
        f64::NAN
    } else if left == 0.0 && right == 0.0 {
        if left.is_sign_positive() || right.is_sign_positive() {
            0.0
        } else {
            -0.0
        }
    } else {
        left.max(right)
    }
}

pub fn java_math_min_f32(left: f32, right: f32) -> f32 {
    if left.is_nan() || right.is_nan() {
        f32::NAN
    } else if left == 0.0 && right == 0.0 {
        if left.is_sign_negative() || right.is_sign_negative() {
            -0.0
        } else {
            0.0
        }
    } else {
        left.min(right)
    }
}

pub fn java_math_min_f64(left: f64, right: f64) -> f64 {
    if left.is_nan() || right.is_nan() {
        f64::NAN
    } else if left == 0.0 && right == 0.0 {
        if left.is_sign_negative() || right.is_sign_negative() {
            -0.0
        } else {
            0.0
        }
    } else {
        left.min(right)
    }
}

pub fn java_parse_i8<V: AsRef<str>>(value: V) -> i8 {
    value.as_ref().parse::<i8>().expect("Java number format")
}

pub fn java_parse_i16<V: AsRef<str>>(value: V) -> i16 {
    value.as_ref().parse::<i16>().expect("Java number format")
}

pub fn java_parse_i32<V: AsRef<str>>(value: V) -> i32 {
    value.as_ref().parse::<i32>().expect("Java number format")
}

pub fn java_parse_i64<V: AsRef<str>>(value: V) -> i64 {
    value.as_ref().parse::<i64>().expect("Java number format")
}

pub fn java_parse_f32<V: AsRef<str>>(value: V) -> f32 {
    value.as_ref().parse::<f32>().expect("Java number format")
}

pub fn java_parse_f64<V: AsRef<str>>(value: V) -> f64 {
    value.as_ref().parse::<f64>().expect("Java number format")
}

pub fn java_string_from_byte_range(bytes: &[i8], offset: i32, len: i32) -> String {
    let start = offset.max(0) as usize;
    let end = start.saturating_add(len.max(0) as usize).min(bytes.len());
    let bytes = bytes[start..end]
        .iter()
        .map(|byte| *byte as u8)
        .collect::<Vec<_>>();
    String::from_utf8_lossy(&bytes).into_owned()
}

pub fn java_string_from_bytes(bytes: &[i8]) -> String {
    let bytes = bytes.iter().map(|byte| *byte as u8).collect::<Vec<_>>();
    String::from_utf8_lossy(&bytes).into_owned()
}

pub fn java_string_split<R: AsRef<str>>(value: &str, regex: R, limit: i32) -> Vec<String> {
    let regex = regex.as_ref();
    let split_whitespace = regex == "\\s+";
    let delimiter = match regex {
        "\\t" | "\\\\t" => "\t",
        "\\n" | "\\\\n" => "\n",
        "\\r" | "\\\\r" => "\r",
        "\\s+" => " ",
        other => other,
    };
    let mut parts = if split_whitespace {
        value.split_whitespace().map(|part| part.to_string()).collect::<Vec<_>>()
    } else {
        value.split(delimiter).map(|part| part.to_string()).collect::<Vec<_>>()
    };
    if limit == 0 {
        while parts.last().is_some_and(|part| part.is_empty()) {
            parts.pop();
        }
    } else if limit > 0 && parts.len() > limit as usize {
        let keep = limit as usize - 1;
        let tail = parts.split_off(keep).join(delimiter);
        parts.push(tail);
    }
    parts
}

pub fn java_string_matches<R: AsRef<str>>(value: &str, regex: R) -> bool {
    match regex.as_ref() {
        "[0-9]" => value.chars().count() == 1 && value.chars().all(|ch| ch.is_ascii_digit()),
        "\\d+" | "[0-9]+" => !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()),
        "\\s+" => !value.is_empty() && value.chars().all(|ch| ch.is_whitespace()),
        regex => value == regex,
    }
}

pub fn java_string_replace_first<R: AsRef<str>, T: AsRef<str>>(
    value: &str,
    regex: R,
    replacement: T,
) -> String {
    let regex = regex.as_ref();
    let replacement = replacement.as_ref();
    value.replacen(java_simple_regex_literal(regex), replacement, 1)
}

pub fn java_string_replace_all<R: AsRef<str>, T: AsRef<str>>(
    value: &str,
    regex: R,
    replacement: T,
) -> String {
    let regex = regex.as_ref();
    let replacement = replacement.as_ref();
    match regex {
        "[^0-9]" => value
            .chars()
            .fold(String::new(), |mut out, ch| {
                if ch.is_ascii_digit() {
                    out.push(ch);
                } else {
                    out.push_str(replacement);
                }
                out
            }),
        "\\s+" => value.split_whitespace().collect::<Vec<_>>().join(replacement),
        regex => value.replace(java_simple_regex_literal(regex), replacement),
    }
}

fn java_simple_regex_literal(regex: &str) -> &str {
    match regex {
        "\\t" | "\\\\t" => "\t",
        "\\n" | "\\\\n" => "\n",
        "\\r" | "\\\\r" => "\r",
        other => other,
    }
}"#
        .to_string(),
    )
}

pub fn java_string_format_support_item() -> crate::rust_ir::RustItem {
    crate::rust_ir::RustItem::Raw(
        r#"pub fn java_string_format<S: AsRef<str>>(template: S, args: Vec<String>) -> String {
    let mut out = String::new();
    let mut chars = template.as_ref().chars().peekable();
    let mut arg_index = 0usize;
    while let Some(ch) = chars.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }
        if matches!(chars.peek(), Some('%')) {
            chars.next();
            out.push('%');
            continue;
        }
        while let Some(next) = chars.next() {
            if next.is_ascii_alphabetic() {
                if matches!(next, 's' | 'd' | 'b' | 'c' | 'f') {
                    if let Some(value) = args.get(arg_index) {
                        out.push_str(value);
                    }
                    arg_index += 1;
                } else {
                    out.push('%');
                    out.push(next);
                }
                break;
            }
        }
    }
    out
}"#
        .to_string(),
    )
}

pub fn external_api_facade_support_items(
    program: &Program,
    facts: &FactSet,
) -> Vec<crate::rust_ir::RustItem> {
    external_api_facade_support_items_with_local_type_program(
        program,
        facts,
        program,
        &Program::default(),
    )
}

pub fn external_api_facade_support_items_with_local_type_program(
    program: &Program,
    facts: &FactSet,
    local_type_program: &Program,
    source_summary_type_program: &Program,
) -> Vec<crate::rust_ir::RustItem> {
    let local_types = local_type_keys_for_program(local_type_program);
    let emitted_local_types = local_type_keys_for_program(program);
    let declared_external_type_exclusions = local_types
        .union(&emitted_local_types)
        .cloned()
        .collect::<BTreeSet<_>>();
    let source_summary_types = local_type_keys_for_program(source_summary_type_program);
    let used_calls = collect_program_call_arities(program);
    let used_constructors = collect_program_constructor_arities(program);
    let display_return_hints = collect_program_display_return_hints(program);
    let imported_static_owner_calls = collect_imported_static_owner_calls(program);

    let mut support = ExternalApiFacadeSupport::default();
    support.add_emitted_source_method_signatures(local_type_program);
    if !std::ptr::eq(program, local_type_program) {
        support.add_emitted_source_method_signatures(program);
    }
    support.add_nested_type_aliases_from_fields(facts, &local_types);
    support.add_declared_external_types(program, &declared_external_type_exclusions);
    support.add_declared_source_summary_types(program, &source_summary_types, &emitted_local_types);
    while support.add_source_summary_method_surfaces(
        facts,
        source_summary_type_program,
        &source_summary_types,
        &emitted_local_types,
        &used_calls,
    ) {}
    support.add_emitted_source_summary_super_method_surfaces(
        facts,
        program,
        source_summary_type_program,
        &source_summary_types,
        &used_calls,
    );
    support.add_emitted_source_summary_super_static_constants(
        program,
        source_summary_type_program,
        &source_summary_types,
    );
    support.add_emitted_external_super_method_surfaces(facts, program, &used_calls);
    support.add_inherited_support_field_types(program, local_type_program, &emitted_local_types);
    support.add_external_inherited_field_types(program, facts);
    support.add_external_inherited_static_final_surfaces(program, facts);
    support.add_used_external_constructors(facts, &used_constructors, &local_types);
    support.add_used_external_api_facts(
        facts,
        &used_calls,
        &local_types,
        &source_summary_types,
        &emitted_local_types,
        &imported_static_owner_calls,
    );
    support.add_builtin_used_external_api_facts(
        facts,
        &used_calls,
        &local_types,
        &source_summary_types,
        &emitted_local_types,
        &imported_static_owner_calls,
    );
    while support.add_source_summary_method_surfaces(
        facts,
        source_summary_type_program,
        &source_summary_types,
        &emitted_local_types,
        &used_calls,
    ) {}
    support.add_emitted_source_summary_super_method_surfaces(
        facts,
        program,
        source_summary_type_program,
        &source_summary_types,
        &used_calls,
    );
    support.add_emitted_source_summary_super_static_constants(
        program,
        source_summary_type_program,
        &source_summary_types,
    );
    support.add_emitted_external_super_method_surfaces(facts, program, &used_calls);
    support.add_imported_static_owner_calls(facts, &imported_static_owner_calls, &local_types);
    support.add_known_jdk_static_support_from_used_calls(&used_calls);
    support.add_known_domain_getter_surfaces_from_used_calls(facts, &used_calls);
    support.add_known_object_inherited_surfaces_from_used_calls(facts, &used_calls);
    support.add_builder_surface_from_used_calls(facts, &used_calls);
    support.add_display_context_return_hint_surfaces(facts, &display_return_hints);
    support.add_external_field_surfaces(facts, &local_types);
    support.add_static_external_api_facts_for_surface_types(facts);
    support.add_external_dyn_trait_surfaces(facts, &declared_external_type_exclusions);
    support.add_external_conversion_surfaces(facts, &local_types);
    support.suppress_rust_crate_mapped_external_types(facts);
    support.suppress_emitted_java_enum_types(program);
    support.into_items()
}

fn local_type_keys_for_program(program: &Program) -> BTreeSet<String> {
    let mut types = BTreeSet::new();
    for ty in &program.types {
        collect_local_type_keys(ty, None, &mut types);
    }
    types
}

fn collect_local_type_keys(
    ty: &crate::java_ir::TypeDecl,
    enclosing: Option<&str>,
    types: &mut BTreeSet<String>,
) {
    let qualified = enclosing
        .map(|outer| format!("{outer}.{}", ty.name))
        .unwrap_or_else(|| ty.name.clone());
    let type_key = canonical_external_receiver_type_key(&ty.name);
    types.insert(type_key.clone());
    types.insert(rust_name_for_java_named_type(&type_key));
    let qualified_key = canonical_external_receiver_type_key(&qualified);
    types.insert(qualified_key.clone());
    types.insert(rust_name_for_java_named_type(&qualified_key));
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_local_type_keys(nested, Some(&qualified), types);
        }
    }
}

fn emitted_java_enum_rust_type_names(program: &Program) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for ty in &program.types {
        collect_emitted_java_enum_rust_type_names(ty, None, &mut names);
    }
    names
}

fn collect_emitted_java_enum_rust_type_names(
    ty: &TypeDecl,
    enclosing: Option<&str>,
    names: &mut BTreeSet<String>,
) {
    let qualified = enclosing
        .map(|outer| format!("{outer}.{}", ty.name))
        .unwrap_or_else(|| ty.name.clone());
    if ty.kind == TypeKind::Enum {
        names.insert(rust_name_for_java_named_type(&ty.name));
        names.insert(rust_name_for_java_named_type(
            ty.name.rsplit(['.', '$']).next().unwrap_or(&ty.name),
        ));
        names.insert(rust_name_for_java_named_type(&qualified));
    }
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_emitted_java_enum_rust_type_names(nested, Some(&qualified), names);
        }
    }
}

fn find_program_type_by_key<'a>(program: &'a Program, type_key: &str) -> Option<&'a TypeDecl> {
    program
        .types
        .iter()
        .find_map(|ty| find_type_by_key_in_type(ty, type_key))
}

fn find_type_by_key_in_type<'a>(ty: &'a TypeDecl, type_key: &str) -> Option<&'a TypeDecl> {
    if canonical_external_receiver_type_key(&ty.name) == type_key {
        return Some(ty);
    }
    ty.members.iter().find_map(|member| match member {
        Member::Type(nested) => find_type_by_key_in_type(nested, type_key),
        _ => None,
    })
}

pub fn source_summary_method_return_nullability(java_type: &JavaType) -> NullabilityState {
    match java_type {
        JavaType::Void
        | JavaType::Primitive(_)
        | JavaType::Named { .. }
        | JavaType::String
        | JavaType::Array(_) => NullabilityState::DefinitelyNonNull,
        JavaType::Wildcard(_) | JavaType::Unknown(_) => NullabilityState::MaybeNull,
    }
}

fn source_summary_method_effective_return_nullability(
    facts: &FactSet,
    type_name: &str,
    method: &MethodDecl,
) -> NullabilityState {
    if let Some(effect) = facts.method_effect_for(type_name, &method.name)
        && effect.return_nullability != NullabilityState::Unknown
    {
        return effect.return_nullability;
    }
    crate::signature::source_method_return_nullability_constraints(method)
        .into_iter()
        .find_map(|constraint| match constraint {
            crate::signature::SignatureConstraint::RequiredStorageNullability(state) => Some(state),
            _ => None,
        })
        .unwrap_or_else(|| source_summary_method_return_nullability(&method.return_type))
}

fn source_summary_method_hierarchy<'a>(
    ty: &'a TypeDecl,
    program: &'a Program,
) -> Vec<&'a MethodDecl> {
    let mut methods = Vec::new();
    let mut current = Some(ty);
    let mut visited = BTreeSet::new();
    while let Some(ty) = current {
        let type_key = canonical_external_receiver_type_key(&ty.name);
        if !visited.insert(type_key) {
            break;
        }
        methods.extend(ty.members.iter().filter_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        }));
        current = ty
            .extends
            .as_deref()
            .map(canonical_external_receiver_type_key)
            .and_then(|super_key| find_program_type_by_key(program, &super_key));
    }
    methods
}

fn source_summary_static_final_field_hierarchy<'a>(
    ty: &'a TypeDecl,
    program: &'a Program,
) -> Vec<&'a FieldDecl> {
    let mut fields = Vec::new();
    let mut current = Some(ty);
    let mut visited = BTreeSet::new();
    while let Some(ty) = current {
        let type_key = canonical_external_receiver_type_key(&ty.name);
        if !visited.insert(type_key) {
            break;
        }
        fields.extend(ty.members.iter().filter_map(|member| match member {
            Member::Field(field)
                if field.modifiers.contains(&Modifier::Static)
                    && field.modifiers.contains(&Modifier::Final) =>
            {
                Some(field)
            }
            _ => None,
        }));
        current = ty
            .extends
            .as_deref()
            .map(canonical_external_receiver_type_key)
            .and_then(|super_key| find_program_type_by_key(program, &super_key));
    }
    fields
}

fn source_summary_supertypes_for_emitted_type<'a>(
    ty: &'a TypeDecl,
    emitted_program: &'a Program,
    source_summary_program: &'a Program,
    source_summary_types: &BTreeSet<String>,
) -> Vec<&'a TypeDecl> {
    let mut out = Vec::new();
    let mut current = ty
        .extends
        .as_deref()
        .map(canonical_external_receiver_type_key);
    let mut visited = BTreeSet::new();
    while let Some(type_key) = current {
        if !visited.insert(type_key.clone()) {
            break;
        }
        if source_summary_types.contains(&type_key) {
            if let Some(summary_ty) = find_program_type_by_key(source_summary_program, &type_key) {
                out.push(summary_ty);
                current = summary_ty
                    .extends
                    .as_deref()
                    .map(canonical_external_receiver_type_key);
            } else {
                break;
            }
        } else if let Some(local_ty) = find_program_type_by_key(emitted_program, &type_key) {
            current = local_ty
                .extends
                .as_deref()
                .map(canonical_external_receiver_type_key);
        } else {
            break;
        }
    }
    out
}

fn source_method_hierarchy_contains(
    program: &Program,
    ty: &TypeDecl,
    method_name: &str,
    arity: usize,
) -> bool {
    let mut current = Some(ty);
    let mut visited = BTreeSet::new();
    while let Some(ty) = current {
        let type_key = canonical_external_receiver_type_key(&ty.name);
        if !visited.insert(type_key) {
            break;
        }
        if ty.members.iter().any(|member| {
            matches!(
                member,
                Member::Method(method)
                    if method.name == method_name && method.params.len() == arity
            )
        }) {
            return true;
        }
        current = ty
            .extends
            .as_deref()
            .map(canonical_external_receiver_type_key)
            .and_then(|super_key| find_program_type_by_key(program, &super_key));
    }
    false
}

fn source_summary_inherited_method_will_be_lowered(
    ty: &TypeDecl,
    emitted_program: &Program,
    source_summary_program: &Program,
    source_summary_types: &BTreeSet<String>,
    method_name: &str,
    arity: usize,
) -> bool {
    source_summary_supertypes_for_emitted_type(
        ty,
        emitted_program,
        source_summary_program,
        source_summary_types,
    )
    .into_iter()
    .any(|summary_super| {
        source_summary_method_hierarchy(summary_super, source_summary_program)
            .into_iter()
            .any(|method| method.name == method_name && method.params.len() == arity)
    })
}

fn source_static_final_field_names(ty: &TypeDecl) -> BTreeSet<String> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field)
                if field.modifiers.contains(&crate::java_ir::Modifier::Static)
                    && field.modifiers.contains(&crate::java_ir::Modifier::Final) =>
            {
                Some(field.name.clone())
            }
            _ => None,
        })
        .collect()
}

#[derive(Default)]
struct ExternalApiFacadeSupport {
    opaque_types: BTreeSet<String>,
    trait_type_names: BTreeSet<String>,
    traits: BTreeMap<String, ExternalFacadeTrait>,
    trait_impls: BTreeMap<String, BTreeSet<String>>,
    aliases: BTreeMap<String, String>,
    fields: BTreeMap<String, BTreeMap<String, String>>,
    impls: BTreeMap<String, BTreeSet<String>>,
    from_impls: BTreeSet<(String, String)>,
    erased_object_methods: BTreeMap<String, ExternalErasedObjectMethod>,
    erased_object_method_conflicts: BTreeSet<String>,
    emitted_source_methods: BTreeSet<(String, String, usize)>,
}

fn is_awt_platform_facade_owner(owner: &str) -> bool {
    matches!(
        canonical_external_receiver_type_key(owner).as_str(),
        "BufferedImage" | "Graphics" | "Graphics2D"
    )
}

fn is_awt_platform_facade_rust_ident(owner: &str) -> bool {
    matches!(
        owner,
        "BufferedImage"
            | "Graphics"
            | "Graphics2D"
            | "JavaAwtImageBufferedImage"
            | "JavaAwtGraphics"
    )
}

#[derive(Clone)]
struct ExternalErasedObjectMethod {
    name: String,
    arg_count: usize,
    return_type: String,
}

#[derive(Clone)]
struct ExternalFacadeTrait {
    trait_name: String,
    methods: BTreeSet<String>,
}

fn collect_source_method_signatures_for_type_name(
    program: &Program,
    type_name: &str,
    visited: &mut HashSet<String>,
    methods: &mut BTreeSet<(String, usize)>,
) {
    let Some(ty) = find_source_type_decl(program, type_name) else {
        return;
    };
    if !visited.insert(ty.name.clone()) {
        return;
    }
    for member in &ty.members {
        if let Member::Method(method) = member {
            methods.insert((
                crate::parser::to_snake_case(&method.name),
                method.params.len(),
            ));
        }
    }
    if let Some(super_name) = ty.extends.as_deref() {
        collect_source_method_signatures_for_type_name(program, super_name, visited, methods);
    }
}

fn find_source_type_decl<'a>(program: &'a Program, type_name: &str) -> Option<&'a TypeDecl> {
    program
        .types
        .iter()
        .find_map(|ty| find_source_type_decl_in_type(ty, type_name))
}

fn find_source_type_decl_in_type<'a>(ty: &'a TypeDecl, type_name: &str) -> Option<&'a TypeDecl> {
    let canonical = canonical_external_receiver_type_key(type_name);
    if ty.name == type_name
        || canonical_external_receiver_type_key(&ty.name) == canonical
        || ty.name.rsplit('.').next() == Some(type_name)
        || rust_name_for_java_named_type(&ty.name) == rust_name_for_java_named_type(type_name)
    {
        return Some(ty);
    }
    ty.members.iter().find_map(|member| match member {
        Member::Type(nested) => find_source_type_decl_in_type(nested, type_name),
        _ => None,
    })
}

impl ExternalApiFacadeSupport {
    fn add_emitted_source_method_signatures(&mut self, program: &Program) {
        for ty in &program.types {
            self.add_emitted_source_method_signatures_from_type(program, ty, None);
        }
    }

    fn add_emitted_source_method_signatures_from_type(
        &mut self,
        program: &Program,
        ty: &crate::java_ir::TypeDecl,
        enclosing: Option<&str>,
    ) {
        let qualified = enclosing
            .map(|outer| format!("{outer}.{}", ty.name))
            .unwrap_or_else(|| ty.name.clone());
        let owners = [
            ty.name.clone(),
            qualified.clone(),
            canonical_external_receiver_type_key(&ty.name),
            canonical_external_receiver_type_key(&qualified),
        ];
        for member in &ty.members {
            match member {
                Member::Method(method) => {
                    let rust_method = crate::parser::to_snake_case(&method.name);
                    for owner in &owners {
                        let rust_owner = rust_name_for_java_named_type(owner);
                        if valid_external_facade_type_ident(&rust_owner) {
                            self.emitted_source_methods.insert((
                                rust_owner,
                                rust_method.clone(),
                                method.params.len(),
                            ));
                        }
                    }
                }
                Member::Type(nested) => {
                    self.add_emitted_source_method_signatures_from_type(
                        program,
                        nested,
                        Some(&qualified),
                    );
                }
                Member::Field(_) | Member::Constructor(_) | Member::Unsupported { .. } => {}
            }
        }
        if let Some(super_name) = ty.extends.as_deref() {
            let mut inherited = BTreeSet::new();
            collect_source_method_signatures_for_type_name(
                program,
                super_name,
                &mut HashSet::new(),
                &mut inherited,
            );
            for (rust_method, arg_count) in inherited {
                for owner in &owners {
                    let rust_owner = rust_name_for_java_named_type(owner);
                    if valid_external_facade_type_ident(&rust_owner) {
                        self.emitted_source_methods.insert((
                            rust_owner,
                            rust_method.clone(),
                            arg_count,
                        ));
                    }
                }
            }
        }
    }

    fn add_declared_external_types(&mut self, program: &Program, local_types: &BTreeSet<String>) {
        for ty in &program.types {
            self.add_declared_external_types_from_type(ty, local_types);
        }
    }

    fn add_declared_external_types_from_type(
        &mut self,
        ty: &crate::java_ir::TypeDecl,
        local_types: &BTreeSet<String>,
    ) {
        for bound in &ty.type_param_bounds {
            if let Some(bound) = bound {
                self.add_declared_java_type(bound, local_types);
            }
        }
        for member in &ty.members {
            match member {
                Member::Field(field) => self.add_declared_java_type(&field.ty, local_types),
                Member::Method(method) => {
                    self.add_declared_java_type(&method.return_type, local_types);
                    for bound in &method.type_param_bounds {
                        if let Some(bound) = bound {
                            self.add_declared_java_type(bound, local_types);
                        }
                    }
                    for thrown in &method.throws {
                        self.add_declared_java_type(thrown, local_types);
                    }
                    for param in &method.params {
                        self.add_declared_java_type(&param.ty, local_types);
                    }
                    if let Some(body) = &method.body {
                        self.add_declared_external_types_from_block(body, local_types);
                    }
                }
                Member::Constructor(constructor) => {
                    for bound in &constructor.type_param_bounds {
                        if let Some(bound) = bound {
                            self.add_declared_java_type(bound, local_types);
                        }
                    }
                    for thrown in &constructor.throws {
                        self.add_declared_java_type(thrown, local_types);
                    }
                    for param in &constructor.params {
                        self.add_declared_java_type(&param.ty, local_types);
                    }
                    if let Some(body) = &constructor.body {
                        self.add_declared_external_types_from_block(body, local_types);
                    }
                }
                Member::Type(nested) => {
                    self.add_declared_external_types_from_type(nested, local_types)
                }
                Member::Unsupported { .. } => {}
            }
        }
    }

    fn add_declared_source_summary_types(
        &mut self,
        program: &Program,
        source_summary_types: &BTreeSet<String>,
        emitted_local_types: &BTreeSet<String>,
    ) {
        for ty in &program.types {
            self.add_declared_source_summary_types_from_type(
                ty,
                source_summary_types,
                emitted_local_types,
            );
        }
    }

    fn add_declared_source_summary_types_from_type(
        &mut self,
        ty: &crate::java_ir::TypeDecl,
        source_summary_types: &BTreeSet<String>,
        emitted_local_types: &BTreeSet<String>,
    ) {
        for bound in &ty.type_param_bounds {
            if let Some(bound) = bound {
                self.add_source_summary_java_type(bound, source_summary_types, emitted_local_types);
            }
        }
        for member in &ty.members {
            match member {
                Member::Field(field) => self.add_source_summary_java_type(
                    &field.ty,
                    source_summary_types,
                    emitted_local_types,
                ),
                Member::Method(method) => {
                    self.add_source_summary_java_type(
                        &method.return_type,
                        source_summary_types,
                        emitted_local_types,
                    );
                    for bound in &method.type_param_bounds {
                        if let Some(bound) = bound {
                            self.add_source_summary_java_type(
                                bound,
                                source_summary_types,
                                emitted_local_types,
                            );
                        }
                    }
                    for thrown in &method.throws {
                        self.add_source_summary_java_type(
                            thrown,
                            source_summary_types,
                            emitted_local_types,
                        );
                    }
                    for param in &method.params {
                        self.add_source_summary_java_type(
                            &param.ty,
                            source_summary_types,
                            emitted_local_types,
                        );
                    }
                }
                Member::Constructor(constructor) => {
                    for bound in &constructor.type_param_bounds {
                        if let Some(bound) = bound {
                            self.add_source_summary_java_type(
                                bound,
                                source_summary_types,
                                emitted_local_types,
                            );
                        }
                    }
                    for thrown in &constructor.throws {
                        self.add_source_summary_java_type(
                            thrown,
                            source_summary_types,
                            emitted_local_types,
                        );
                    }
                    for param in &constructor.params {
                        self.add_source_summary_java_type(
                            &param.ty,
                            source_summary_types,
                            emitted_local_types,
                        );
                    }
                }
                Member::Type(nested) => self.add_declared_source_summary_types_from_type(
                    nested,
                    source_summary_types,
                    emitted_local_types,
                ),
                Member::Unsupported { .. } => {}
            }
        }
    }

    fn add_source_summary_java_type(
        &mut self,
        java_type: &JavaType,
        source_summary_types: &BTreeSet<String>,
        emitted_local_types: &BTreeSet<String>,
    ) {
        match java_type {
            JavaType::Named { name, args } => {
                let type_key = canonical_external_receiver_type_key(name);
                if source_summary_types.contains(&type_key)
                    && !emitted_local_types.contains(&type_key)
                {
                    self.add_opaque_type(rust_name_for_java_named_type(&type_key));
                }
                for arg in args {
                    self.add_source_summary_java_type(
                        arg,
                        source_summary_types,
                        emitted_local_types,
                    );
                }
            }
            JavaType::Array(inner) => {
                self.add_source_summary_java_type(inner, source_summary_types, emitted_local_types)
            }
            JavaType::Wildcard(Some(bound)) => match bound {
                WildcardBound::Extends(inner) | WildcardBound::Super(inner) => self
                    .add_source_summary_java_type(inner, source_summary_types, emitted_local_types),
            },
            JavaType::Primitive(_)
            | JavaType::Void
            | JavaType::String
            | JavaType::Wildcard(None)
            | JavaType::Unknown(_) => {}
        }
    }

    fn add_source_summary_method_surfaces(
        &mut self,
        facts: &FactSet,
        source_summary_program: &Program,
        source_summary_types: &BTreeSet<String>,
        emitted_local_types: &BTreeSet<String>,
        used_calls: &BTreeMap<String, BTreeSet<usize>>,
    ) -> bool {
        let mut changed = false;
        for type_key in source_summary_types {
            if emitted_local_types.contains(type_key) {
                continue;
            }
            let owner = rust_name_for_java_named_type(type_key);
            if !valid_external_facade_type_ident(&owner) || !self.opaque_types.contains(&owner) {
                continue;
            }
            let Some(ty) = find_program_type_by_key(source_summary_program, type_key) else {
                continue;
            };
            for method in source_summary_method_hierarchy(ty, source_summary_program) {
                let rust_method = crate::parser::to_snake_case(&method.name);
                if !used_calls
                    .get(&rust_method)
                    .is_some_and(|arities| arities.contains(&method.params.len()))
                {
                    continue;
                }
                let return_type = external_facade_method_return_type_with_facts(
                    facts,
                    Some(type_key),
                    &method.name,
                    Some(method.params.len()),
                    &method.return_type,
                    source_summary_method_effective_return_nullability(facts, type_key, method),
                );
                let opaque_count_before = self.opaque_types.len();
                self.add_source_summary_java_type(
                    &method.return_type,
                    source_summary_types,
                    emitted_local_types,
                );
                self.add_java_type(&method.return_type);
                for param in &method.params {
                    self.add_source_summary_java_type(
                        &param.ty,
                        source_summary_types,
                        emitted_local_types,
                    );
                    self.add_java_type(&param.ty);
                }
                changed |= self.opaque_types.len() != opaque_count_before;
                let method_body = external_facade_instance_method(
                    &rust_method,
                    method.params.len(),
                    &return_type,
                );
                self.add_external_facade_return_opaque_type(&return_type);
                if self.has_method_signature(&owner, &rust_method, method.params.len())
                    && !self.replace_weak_method_return(
                        &owner,
                        &rust_method,
                        method.params.len(),
                        &method_body,
                    )
                {
                    continue;
                }
                changed = true;
                self.add_method(owner.clone(), method_body);
            }
        }
        changed
    }

    fn add_emitted_source_summary_super_method_surfaces(
        &mut self,
        facts: &FactSet,
        program: &Program,
        source_summary_program: &Program,
        source_summary_types: &BTreeSet<String>,
        used_calls: &BTreeMap<String, BTreeSet<usize>>,
    ) {
        for ty in &program.types {
            self.add_emitted_source_summary_super_method_surfaces_for_type(
                facts,
                ty,
                program,
                source_summary_program,
                source_summary_types,
                used_calls,
            );
        }
    }

    fn add_emitted_source_summary_super_static_constants(
        &mut self,
        program: &Program,
        source_summary_program: &Program,
        source_summary_types: &BTreeSet<String>,
    ) {
        for ty in &program.types {
            self.add_emitted_source_summary_super_static_constants_for_type(
                ty,
                program,
                source_summary_program,
                source_summary_types,
            );
        }
    }

    fn add_emitted_source_summary_super_static_constants_for_type(
        &mut self,
        ty: &TypeDecl,
        program: &Program,
        source_summary_program: &Program,
        source_summary_types: &BTreeSet<String>,
    ) {
        let type_key = canonical_external_receiver_type_key(&ty.name);
        let owner = rust_name_for_java_named_type(&type_key);
        if valid_external_facade_type_ident(&owner) {
            let mut seen_fields = source_static_final_field_names(ty);
            for summary_super in source_summary_supertypes_for_emitted_type(
                ty,
                program,
                source_summary_program,
                source_summary_types,
            ) {
                for field in source_summary_static_final_field_hierarchy(
                    summary_super,
                    source_summary_program,
                ) {
                    if !seen_fields.insert(field.name.clone()) {
                        continue;
                    }
                    let Some(rust_type) = static_constant_rust_type(&field.ty) else {
                        continue;
                    };
                    let Some(value) = source_summary_static_constant_value(
                        &owner,
                        &field.name,
                        &field.ty,
                        field.initializer.as_ref(),
                    ) else {
                        continue;
                    };
                    if valid_rust_const_ident(&field.name) {
                        self.add_method(
                            owner.clone(),
                            format!("    pub const {}: {rust_type} = {value};", field.name),
                        );
                    }
                }
            }
        }
        for member in &ty.members {
            if let Member::Type(nested) = member {
                self.add_emitted_source_summary_super_static_constants_for_type(
                    nested,
                    program,
                    source_summary_program,
                    source_summary_types,
                );
            }
        }
    }

    fn add_emitted_source_summary_super_method_surfaces_for_type(
        &mut self,
        facts: &FactSet,
        ty: &TypeDecl,
        program: &Program,
        source_summary_program: &Program,
        source_summary_types: &BTreeSet<String>,
        used_calls: &BTreeMap<String, BTreeSet<usize>>,
    ) {
        let type_key = canonical_external_receiver_type_key(&ty.name);
        let owner = rust_name_for_java_named_type(&type_key);
        if valid_external_facade_type_ident(&owner) {
            for summary_super in source_summary_supertypes_for_emitted_type(
                ty,
                program,
                source_summary_program,
                source_summary_types,
            ) {
                for method in source_summary_method_hierarchy(summary_super, source_summary_program)
                {
                    let rust_method = crate::parser::to_snake_case(&method.name);
                    let arity = method.params.len();
                    if !used_calls
                        .get(&rust_method)
                        .is_some_and(|arities| arities.contains(&arity))
                    {
                        continue;
                    }
                    if source_method_hierarchy_contains(program, ty, &method.name, arity) {
                        continue;
                    }
                    if source_summary_inherited_method_will_be_lowered(
                        ty,
                        program,
                        source_summary_program,
                        source_summary_types,
                        &method.name,
                        arity,
                    ) {
                        continue;
                    }
                    let return_type = external_facade_method_return_type_with_facts(
                        facts,
                        Some(&type_key),
                        &method.name,
                        Some(arity),
                        &method.return_type,
                        source_summary_method_effective_return_nullability(
                            facts,
                            &summary_super.name,
                            method,
                        ),
                    );
                    self.add_java_type(&method.return_type);
                    for param in &method.params {
                        self.add_java_type(&param.ty);
                    }
                    self.add_external_facade_return_opaque_type(&return_type);
                    let method_body =
                        external_facade_instance_method(&rust_method, arity, &return_type);
                    if self.has_method_signature(&owner, &rust_method, arity)
                        && !self.replace_weak_method_return(
                            &owner,
                            &rust_method,
                            arity,
                            &method_body,
                        )
                    {
                        continue;
                    }
                    self.add_method(owner.clone(), method_body);
                }
            }
        }
        for member in &ty.members {
            if let Member::Type(nested) = member {
                self.add_emitted_source_summary_super_method_surfaces_for_type(
                    facts,
                    nested,
                    program,
                    source_summary_program,
                    source_summary_types,
                    used_calls,
                );
            }
        }
    }

    fn add_inherited_support_field_types(
        &mut self,
        program: &Program,
        support_program: &Program,
        emitted_local_types: &BTreeSet<String>,
    ) {
        for ty in &program.types {
            self.add_inherited_support_field_types_for_type(
                ty,
                support_program,
                emitted_local_types,
            );
        }
    }

    fn add_inherited_support_field_types_for_type(
        &mut self,
        ty: &crate::java_ir::TypeDecl,
        support_program: &Program,
        emitted_local_types: &BTreeSet<String>,
    ) {
        for inherited in
            crate::inherited_members::inherited_instance_field_refs_for_type(ty, support_program)
        {
            self.add_declared_java_type(&inherited.field.ty, emitted_local_types);
        }
        for member in &ty.members {
            if let Member::Type(nested) = member {
                self.add_inherited_support_field_types_for_type(
                    nested,
                    support_program,
                    emitted_local_types,
                );
            }
        }
    }

    fn add_emitted_external_super_method_surfaces(
        &mut self,
        facts: &FactSet,
        program: &Program,
        used_calls: &BTreeMap<String, BTreeSet<usize>>,
    ) {
        for ty in &program.types {
            self.add_emitted_external_super_method_surfaces_for_type(
                facts, ty, program, used_calls,
            );
        }
    }

    fn add_emitted_external_super_method_surfaces_for_type(
        &mut self,
        facts: &FactSet,
        ty: &TypeDecl,
        program: &Program,
        used_calls: &BTreeMap<String, BTreeSet<usize>>,
    ) {
        let owner = rust_name_for_java_named_type(&canonical_external_receiver_type_key(&ty.name));
        if valid_external_facade_type_ident(&owner) {
            for inherited in
                crate::inherited_members::external_inherited_instance_methods_for_type(ty, facts)
            {
                if !used_calls
                    .get(&inherited.rust_name)
                    .is_some_and(|arities| arities.contains(&inherited.arg_count))
                {
                    continue;
                }
                if source_method_hierarchy_contains(
                    &Program {
                        package: None,
                        imports: Vec::new(),
                        types: vec![ty.clone()],
                    },
                    ty,
                    &inherited.method_name,
                    inherited.arg_count,
                ) {
                    continue;
                }
                let (return_rust, unmodelled_return) =
                    self.external_inherited_method_return(facts, &inherited);
                let method_body = if unmodelled_return {
                    external_facade_unmodelled_method_with_facts(
                        facts,
                        &inherited.declaring_type,
                        &inherited.method_name,
                        &inherited.rust_name,
                        inherited.arg_count,
                        &return_rust,
                        false,
                    )
                } else {
                    external_facade_instance_method_with_facts(
                        facts,
                        &inherited.declaring_type,
                        &inherited.method_name,
                        &inherited.rust_name,
                        inherited.arg_count,
                        &return_rust,
                    )
                };
                if self.has_method_signature(&owner, &inherited.rust_name, inherited.arg_count)
                    && !self.replace_weak_method_return(
                        &owner,
                        &inherited.rust_name,
                        inherited.arg_count,
                        &method_body,
                    )
                {
                    continue;
                }
                self.add_method(owner.clone(), method_body);
            }
            for inherited in
                query_backed_external_inherited_methods_for_type(facts, ty, program, used_calls)
            {
                if source_method_hierarchy_contains(
                    &Program {
                        package: None,
                        imports: Vec::new(),
                        types: vec![ty.clone()],
                    },
                    ty,
                    &inherited.method_name,
                    inherited.arg_count,
                ) {
                    continue;
                }
                let (return_rust, unmodelled_return) =
                    self.external_inherited_method_return(facts, &inherited);
                let method_body = if unmodelled_return {
                    external_facade_unmodelled_method_with_facts(
                        facts,
                        &inherited.declaring_type,
                        &inherited.method_name,
                        &inherited.rust_name,
                        inherited.arg_count,
                        &return_rust,
                        false,
                    )
                } else {
                    external_facade_instance_method_with_facts(
                        facts,
                        &inherited.declaring_type,
                        &inherited.method_name,
                        &inherited.rust_name,
                        inherited.arg_count,
                        &return_rust,
                    )
                };
                if self.has_method_signature(&owner, &inherited.rust_name, inherited.arg_count)
                    && !self.replace_weak_method_return(
                        &owner,
                        &inherited.rust_name,
                        inherited.arg_count,
                        &method_body,
                    )
                {
                    continue;
                }
                self.add_method(owner.clone(), method_body);
            }
        }
        for member in &ty.members {
            if let Member::Type(nested) = member {
                self.add_emitted_external_super_method_surfaces_for_type(
                    facts, nested, program, used_calls,
                );
            }
        }
    }

    fn external_inherited_method_return(
        &mut self,
        facts: &FactSet,
        inherited: &crate::inherited_members::ExternalInheritedInstanceMethod,
    ) -> (String, bool) {
        if let Some(return_type) = &inherited.fact.return_type {
            self.add_java_type(return_type);
            return (
                external_facade_method_return_type_with_facts(
                    facts,
                    Some(&inherited.declaring_type),
                    &inherited.method_name,
                    Some(inherited.arg_count),
                    return_type,
                    inherited.fact.return_nullability,
                ),
                false,
            );
        }
        if external_api_role_returns_value(inherited.fact.role) {
            let receiver_key = canonical_external_receiver_type_key(&inherited.declaring_type);
            let receiver_rust = rust_name_for_java_named_type(&receiver_key);
            let return_rust = external_facade_unmodelled_result_type(
                &receiver_rust,
                &inherited.rust_name,
                inherited.arg_count,
            );
            self.add_opaque_type(return_rust.clone());
            (return_rust, true)
        } else {
            ("()".to_string(), false)
        }
    }

    fn add_external_inherited_field_types(&mut self, program: &Program, facts: &FactSet) {
        for ty in &program.types {
            self.add_external_inherited_field_types_for_type(ty, facts);
        }
    }

    fn add_external_inherited_field_types_for_type(&mut self, ty: &TypeDecl, facts: &FactSet) {
        for inherited in
            crate::inherited_members::external_inherited_instance_fields_for_type(ty, facts)
        {
            self.add_java_type(&inherited.field_type);
        }
        for member in &ty.members {
            if let Member::Type(nested) = member {
                self.add_external_inherited_field_types_for_type(nested, facts);
            }
        }
    }

    fn add_declared_external_types_from_block(
        &mut self,
        block: &Block,
        local_types: &BTreeSet<String>,
    ) {
        for stmt in &block.statements {
            self.add_declared_external_types_from_stmt(stmt, local_types);
        }
    }

    fn add_declared_external_types_from_stmt(
        &mut self,
        stmt: &Stmt,
        local_types: &BTreeSet<String>,
    ) {
        match stmt {
            Stmt::VarDecl { ty, .. } => self.add_declared_java_type(ty, local_types),
            Stmt::If {
                then_block,
                else_block,
                ..
            } => {
                self.add_declared_external_types_from_block(then_block, local_types);
                if let Some(else_block) = else_block {
                    self.add_declared_external_types_from_block(else_block, local_types);
                }
            }
            Stmt::While { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::EnhancedFor { body, .. } => {
                self.add_declared_external_types_from_block(body, local_types);
            }
            Stmt::For { init, body, .. } => {
                for stmt in init {
                    self.add_declared_external_types_from_stmt(stmt, local_types);
                }
                self.add_declared_external_types_from_block(body, local_types);
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    for stmt in &case.statements {
                        self.add_declared_external_types_from_stmt(stmt, local_types);
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
                    self.add_declared_external_types_from_stmt(resource, local_types);
                }
                self.add_declared_external_types_from_block(body, local_types);
                for catch in catches {
                    for ty in &catch.types {
                        self.add_declared_java_type(ty, local_types);
                    }
                    self.add_declared_external_types_from_block(&catch.body, local_types);
                }
                if let Some(finally_block) = finally_block {
                    self.add_declared_external_types_from_block(finally_block, local_types);
                }
            }
            Stmt::LocalType(ty) => self.add_declared_external_types_from_type(ty, local_types),
            Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
                self.add_declared_external_types_from_block(block, local_types)
            }
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

    fn add_declared_java_type(&mut self, java_type: &JavaType, local_types: &BTreeSet<String>) {
        match java_type {
            JavaType::Named { name, args } => {
                if !local_types.contains(&canonical_external_receiver_type_key(name)) {
                    self.add_opaque_type(rust_name_for_java_named_type(name));
                }
                for arg in args {
                    self.add_declared_java_type(arg, local_types);
                }
            }
            JavaType::Array(inner) => self.add_declared_java_type(inner, local_types),
            JavaType::Wildcard(Some(bound)) => match bound {
                WildcardBound::Extends(inner) | WildcardBound::Super(inner) => {
                    self.add_declared_java_type(inner, local_types)
                }
            },
            JavaType::Primitive(_)
            | JavaType::Void
            | JavaType::String
            | JavaType::Wildcard(None)
            | JavaType::Unknown(_) => {}
        }
    }

    fn add_nested_type_aliases_from_fields(
        &mut self,
        facts: &FactSet,
        local_types: &BTreeSet<String>,
    ) {
        for field in &facts.external_field_facts {
            if field.field_kind != ExternalFieldKind::NestedType {
                continue;
            }
            if local_types.contains(&canonical_external_receiver_type_key(
                &field.field.type_name,
            )) {
                continue;
            }
            let Some(JavaType::Named {
                name: field_type_name,
                ..
            }) = &field.field_type
            else {
                continue;
            };
            let owner = rust_name_for_java_named_type(&canonical_external_receiver_type_key(
                &field.field.type_name,
            ));
            let alias = format!(
                "{owner}{}",
                rust_name_for_java_named_type(&field.field.field_name)
            );
            let target = rust_name_for_java_named_type(field_type_name);
            self.add_type_alias(alias, target);
        }
    }

    fn add_external_inherited_static_final_surfaces(&mut self, program: &Program, facts: &FactSet) {
        for ty in &program.types {
            self.add_external_inherited_static_final_surfaces_for_type(ty, facts);
        }
    }

    fn add_external_inherited_static_final_surfaces_for_type(
        &mut self,
        ty: &TypeDecl,
        facts: &FactSet,
    ) {
        let type_key = canonical_external_receiver_type_key(&ty.name);
        let owner = rust_name_for_java_named_type(&type_key);
        if valid_external_facade_type_ident(&owner) {
            for inherited in
                crate::inherited_members::external_inherited_static_final_fields_for_type(ty, facts)
            {
                let Some(rust_type) = static_constant_rust_type(&inherited.field_type) else {
                    continue;
                };
                let Some(value) = static_constant_default_value(
                    &owner,
                    &inherited.field_name,
                    &inherited.field_type,
                    &BTreeMap::new(),
                ) else {
                    continue;
                };
                if valid_rust_const_ident(&inherited.field_name) {
                    self.add_method(
                        owner.clone(),
                        format!(
                            "    pub const {}: {rust_type} = {value};",
                            inherited.field_name
                        ),
                    );
                }
            }
        }
        for member in &ty.members {
            if let Member::Type(nested) = member {
                self.add_external_inherited_static_final_surfaces_for_type(nested, facts);
            }
        }
    }

    fn add_used_external_constructors(
        &mut self,
        facts: &FactSet,
        used_constructors: &BTreeMap<String, BTreeSet<usize>>,
        local_types: &BTreeSet<String>,
    ) {
        for (type_name, arities) in used_constructors {
            let java_type = JavaType::Named {
                name: type_name.clone(),
                args: Vec::new(),
            };
            if native_collection_constructor_kind_for_type(&java_type).is_some() {
                continue;
            }
            let type_key = canonical_external_receiver_type_key(type_name);
            if local_types.contains(&type_key) {
                continue;
            }
            let owner = rust_name_for_java_named_type(&type_key);
            if !valid_external_facade_type_ident(&owner) || is_builtin_external_facade_type(&owner)
            {
                continue;
            }
            self.add_opaque_type(owner.clone());
            let overload_by_arity =
                arities.len() > 1 || external_constructor_fact_arity_count(facts, type_name) > 1;
            for arg_count in arities {
                let rust_name = external_facade_constructor_name(*arg_count, overload_by_arity);
                self.add_external_constructor_argument_types(facts, &type_key, *arg_count);
                if self.has_method_named(&owner, &rust_name) {
                    continue;
                }
                self.add_method(
                    owner.clone(),
                    external_facade_constructor_method_with_facts(
                        facts, &type_key, &rust_name, *arg_count,
                    ),
                );
            }
        }
    }

    fn add_external_constructor_argument_types(
        &mut self,
        facts: &FactSet,
        type_key: &str,
        arg_count: usize,
    ) {
        for index in 0..arg_count {
            let parameter_type = facts
                .external_constructor_argument_effect_for(type_key, arg_count, index)
                .and_then(|effect| effect.parameter_type.clone())
                .or_else(|| {
                    crate::external_packs::java_platform_constructor_argument_effect_for(
                        type_key, arg_count, index,
                    )
                    .and_then(|effect| effect.parameter_type)
                });
            if let Some(parameter_type) = parameter_type {
                self.add_java_type(&parameter_type);
            }
        }
    }

    fn add_used_external_api_facts(
        &mut self,
        facts: &FactSet,
        used_calls: &BTreeMap<String, BTreeSet<usize>>,
        local_types: &BTreeSet<String>,
        source_summary_types: &BTreeSet<String>,
        emitted_local_types: &BTreeSet<String>,
        imported_static_owner_calls: &BTreeMap<(String, String), BTreeSet<usize>>,
    ) {
        for fact in &facts.external_api_effect_facts {
            let rust_base_method = crate::parser::to_snake_case(&fact.method_name);
            let Some(arities) = used_calls.get(&rust_base_method) else {
                continue;
            };
            if let Some(arg_count) = fact.arg_count
                && !arities.contains(&arg_count)
            {
                continue;
            }
            let Some(receiver_type) = fact.receiver_type.as_deref() else {
                continue;
            };
            let receiver_key = canonical_external_receiver_type_key(receiver_type);
            let receiver_is_summary_only = source_summary_types.contains(&receiver_key)
                && !emitted_local_types.contains(&receiver_key);
            if local_types.contains(&receiver_key) && !receiver_is_summary_only {
                continue;
            }
            if imported_static_owner_call_contains(
                imported_static_owner_calls,
                &receiver_key,
                &rust_base_method,
                fact.arg_count,
                arities,
            ) {
                continue;
            }
            let receiver_rust = rust_name_for_java_named_type(&receiver_key);
            if !valid_rust_type_ident(&receiver_rust) {
                continue;
            }
            if matches!(
                receiver_rust.as_str(),
                "Math" | "StrictMath" | "Double" | "Float" | "Integer" | "System" | "Objects"
            ) {
                continue;
            }
            let arg_count = fact
                .arg_count
                .unwrap_or_else(|| arities.iter().next().copied().unwrap_or_default());
            let is_static_facade_method = crate::javalibs::is_static_facade_method(
                &receiver_key,
                &fact.method_name,
                arg_count,
            );
            if external_facade_receiver_is_dyn_trait(facts, &receiver_key)
                && !is_static_facade_method
            {
                if let Some(return_type) = &fact.return_type {
                    self.add_java_type(return_type);
                }
                self.add_external_argument_types(
                    facts,
                    &receiver_key,
                    &fact.method_name,
                    arg_count,
                );
                continue;
            }
            self.add_opaque_type(receiver_rust.clone());
            if let Some(return_type) = &fact.return_type {
                self.add_java_type(return_type);
            }

            if rust_base_method == "new_builder" {
                let mut unmodelled_value_return = false;
                let return_rust = fact
                    .return_type
                    .as_ref()
                    .map(|return_type| external_facade_return_type_with_facts(return_type, facts))
                    .unwrap_or_else(|| {
                        unmodelled_value_return = true;
                        external_facade_new_builder_result_type(&receiver_rust)
                    });
                if unmodelled_value_return {
                    self.add_opaque_type(return_rust.clone());
                }
                let method_body = if unmodelled_value_return {
                    external_facade_unmodelled_method_with_facts(
                        facts,
                        &receiver_key,
                        &fact.method_name,
                        &rust_base_method,
                        0,
                        &return_rust,
                        true,
                    )
                } else {
                    external_facade_static_method(&rust_base_method, 0, &return_rust)
                };
                self.add_method(receiver_rust, method_body);
            } else if receiver_rust.ends_with("Builder") {
                continue;
            } else {
                let rust_method = crate::call_plan::external_method_rust_name(
                    &receiver_key,
                    &fact.method_name,
                    arg_count,
                    facts,
                );
                if !valid_rust_method_ident(&rust_method) {
                    continue;
                }
                let mut unmodelled_value_return = false;
                let mut return_rust = fact
                    .return_type
                    .as_ref()
                    .map(|return_type| {
                        external_facade_method_return_type_with_facts(
                            facts,
                            Some(&receiver_key),
                            &fact.method_name,
                            Some(arg_count),
                            return_type,
                            fact.return_nullability,
                        )
                    })
                    .unwrap_or_else(|| {
                        if external_api_role_returns_value(fact.role) {
                            unmodelled_value_return = true;
                            external_facade_unmodelled_result_type(
                                &receiver_rust,
                                &rust_method,
                                arg_count,
                            )
                        } else {
                            "()".to_string()
                        }
                    });
                if unmodelled_value_return {
                    self.add_opaque_type(return_rust.clone());
                }
                self.add_external_argument_types(
                    facts,
                    &receiver_key,
                    &fact.method_name,
                    arg_count,
                );
                if receiver_rust.ends_with("ExecutorService")
                    && rust_method == "submit"
                    && arg_count == 1
                {
                    self.add_opaque_type("Future".to_string());
                    return_rust = "Future".to_string();
                }
                if fact.provenance != FactProvenance::ConservativeFallback
                    && !external_facade_method_is_self_returning_fluent(
                        facts,
                        &receiver_key,
                        &fact.method_name,
                        arg_count,
                    )
                    && !is_static_facade_method
                    && !unmodelled_value_return
                {
                    self.add_erased_object_method(
                        facts,
                        Some(&receiver_key),
                        rust_method.clone(),
                        arg_count,
                        return_rust.clone(),
                    );
                }
                let method_body = if unmodelled_value_return {
                    external_facade_unmodelled_method_with_facts(
                        facts,
                        &receiver_key,
                        &fact.method_name,
                        &rust_method,
                        arg_count,
                        &return_rust,
                        is_static_facade_method,
                    )
                } else if is_static_facade_method {
                    external_facade_static_method_with_facts(
                        facts,
                        &receiver_key,
                        &fact.method_name,
                        &rust_method,
                        arg_count,
                        &return_rust,
                    )
                } else {
                    external_facade_instance_method_with_facts(
                        facts,
                        &receiver_key,
                        &fact.method_name,
                        &rust_method,
                        arg_count,
                        &return_rust,
                    )
                };
                if self.has_method_signature(&receiver_rust, &rust_method, arg_count)
                    && !self.replace_weak_method_return(
                        &receiver_rust,
                        &rust_method,
                        arg_count,
                        &method_body,
                    )
                {
                    continue;
                }
                self.add_method(receiver_rust, method_body);
            }
        }
    }

    fn add_builtin_used_external_api_facts(
        &mut self,
        facts: &FactSet,
        used_calls: &BTreeMap<String, BTreeSet<usize>>,
        local_types: &BTreeSet<String>,
        source_summary_types: &BTreeSet<String>,
        emitted_local_types: &BTreeSet<String>,
        imported_static_owner_calls: &BTreeMap<(String, String), BTreeSet<usize>>,
    ) {
        for (rust_base_method, arities) in used_calls {
            for arg_count in arities {
                for fact in crate::external_packs::external_api_effects_for_method_arity(
                    rust_base_method,
                    *arg_count,
                ) {
                    let Some(receiver_type) = fact.receiver_type.as_deref() else {
                        continue;
                    };
                    let receiver_key = canonical_external_receiver_type_key(receiver_type);
                    let receiver_is_summary_only = source_summary_types.contains(&receiver_key)
                        && !emitted_local_types.contains(&receiver_key);
                    if local_types.contains(&receiver_key) && !receiver_is_summary_only {
                        continue;
                    }
                    if imported_static_owner_call_contains(
                        imported_static_owner_calls,
                        &receiver_key,
                        rust_base_method,
                        fact.arg_count,
                        arities,
                    ) {
                        continue;
                    }
                    let receiver_rust = rust_name_for_java_named_type(&receiver_key);
                    if !valid_rust_type_ident(&receiver_rust)
                        || !self.has_surface_type(&receiver_rust)
                    {
                        continue;
                    }
                    if matches!(
                        receiver_rust.as_str(),
                        "Math"
                            | "StrictMath"
                            | "Double"
                            | "Float"
                            | "Integer"
                            | "System"
                            | "Objects"
                    ) {
                        continue;
                    }
                    let mut rust_method = crate::call_plan::external_method_rust_name(
                        &receiver_key,
                        &fact.method_name,
                        *arg_count,
                        facts,
                    );
                    if matches!(fact.method_name.as_str(), "read" | "write") && *arg_count > 1 {
                        rust_method = format!(
                            "{}_{}",
                            crate::parser::to_snake_case(&fact.method_name),
                            arg_count
                        );
                    }
                    if !valid_rust_method_ident(&rust_method) {
                        continue;
                    }
                    if let Some(return_type) = &fact.return_type {
                        self.add_java_type(return_type);
                    }
                    self.add_external_argument_types(
                        facts,
                        &receiver_key,
                        &fact.method_name,
                        *arg_count,
                    );
                    let return_rust = fact
                        .return_type
                        .as_ref()
                        .map(|return_type| {
                            external_facade_method_return_type_with_facts(
                                facts,
                                Some(&receiver_key),
                                &fact.method_name,
                                Some(*arg_count),
                                return_type,
                                fact.return_nullability,
                            )
                        })
                        .unwrap_or_else(|| "()".to_string());
                    let method_body = external_facade_instance_method_with_facts(
                        facts,
                        &receiver_key,
                        &fact.method_name,
                        &rust_method,
                        *arg_count,
                        &return_rust,
                    );
                    if matches!(fact.method_name.as_str(), "write" | "write_1" | "write_int")
                        && *arg_count == 1
                        && is_known_java_platform_output_stream_receiver(&receiver_key)
                    {
                        for overload_method in ["write_1", "write_int"] {
                            let overload_body = external_facade_instance_method_with_facts(
                                facts,
                                &receiver_key,
                                overload_method,
                                overload_method,
                                *arg_count,
                                &return_rust,
                            );
                            self.add_method(receiver_rust.clone(), overload_body);
                        }
                    }
                    if receiver_key == "PrintStream"
                        && matches!(fact.method_name.as_str(), "print" | "println")
                        && *arg_count == 1
                    {
                        let base_method = crate::parser::to_snake_case(&fact.method_name);
                        if !self.has_method_named(&receiver_rust, &base_method) {
                            let alias_body = external_facade_instance_method_with_facts(
                                facts,
                                &receiver_key,
                                &fact.method_name,
                                &base_method,
                                *arg_count,
                                &return_rust,
                            );
                            self.add_method(receiver_rust.clone(), alias_body);
                        }
                    }
                    if self.has_method_named(&receiver_rust, &rust_method)
                        && !self.has_method_signature(&receiver_rust, &rust_method, *arg_count)
                    {
                        continue;
                    }
                    if self.has_method_signature(&receiver_rust, &rust_method, *arg_count)
                        && !self.replace_weak_method_return(
                            &receiver_rust,
                            &rust_method,
                            *arg_count,
                            &method_body,
                        )
                    {
                        continue;
                    }
                    self.add_method(receiver_rust, method_body);
                }
            }
        }
    }

    fn add_static_external_api_facts_for_surface_types(&mut self, facts: &FactSet) {
        let surface_types = self.opaque_types.iter().cloned().collect::<BTreeSet<_>>();
        let mut merged_facts = facts.clone();
        merged_facts.extend_from(crate::external_packs::builtin_external_library_packs());
        for fact in &merged_facts.external_api_effect_facts {
            let Some(receiver_type) = fact.receiver_type.as_deref() else {
                continue;
            };
            let receiver_key = canonical_external_receiver_type_key(receiver_type);
            let receiver_rust = rust_name_for_java_named_type(&receiver_key);
            if !surface_types.contains(&receiver_rust) {
                continue;
            }
            for arg_count in Self::static_external_api_fact_arities_for_surface(
                &merged_facts,
                fact,
                &receiver_key,
            ) {
                if !crate::javalibs::is_static_facade_method(
                    &receiver_key,
                    &fact.method_name,
                    arg_count,
                ) {
                    continue;
                }
                let rust_method = crate::call_plan::external_method_rust_name(
                    &receiver_key,
                    &fact.method_name,
                    arg_count,
                    &merged_facts,
                );
                if !valid_rust_method_ident(&rust_method) {
                    continue;
                }
                if let Some(return_type) = &fact.return_type {
                    self.add_java_type(return_type);
                }
                self.add_external_argument_types(
                    &merged_facts,
                    &receiver_key,
                    &fact.method_name,
                    arg_count,
                );
                let return_rust = fact
                    .return_type
                    .as_ref()
                    .map(|return_type| {
                        external_facade_method_return_type_with_facts(
                            &merged_facts,
                            Some(&receiver_key),
                            &fact.method_name,
                            Some(arg_count),
                            return_type,
                            fact.return_nullability,
                        )
                    })
                    .unwrap_or_else(|| "()".to_string());
                let method_body = external_facade_static_method_with_facts(
                    &merged_facts,
                    &receiver_key,
                    &fact.method_name,
                    &rust_method,
                    arg_count,
                    &return_rust,
                );
                self.add_method(receiver_rust.clone(), method_body);
            }
        }
    }

    fn static_external_api_fact_arities_for_surface(
        facts: &FactSet,
        fact: &ExternalApiEffectFact,
        receiver_key: &str,
    ) -> Vec<usize> {
        if let Some(arg_count) = fact.arg_count {
            return vec![arg_count];
        }
        let mut arities = Vec::new();
        for arg_count in 0..16 {
            if crate::javalibs::is_static_facade_method(receiver_key, &fact.method_name, arg_count)
            {
                arities.push(arg_count);
            }
        }
        if !arities.is_empty() {
            return arities;
        }
        let mut inferred = None;
        for index in 0..16 {
            if external_facade_argument_effect(facts, receiver_key, &fact.method_name, None, index)
                .is_some()
            {
                inferred = Some(index + 1);
                continue;
            }
            if inferred.is_some() {
                break;
            }
        }
        inferred.into_iter().collect()
    }

    fn add_external_argument_types(
        &mut self,
        facts: &FactSet,
        receiver_key: &str,
        method: &str,
        arg_count: usize,
    ) {
        for index in 0..arg_count {
            if let Some(java_type) =
                external_facade_argument_java_type(facts, receiver_key, method, arg_count, index)
            {
                self.add_java_type(&java_type);
            }
        }
    }

    fn add_imported_static_owner_calls(
        &mut self,
        facts: &FactSet,
        imported_static_owner_calls: &BTreeMap<(String, String), BTreeSet<usize>>,
        local_types: &BTreeSet<String>,
    ) {
        for ((owner, method), arities) in imported_static_owner_calls {
            let owner_key = canonical_external_receiver_type_key(&owner);
            if local_types.contains(&owner_key) {
                continue;
            }
            let mut owner_rust = rust_name_for_java_named_type(&owner_key);
            if !valid_external_facade_type_ident(&owner_rust) {
                continue;
            }
            self.add_opaque_type(owner_rust.clone());
            let simple_owner = owner_key
                .split('.')
                .filter(|segment| !segment.is_empty())
                .next_back()
                .unwrap_or(owner_key.as_str());
            let simple_owner_rust = rust_name_for_java_named_type(simple_owner);
            if !self.opaque_types.contains(&simple_owner_rust) {
                self.add_type_alias(simple_owner_rust.clone(), owner_rust.clone());
            }
            if let Some(alias) = arities.iter().find_map(|arg_count| {
                crate::javalibs::preferred_static_owner_alias(&owner_key, method, *arg_count)
            }) {
                let alias_rust = rust_name_for_java_named_type(alias);
                self.add_type_alias(owner_rust.clone(), alias_rust.clone());
                owner_rust = alias_rust;
            }
            let mut method_owners = vec![owner_rust.clone()];
            if simple_owner_rust != owner_rust && self.opaque_types.contains(&simple_owner_rust) {
                method_owners.push(simple_owner_rust);
            }
            for arg_count in arities {
                if let Some(effect) = facts.external_api_effect_for_receiver_with_arity(
                    Some(&owner_key),
                    method,
                    Some(*arg_count),
                ) && let Some(return_type) = &effect.return_type
                {
                    self.add_java_type(return_type);
                }
                self.add_external_argument_types(facts, &owner_key, method, *arg_count);
                if let Some(return_type) =
                    known_imported_static_method_return_type(&owner_key, method, *arg_count)
                {
                    self.add_java_type(&return_type);
                }
                if let Some(return_rust) =
                    imported_static_unmodelled_result_type(facts, &owner_key, method, *arg_count)
                {
                    self.add_opaque_type(return_rust);
                }
                let method_body = external_facade_imported_static_method_with_facts(
                    facts, &owner_key, method, *arg_count,
                );
                for method_owner in &method_owners {
                    if self.has_method_signature(method_owner, method, *arg_count) {
                        self.replace_method_signature(
                            method_owner,
                            method,
                            *arg_count,
                            &method_body,
                        );
                    }
                    self.add_method(method_owner.clone(), method_body.clone());
                }
            }
        }
    }

    fn add_known_domain_getter_surfaces_from_used_calls(
        &mut self,
        facts: &FactSet,
        used_calls: &BTreeMap<String, BTreeSet<usize>>,
    ) {
        let opaque_types = self.opaque_types.iter().cloned().collect::<Vec<_>>();
        for owner in opaque_types {
            if is_builtin_external_facade_type(&owner) || owner.ends_with("Builder") {
                continue;
            }
            if matches!(
                owner.as_str(),
                "Math" | "StrictMath" | "Double" | "Float" | "Integer" | "System" | "Objects"
            ) {
                continue;
            }
            for (rust_method, arities) in used_calls {
                let rust_lookup_method = crate::parser::to_snake_case(rust_method);
                for arg_count in arities {
                    let java_method = Self::support_lookup_java_method_name(
                        facts,
                        &owner,
                        &rust_lookup_method,
                        *arg_count,
                    );
                    let planned_rust_method = crate::call_plan::external_method_rust_name(
                        &owner,
                        &java_method,
                        *arg_count,
                        facts,
                    );
                    let arity_rust_method = format!("{planned_rust_method}_{arg_count}");
                    let support_rust_method = if Self::has_facade_abi_for_rust_method(
                        facts,
                        &owner,
                        &rust_lookup_method,
                        *arg_count,
                    ) {
                        rust_lookup_method.clone()
                    } else if Self::has_facade_abi_for_rust_method(
                        facts,
                        &owner,
                        &arity_rust_method,
                        *arg_count,
                    ) {
                        arity_rust_method
                    } else if matches!(java_method.as_str(), "read" | "write") && *arg_count > 1 {
                        arity_rust_method
                    } else {
                        planned_rust_method
                    };
                    if owner.ends_with("ExecutorService")
                        && rust_lookup_method == "submit"
                        && *arg_count == 1
                    {
                        let method_body = external_facade_instance_method(
                            &rust_lookup_method,
                            *arg_count,
                            "Future",
                        );
                        if self.has_method_signature(&owner, &rust_lookup_method, *arg_count) {
                            self.replace_method_signature(
                                &owner,
                                &rust_lookup_method,
                                *arg_count,
                                &method_body,
                            );
                        } else {
                            self.add_method(owner.clone(), method_body);
                        }
                        continue;
                    }
                    if Self::known_missing_opaque_support_generic_method(
                        &owner,
                        &java_method,
                        *arg_count,
                    ) {
                        let return_rust = external_facade_unmodelled_result_type(
                            &owner,
                            &java_method,
                            *arg_count,
                        );
                        self.add_opaque_type(return_rust.clone());
                        let method_body = external_facade_builder_unmodelled_getter(
                            &owner,
                            &support_rust_method,
                            &java_method,
                            *arg_count,
                            &return_rust,
                        );
                        if self.has_method_signature(&owner, &support_rust_method, *arg_count) {
                            self.replace_method_signature(
                                &owner,
                                &support_rust_method,
                                *arg_count,
                                &method_body,
                            );
                        }
                        self.add_method(owner.clone(), method_body);
                        continue;
                    }
                    if Self::known_missing_opaque_support_side_effect_method(
                        &owner,
                        &java_method,
                        *arg_count,
                    ) {
                        let method_body =
                            external_facade_instance_method(&support_rust_method, *arg_count, "()");
                        if !self.has_method_signature(&owner, &support_rust_method, *arg_count) {
                            self.add_method(owner.clone(), method_body);
                        }
                        continue;
                    }
                    if let Some(effect) = facts
                        .external_api_effect_for_receiver_with_arity(
                            Some(&owner),
                            &java_method,
                            Some(*arg_count),
                        )
                        .cloned()
                        .or_else(|| {
                            crate::external_packs::external_api_effect_for_receiver_with_arity(
                                Some(&owner),
                                &java_method,
                                Some(*arg_count),
                            )
                        })
                    {
                        let return_rust = effect
                            .return_type
                            .as_ref()
                            .map(|return_type| {
                                external_facade_method_return_type_with_facts(
                                    facts,
                                    Some(&owner),
                                    &java_method,
                                    Some(*arg_count),
                                    return_type,
                                    effect.return_nullability,
                                )
                            })
                            .unwrap_or_else(|| "()".to_string());
                        if let Some(return_type) = &effect.return_type {
                            self.add_java_type(return_type);
                        }
                        self.add_external_argument_types(facts, &owner, &java_method, *arg_count);
                        let method_body = if crate::javalibs::is_static_facade_method(
                            &owner,
                            &java_method,
                            *arg_count,
                        ) {
                            external_facade_static_method_with_facts(
                                facts,
                                &owner,
                                &java_method,
                                &support_rust_method,
                                *arg_count,
                                &return_rust,
                            )
                        } else {
                            external_facade_instance_method_with_facts(
                                facts,
                                &owner,
                                &java_method,
                                &support_rust_method,
                                *arg_count,
                                &return_rust,
                            )
                        };
                        let method_receiver_kind = facade_method_receiver_kind(&method_body);
                        if self.has_method_signature_with_receiver(
                            &owner,
                            &support_rust_method,
                            *arg_count,
                            method_receiver_kind,
                        ) && !self.replace_weak_method_return_with_receiver(
                            &owner,
                            &support_rust_method,
                            *arg_count,
                            method_receiver_kind,
                            &method_body,
                        ) {
                            continue;
                        }
                        self.add_method(owner.clone(), method_body);
                        continue;
                    }
                    let Some(return_type) =
                        known_external_method_return_type(Some(&owner), &java_method, *arg_count)
                            .or_else(|| {
                                Self::known_missing_opaque_support_method_return_type(
                                    &owner,
                                    &java_method,
                                    *arg_count,
                                )
                            })
                    else {
                        continue;
                    };
                    let return_rust = external_facade_method_return_type_with_facts(
                        facts,
                        Some(&owner),
                        &java_method,
                        Some(*arg_count),
                        &return_type,
                        NullabilityState::DefinitelyNonNull,
                    );
                    if weak_external_facade_return_type(&return_rust) {
                        continue;
                    }
                    self.add_java_type(&return_type);
                    let method_body = if crate::javalibs::is_static_facade_method(
                        &owner,
                        &java_method,
                        *arg_count,
                    ) {
                        external_facade_static_method_with_facts(
                            facts,
                            &owner,
                            &java_method,
                            &support_rust_method,
                            *arg_count,
                            &return_rust,
                        )
                    } else {
                        external_facade_instance_method(
                            &support_rust_method,
                            *arg_count,
                            &return_rust,
                        )
                    };
                    let method_receiver_kind = facade_method_receiver_kind(&method_body);
                    if self.has_method_signature_with_receiver(
                        &owner,
                        &support_rust_method,
                        *arg_count,
                        method_receiver_kind,
                    ) && !self.replace_weak_method_return_with_receiver(
                        &owner,
                        &support_rust_method,
                        *arg_count,
                        method_receiver_kind,
                        &method_body,
                    ) {
                        continue;
                    }

                    if !self.has_method_signature_with_receiver(
                        &owner,
                        &support_rust_method,
                        *arg_count,
                        method_receiver_kind,
                    ) && Self::known_missing_opaque_support_method_return_type(
                        &owner,
                        &java_method,
                        *arg_count,
                    )
                    .is_none()
                        && !crate::external_packs::builtin_external_pack_models_api(
                            &owner,
                            &java_method,
                        )
                    {
                        continue;
                    }

                    if !self.has_method_signature_with_receiver(
                        &owner,
                        &support_rust_method,
                        *arg_count,
                        method_receiver_kind,
                    ) {
                        self.add_method(owner.clone(), method_body);
                    }
                }
            }
        }
    }

    fn support_lookup_java_method_name(
        facts: &FactSet,
        owner: &str,
        rust_method: &str,
        arg_count: usize,
    ) -> String {
        let java_method = lower_camel_from_snake(rust_method);
        if Self::external_support_method_has_effect(facts, owner, &java_method, arg_count) {
            return java_method;
        }
        let suffix = format!("_{arg_count}");
        if let Some(base) = rust_method.strip_suffix(&suffix) {
            let base_java_method = lower_camel_from_snake(base);
            if Self::external_support_method_has_effect(facts, owner, &base_java_method, arg_count)
            {
                return base_java_method;
            }
        }
        java_method
    }

    fn external_support_method_has_effect(
        facts: &FactSet,
        owner: &str,
        java_method: &str,
        arg_count: usize,
    ) -> bool {
        facts
            .external_api_effect_for_receiver_with_arity(Some(owner), java_method, Some(arg_count))
            .is_some()
            || crate::external_packs::external_api_effect_for_receiver_with_arity(
                Some(owner),
                java_method,
                Some(arg_count),
            )
            .is_some()
    }

    fn has_facade_abi_for_rust_method(
        facts: &FactSet,
        owner: &str,
        rust_method: &str,
        arg_count: usize,
    ) -> bool {
        facts.facade_abi_facts.iter().any(|abi| {
            canonical_external_receiver_type_key(&abi.key.owner.type_name)
                == canonical_external_receiver_type_key(owner)
                && abi.key.member.0 == rust_method
                && abi.args.len() == arg_count
        })
    }

    fn add_known_object_inherited_surfaces_from_used_calls(
        &mut self,
        facts: &FactSet,
        used_calls: &BTreeMap<String, BTreeSet<usize>>,
    ) {
        let opaque_types = self.opaque_types.iter().cloned().collect::<Vec<_>>();
        for (rust_method, arities) in used_calls {
            let java_method = lower_camel_from_snake(rust_method);
            for arg_count in arities {
                if !is_known_object_inherited_method(&java_method, *arg_count) {
                    continue;
                }
                let Some(return_type) =
                    known_external_method_return_type(Some("Object"), &java_method, *arg_count)
                else {
                    continue;
                };
                let support_rust_method = crate::call_plan::external_method_rust_name(
                    "Object",
                    &java_method,
                    *arg_count,
                    facts,
                );
                let return_rust = external_facade_method_return_type_with_facts(
                    facts,
                    Some("Object"),
                    &java_method,
                    Some(*arg_count),
                    &return_type,
                    NullabilityState::DefinitelyNonNull,
                );
                if weak_external_facade_return_type(&return_rust) {
                    continue;
                }
                self.add_java_type(&return_type);
                self.add_erased_object_method(
                    facts,
                    Some("Object"),
                    support_rust_method.clone(),
                    *arg_count,
                    return_rust.clone(),
                );
                let method_body =
                    external_facade_instance_method(&support_rust_method, *arg_count, &return_rust);
                for owner in &opaque_types {
                    if is_builtin_external_facade_type(owner) || owner.ends_with("Builder") {
                        continue;
                    }
                    if self.has_method_signature(owner, &support_rust_method, *arg_count)
                        && !self.replace_weak_method_return(
                            owner,
                            &support_rust_method,
                            *arg_count,
                            &method_body,
                        )
                    {
                        continue;
                    }
                    self.add_method(owner.clone(), method_body.clone());
                }
            }
        }
    }

    fn add_display_context_return_hint_surfaces(
        &mut self,
        facts: &FactSet,
        hints: &BTreeSet<DisplayContextReturnHint>,
    ) {
        for hint in hints {
            if !valid_external_facade_type_ident(&hint.owner)
                || is_builtin_external_facade_type(&hint.owner)
                || !self.has_surface_type(&hint.owner)
            {
                continue;
            }
            let method_body = external_facade_instance_method_with_facts(
                facts,
                &hint.owner,
                &hint.java_method,
                &hint.rust_method,
                hint.arg_count,
                "String",
            );
            if self.has_method_signature(&hint.owner, &hint.rust_method, hint.arg_count)
                && !self.replace_weak_method_return(
                    &hint.owner,
                    &hint.rust_method,
                    hint.arg_count,
                    &method_body,
                )
            {
                continue;
            }
            self.add_method(hint.owner.clone(), method_body);
        }
    }

    fn add_known_jdk_static_support_from_used_calls(
        &mut self,
        used_calls: &BTreeMap<String, BTreeSet<usize>>,
    ) {
        if used_calls
            .get("pow")
            .is_some_and(|arities| arities.contains(&2))
        {
            self.add_opaque_type("Math".to_string());
            self.add_method(
                "Math".to_string(),
                "    pub fn pow<T0: Into<f64>, T1: Into<f64>>(base: T0, exponent: T1) -> f64 { base.into().powf(exponent.into()) }".to_string(),
            );
        }
        for method in ["abs", "ceil", "floor", "log", "log10", "exp", "sqrt"] {
            if used_calls
                .get(method)
                .is_some_and(|arities| arities.contains(&1))
            {
                self.add_opaque_type("Math".to_string());
                let rust_method = if method == "log" { "ln" } else { method };
                self.add_method(
                    "Math".to_string(),
                    format!(
                        "    pub fn {method}<T0: Into<f64>>(value: T0) -> f64 {{ value.into().{rust_method}() }}"
                    ),
                );
            }
        }
        if used_calls
            .get("round")
            .is_some_and(|arities| arities.contains(&1))
        {
            self.add_opaque_type("Math".to_string());
            self.add_method(
                "Math".to_string(),
                "    pub fn round<T0: Into<f64>>(value: T0) -> i64 { value.into().round() as i64 }"
                    .to_string(),
            );
        }
        if used_calls
            .get("random")
            .is_some_and(|arities| arities.contains(&0))
        {
            self.add_opaque_type("Math".to_string());
            self.add_method(
                "Math".to_string(),
                "    pub fn random() -> f64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| (duration.subsec_nanos() as f64) / 1_000_000_000.0)
            .unwrap_or(0.0)
    }"
                .to_string(),
            );
        }
        if used_calls
            .get("i_e_e_eremainder")
            .is_some_and(|arities| arities.contains(&2))
        {
            self.add_opaque_type("StrictMath".to_string());
            self.add_method(
                "StrictMath".to_string(),
                "    pub fn i_e_e_eremainder<T0: Into<f64>, T1: Into<f64>>(dividend: T0, divisor: T1) -> f64 { let dividend = dividend.into(); let divisor = divisor.into(); dividend - (dividend / divisor).round() * divisor }"
                    .to_string(),
            );
        }
        for owner in ["Double", "Float"] {
            if used_calls
                .get("compare")
                .is_some_and(|arities| arities.contains(&2))
            {
                self.add_opaque_type(owner.to_string());
                self.add_method(
                    owner.to_string(),
                    "    pub fn compare<T0: PartialOrd>(left: T0, right: T0) -> i32 {
        if left < right {
            -1
        } else if left > right {
            1
        } else {
            0
        }
    }"
                    .to_string(),
                );
            }
            for method in ["is_nan", "is_infinite"] {
                if used_calls
                    .get(method)
                    .is_some_and(|arities| arities.contains(&1))
                {
                    self.add_opaque_type(owner.to_string());
                    self.add_method(
                        owner.to_string(),
                        format!(
                            "    pub fn {method}<T0: Into<f64>>(value: T0) -> bool {{ value.into().{method}() }}"
                        ),
                    );
                }
            }
        }
        if used_calls
            .get("double_to_long_bits")
            .is_some_and(|arities| arities.contains(&1))
        {
            self.add_opaque_type("Double".to_string());
            self.add_method(
                "Double".to_string(),
                "    pub fn double_to_long_bits<T0: Into<f64>>(value: T0) -> i64 { value.into().to_bits() as i64 }".to_string(),
            );
        }
        if used_calls
            .get("double_to_raw_long_bits")
            .is_some_and(|arities| arities.contains(&1))
        {
            self.add_opaque_type("Double".to_string());
            self.add_method(
                "Double".to_string(),
                "    pub fn double_to_raw_long_bits<T0: Into<f64>>(value: T0) -> i64 { value.into().to_bits() as i64 }".to_string(),
            );
        }
        if used_calls
            .get("long_bits_to_double")
            .is_some_and(|arities| arities.contains(&1))
        {
            self.add_opaque_type("Double".to_string());
            self.add_method(
                "Double".to_string(),
                "    pub fn long_bits_to_double(value: i64) -> f64 { f64::from_bits(value as u64) }"
                    .to_string(),
            );
        }
        if used_calls
            .get("float_to_int_bits")
            .is_some_and(|arities| arities.contains(&1))
        {
            self.add_opaque_type("Float".to_string());
            self.add_method(
                "Float".to_string(),
                "    pub fn float_to_int_bits<T0: Into<f32>>(value: T0) -> i32 { value.into().to_bits() as i32 }".to_string(),
            );
        }
        if used_calls
            .get("float_to_raw_int_bits")
            .is_some_and(|arities| arities.contains(&1))
        {
            self.add_opaque_type("Float".to_string());
            self.add_method(
                "Float".to_string(),
                "    pub fn float_to_raw_int_bits<T0: Into<f32>>(value: T0) -> i32 { value.into().to_bits() as i32 }".to_string(),
            );
        }
        if used_calls
            .get("int_bits_to_float")
            .is_some_and(|arities| arities.contains(&1))
        {
            self.add_opaque_type("Float".to_string());
            self.add_method(
                "Float".to_string(),
                "    pub fn int_bits_to_float(value: i32) -> f32 { f32::from_bits(value as u32) }"
                    .to_string(),
            );
        }
        if used_calls
            .get("value_of")
            .is_some_and(|arities| arities.contains(&1))
        {
            self.add_opaque_type("Integer".to_string());
            self.add_method(
                "Integer".to_string(),
                "    pub fn value_of(value: i32) -> i32 { value }".to_string(),
            );
        }
        if used_calls
            .get("to_hex_string")
            .is_some_and(|arities| arities.contains(&1))
        {
            self.add_opaque_type("Integer".to_string());
            self.add_method(
                "Integer".to_string(),
                "    pub fn to_hex_string(value: i32) -> String { format!(\"{:x}\", value) }"
                    .to_string(),
            );
        }
        if used_calls
            .get("to_string")
            .is_some_and(|arities| arities.contains(&1))
        {
            for owner in ["Double", "Float", "Integer"] {
                self.add_opaque_type(owner.to_string());
                self.add_method(
                    owner.to_string(),
                    "    pub fn to_string<T0: std::fmt::Display>(_arg0: T0) -> String { _arg0.to_string() }".to_string(),
                );
            }
        }
        if used_calls
            .get("get_property")
            .is_some_and(|arities| arities.contains(&1) || arities.contains(&2))
        {
            self.add_opaque_type("System".to_string());
            self.add_method(
                "System".to_string(),
                "    pub fn get_property<K: AsRef<str>>(key: K) -> Option<String> { std::env::var(key.as_ref()).ok() }".to_string(),
            );
            self.add_method(
                "System".to_string(),
                "    pub fn get_property_2<K: AsRef<str>, D: Into<String>>(key: K, default_value: D) -> String { std::env::var(key.as_ref()).unwrap_or_else(|_| default_value.into()) }".to_string(),
            );
        }
        if used_calls
            .get("line_separator")
            .is_some_and(|arities| arities.contains(&0))
        {
            self.add_opaque_type("System".to_string());
            self.add_method(
                "System".to_string(),
                "    pub fn line_separator() -> String { \"\\n\".to_string() }".to_string(),
            );
        }
        if used_calls
            .get("current_time_millis")
            .is_some_and(|arities| arities.contains(&0))
        {
            self.add_opaque_type("System".to_string());
            self.add_method(
                "System".to_string(),
                "    pub fn current_time_millis() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or(0)
    }"
                .to_string(),
            );
        }
        if used_calls
            .get("identity_hash_code")
            .is_some_and(|arities| arities.contains(&1))
        {
            self.add_opaque_type("System".to_string());
            self.add_method(
                "System".to_string(),
                "    pub fn identity_hash_code<T0: ?Sized>(value: &T0) -> i32 { (value as *const T0 as *const () as usize) as i32 }".to_string(),
            );
        }
        if used_calls
            .get("arraycopy")
            .is_some_and(|arities| arities.contains(&5))
        {
            self.add_opaque_type("System".to_string());
            self.add_method(
                "System".to_string(),
                "    pub fn arraycopy<T0, T1, T2, T3, T4>(_src: T0, _src_pos: T1, _dest: T2, _dest_pos: T3, _length: T4) {}".to_string(),
            );
        }
        if used_calls
            .get("set_property")
            .is_some_and(|arities| arities.contains(&2))
        {
            self.add_opaque_type("System".to_string());
            self.add_method(
                "System".to_string(),
                "    pub fn set_property<K: AsRef<str>, V: AsRef<str>>(key: K, value: V) -> Option<String> {
        std::env::set_var(key.as_ref(), value.as_ref());
        None
    }"
                .to_string(),
            );
        }
    }

    fn known_missing_opaque_support_generic_method(
        owner: &str,
        name: &str,
        arg_count: usize,
    ) -> bool {
        owner.ends_with("Future") && name == "get" && arg_count == 0
    }

    fn known_missing_opaque_support_method_return_type(
        owner: &str,
        name: &str,
        arg_count: usize,
    ) -> Option<JavaType> {
        match (owner, name, arg_count) {
            ("IntervalArgumentCollection", "intervalsSpecified", 0) => {
                Some(JavaType::Primitive(PrimitiveType::Boolean))
            }
            ("IntervalArgumentCollection", "getIntervals", 1) => Some(JavaType::Named {
                name: "List".to_string(),
                args: vec![JavaType::Named {
                    name: "SimpleInterval".to_string(),
                    args: Vec::new(),
                }],
            }),
            ("ObjectInputStream", "readInt" | "read_int", 0) => {
                Some(JavaType::Primitive(PrimitiveType::Int))
            }
            ("ObjectInputStream", "readObject" | "read_object", 0) => Some(JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            }),
            (_, "getClass" | "get_class", 0) => Some(JavaType::Named {
                name: "Class".to_string(),
                args: Vec::new(),
            }),
            (_, "getName" | "get_name" | "name", 0) => Some(JavaType::String),
            _ => None,
        }
    }

    fn known_missing_opaque_support_side_effect_method(
        owner: &str,
        name: &str,
        arg_count: usize,
    ) -> bool {
        matches!((owner, name, arg_count), ("Parser", "merge", 2))
    }

    fn add_external_field_surfaces(&mut self, facts: &FactSet, local_types: &BTreeSet<String>) {
        let mut merged_facts = facts.clone();
        merged_facts.extend_from(crate::external_packs::builtin_external_library_packs());
        for fact in &facts.external_field_facts {
            self.add_external_field_surface_fact(&merged_facts, fact, local_types);
        }
        for fact in &crate::external_packs::builtin_external_library_packs().external_field_facts {
            let receiver_key = canonical_external_receiver_type_key(&fact.field.type_name);
            let is_enum_like = fact.field_type.as_ref().is_some_and(|field_type| {
                is_self_typed_external_enum_constant_field(
                    &receiver_key,
                    &fact.field.field_name,
                    field_type,
                ) || fact.field_kind == ExternalFieldKind::EnumVariant
            });
            if is_enum_like
                || fact.field_kind == ExternalFieldKind::InstanceField
                || (fact.field_kind == ExternalFieldKind::StaticFinal
                    && fact
                        .field_type
                        .as_ref()
                        .is_some_and(auto_external_static_constant_surface_type_supported))
            {
                self.add_external_field_surface_fact(&merged_facts, fact, local_types);
            }
        }
    }

    fn add_external_field_surface_fact(
        &mut self,
        facts: &FactSet,
        fact: &crate::facts::ExternalFieldFact,
        local_types: &BTreeSet<String>,
    ) {
        let receiver_key = canonical_external_receiver_type_key(&fact.field.type_name);
        let owner = rust_name_for_java_named_type(&receiver_key);
        if local_types.contains(&receiver_key) && !self.opaque_types.contains(&owner) {
            return;
        }
        if !valid_external_facade_type_ident(&owner)
            || !self.opaque_types.contains(&owner)
            || is_builtin_external_facade_type(&owner)
        {
            return;
        }
        let Some(field_type) = &fact.field_type else {
            return;
        };
        let self_typed_enum_constant = is_self_typed_external_enum_constant_field(
            &receiver_key,
            &fact.field.field_name,
            field_type,
        );
        if fact.field_kind == ExternalFieldKind::EnumVariant {
            self.add_java_type(field_type);
            self.add_external_trait_impl_surfaces_for_type(facts, field_type);
            return;
        }
        if fact.field_kind == ExternalFieldKind::StaticFinal
            && (is_known_value_object_static_constant(&receiver_key, &fact.field.field_name)
                || !self_typed_enum_constant)
        {
            return;
        }
        if self_typed_enum_constant || fact.field_kind == ExternalFieldKind::EnumVariant {
            self.add_java_type(field_type);
            self.add_external_trait_impl_surfaces_for_type(facts, field_type);
            let rust_type = if fact.field_kind == ExternalFieldKind::EnumVariant
                || external_enum_constant_field_type_matches_owner(&receiver_key, field_type)
            {
                owner.clone()
            } else {
                external_facade_return_type_with_facts(field_type, facts)
            };
            let mut rust_names = BTreeSet::new();
            rust_names.insert(crate::parser::to_snake_case(&fact.field.field_name));
            if valid_rust_const_ident(&fact.field.field_name) {
                rust_names.insert(fact.field.field_name.clone());
            }
            for rust_name in rust_names {
                if valid_rust_const_ident(&rust_name) {
                    self.add_method(
                        owner.clone(),
                        format!("    pub const {rust_name}: {rust_type} = {rust_type};"),
                    );
                }
            }
            return;
        }
        let rust_name = crate::parser::to_snake_case(&fact.field.field_name);
        if !valid_rust_field_ident(&rust_name) {
            return;
        }
        let rust_type = external_facade_return_type_with_facts(field_type, facts);
        self.add_java_type(field_type);
        self.fields
            .entry(owner)
            .or_default()
            .insert(rust_name, rust_type);
    }

    fn add_external_trait_impl_surfaces_for_type(&mut self, facts: &FactSet, java_type: &JavaType) {
        let Some(type_name) = java_type_name_for_external_facade(java_type) else {
            return;
        };
        let implementors = external_trait_impl_rust_type_candidates(type_name);
        if implementors.is_empty() {
            return;
        }
        for capability in facts.type_capability_overrides.iter().filter(|capability| {
            crate::facts::external_type_name_matches(&capability.type_name, type_name)
        }) {
            for implemented in &capability.implements {
                let Some(implemented_name) = java_type_name_for_external_facade(implemented) else {
                    continue;
                };
                if facts.object_safe_trait_fact_for_name(implemented_name) != Some(true) {
                    continue;
                }
                for trait_name in
                    external_trait_impl_trait_name_candidates(implemented_name, &BTreeMap::new())
                {
                    for implementor in &implementors {
                        self.trait_impls
                            .entry(trait_name.clone())
                            .or_default()
                            .insert(implementor.clone());
                    }
                }
            }
        }
    }

    fn add_builder_surface_from_used_calls(
        &mut self,
        facts: &FactSet,
        used_calls: &BTreeMap<String, BTreeSet<usize>>,
    ) {
        let builder_types = self
            .opaque_types
            .iter()
            .filter(|name| name.ends_with("Builder"))
            .cloned()
            .collect::<Vec<_>>();
        for builder in builder_types {
            self.add_opaque_type(builder.clone());
            for (method, arities) in used_calls {
                for arg_count in arities {
                    if method == "build" {
                        if self.has_method_signature(&builder, method, *arg_count) {
                            continue;
                        }
                        let return_type =
                            self.external_builder_build_return_type(facts, &builder, *arg_count);
                        self.add_method(
                            builder.clone(),
                            external_facade_instance_method(
                                method,
                                *arg_count,
                                return_type.as_deref().unwrap_or("Box<dyn std::any::Any>"),
                            ),
                        );
                    } else if is_external_facade_builder_fluent_method(method) {
                        if self.has_method_signature(&builder, method, *arg_count) {
                            continue;
                        }
                        self.add_method(
                            builder.clone(),
                            external_facade_builder_fluent_method(method, *arg_count),
                        );
                    } else if method.starts_with("get_") && method.ends_with("_builder") {
                        if self.has_method_signature_on_owner_or_alias(&builder, method, *arg_count)
                        {
                            continue;
                        }
                        let return_rust =
                            external_facade_unmodelled_result_type(&builder, method, *arg_count);
                        self.add_opaque_type(return_rust.clone());
                        self.add_method(
                            builder.clone(),
                            external_facade_builder_unmodelled_getter(
                                &builder,
                                method,
                                method,
                                *arg_count,
                                &return_rust,
                            ),
                        );
                    }
                }
            }
        }
    }

    fn external_builder_build_return_type(
        &mut self,
        facts: &FactSet,
        builder: &str,
        arg_count: usize,
    ) -> Option<String> {
        if let Some(return_rust) =
            self.external_builder_build_return_type_from_facts(facts, builder, arg_count)
        {
            return Some(return_rust);
        }
        let product = external_builder_product_type(builder)?;
        let product_rust = rust_name_for_java_named_type(&product);
        if !valid_rust_type_ident(&product_rust) || weak_external_facade_return_type(&product_rust)
        {
            return None;
        }
        self.add_opaque_type(product_rust.clone());
        Some(product_rust)
    }

    fn external_builder_build_return_type_from_facts(
        &mut self,
        facts: &FactSet,
        builder: &str,
        arg_count: usize,
    ) -> Option<String> {
        let effect = facts
            .external_api_effect_facts
            .iter()
            .rev()
            .find(|effect| {
                effect.arg_count.is_none_or(|count| count == arg_count)
                    && crate::parser::to_snake_case(&effect.method_name) == "build"
                    && effect.receiver_type.as_deref().is_some_and(|receiver| {
                        let receiver_rust = rust_name_for_java_named_type(
                            &canonical_external_receiver_type_key(receiver),
                        );
                        receiver_rust == builder
                            || self
                                .aliases
                                .get(&receiver_rust)
                                .is_some_and(|target| target == builder)
                    })
            })?;
        let return_type = effect.return_type.as_ref()?;
        let return_rust = external_facade_method_return_type_with_facts(
            facts,
            Some(builder),
            "build",
            effect.arg_count,
            return_type,
            effect.return_nullability,
        );
        if weak_external_facade_return_type(&return_rust) {
            return None;
        }
        self.add_java_type(return_type);
        Some(return_rust)
    }

    fn add_java_type(&mut self, java_type: &JavaType) {
        match java_type {
            JavaType::Named { name, args } => {
                let rendered = rust_name_for_java_named_type(name);
                if valid_rust_type_ident(&rendered) {
                    self.add_opaque_type(rendered);
                }
                self.add_aliases_for_nested_return(name);
                for arg in args {
                    self.add_java_type(arg);
                }
            }
            JavaType::Array(inner) => self.add_java_type(inner),
            JavaType::Wildcard(Some(bound)) => match bound {
                WildcardBound::Extends(inner) | WildcardBound::Super(inner) => {
                    self.add_java_type(inner)
                }
            },
            JavaType::Primitive(_)
            | JavaType::Void
            | JavaType::String
            | JavaType::Wildcard(None)
            | JavaType::Unknown(_) => {}
        }
    }

    fn add_aliases_for_nested_return(&mut self, name: &str) {
        let Some((base, rest)) = name.split_once('.') else {
            return;
        };
        let base_rust = rust_name_for_java_named_type(base);
        let rest_rust = rust_name_for_java_named_type(rest);
        let target = format!("{base_rust}{rest_rust}");
        let aliases = self
            .aliases
            .iter()
            .filter_map(|(alias, alias_target)| {
                (alias_target == &base_rust).then(|| format!("{alias}{rest_rust}"))
            })
            .collect::<Vec<_>>();
        for alias in aliases {
            self.add_type_alias(alias, target.clone());
        }
    }

    fn add_opaque_type(&mut self, name: String) {
        if is_awt_platform_facade_owner(&name) || is_awt_platform_facade_rust_ident(&name) {
            return;
        }
        if valid_external_facade_type_ident(&name) {
            self.opaque_types.insert(name);
        }
    }

    fn add_external_facade_return_opaque_type(&mut self, return_type: &str) {
        if let Some(inner) = raw_rust_option_inner(return_type) {
            self.add_opaque_type(inner.to_string());
            return;
        }
        self.add_opaque_type(return_type.to_string());
    }

    fn add_type_alias(&mut self, alias: String, target: String) {
        if is_awt_platform_facade_owner(&alias)
            || is_awt_platform_facade_owner(&target)
            || is_awt_platform_facade_rust_ident(&alias)
            || is_awt_platform_facade_rust_ident(&target)
        {
            return;
        }
        if alias != target
            && valid_external_facade_type_ident(&alias)
            && valid_external_facade_type_ident(&target)
        {
            self.add_opaque_type(target.clone());
            self.aliases.insert(alias, target);
        }
    }

    fn add_method(&mut self, owner: String, method: String) {
        if is_awt_platform_facade_owner(&owner) {
            return;
        }
        if !valid_external_facade_type_ident(&owner) {
            return;
        }
        let method = self.upgrade_known_weak_opaque_method(&owner, method);
        if let Some(const_name) = facade_const_name(&method)
            && self.has_const_named(&owner, const_name)
        {
            return;
        }
        let Some(method_name) = facade_method_name(&method) else {
            self.impls.entry(owner).or_default().insert(method);
            return;
        };
        let arg_count = facade_method_arg_count(&method);
        if self.emitted_source_methods.contains(&(
            owner.clone(),
            method_name.to_string(),
            arg_count,
        )) {
            return;
        }
        let receiver_kind = facade_method_receiver_kind(&method);
        if self.has_method_signature_with_receiver(&owner, method_name, arg_count, receiver_kind) {
            if !self.replace_weak_method_return_with_receiver(
                &owner,
                method_name,
                arg_count,
                receiver_kind,
                &method,
            ) {
                return;
            }
        }
        if self.has_method_signature(&owner, method_name, arg_count) {
            if receiver_kind == FacadeMethodReceiverKind::Static {
                self.remove_method_signature(&owner, method_name, arg_count);
            } else if !self.replace_weak_method_return(&owner, method_name, arg_count, &method) {
                return;
            }
        }
        if matches!(method_name, "read" | "write") && arg_count > 1 {
            let overload_name = format!("{method_name}_{arg_count}");
            if !self.has_method_signature(&owner, &overload_name, arg_count)
                && let Some(overload_method) =
                    rename_facade_method(&method, method_name, &overload_name)
            {
                self.impls
                    .entry(owner.clone())
                    .or_default()
                    .insert(overload_method);
            }
        }
        self.impls.entry(owner).or_default().insert(method);
    }

    fn upgrade_known_weak_opaque_method(&self, owner: &str, method: String) -> String {
        let Some(method_name) = facade_method_name(&method) else {
            return method;
        };
        let Some(return_type) = facade_method_return_type(&method, method_name) else {
            return method;
        };
        if !weak_external_facade_return_type(return_type) {
            return method;
        }
        let arg_count = facade_method_arg_count(&method);
        let java_method = lower_camel_from_snake(method_name);
        let Some(java_return_type) =
            Self::known_missing_opaque_support_method_return_type(owner, &java_method, arg_count)
        else {
            return method;
        };
        let return_type =
            external_facade_return_type_with_facts(&java_return_type, &FactSet::default());
        match facade_method_receiver_kind(&method) {
            FacadeMethodReceiverKind::Static => {
                external_facade_static_method(method_name, arg_count, &return_type)
            }
            FacadeMethodReceiverKind::SharedSelf => {
                external_facade_instance_method(method_name, arg_count, &return_type)
            }
            FacadeMethodReceiverKind::MutableSelf | FacadeMethodReceiverKind::OwnedSelf => method,
        }
    }

    fn has_surface_type(&self, owner: &str) -> bool {
        self.opaque_types.contains(owner)
            || self.impls.contains_key(owner)
            || self.fields.contains_key(owner)
            || self.trait_type_names.contains(owner)
            || self.aliases.contains_key(owner)
            || self.aliases.values().any(|target| target == owner)
    }

    fn has_method_named(&self, owner: &str, method: &str) -> bool {
        self.impls.get(owner).is_some_and(|methods| {
            methods
                .iter()
                .any(|body| facade_method_has_name(body, method))
        })
    }

    fn has_const_named(&self, owner: &str, const_name: &str) -> bool {
        self.impls.get(owner).is_some_and(|methods| {
            methods
                .iter()
                .any(|body| facade_const_name(body) == Some(const_name))
        })
    }

    fn has_method_signature(&self, owner: &str, method: &str, arg_count: usize) -> bool {
        self.impls.get(owner).is_some_and(|methods| {
            methods
                .iter()
                .any(|body| facade_method_matches_signature(body, method, arg_count))
        })
    }

    fn has_method_signature_on_owner_or_alias(
        &self,
        owner: &str,
        method: &str,
        arg_count: usize,
    ) -> bool {
        if self.has_method_signature(owner, method, arg_count) {
            return true;
        }
        if let Some(target) = self.aliases.get(owner)
            && self.has_method_signature(target, method, arg_count)
        {
            return true;
        }
        self.aliases.iter().any(|(alias, target)| {
            target == owner && self.has_method_signature(alias, method, arg_count)
        })
    }

    fn has_method_signature_with_receiver(
        &self,
        owner: &str,
        method: &str,
        arg_count: usize,
        receiver_kind: FacadeMethodReceiverKind,
    ) -> bool {
        self.impls.get(owner).is_some_and(|methods| {
            methods.iter().any(|body| {
                facade_method_matches_signature(body, method, arg_count)
                    && facade_method_receiver_kind(body) == receiver_kind
            })
        })
    }

    fn remove_method_signature(&mut self, owner: &str, method: &str, arg_count: usize) {
        let Some(methods) = self.impls.get_mut(owner) else {
            return;
        };
        methods.retain(|body| !facade_method_matches_signature(body, method, arg_count));
    }

    fn replace_weak_method_return(
        &mut self,
        owner: &str,
        method: &str,
        arg_count: usize,
        replacement: &str,
    ) -> bool {
        let Some(replacement_return) = facade_method_return_type(replacement, method) else {
            return false;
        };
        if weak_external_facade_return_type(replacement_return) {
            return false;
        }
        let Some(methods) = self.impls.get_mut(owner) else {
            return false;
        };
        let weak_methods = methods
            .iter()
            .filter_map(|body| {
                if !facade_method_matches_signature(body, method, arg_count) {
                    return None;
                }
                let return_type = facade_method_return_type(body, method)?;
                (weak_external_facade_return_type(return_type)).then(|| body.clone())
            })
            .collect::<Vec<_>>();
        if weak_methods.is_empty() {
            return false;
        }
        for weak in weak_methods {
            methods.remove(&weak);
        }
        true
    }

    fn replace_weak_method_return_with_receiver(
        &mut self,
        owner: &str,
        method: &str,
        arg_count: usize,
        receiver_kind: FacadeMethodReceiverKind,
        replacement: &str,
    ) -> bool {
        let Some(replacement_return) = facade_method_return_type(replacement, method) else {
            return false;
        };
        if weak_external_facade_return_type(replacement_return) {
            return false;
        }
        let Some(methods) = self.impls.get_mut(owner) else {
            return false;
        };
        let weak_methods = methods
            .iter()
            .filter_map(|body| {
                if !facade_method_matches_signature(body, method, arg_count)
                    || facade_method_receiver_kind(body) != receiver_kind
                {
                    return None;
                }
                let return_type = facade_method_return_type(body, method)?;
                (weak_external_facade_return_type(return_type)).then(|| body.clone())
            })
            .collect::<Vec<_>>();
        if weak_methods.is_empty() {
            return false;
        }
        for weak in weak_methods {
            methods.remove(&weak);
        }
        true
    }

    fn replace_method_signature(
        &mut self,
        owner: &str,
        method: &str,
        arg_count: usize,
        replacement: &str,
    ) -> bool {
        let Some(methods) = self.impls.get_mut(owner) else {
            return false;
        };
        let matching_methods = methods
            .iter()
            .filter(|body| facade_method_matches_signature(body, method, arg_count))
            .cloned()
            .collect::<Vec<_>>();
        if matching_methods.is_empty() {
            return false;
        }
        for matching in matching_methods {
            if matching != replacement {
                methods.remove(&matching);
            }
        }
        true
    }

    fn add_erased_object_method(
        &mut self,
        facts: &FactSet,
        receiver_type: Option<&str>,
        name: String,
        arg_count: usize,
        return_type: String,
    ) {
        if self.erased_object_method_conflicts.contains(&name) {
            return;
        }
        let return_type =
            normalize_external_facade_support_return_type(facts, receiver_type, &return_type);
        match self.erased_object_methods.get(&name) {
            Some(existing)
                if existing.arg_count == arg_count && existing.return_type == return_type => {}
            Some(existing)
                if existing.arg_count == arg_count
                    && weak_external_facade_return_type(&existing.return_type)
                    && !weak_external_facade_return_type(&return_type) =>
            {
                self.erased_object_methods.insert(
                    name.clone(),
                    ExternalErasedObjectMethod {
                        name,
                        arg_count,
                        return_type,
                    },
                );
            }
            Some(_) => {
                self.erased_object_methods.remove(&name);
                self.erased_object_method_conflicts.insert(name);
            }
            None => {
                self.erased_object_methods.insert(
                    name.clone(),
                    ExternalErasedObjectMethod {
                        name,
                        arg_count,
                        return_type,
                    },
                );
            }
        }
    }

    fn add_external_dyn_trait_surfaces(&mut self, facts: &FactSet, local_types: &BTreeSet<String>) {
        let mut merged_facts = facts.clone();
        merged_facts.extend_from(crate::external_packs::builtin_external_library_packs());
        let mut trait_type_to_name = BTreeMap::new();
        for capability in &merged_facts.type_capability_overrides {
            if capability.object_safe_trait != Some(true) {
                continue;
            }
            if local_type_set_matches_external_name(local_types, &capability.type_name) {
                continue;
            }
            let type_key = canonical_external_receiver_type_key(&capability.type_name);
            let trait_name = rust_name_for_java_named_type(&type_key);
            if !valid_external_facade_type_ident(&type_key)
                || is_builtin_external_facade_type(&type_key)
                || !valid_external_facade_type_ident(&trait_name)
            {
                continue;
            }
            self.trait_type_names.insert(type_key.clone());
            self.traits
                .entry(type_key.clone())
                .or_insert_with(|| ExternalFacadeTrait {
                    trait_name: trait_name.clone(),
                    methods: BTreeSet::new(),
                });
            trait_type_to_name.insert(type_key, trait_name);
        }
        for override_fact in &merged_facts.hierarchy_overrides {
            let HierarchyStrategy::DynTrait { trait_name } = &override_fact.strategy else {
                continue;
            };
            let type_key = canonical_external_receiver_type_key(&override_fact.type_name);
            if !valid_external_facade_type_ident(&type_key)
                || is_builtin_external_facade_type(&type_key)
                || !valid_external_facade_type_ident(trait_name)
            {
                continue;
            }
            self.trait_type_names.insert(type_key.clone());
            self.traits
                .entry(type_key.clone())
                .or_insert_with(|| ExternalFacadeTrait {
                    trait_name: trait_name.clone(),
                    methods: BTreeSet::new(),
                });
            trait_type_to_name.insert(type_key, trait_name.clone());
        }

        if trait_type_to_name.is_empty() {
            return;
        }

        for effect in &merged_facts.external_api_effect_facts {
            let Some(receiver) = effect.receiver_type.as_deref() else {
                continue;
            };
            let receiver_key = canonical_external_receiver_type_key(receiver);
            let Some(trait_surface) = self.traits.get_mut(&receiver_key) else {
                continue;
            };
            let arg_count = effect.arg_count.unwrap_or(0);
            let rust_method = crate::call_plan::external_method_rust_name(
                receiver,
                &effect.method_name,
                arg_count,
                &merged_facts,
            );
            if !valid_rust_method_ident(&rust_method) {
                continue;
            }
            let return_type = effect
                .return_type
                .as_ref()
                .map(|return_type| {
                    external_facade_method_return_type_with_facts(
                        &merged_facts,
                        Some(receiver),
                        &effect.method_name,
                        Some(arg_count),
                        return_type,
                        effect.return_nullability,
                    )
                })
                .unwrap_or_else(|| "()".to_string());
            trait_surface.methods.insert(external_facade_trait_method(
                &rust_method,
                arg_count,
                &return_type,
            ));
        }

        for capability in &merged_facts.type_capability_overrides {
            let implementors = external_trait_impl_rust_type_candidates(&capability.type_name);
            for implemented in &capability.implements {
                let Some(implemented_name) = java_type_name_for_external_facade(implemented) else {
                    continue;
                };
                if merged_facts.object_safe_trait_fact_for_name(implemented_name) != Some(true) {
                    continue;
                }
                if local_type_set_matches_external_name(local_types, implemented_name) {
                    continue;
                }
                let trait_names = external_trait_impl_trait_name_candidates(
                    implemented_name,
                    &trait_type_to_name,
                );
                for implementor in &implementors {
                    for trait_name in &trait_names {
                        self.trait_impls
                            .entry(trait_name.clone())
                            .or_default()
                            .insert(implementor.clone());
                    }
                }
            }
        }
    }

    fn add_external_conversion_surfaces(
        &mut self,
        facts: &FactSet,
        local_types: &BTreeSet<String>,
    ) {
        for adapter in facts
            .external_conversion_adapter_facts
            .iter()
            .cloned()
            .chain(crate::external_packs::external_conversion_adapters())
        {
            let ExternalConversionAdapterKind::Into = adapter.kind;
            let from_key = canonical_external_receiver_type_key(&adapter.from_type);
            let to_key = canonical_external_receiver_type_key(&adapter.to_type);
            if local_types.contains(&from_key)
                || (local_types.contains(&to_key) && !is_builtin_external_facade_type(&to_key))
            {
                continue;
            }
            let from = rust_name_for_java_named_type(&from_key);
            let to = rust_name_for_java_named_type(&to_key);
            if local_types.contains(&from)
                || (local_types.contains(&to) && !is_builtin_external_facade_type(&to_key))
            {
                continue;
            }
            if !valid_external_facade_type_ident(&from)
                || !valid_external_facade_type_ident(&to)
                || is_builtin_external_facade_type(&from)
                || from == to
            {
                continue;
            }
            self.add_opaque_type(from.clone());
            if !is_builtin_external_facade_type(&to) {
                self.add_opaque_type(to.clone());
            }
            self.from_impls.insert((from, to));
        }
        self.add_jdk_input_stream_conversion_surfaces(local_types);
        self.add_jdk_exact_stream_conversion_surfaces(local_types);
    }

    fn add_jdk_input_stream_conversion_surfaces(&mut self, local_types: &BTreeSet<String>) {
        for from in [
            "FileInputStream",
            "ByteArrayInputStream",
            "BufferedInputStream",
            "DataInputStream",
            "GZIPInputStream",
            "ObjectInputStream",
            "FilterInputStream",
            "PushbackInputStream",
            "SequenceInputStream",
        ] {
            let from_key = canonical_external_receiver_type_key(from);
            if local_types.contains(&from_key) {
                continue;
            }
            let from = rust_name_for_java_named_type(&from_key);
            if !valid_rust_type_ident(&from) || from == "InputStream" {
                continue;
            }
            self.add_opaque_type(from.clone());
            self.from_impls.insert((from, "InputStream".to_string()));
        }
    }

    fn add_jdk_exact_stream_conversion_surfaces(&mut self, local_types: &BTreeSet<String>) {
        for (from, to) in [
            ("InputStream", "java.io.InputStream"),
            ("java.io.InputStream", "InputStream"),
            ("OutputStream", "java.io.OutputStream"),
            ("java.io.OutputStream", "OutputStream"),
            ("FileInputStream", "java.io.InputStream"),
            ("ByteArrayInputStream", "java.io.InputStream"),
            ("BufferedInputStream", "java.io.InputStream"),
            ("DataInputStream", "java.io.InputStream"),
            ("GZIPInputStream", "java.io.InputStream"),
            ("ObjectInputStream", "java.io.InputStream"),
            ("FilterInputStream", "java.io.InputStream"),
            ("PushbackInputStream", "java.io.InputStream"),
            ("SequenceInputStream", "java.io.InputStream"),
            ("FileOutputStream", "java.io.OutputStream"),
            ("java.io.ByteArrayOutputStream", "java.io.OutputStream"),
            ("ByteArrayOutputStream", "java.io.OutputStream"),
            ("BufferedOutputStream", "java.io.OutputStream"),
            ("FilterOutputStream", "java.io.OutputStream"),
            ("ObjectOutputStream", "java.io.OutputStream"),
            ("GZIPOutputStream", "java.io.OutputStream"),
        ] {
            let from_key = canonical_external_receiver_type_key(from);
            let to_key = canonical_external_receiver_type_key(to);
            if (local_types.contains(&from_key)
                && !matches!(from, "InputStream" | "OutputStream" | "java.io.InputStream"))
                || (local_types.contains(&to_key) && !is_builtin_external_facade_type(&to_key))
            {
                continue;
            }
            let from = rust_name_for_exact_jdk_stream_adapter_endpoint(from);
            let to = rust_name_for_exact_jdk_stream_adapter_endpoint(to);
            if (!valid_external_facade_type_ident(&to) && to != "InputStream") || from == to {
                continue;
            }
            if !is_builtin_external_facade_type(&from) && valid_external_facade_type_ident(&from) {
                self.add_opaque_type(from.clone());
            }
            self.add_opaque_type(to.clone());
            self.from_impls.insert((from, to));
        }
    }

    fn suppress_rust_crate_mapped_external_types(&mut self, facts: &FactSet) {
        let mapped_types = rust_crate_mapped_external_facade_type_names(facts);
        if mapped_types.is_empty() {
            return;
        }

        self.opaque_types
            .retain(|name| !mapped_types.contains(name));
        self.trait_type_names
            .retain(|name| !mapped_types.contains(name));
        self.traits.retain(|type_name, trait_surface| {
            !mapped_types.contains(type_name) && !mapped_types.contains(&trait_surface.trait_name)
        });
        self.aliases.retain(|alias, target| {
            !mapped_types.contains(alias) && !mapped_types.contains(target)
        });
        self.fields.retain(|owner, _| !mapped_types.contains(owner));
        self.impls.retain(|owner, _| !mapped_types.contains(owner));
        self.trait_impls.retain(|trait_name, implementors| {
            if mapped_types.contains(trait_name) {
                return false;
            }
            implementors.retain(|implementor| !mapped_types.contains(implementor));
            !implementors.is_empty()
        });
        self.from_impls
            .retain(|(from, to)| !mapped_types.contains(from) && !mapped_types.contains(to));
    }

    fn suppress_emitted_java_enum_types(&mut self, program: &Program) {
        let enum_types = emitted_java_enum_rust_type_names(program);
        if enum_types.is_empty() {
            return;
        }

        self.opaque_types.retain(|name| !enum_types.contains(name));
        self.fields.retain(|owner, _| !enum_types.contains(owner));
        self.impls.retain(|owner, _| !enum_types.contains(owner));
        self.trait_impls.retain(|_, implementors| {
            implementors.retain(|implementor| !enum_types.contains(implementor));
            !implementors.is_empty()
        });
        self.aliases
            .retain(|alias, target| !enum_types.contains(alias) && !enum_types.contains(target));
        self.from_impls
            .retain(|(from, to)| !enum_types.contains(from) && !enum_types.contains(to));
    }

    fn add_javalibs_static_owner_aliases(&mut self) {
        for (alias, target) in [
            ("OrgDisqBioDisqBaiWriteOption", "BaiWriteOption"),
            ("OrgDisqBioDisqSbiWriteOption", "SbiWriteOption"),
            (
                "OrgDisqBioDisqTabixIndexWriteOption",
                "TabixIndexWriteOption",
            ),
        ] {
            if self.opaque_types.contains(target) || self.impls.contains_key(target) {
                self.add_type_alias(alias.to_string(), target.to_string());
            }
        }
    }

    fn into_items(mut self) -> Vec<crate::rust_ir::RustItem> {
        self.add_javalibs_static_owner_aliases();
        let alias_names = self.aliases.keys().cloned().collect::<BTreeSet<_>>();
        let aliases = self.aliases.clone();
        for (alias, target) in aliases {
            if alias == target {
                continue;
            }
            if let Some(methods) = self.impls.remove(&alias) {
                self.impls
                    .entry(target.clone())
                    .or_default()
                    .extend(methods);
            }
            if let Some(fields) = self.fields.remove(&alias) {
                self.fields.entry(target).or_default().extend(fields);
            }
        }
        self.opaque_types.retain(|name| {
            !alias_names.contains(name)
                && !self.trait_type_names.contains(name)
                && !is_builtin_external_facade_type(name)
        });

        self.add_known_jdk_static_field_surfaces();

        let emitted_support_type_names = self
            .opaque_types
            .iter()
            .cloned()
            .chain(self.aliases.keys().cloned())
            .chain(self.aliases.values().cloned())
            .collect::<BTreeSet<_>>();
        let should_emit_level_constants_fallback = emitted_support_type_names.contains("Level")
            && !self.has_const_named("Level", "WARNING");
        let display_and_eq_types = self.opaque_types.iter().cloned().collect::<Vec<_>>();
        for ty in &display_and_eq_types {
            if crate::javalibs::needs_conversion_constructor_for_facade(ty)
                && !self.has_method_named(ty, "new_1")
            {
                self.add_method(
                    ty.clone(),
                    "    pub fn new_1<T0>(_arg0: T0) -> Self { Default::default() }".to_string(),
                );
            }
            if ty == "Tuple2" {
                if !self.has_method_named(ty, "_1") {
                    self.add_method(
                        ty.clone(),
                        "    pub fn _1(&self) -> Box<dyn std::any::Any> { Box::new(()) }"
                            .to_string(),
                    );
                }
                if !self.has_method_named(ty, "_2") {
                    self.add_method(
                        ty.clone(),
                        "    pub fn _2(&self) -> Box<dyn std::any::Any> { Box::new(()) }"
                            .to_string(),
                    );
                }
            }
        }
        let mut lines = Vec::new();
        for ty in self.opaque_types {
            if let Some(fields) = self.fields.remove(&ty)
                && !fields.is_empty()
            {
                let field_lines = fields
                    .into_iter()
                    .map(|(name, ty)| {
                        let name = rust_raw_ident_if_needed(&name);
                        format!("    pub {name}: {ty},")
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                lines.push(format!(
                    "#[derive(Clone, Debug)]\npub struct {ty} {{\n{field_lines}\n}}\n\nimpl Default for {ty} {{\n    fn default() -> Self {{ panic!(\"external facade placeholder {ty} has no default value\") }}\n}}"
                ));
            } else {
                lines.push(format!(
                    "#[derive(Clone, Debug, Default)]\npub struct {ty};"
                ));
            }
        }
        for ty in &display_and_eq_types {
            lines.push(external_facade_eq_impl(ty));
            lines.push(external_facade_hash_impl(ty));
            lines.push(external_facade_display_impl(ty));
        }
        if should_emit_level_constants_fallback {
            lines.push(java_logging_level_constants_impl());
        }
        for (alias, target) in self.aliases {
            if !is_builtin_external_facade_type(&alias) && !is_builtin_external_facade_type(&target)
            {
                lines.push(format!("pub type {alias} = {target};"));
            }
        }
        for trait_surface in self.traits.into_values() {
            lines.push(external_facade_trait_item(&trait_surface));
        }
        for (trait_name, implementors) in self.trait_impls {
            for implementor in implementors {
                if self.trait_type_names.contains(&implementor)
                    || !emitted_support_type_names.contains(&implementor)
                {
                    continue;
                }
                lines.push(format!("impl {trait_name} for {implementor} {{}}"));
            }
        }
        for (from, to) in self.from_impls {
            if alias_names.contains(&from)
                || alias_names.contains(&to)
                || (!is_builtin_external_facade_type(&from)
                    && !emitted_support_type_names.contains(&from))
                || (is_builtin_external_facade_type(&from)
                    && !is_allowed_builtin_external_from_impl(&from, &to))
                || matches!(
                    (from.as_str(), to.as_str()),
                    ("OutputStream", "JavaIoOutputStream")
                )
            {
                continue;
            }
            lines.push(format!(
                "impl From<{from}> for {to} {{\n    fn from(_value: {from}) -> Self {{ Default::default() }}\n}}"
            ));
        }
        for (owner, methods) in self.impls {
            if is_builtin_external_facade_type(&owner)
                || alias_names.contains(&owner)
                || self.trait_type_names.contains(&owner)
                || methods.is_empty()
            {
                continue;
            }
            lines.push(format!(
                "impl {owner} {{\n{}\n}}",
                methods.into_iter().collect::<Vec<_>>().join("\n")
            ));
        }
        if !self.erased_object_methods.is_empty() {
            let methods = self
                .erased_object_methods
                .values()
                .cloned()
                .collect::<Vec<_>>();
            let trait_methods = methods
                .iter()
                .map(external_erased_object_trait_method)
                .collect::<Vec<_>>()
                .join("\n");
            let impl_methods = methods
                .iter()
                .map(external_erased_object_impl_method)
                .collect::<Vec<_>>()
                .join("\n");
            lines.push(format!(
                "pub trait JavaErasedObjectFacade {{\n{trait_methods}\n}}\n\nimpl JavaErasedObjectFacade for Box<dyn std::any::Any> {{\n{impl_methods}\n}}"
            ));
        }

        if lines.is_empty() {
            Vec::new()
        } else {
            vec![crate::rust_ir::RustItem::Raw(lines.join("\n\n"))]
        }
    }

    fn add_known_jdk_static_field_surfaces(&mut self) {
        if self.has_surface_type("System") {
            self.add_opaque_type("System".to_string());
            self.add_opaque_type("InputStream".to_string());
            self.add_opaque_type("PrintStream".to_string());
            for (field, rust_type, value) in [
                ("out", "PrintStream", "PrintStream"),
                ("err", "PrintStream", "PrintStream"),
                ("r#in", "InputStream", "InputStream"),
            ] {
                if !self.has_method_named("System", field) {
                    self.add_method(
                        "System".to_string(),
                        format!("    pub const {field}: {rust_type} = {value};"),
                    );
                }
            }
        }
        if self.has_surface_type("Level") {
            self.add_opaque_type("Level".to_string());
            for field in ["SEVERE", "WARNING", "INFO", "DEBUG", "ERROR"] {
                if !self.has_const_named("Level", field) {
                    self.add_method(
                        "Level".to_string(),
                        format!("    pub const {field}: Level = Level;"),
                    );
                }
            }
        }
    }
}

fn is_known_value_object_static_constant(receiver_key: &str, field_name: &str) -> bool {
    let receiver = receiver_key.rsplit('.').next().unwrap_or(receiver_key);
    let field = canonical_external_field_name(field_name);
    matches!((receiver, field.as_str()), ("BigInteger", "ZERO" | "ONE"))
}

fn java_logging_level_constants_impl() -> String {
    "impl Level {\n    pub const SEVERE: Level = Level;\n    pub const WARNING: Level = Level;\n    pub const INFO: Level = Level;\n    pub const DEBUG: Level = Level;\n    pub const ERROR: Level = Level;\n}"
        .to_string()
}

fn rust_crate_mapped_external_facade_type_names(facts: &FactSet) -> BTreeSet<String> {
    facts
        .external_library_packs
        .iter()
        .filter(|pack| pack.mode == crate::facts::ExternalLibraryMode::RustCrateMapping)
        .flat_map(|pack| external_facade_type_name_aliases_for_pack_type(&pack.package_prefix))
        .collect()
}

fn external_facade_type_name_aliases_for_pack_type(type_name: &str) -> Vec<String> {
    let canonical = canonical_external_receiver_type_key(type_name);
    let mut aliases = vec![
        rust_name_for_java_named_type(type_name),
        rust_name_for_java_named_type(&canonical),
    ];
    if let Some(simple) = type_name.rsplit('.').next() {
        aliases.push(rust_name_for_java_named_type(simple));
        aliases.push(rust_name_for_java_named_type(
            &canonical_external_receiver_type_key(simple),
        ));
    }
    if let Some(simple) = canonical.rsplit('.').next() {
        aliases.push(rust_name_for_java_named_type(simple));
        aliases.push(rust_name_for_java_named_type(
            &canonical_external_receiver_type_key(simple),
        ));
    }
    aliases.retain(|alias| valid_external_facade_type_ident(alias));
    aliases.sort();
    aliases.dedup();
    aliases
}

fn collect_program_call_arities(program: &Program) -> BTreeMap<String, BTreeSet<usize>> {
    let mut calls = BTreeMap::new();
    for ty in &program.types {
        collect_type_call_arities(ty, &mut calls);
    }
    calls
}

fn collect_program_constructor_arities(program: &Program) -> BTreeMap<String, BTreeSet<usize>> {
    let mut constructors = BTreeMap::new();
    for ty in &program.types {
        collect_type_constructor_arities(ty, &mut constructors);
    }
    constructors
}

fn collect_imported_static_owner_calls(
    program: &Program,
) -> BTreeMap<(String, String), BTreeSet<usize>> {
    let aliases = imported_static_owner_aliases(program);
    let static_members = imported_static_member_owners(program);
    let mut calls = BTreeMap::new();
    for ty in &program.types {
        collect_type_imported_static_owner_calls(ty, &aliases, &static_members, &mut calls);
    }
    calls
}

fn imported_static_owner_call_contains(
    imported_static_owner_calls: &BTreeMap<(String, String), BTreeSet<usize>>,
    receiver_key: &str,
    rust_method: &str,
    fact_arg_count: Option<usize>,
    used_arities: &BTreeSet<usize>,
) -> bool {
    let candidate = imported_static_owner_calls
        .iter()
        .find(|((owner, method), _)| {
            imported_static_owner_matches_receiver_key(owner, receiver_key) && method == rust_method
        })
        .map(|(_, arities)| arities);
    let Some(static_arities) = candidate else {
        return false;
    };
    if let Some(arg_count) = fact_arg_count {
        return static_arities.contains(&arg_count);
    }
    used_arities
        .iter()
        .any(|arg_count| static_arities.contains(arg_count))
}

fn imported_static_owner_matches_receiver_key(owner: &str, receiver_key: &str) -> bool {
    if canonical_external_receiver_type_key(owner) == receiver_key {
        return true;
    }
    owner
        .split('.')
        .filter(|segment| !segment.is_empty())
        .next_back()
        .is_some_and(|simple| canonical_external_receiver_type_key(simple) == receiver_key)
}

fn imported_static_owner_aliases(program: &Program) -> BTreeMap<String, String> {
    let mut aliases = BTreeMap::new();
    for import in &program.imports {
        if import.is_wildcard && !import.is_static {
            continue;
        }
        let Some(type_name) = imported_static_owner_type_name(import) else {
            continue;
        };
        let simple = type_name
            .split('.')
            .filter(|segment| !segment.is_empty())
            .next_back()
            .unwrap_or(type_name.as_str());
        aliases.insert(simple.to_string(), type_name.clone());
        aliases.insert(crate::parser::to_snake_case(simple), type_name);
    }
    aliases
}

fn imported_static_member_owners(program: &Program) -> BTreeMap<String, String> {
    let mut members = BTreeMap::new();
    for import in &program.imports {
        if !import.is_static || import.is_wildcard {
            continue;
        }
        let Some((owner, member)) = import.path.rsplit_once('.') else {
            continue;
        };
        members.insert(member.to_string(), owner.to_string());
        members.insert(crate::parser::to_snake_case(member), owner.to_string());
    }
    members
}

fn imported_static_owner_type_name(import: &crate::java_ir::ImportDecl) -> Option<String> {
    if import.path.is_empty() {
        return None;
    }
    if !import.is_static {
        return Some(import.path.clone());
    }
    if import.is_wildcard {
        return Some(import.path.clone());
    }
    let owner = import.path.rsplit_once('.')?.0;
    (!owner.is_empty()).then(|| owner.to_string())
}

fn add_imported_static_shadow_name(shadows: &mut BTreeSet<String>, name: &str) {
    shadows.insert(name.to_string());
    shadows.insert(crate::parser::to_snake_case(name));
}

fn collect_block_imported_static_shadows(block: &Block, shadows: &mut BTreeSet<String>) {
    for stmt in &block.statements {
        collect_stmt_imported_static_shadows(stmt, shadows);
    }
}

fn collect_stmt_imported_static_shadows(stmt: &Stmt, shadows: &mut BTreeSet<String>) {
    match stmt {
        Stmt::VarDecl { name, .. } => add_imported_static_shadow_name(shadows, name),
        Stmt::For { init, body, .. } => {
            for stmt in init {
                collect_stmt_imported_static_shadows(stmt, shadows);
            }
            collect_block_imported_static_shadows(body, shadows);
        }
        Stmt::EnhancedFor { var_name, body, .. } => {
            add_imported_static_shadow_name(shadows, var_name);
            collect_block_imported_static_shadows(body, shadows);
        }
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            collect_block_imported_static_shadows(then_block, shadows);
            if let Some(else_block) = else_block {
                collect_block_imported_static_shadows(else_block, shadows);
            }
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::Block(body)
        | Stmt::LabeledBlock { body, .. } => {
            collect_block_imported_static_shadows(body, shadows);
        }
        Stmt::Switch { cases, .. } => {
            for case in cases {
                for stmt in &case.statements {
                    collect_stmt_imported_static_shadows(stmt, shadows);
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
                collect_stmt_imported_static_shadows(resource, shadows);
            }
            collect_block_imported_static_shadows(body, shadows);
            for catch in catches {
                add_imported_static_shadow_name(shadows, &catch.name);
                collect_block_imported_static_shadows(&catch.body, shadows);
            }
            if let Some(finally_block) = finally_block {
                collect_block_imported_static_shadows(finally_block, shadows);
            }
        }
        Stmt::LocalType(ty) => {
            for member in &ty.members {
                if let Member::Field(field) = member {
                    add_imported_static_shadow_name(shadows, &field.name);
                }
            }
        }
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

fn collect_type_imported_static_owner_calls(
    ty: &crate::java_ir::TypeDecl,
    aliases: &BTreeMap<String, String>,
    static_members: &BTreeMap<String, String>,
    calls: &mut BTreeMap<(String, String), BTreeSet<usize>>,
) {
    let mut type_shadows = BTreeSet::new();
    for member in &ty.members {
        if let Member::Field(field) = member {
            add_imported_static_shadow_name(&mut type_shadows, &field.name);
        }
    }
    for member in &ty.members {
        match member {
            Member::Field(field) => {
                if let Some(initializer) = &field.initializer {
                    collect_expr_imported_static_owner_calls(
                        initializer,
                        aliases,
                        static_members,
                        &type_shadows,
                        calls,
                    );
                }
            }
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    let mut shadows = type_shadows.clone();
                    for param in &method.params {
                        add_imported_static_shadow_name(&mut shadows, &param.name);
                    }
                    collect_block_imported_static_shadows(body, &mut shadows);
                    collect_block_imported_static_owner_calls(
                        body,
                        aliases,
                        static_members,
                        &shadows,
                        calls,
                    );
                }
            }
            Member::Constructor(constructor) => {
                if let Some(body) = &constructor.body {
                    let mut shadows = type_shadows.clone();
                    for param in &constructor.params {
                        add_imported_static_shadow_name(&mut shadows, &param.name);
                    }
                    collect_block_imported_static_shadows(body, &mut shadows);
                    collect_block_imported_static_owner_calls(
                        body,
                        aliases,
                        static_members,
                        &shadows,
                        calls,
                    );
                }
            }
            Member::Type(nested) => {
                collect_type_imported_static_owner_calls(nested, aliases, static_members, calls)
            }
            Member::Unsupported { .. } => {}
        }
    }
}

fn collect_block_imported_static_owner_calls(
    block: &Block,
    aliases: &BTreeMap<String, String>,
    static_members: &BTreeMap<String, String>,
    shadows: &BTreeSet<String>,
    calls: &mut BTreeMap<(String, String), BTreeSet<usize>>,
) {
    for stmt in &block.statements {
        collect_stmt_imported_static_owner_calls(stmt, aliases, static_members, shadows, calls);
    }
}

fn collect_stmt_imported_static_owner_calls(
    stmt: &Stmt,
    aliases: &BTreeMap<String, String>,
    static_members: &BTreeMap<String, String>,
    shadows: &BTreeSet<String>,
    calls: &mut BTreeMap<(String, String), BTreeSet<usize>>,
) {
    match stmt {
        Stmt::VarDecl {
            initializer: Some(expr),
            ..
        }
        | Stmt::Expr(expr)
        | Stmt::Throw(expr)
        | Stmt::Return(Some(expr)) => {
            collect_expr_imported_static_owner_calls(expr, aliases, static_members, shadows, calls)
        }
        Stmt::Assign { target, value } => {
            collect_expr_imported_static_owner_calls(
                target,
                aliases,
                static_members,
                shadows,
                calls,
            );
            collect_expr_imported_static_owner_calls(
                value,
                aliases,
                static_members,
                shadows,
                calls,
            );
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_expr_imported_static_owner_calls(
                condition,
                aliases,
                static_members,
                shadows,
                calls,
            );
            collect_block_imported_static_owner_calls(
                then_block,
                aliases,
                static_members,
                shadows,
                calls,
            );
            if let Some(else_block) = else_block {
                collect_block_imported_static_owner_calls(
                    else_block,
                    aliases,
                    static_members,
                    shadows,
                    calls,
                );
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            collect_expr_imported_static_owner_calls(
                condition,
                aliases,
                static_members,
                shadows,
                calls,
            );
            collect_block_imported_static_owner_calls(
                body,
                aliases,
                static_members,
                shadows,
                calls,
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
                collect_stmt_imported_static_owner_calls(
                    stmt,
                    aliases,
                    static_members,
                    shadows,
                    calls,
                );
            }
            if let Some(condition) = condition {
                collect_expr_imported_static_owner_calls(
                    condition,
                    aliases,
                    static_members,
                    shadows,
                    calls,
                );
            }
            for expr in update {
                collect_expr_imported_static_owner_calls(
                    expr,
                    aliases,
                    static_members,
                    shadows,
                    calls,
                );
            }
            collect_block_imported_static_owner_calls(
                body,
                aliases,
                static_members,
                shadows,
                calls,
            );
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            collect_expr_imported_static_owner_calls(
                iterable,
                aliases,
                static_members,
                shadows,
                calls,
            );
            collect_block_imported_static_owner_calls(
                body,
                aliases,
                static_members,
                shadows,
                calls,
            );
        }
        Stmt::Switch { selector, cases } => {
            collect_expr_imported_static_owner_calls(
                selector,
                aliases,
                static_members,
                shadows,
                calls,
            );
            for case in cases {
                for stmt in &case.statements {
                    collect_stmt_imported_static_owner_calls(
                        stmt,
                        aliases,
                        static_members,
                        shadows,
                        calls,
                    );
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
                collect_stmt_imported_static_owner_calls(
                    resource,
                    aliases,
                    static_members,
                    shadows,
                    calls,
                );
            }
            collect_block_imported_static_owner_calls(
                body,
                aliases,
                static_members,
                shadows,
                calls,
            );
            for catch in catches {
                collect_block_imported_static_owner_calls(
                    &catch.body,
                    aliases,
                    static_members,
                    shadows,
                    calls,
                );
            }
            if let Some(finally_block) = finally_block {
                collect_block_imported_static_owner_calls(
                    finally_block,
                    aliases,
                    static_members,
                    shadows,
                    calls,
                );
            }
        }
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                collect_expr_imported_static_owner_calls(
                    arg,
                    aliases,
                    static_members,
                    shadows,
                    calls,
                );
            }
        }
        Stmt::LocalType(ty) => {
            collect_type_imported_static_owner_calls(ty, aliases, static_members, calls)
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            collect_block_imported_static_owner_calls(
                block,
                aliases,
                static_members,
                shadows,
                calls,
            )
        }
        Stmt::VarDecl {
            initializer: None, ..
        }
        | Stmt::Return(None)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => {}
    }
}

fn unproven_simple_static_owner_candidate(receiver_name: &str) -> bool {
    receiver_name
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
        || receiver_name.ends_with("_utils")
}

fn collect_expr_imported_static_owner_calls(
    expr: &Expr,
    aliases: &BTreeMap<String, String>,
    static_members: &BTreeMap<String, String>,
    shadows: &BTreeSet<String>,
    calls: &mut BTreeMap<(String, String), BTreeSet<usize>>,
) {
    match expr {
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } => {
            if let Some(Expr::Var(receiver_name)) = receiver.as_deref()
                && !shadows.contains(receiver_name)
                && let Some(owner) = aliases.get(receiver_name)
            {
                calls
                    .entry((owner.clone(), crate::parser::to_snake_case(name)))
                    .or_default()
                    .insert(args.len());
            }
            if let Some(Expr::Var(receiver_name)) = receiver.as_deref()
                && !shadows.contains(receiver_name)
                && let Some(owner) =
                    external_pack_static_owner_for_call(receiver_name, name, args.len())
            {
                calls
                    .entry((owner, crate::parser::to_snake_case(name)))
                    .or_default()
                    .insert(args.len());
            }
            if let Some(Expr::Var(receiver_name)) = receiver.as_deref()
                && !shadows.contains(receiver_name)
                && aliases.get(receiver_name).is_none()
                && external_pack_static_owner_for_call(receiver_name, name, args.len()).is_none()
                && unproven_simple_static_owner_candidate(receiver_name)
            {
                calls
                    .entry((
                        upper_camel_from_snake(receiver_name),
                        crate::parser::to_snake_case(name),
                    ))
                    .or_default()
                    .insert(args.len());
            }
            if receiver.is_none() {
                let rust_name = crate::parser::to_snake_case(name);
                if let Some(owner) = static_members
                    .get(name)
                    .or_else(|| static_members.get(&rust_name))
                {
                    calls
                        .entry((owner.clone(), rust_name))
                        .or_default()
                        .insert(args.len());
                }
            }
            if let Some(receiver) = receiver.as_deref()
                && let Some(owner) = qualified_static_owner_java_name(receiver)
            {
                calls
                    .entry((owner, crate::parser::to_snake_case(name)))
                    .or_default()
                    .insert(args.len());
            }
            if let Some(receiver) = receiver {
                collect_expr_imported_static_owner_calls(
                    receiver,
                    aliases,
                    static_members,
                    shadows,
                    calls,
                );
            }
            for arg in args {
                collect_expr_imported_static_owner_calls(
                    arg,
                    aliases,
                    static_members,
                    shadows,
                    calls,
                );
            }
        }
        Expr::FieldAccess(receiver, _) => collect_expr_imported_static_owner_calls(
            receiver,
            aliases,
            static_members,
            shadows,
            calls,
        ),
        Expr::ArrayAccess(array, index) => {
            collect_expr_imported_static_owner_calls(
                array,
                aliases,
                static_members,
                shadows,
                calls,
            );
            collect_expr_imported_static_owner_calls(
                index,
                aliases,
                static_members,
                shadows,
                calls,
            );
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for dimension in dimensions {
                collect_expr_imported_static_owner_calls(
                    dimension,
                    aliases,
                    static_members,
                    shadows,
                    calls,
                );
            }
            if let Some(initializer) = initializer {
                for value in initializer {
                    collect_expr_imported_static_owner_calls(
                        value,
                        aliases,
                        static_members,
                        shadows,
                        calls,
                    );
                }
            }
        }
        Expr::ArrayInitializer(values) => {
            for value in values {
                collect_expr_imported_static_owner_calls(
                    value,
                    aliases,
                    static_members,
                    shadows,
                    calls,
                );
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_expr_imported_static_owner_calls(left, aliases, static_members, shadows, calls);
            collect_expr_imported_static_owner_calls(
                right,
                aliases,
                static_members,
                shadows,
                calls,
            );
        }
        Expr::Unary { expr, .. } => {
            collect_expr_imported_static_owner_calls(expr, aliases, static_members, shadows, calls)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_expr_imported_static_owner_calls(
                condition,
                aliases,
                static_members,
                shadows,
                calls,
            );
            collect_expr_imported_static_owner_calls(
                then_expr,
                aliases,
                static_members,
                shadows,
                calls,
            );
            collect_expr_imported_static_owner_calls(
                else_expr,
                aliases,
                static_members,
                shadows,
                calls,
            );
        }
        Expr::New { args, body, .. } => {
            for arg in args {
                collect_expr_imported_static_owner_calls(
                    arg,
                    aliases,
                    static_members,
                    shadows,
                    calls,
                );
            }
            if let Some(body) = body {
                for member in body {
                    match member {
                        Member::Field(field) => {
                            if let Some(initializer) = &field.initializer {
                                collect_expr_imported_static_owner_calls(
                                    initializer,
                                    aliases,
                                    static_members,
                                    shadows,
                                    calls,
                                );
                            }
                        }
                        Member::Method(method) => {
                            if let Some(body) = &method.body {
                                collect_block_imported_static_owner_calls(
                                    body,
                                    aliases,
                                    static_members,
                                    shadows,
                                    calls,
                                );
                            }
                        }
                        Member::Constructor(constructor) => {
                            if let Some(body) = &constructor.body {
                                collect_block_imported_static_owner_calls(
                                    body,
                                    aliases,
                                    static_members,
                                    shadows,
                                    calls,
                                );
                            }
                        }
                        Member::Type(nested) => collect_type_imported_static_owner_calls(
                            nested,
                            aliases,
                            static_members,
                            calls,
                        ),
                        Member::Unsupported { .. } => {}
                    }
                }
            }
        }
        Expr::Cast { expr, .. } => {
            collect_expr_imported_static_owner_calls(expr, aliases, static_members, shadows, calls)
        }
        Expr::InstanceOf { expr, .. } => {
            collect_expr_imported_static_owner_calls(expr, aliases, static_members, shadows, calls)
        }
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => collect_expr_imported_static_owner_calls(
                expr,
                aliases,
                static_members,
                shadows,
                calls,
            ),
            crate::java_ir::LambdaBody::Block(block) => collect_block_imported_static_owner_calls(
                block,
                aliases,
                static_members,
                shadows,
                calls,
            ),
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

fn external_pack_static_owner_for_call(
    receiver_name: &str,
    method_name: &str,
    arg_count: usize,
) -> Option<String> {
    let receiver_key = canonical_external_receiver_type_key(receiver_name);
    if receiver_name
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_lowercase())
        && receiver_key == receiver_name
        && !lowercase_static_owner_candidate(receiver_name)
    {
        return None;
    }
    let effect = crate::external_packs::external_api_effect_for_receiver_with_arity(
        Some(&receiver_key),
        method_name,
        Some(arg_count),
    )
    .or_else(|| {
        crate::external_packs::external_api_effect_for_receiver(Some(&receiver_key), method_name)
    })?;
    effect.receiver_type.or(Some(receiver_key))
}

fn lowercase_static_owner_candidate(receiver_name: &str) -> bool {
    receiver_name.contains('_')
        && matches!(
            receiver_name.rsplit('_').next(),
            Some(
                "adapter"
                    | "adapters"
                    | "builder"
                    | "builders"
                    | "factory"
                    | "factories"
                    | "format"
                    | "formats"
                    | "option"
                    | "options"
                    | "storage"
                    | "utils"
                    | "utilities"
            )
        )
}

fn qualified_static_owner_java_name(expr: &Expr) -> Option<String> {
    let Expr::FieldAccess(_, _) = expr else {
        return None;
    };
    let root = static_owner_root_segment(expr)?;
    if !root
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_lowercase())
    {
        return None;
    }
    let owner = static_owner_java_name(expr)?;
    let class_like_segments = owner
        .split('.')
        .filter(|segment| {
            segment
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_uppercase())
        })
        .count();
    (class_like_segments == 1).then_some(owner)
}

fn static_owner_root_segment(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Var(name) => Some(name.as_str()),
        Expr::FieldAccess(receiver, _) => static_owner_root_segment(receiver),
        _ => None,
    }
}

fn static_owner_java_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Var(name) => Some(name.clone()),
        Expr::FieldAccess(receiver, field) => {
            Some(format!("{}.{}", static_owner_java_name(receiver)?, field))
        }
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct DisplayContextReturnHint {
    owner: String,
    java_method: String,
    rust_method: String,
    arg_count: usize,
}

#[derive(Debug, Clone, Default)]
struct DisplayHintTypeContext {
    bindings: BTreeMap<String, JavaType>,
}

impl DisplayHintTypeContext {
    fn for_type(ty: &TypeDecl) -> Self {
        let mut ctx = Self::default();
        for member in &ty.members {
            if let Member::Field(field) = member {
                ctx.bindings.insert(field.name.clone(), field.ty.clone());
            }
        }
        ctx
    }

    fn with_params(&self, params: &[crate::java_ir::Param]) -> Self {
        let mut ctx = self.clone();
        for param in params {
            ctx.bindings.insert(param.name.clone(), param.ty.clone());
        }
        ctx
    }

    fn insert_local(&mut self, name: &str, ty: &JavaType) {
        ctx_insert_binding_aliases(&mut self.bindings, name, ty);
    }

    fn java_type_for_expr(&self, expr: &Expr) -> Option<&JavaType> {
        match expr {
            Expr::Var(name) => self.bindings.get(name),
            Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
                self.bindings.get(field)
            }
            _ => None,
        }
    }
}

fn ctx_insert_binding_aliases(
    bindings: &mut BTreeMap<String, JavaType>,
    name: &str,
    ty: &JavaType,
) {
    bindings.insert(name.to_string(), ty.clone());
    let snake = crate::parser::to_snake_case(name);
    if snake != name {
        bindings.insert(snake, ty.clone());
    }
}

fn collect_program_display_return_hints(program: &Program) -> BTreeSet<DisplayContextReturnHint> {
    let mut hints = BTreeSet::new();
    for ty in &program.types {
        collect_type_display_return_hints(ty, &mut hints);
    }
    hints
}

fn collect_type_display_return_hints(
    ty: &TypeDecl,
    hints: &mut BTreeSet<DisplayContextReturnHint>,
) {
    let type_ctx = DisplayHintTypeContext::for_type(ty);
    for member in &ty.members {
        match member {
            Member::Field(field) => {
                if let Some(initializer) = &field.initializer {
                    collect_expr_display_return_hints(initializer, &type_ctx, hints);
                }
            }
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    let mut ctx = type_ctx.with_params(&method.params);
                    collect_block_display_return_hints(body, &mut ctx, hints);
                }
            }
            Member::Constructor(constructor) => {
                if let Some(body) = &constructor.body {
                    let mut ctx = type_ctx.with_params(&constructor.params);
                    collect_block_display_return_hints(body, &mut ctx, hints);
                }
            }
            Member::Type(nested) => collect_type_display_return_hints(nested, hints),
            Member::Unsupported { .. } => {}
        }
    }
}

fn collect_block_display_return_hints(
    block: &Block,
    ctx: &mut DisplayHintTypeContext,
    hints: &mut BTreeSet<DisplayContextReturnHint>,
) {
    for stmt in &block.statements {
        collect_stmt_display_return_hints(stmt, ctx, hints);
    }
}

fn collect_stmt_display_return_hints(
    stmt: &Stmt,
    ctx: &mut DisplayHintTypeContext,
    hints: &mut BTreeSet<DisplayContextReturnHint>,
) {
    match stmt {
        Stmt::VarDecl {
            name,
            ty,
            initializer,
            ..
        } => {
            if let Some(expr) = initializer {
                collect_expr_display_return_hints(expr, ctx, hints);
            }
            ctx.insert_local(name, ty);
        }
        Stmt::Return(expr) => {
            if let Some(expr) = expr {
                collect_expr_display_return_hints(expr, ctx, hints);
            }
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => collect_expr_display_return_hints(expr, ctx, hints),
        Stmt::Assign { target, value } => {
            collect_expr_display_return_hints(target, ctx, hints);
            collect_expr_display_return_hints(value, ctx, hints);
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_expr_display_return_hints(condition, ctx, hints);
            let mut then_ctx = ctx.clone();
            collect_block_display_return_hints(then_block, &mut then_ctx, hints);
            if let Some(else_block) = else_block {
                let mut else_ctx = ctx.clone();
                collect_block_display_return_hints(else_block, &mut else_ctx, hints);
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            collect_expr_display_return_hints(condition, ctx, hints);
            let mut body_ctx = ctx.clone();
            collect_block_display_return_hints(body, &mut body_ctx, hints);
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
                collect_stmt_display_return_hints(stmt, &mut loop_ctx, hints);
            }
            if let Some(condition) = condition {
                collect_expr_display_return_hints(condition, &loop_ctx, hints);
            }
            for expr in update {
                collect_expr_display_return_hints(expr, &loop_ctx, hints);
            }
            collect_block_display_return_hints(body, &mut loop_ctx, hints);
        }
        Stmt::EnhancedFor {
            var_name,
            var_type,
            iterable,
            body,
            ..
        } => {
            collect_expr_display_return_hints(iterable, ctx, hints);
            let mut body_ctx = ctx.clone();
            body_ctx.insert_local(var_name, var_type);
            collect_block_display_return_hints(body, &mut body_ctx, hints);
        }
        Stmt::Switch { selector, cases } => {
            collect_expr_display_return_hints(selector, ctx, hints);
            for case in cases {
                let mut case_ctx = ctx.clone();
                for stmt in &case.statements {
                    collect_stmt_display_return_hints(stmt, &mut case_ctx, hints);
                }
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            let mut try_ctx = ctx.clone();
            for resource in resources {
                collect_stmt_display_return_hints(resource, &mut try_ctx, hints);
            }
            collect_block_display_return_hints(body, &mut try_ctx, hints);
            for catch in catches {
                let mut catch_ctx = ctx.clone();
                collect_block_display_return_hints(&catch.body, &mut catch_ctx, hints);
            }
            if let Some(finally_block) = finally_block {
                let mut finally_ctx = ctx.clone();
                collect_block_display_return_hints(finally_block, &mut finally_ctx, hints);
            }
        }
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                collect_expr_display_return_hints(arg, ctx, hints);
            }
        }
        Stmt::LocalType(ty) => collect_type_display_return_hints(ty, hints),
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            let mut block_ctx = ctx.clone();
            collect_block_display_return_hints(block, &mut block_ctx, hints);
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
    }
}

fn collect_expr_display_return_hints(
    expr: &Expr,
    ctx: &DisplayHintTypeContext,
    hints: &mut BTreeSet<DisplayContextReturnHint>,
) {
    match expr {
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } => {
            if display_sink_method(name) {
                for arg in args {
                    collect_display_sink_arg_return_hints(arg, ctx, hints);
                }
            }
            if let Some(receiver) = receiver {
                collect_expr_display_return_hints(receiver, ctx, hints);
            }
            for arg in args {
                collect_expr_display_return_hints(arg, ctx, hints);
            }
        }
        Expr::FieldAccess(receiver, _) => collect_expr_display_return_hints(receiver, ctx, hints),
        Expr::ArrayAccess(array, index) => {
            collect_expr_display_return_hints(array, ctx, hints);
            collect_expr_display_return_hints(index, ctx, hints);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for dimension in dimensions {
                collect_expr_display_return_hints(dimension, ctx, hints);
            }
            for value in initializer.iter().flatten() {
                collect_expr_display_return_hints(value, ctx, hints);
            }
        }
        Expr::ArrayInitializer(values) => {
            for value in values {
                collect_expr_display_return_hints(value, ctx, hints);
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_expr_display_return_hints(left, ctx, hints);
            collect_expr_display_return_hints(right, ctx, hints);
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_expr_display_return_hints(expr, ctx, hints);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_expr_display_return_hints(condition, ctx, hints);
            collect_expr_display_return_hints(then_expr, ctx, hints);
            collect_expr_display_return_hints(else_expr, ctx, hints);
        }
        Expr::New { args, body, .. } => {
            for arg in args {
                collect_expr_display_return_hints(arg, ctx, hints);
            }
            for member in body.iter().flatten() {
                match member {
                    Member::Field(field) => {
                        if let Some(initializer) = &field.initializer {
                            collect_expr_display_return_hints(initializer, ctx, hints);
                        }
                    }
                    Member::Method(method) => {
                        if let Some(body) = &method.body {
                            let mut nested_ctx = ctx.with_params(&method.params);
                            collect_block_display_return_hints(body, &mut nested_ctx, hints);
                        }
                    }
                    Member::Constructor(constructor) => {
                        if let Some(body) = &constructor.body {
                            let mut nested_ctx = ctx.with_params(&constructor.params);
                            collect_block_display_return_hints(body, &mut nested_ctx, hints);
                        }
                    }
                    Member::Type(nested) => collect_type_display_return_hints(nested, hints),
                    Member::Unsupported { .. } => {}
                }
            }
        }
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => {
                collect_expr_display_return_hints(expr, ctx, hints)
            }
            crate::java_ir::LambdaBody::Block(block) => {
                let mut lambda_ctx = ctx.clone();
                collect_block_display_return_hints(block, &mut lambda_ctx, hints);
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

fn collect_display_sink_arg_return_hints(
    expr: &Expr,
    ctx: &DisplayHintTypeContext,
    hints: &mut BTreeSet<DisplayContextReturnHint>,
) {
    match expr {
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } => {
            if let Some(owner) = ctx
                .java_type_for_expr(receiver)
                .and_then(display_hint_owner_for_type)
            {
                if display_hint_has_concrete_external_return(&owner, name, args.len()) {
                    collect_expr_display_return_hints(expr, ctx, hints);
                    return;
                }
                let rust_method = crate::parser::to_snake_case(name);
                hints.insert(DisplayContextReturnHint {
                    owner,
                    java_method: name.clone(),
                    rust_method,
                    arg_count: args.len(),
                });
            }
            collect_expr_display_return_hints(expr, ctx, hints);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_expr_display_return_hints(condition, ctx, hints);
            collect_display_sink_arg_return_hints(then_expr, ctx, hints);
            collect_display_sink_arg_return_hints(else_expr, ctx, hints);
        }
        Expr::Cast { expr, .. } | Expr::Unary { expr, .. } => {
            collect_display_sink_arg_return_hints(expr, ctx, hints)
        }
        _ => collect_expr_display_return_hints(expr, ctx, hints),
    }
}

fn display_hint_has_concrete_external_return(owner: &str, method: &str, arg_count: usize) -> bool {
    known_external_method_return_type(Some(owner), method, arg_count)
        .is_some_and(|return_type| return_type != JavaType::Void)
        || crate::external_packs::external_api_effect_for_receiver_with_arity(
            Some(owner),
            method,
            Some(arg_count),
        )
        .and_then(|effect| effect.return_type)
        .is_some_and(|return_type| return_type != JavaType::Void)
}

fn display_sink_method(name: &str) -> bool {
    matches!(
        name,
        "append" | "write" | "print" | "println" | "writeCharacters" | "writeAttribute"
    )
}

fn display_hint_owner_for_type(java_type: &JavaType) -> Option<String> {
    match java_type {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(
            rust_name_for_java_named_type(&canonical_external_receiver_type_key(name)),
        ),
        _ => None,
    }
}

fn collect_type_call_arities(
    ty: &crate::java_ir::TypeDecl,
    calls: &mut BTreeMap<String, BTreeSet<usize>>,
) {
    for member in &ty.members {
        match member {
            Member::Field(field) => {
                if let Some(initializer) = &field.initializer {
                    collect_expr_call_arities(initializer, calls);
                }
            }
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    collect_block_call_arities(body, calls);
                }
            }
            Member::Constructor(constructor) => {
                if let Some(body) = &constructor.body {
                    collect_block_call_arities(body, calls);
                }
            }
            Member::Type(nested) => collect_type_call_arities(nested, calls),
            Member::Unsupported { .. } => {}
        }
    }
}

fn collect_type_constructor_arities(
    ty: &crate::java_ir::TypeDecl,
    constructors: &mut BTreeMap<String, BTreeSet<usize>>,
) {
    for member in &ty.members {
        match member {
            Member::Field(field) => {
                if let Some(initializer) = &field.initializer {
                    collect_expr_constructor_arities(initializer, constructors);
                }
            }
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    collect_block_constructor_arities(body, constructors);
                }
            }
            Member::Constructor(constructor) => {
                if let Some(body) = &constructor.body {
                    collect_block_constructor_arities(body, constructors);
                }
            }
            Member::Type(nested) => collect_type_constructor_arities(nested, constructors),
            Member::Unsupported { .. } => {}
        }
    }
}

fn collect_block_constructor_arities(
    block: &Block,
    constructors: &mut BTreeMap<String, BTreeSet<usize>>,
) {
    for stmt in &block.statements {
        collect_stmt_constructor_arities(stmt, constructors);
    }
}

fn collect_stmt_constructor_arities(
    stmt: &Stmt,
    constructors: &mut BTreeMap<String, BTreeSet<usize>>,
) {
    match stmt {
        Stmt::VarDecl { initializer, .. } | Stmt::Return(initializer) => {
            if let Some(expr) = initializer {
                collect_expr_constructor_arities(expr, constructors);
            }
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => {
            collect_expr_constructor_arities(expr, constructors)
        }
        Stmt::Assign { target, value } => {
            collect_expr_constructor_arities(target, constructors);
            collect_expr_constructor_arities(value, constructors);
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_expr_constructor_arities(condition, constructors);
            collect_block_constructor_arities(then_block, constructors);
            if let Some(else_block) = else_block {
                collect_block_constructor_arities(else_block, constructors);
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            collect_expr_constructor_arities(condition, constructors);
            collect_block_constructor_arities(body, constructors);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                collect_stmt_constructor_arities(stmt, constructors);
            }
            if let Some(condition) = condition {
                collect_expr_constructor_arities(condition, constructors);
            }
            for expr in update {
                collect_expr_constructor_arities(expr, constructors);
            }
            collect_block_constructor_arities(body, constructors);
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            collect_expr_constructor_arities(iterable, constructors);
            collect_block_constructor_arities(body, constructors);
        }
        Stmt::Switch { selector, cases } => {
            collect_expr_constructor_arities(selector, constructors);
            for case in cases {
                for stmt in &case.statements {
                    collect_stmt_constructor_arities(stmt, constructors);
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
                collect_stmt_constructor_arities(resource, constructors);
            }
            collect_block_constructor_arities(body, constructors);
            for catch in catches {
                collect_block_constructor_arities(&catch.body, constructors);
            }
            if let Some(finally_block) = finally_block {
                collect_block_constructor_arities(finally_block, constructors);
            }
        }
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                collect_expr_constructor_arities(arg, constructors);
            }
        }
        Stmt::LocalType(ty) => collect_type_constructor_arities(ty, constructors),
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            collect_block_constructor_arities(block, constructors)
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
    }
}

fn collect_expr_constructor_arities(
    expr: &Expr,
    constructors: &mut BTreeMap<String, BTreeSet<usize>>,
) {
    match expr {
        Expr::New { ty, args, body, .. } => {
            if let Some(type_name) = constructed_type_name(ty) {
                constructors
                    .entry(type_name.to_string())
                    .or_default()
                    .insert(args.len());
            }
            for arg in args {
                collect_expr_constructor_arities(arg, constructors);
            }
            for member in body.iter().flatten() {
                match member {
                    Member::Field(field) => {
                        if let Some(initializer) = &field.initializer {
                            collect_expr_constructor_arities(initializer, constructors);
                        }
                    }
                    Member::Method(method) => {
                        if let Some(body) = &method.body {
                            collect_block_constructor_arities(body, constructors);
                        }
                    }
                    Member::Constructor(constructor) => {
                        if let Some(body) = &constructor.body {
                            collect_block_constructor_arities(body, constructors);
                        }
                    }
                    Member::Type(nested) => collect_type_constructor_arities(nested, constructors),
                    Member::Unsupported { .. } => {}
                }
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_expr_constructor_arities(receiver, constructors);
            }
            for arg in args {
                collect_expr_constructor_arities(arg, constructors);
            }
        }
        Expr::FieldAccess(receiver, _) => collect_expr_constructor_arities(receiver, constructors),
        Expr::ArrayAccess(array, index) => {
            collect_expr_constructor_arities(array, constructors);
            collect_expr_constructor_arities(index, constructors);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for dimension in dimensions {
                collect_expr_constructor_arities(dimension, constructors);
            }
            for value in initializer.iter().flatten() {
                collect_expr_constructor_arities(value, constructors);
            }
        }
        Expr::ArrayInitializer(values) => {
            for value in values {
                collect_expr_constructor_arities(value, constructors);
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_expr_constructor_arities(left, constructors);
            collect_expr_constructor_arities(right, constructors);
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_expr_constructor_arities(expr, constructors);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_expr_constructor_arities(condition, constructors);
            collect_expr_constructor_arities(then_expr, constructors);
            collect_expr_constructor_arities(else_expr, constructors);
        }
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => {
                collect_expr_constructor_arities(expr, constructors)
            }
            crate::java_ir::LambdaBody::Block(block) => {
                collect_block_constructor_arities(block, constructors)
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

fn constructed_type_name(ty: &JavaType) -> Option<&str> {
    match ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
        _ => None,
    }
}

fn external_constructor_fact_arity_count(facts: &FactSet, type_name: &str) -> usize {
    let mut arities = facts
        .external_constructor_facts
        .iter()
        .filter(|fact| external_constructor_type_matches(&fact.type_name, type_name))
        .map(|fact| fact.arg_types.len())
        .collect::<BTreeSet<_>>();
    arities.extend(
        crate::external_packs::builtin_external_library_packs()
            .external_constructor_facts
            .iter()
            .filter(|fact| external_constructor_type_matches(&fact.type_name, type_name))
            .map(|fact| fact.arg_types.len()),
    );
    arities.len()
}

fn external_constructor_type_matches(fact_type_name: &str, query_type_name: &str) -> bool {
    if fact_type_name == query_type_name {
        return true;
    }
    let fact_key = canonical_external_receiver_type_key(fact_type_name);
    let query_key = canonical_external_receiver_type_key(query_type_name);
    if fact_key == query_key {
        return true;
    }
    fact_key
        .rsplit('.')
        .next()
        .is_some_and(|simple| simple == query_key)
        || query_key
            .rsplit('.')
            .next()
            .is_some_and(|simple| simple == fact_key)
}

fn collect_block_call_arities(block: &Block, calls: &mut BTreeMap<String, BTreeSet<usize>>) {
    for stmt in &block.statements {
        collect_stmt_call_arities(stmt, calls);
    }
}

fn collect_stmt_call_arities(stmt: &Stmt, calls: &mut BTreeMap<String, BTreeSet<usize>>) {
    match stmt {
        Stmt::VarDecl { initializer, .. } | Stmt::Return(initializer) => {
            if let Some(expr) = initializer {
                collect_expr_call_arities(expr, calls);
            }
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => collect_expr_call_arities(expr, calls),
        Stmt::Assign { target, value } => {
            collect_expr_call_arities(target, calls);
            collect_expr_call_arities(value, calls);
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_expr_call_arities(condition, calls);
            collect_block_call_arities(then_block, calls);
            if let Some(else_block) = else_block {
                collect_block_call_arities(else_block, calls);
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            collect_expr_call_arities(condition, calls);
            collect_block_call_arities(body, calls);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                collect_stmt_call_arities(stmt, calls);
            }
            if let Some(condition) = condition {
                collect_expr_call_arities(condition, calls);
            }
            for expr in update {
                collect_expr_call_arities(expr, calls);
            }
            collect_block_call_arities(body, calls);
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            collect_expr_call_arities(iterable, calls);
            collect_block_call_arities(body, calls);
        }
        Stmt::Switch { selector, cases } => {
            collect_expr_call_arities(selector, calls);
            for case in cases {
                for stmt in &case.statements {
                    collect_stmt_call_arities(stmt, calls);
                }
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            if !resources.is_empty() {
                calls.entry("close".to_string()).or_default().insert(0);
            }
            for resource in resources {
                collect_stmt_call_arities(resource, calls);
            }
            collect_block_call_arities(body, calls);
            for catch in catches {
                collect_block_call_arities(&catch.body, calls);
            }
            if let Some(finally_block) = finally_block {
                collect_block_call_arities(finally_block, calls);
            }
        }
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                collect_expr_call_arities(arg, calls);
            }
        }
        Stmt::LocalType(ty) => collect_type_call_arities(ty, calls),
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            collect_block_call_arities(block, calls)
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
    }
}

fn collect_expr_call_arities(expr: &Expr, calls: &mut BTreeMap<String, BTreeSet<usize>>) {
    match expr {
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } => {
            calls
                .entry(crate::parser::to_snake_case(name))
                .or_default()
                .insert(args.len());
            if let Some(receiver) = receiver {
                collect_expr_call_arities(receiver, calls);
            }
            for arg in args {
                collect_expr_call_arities(arg, calls);
            }
        }
        Expr::FieldAccess(receiver, _) => collect_expr_call_arities(receiver, calls),
        Expr::ArrayAccess(array, index) => {
            collect_expr_call_arities(array, calls);
            collect_expr_call_arities(index, calls);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for dimension in dimensions {
                collect_expr_call_arities(dimension, calls);
            }
            for value in initializer.iter().flatten() {
                collect_expr_call_arities(value, calls);
            }
        }
        Expr::ArrayInitializer(values) => {
            for value in values {
                collect_expr_call_arities(value, calls);
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_expr_call_arities(left, calls);
            collect_expr_call_arities(right, calls);
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_expr_call_arities(expr, calls);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_expr_call_arities(condition, calls);
            collect_expr_call_arities(then_expr, calls);
            collect_expr_call_arities(else_expr, calls);
        }
        Expr::New { args, body, .. } => {
            for arg in args {
                collect_expr_call_arities(arg, calls);
            }
            for member in body.iter().flatten() {
                match member {
                    Member::Field(field) => {
                        if let Some(initializer) = &field.initializer {
                            collect_expr_call_arities(initializer, calls);
                        }
                    }
                    Member::Method(method) => {
                        if let Some(body) = &method.body {
                            collect_block_call_arities(body, calls);
                        }
                    }
                    Member::Constructor(constructor) => {
                        if let Some(body) = &constructor.body {
                            collect_block_call_arities(body, calls);
                        }
                    }
                    Member::Type(nested) => collect_type_call_arities(nested, calls),
                    Member::Unsupported { .. } => {}
                }
            }
        }
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => collect_expr_call_arities(expr, calls),
            crate::java_ir::LambdaBody::Block(block) => collect_block_call_arities(block, calls),
        },
        Expr::MethodReference { method, .. } => {
            let method = crate::parser::to_snake_case(method);
            let arities = calls.entry(method).or_default();
            arities.insert(0);
            arities.insert(1);
        }
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::ClassLiteral(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => {}
    }
}

fn external_facade_return_type_with_facts(java_type: &JavaType, facts: &FactSet) -> String {
    if let Some(rust_type) = external_opaque_final_facade_return_type(java_type, facts) {
        return rust_type;
    }
    if let Some(rust_type) = external_typed_erased_facade_return_type(java_type, facts) {
        return rust_type;
    }
    match java_type {
        JavaType::Primitive(PrimitiveType::Boolean) => "bool".to_string(),
        JavaType::Primitive(PrimitiveType::Byte) => "i8".to_string(),
        JavaType::Primitive(PrimitiveType::Short) => "i16".to_string(),
        JavaType::Primitive(PrimitiveType::Int) => "i32".to_string(),
        JavaType::Primitive(PrimitiveType::Long) => "i64".to_string(),
        JavaType::Primitive(PrimitiveType::Float) => "f32".to_string(),
        JavaType::Primitive(PrimitiveType::Double) => "f64".to_string(),
        JavaType::Primitive(PrimitiveType::Char) => "char".to_string(),
        JavaType::Void => "()".to_string(),
        JavaType::String => "String".to_string(),
        JavaType::Named { name, .. } if name == "Object" => "Box<dyn std::any::Any>".to_string(),
        JavaType::Named { name, .. } if name == "Properties" => {
            "std::collections::HashMap<String, String>".to_string()
        }
        JavaType::Named { name, args } if matches!(name.as_str(), "List" | "Collection") => {
            let element = args
                .first()
                .map(|arg| external_facade_return_type_with_facts(arg, facts))
                .unwrap_or_else(|| "Box<dyn std::any::Any>".to_string());
            format!("Vec<{element}>")
        }
        JavaType::Named { name, args } if matches!(name.as_str(), "Set" | "HashSet") => {
            let element = args
                .first()
                .map(|arg| external_facade_return_type_with_facts(arg, facts))
                .unwrap_or_else(|| "Box<dyn std::any::Any>".to_string());
            format!("std::collections::HashSet<{element}>")
        }
        JavaType::Named { name, args }
            if matches!(name.as_str(), "Map" | "HashMap" | "LinkedHashMap") =>
        {
            let key = args
                .first()
                .map(|arg| external_facade_return_type_with_facts(arg, facts))
                .unwrap_or_else(|| "String".to_string());
            let value = args
                .get(1)
                .map(|arg| external_facade_return_type_with_facts(arg, facts))
                .unwrap_or_else(|| "Box<dyn std::any::Any>".to_string());
            format!("std::collections::HashMap<{key}, {value}>")
        }
        JavaType::Named { name, args } if matches!(name.as_str(), "Iterable") => {
            let element = args
                .first()
                .map(|arg| external_facade_return_type_with_facts(arg, facts))
                .unwrap_or_else(|| "Box<dyn std::any::Any>".to_string());
            format!("Vec<{element}>")
        }
        JavaType::Named { name, .. }
            if matches!(name.as_str(), "Iterator" | "java.util.Iterator") =>
        {
            "CloseableIterator".to_string()
        }
        JavaType::Named { name, args } => {
            external_facade_crate_mapped_return_type(name, args, facts).unwrap_or_else(|| {
                crate::emit::emit_type(&rust_type_for_java_type_with_facts(java_type, facts))
            })
        }
        JavaType::Array(_) => {
            crate::emit::emit_type(&rust_type_for_java_type_with_facts(java_type, facts))
        }
        JavaType::Unknown(name) => external_facade_crate_mapped_return_type(name, &[], facts)
            .unwrap_or_else(|| "Box<dyn std::any::Any>".to_string()),
        JavaType::Wildcard(_) => "Box<dyn std::any::Any>".to_string(),
    }
}

fn external_typed_erased_facade_return_type(
    java_type: &JavaType,
    facts: &FactSet,
) -> Option<String> {
    let name = match java_type {
        JavaType::Named { name, args } if args.is_empty() => name.as_str(),
        JavaType::Unknown(name) => name.as_str(),
        _ => return None,
    };
    if matches!(name, "Object" | "java.lang.Object") {
        return None;
    }
    let erased = JavaType::Named {
        name: name.to_string(),
        args: Vec::new(),
    };
    let effective_facts = EffectiveFacts::new(facts);
    matches!(
        effective_facts.object_representation_override_strategy_for(&erased),
        Some(HierarchyStrategy::ErasedObject)
    )
    .then(|| rust_name_for_java_named_type(name))
}

fn external_opaque_final_facade_return_type(
    java_type: &JavaType,
    facts: &FactSet,
) -> Option<String> {
    let name = match java_type {
        JavaType::Named { name, args } if !args.is_empty() => name.as_str(),
        JavaType::Named { name, args } if args.is_empty() => raw_generic_type_head(name)?,
        JavaType::Unknown(raw) => raw_generic_type_head(raw)?,
        _ => return None,
    };
    let erased = JavaType::Named {
        name: name.to_string(),
        args: Vec::new(),
    };
    let effective_facts = EffectiveFacts::new(facts);
    if (matches!(
        effective_facts.object_representation_override_strategy_for(&erased),
        Some(HierarchyStrategy::ErasedObject)
    ) && effective_facts.is_final_type(&erased))
        || crate::external_packs::builtin_external_final_erased_object_type(name)
    {
        Some(rust_name_for_java_named_type(name))
    } else {
        None
    }
}

fn external_facade_crate_mapped_return_type(
    name: &str,
    args: &[JavaType],
    facts: &FactSet,
) -> Option<String> {
    let pack = facts.external_library_pack_for_type(name)?;
    if pack.mode != crate::facts::ExternalLibraryMode::RustCrateMapping {
        return None;
    }
    let crate_name = pack.rust_target.as_deref()?;
    let type_name = facts
        .external_crate_rust_type_name_for(name)
        .map(str::to_string)
        .unwrap_or_else(|| default_external_crate_rust_type_name(name));
    if args.is_empty() {
        Some(format!("{crate_name}::{type_name}"))
    } else {
        let generic_args = args
            .iter()
            .map(|arg| external_facade_return_type_with_facts(arg, facts))
            .collect::<Vec<_>>();
        Some(format!(
            "{crate_name}::{type_name}<{}>",
            generic_args.join(", ")
        ))
    }
}

fn external_facade_method_return_type_with_facts(
    facts: &FactSet,
    receiver_type: Option<&str>,
    method_name: &str,
    arg_count: Option<usize>,
    java_type: &JavaType,
    nullability: NullabilityState,
) -> String {
    let rendered = if let Some(return_type) = external_facade_method_abi_return_type_with_facts(
        facts,
        receiver_type,
        method_name,
        arg_count,
        java_type,
    ) {
        normalize_external_facade_return_type_text(&return_type)
    } else if let Some(rust_return) =
        facts.external_rust_return_type_for_receiver(receiver_type, method_name, arg_count)
    {
        normalize_external_facade_return_type_text(&rust_return.rust_type_name)
    } else {
        normalize_external_facade_return_type_text(
            &external_facade_return_type_with_facts_and_nullability(facts, java_type, nullability),
        )
    };
    external_facade_wrap_result_for_effects(facts, receiver_type, method_name, arg_count, rendered)
}

fn external_facade_wrap_result_for_effects(
    facts: &FactSet,
    receiver_type: Option<&str>,
    method_name: &str,
    arg_count: Option<usize>,
    rendered: String,
) -> String {
    if let Some(result_abi) =
        explicit_external_facade_result_abi(facts, receiver_type, method_name, arg_count)
    {
        return match result_abi {
            ResultAbi::Plain | ResultAbi::OptionWrapped => rendered,
            ResultAbi::ResultWrapped if raw_rust_type_is_result(&rendered) => rendered,
            ResultAbi::ResultWrapped => format!("Result<{rendered}, JavaException>"),
        };
    }
    let user_effect_throws = facts
        .external_api_effect_for_receiver_with_arity(receiver_type, method_name, arg_count)
        .is_some_and(|effect| !effect.thrown.is_empty());
    let builtin_effect_throws = crate::external_packs::external_api_effect_for_receiver_with_arity(
        receiver_type,
        method_name,
        arg_count,
    )
    .is_some_and(|effect| !effect.thrown.is_empty());
    if (user_effect_throws || builtin_effect_throws) && !raw_rust_type_is_result(&rendered) {
        format!("Result<{rendered}, JavaException>")
    } else {
        rendered
    }
}

fn explicit_external_facade_result_abi(
    facts: &FactSet,
    receiver_type: Option<&str>,
    method_name: &str,
    arg_count: Option<usize>,
) -> Option<ResultAbi> {
    let arg_count = arg_count?;
    if let Some(receiver_type) = receiver_type {
        let key = CallableKey::method(
            receiver_type.to_string(),
            method_name.to_string(),
            Staticness::Unknown,
            arg_count,
        );
        if let Some(abi) = facts.facade_abi_for_key(&key) {
            return Some(abi.result);
        }
    }
    facts
        .external_rust_result_for_receiver(receiver_type, method_name, Some(arg_count))
        .map(|fact| {
            if fact.returns_result {
                ResultAbi::ResultWrapped
            } else {
                ResultAbi::Plain
            }
        })
}

fn external_facade_method_abi_return_type_with_facts(
    facts: &FactSet,
    receiver_type: Option<&str>,
    method_name: &str,
    arg_count: Option<usize>,
    java_type: &JavaType,
) -> Option<String> {
    let receiver_type = receiver_type?;
    let arg_count = arg_count?;
    let key = CallableKey::method(
        receiver_type.to_string(),
        method_name.to_string(),
        Staticness::Unknown,
        arg_count,
    );
    let abi = facts.facade_abi_for_key(&key)?;
    external_facade_abi_return_type(facts, receiver_type, abi, java_type)
}

fn external_facade_abi_return_type(
    facts: &FactSet,
    receiver_type: &str,
    abi: &FacadeAbiFact,
    java_type: &JavaType,
) -> Option<String> {
    let base = external_facade_return_type_with_facts(java_type, facts);
    let rendered =
        external_facade_slot_return_type(facts, Some(receiver_type), &base, &abi.return_slot)?;
    Some(match abi.result {
        ResultAbi::Plain => rendered,
        ResultAbi::OptionWrapped if raw_rust_type_is_option(&rendered) => rendered,
        ResultAbi::OptionWrapped => format!("Option<{rendered}>"),
        ResultAbi::ResultWrapped => format!("Result<{rendered}, JavaException>"),
    })
}

fn external_facade_slot_return_type(
    facts: &FactSet,
    receiver_type: Option<&str>,
    base: &str,
    slot: &FacadeSlot,
) -> Option<String> {
    match slot {
        FacadeSlot::Owned => Some(base.to_string()),
        FacadeSlot::Index => Some("usize".to_string()),
        FacadeSlot::Borrowed => Some(format!(
            "&{}",
            normalize_external_facade_support_return_type(facts, receiver_type, base)
        )),
        FacadeSlot::MutBorrowed => Some(format!(
            "&mut {}",
            normalize_external_facade_support_return_type(facts, receiver_type, base)
        )),
        FacadeSlot::Optional(inner) => {
            let inner = external_facade_slot_return_type(facts, receiver_type, base, inner)?;
            if raw_rust_type_is_option(&inner) {
                Some(inner)
            } else {
                Some(format!("Option<{inner}>"))
            }
        }
        FacadeSlot::Slice(inner) => {
            let inner = external_facade_slot_return_type(facts, receiver_type, base, inner)?;
            Some(format!("&[{inner}]"))
        }
        FacadeSlot::DynTrait(path) | FacadeSlot::CrateHandle(path) => Some(path.clone()),
    }
}

fn normalize_external_facade_return_type_text(return_type: &str) -> String {
    let mut normalized = return_type.to_string();
    normalized = normalized.replace("HashMap<", "std::collections::HashMap<");
    normalized = normalized.replace(
        "std::collections::std::collections::HashMap<",
        "std::collections::HashMap<",
    );
    normalized = normalized.replace("ObjectAssert<T>", "ObjectAssert");
    normalized = normalized.replace("AbstractAssert<T>", "AbstractAssert");
    if normalized == "test_facade::ObjectAssert" {
        normalized = "test_facade::ObjectAssert<Box<dyn std::any::Any>>".to_string();
    }
    if normalized == "test_facade::AbstractAssert" {
        normalized = "test_facade::AbstractAssert<Box<dyn std::any::Any>>".to_string();
    }
    normalized
}

fn default_external_crate_rust_type_name(name: &str) -> String {
    let simple = name.rsplit('.').next().unwrap_or(name);
    if valid_rust_type_ident(simple) && simple.contains(|ch: char| ch.is_ascii_digit()) {
        return simple.to_string();
    }
    acronym_prefix_rust_type_name(&rust_name_for_java_named_type(simple))
}

fn acronym_prefix_rust_type_name(name: &str) -> String {
    let uppercase_run = name
        .char_indices()
        .take_while(|(_, ch)| ch.is_ascii_uppercase())
        .collect::<Vec<_>>();
    if uppercase_run.len() <= 3 {
        return name.to_string();
    }
    let run_end = uppercase_run
        .last()
        .map(|(idx, ch)| idx + ch.len_utf8())
        .unwrap_or_default();
    let next = name[run_end..].chars().next();
    let prefix_end = if next.is_some_and(|ch| ch.is_ascii_lowercase()) {
        uppercase_run
            .get(uppercase_run.len().saturating_sub(2))
            .map(|(idx, ch)| idx + ch.len_utf8())
            .unwrap_or(run_end)
    } else {
        run_end
    };
    if prefix_end <= 1 {
        return name.to_string();
    }
    let prefix = &name[..prefix_end];
    if !prefix.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return name.to_string();
    }
    let suffix = &name[prefix_end..];
    let mut out = String::new();
    if let Some(first) = prefix.chars().next() {
        out.push(first);
        out.push_str(&prefix[first.len_utf8()..].to_ascii_lowercase());
    }
    out.push_str(suffix);
    out
}

fn normalize_external_facade_support_return_type(
    facts: &FactSet,
    receiver_type: Option<&str>,
    return_type: &str,
) -> String {
    let trimmed = return_type.trim();
    if let Some(inner) = raw_rust_option_inner(trimmed) {
        return format!(
            "Option<{}>",
            normalize_external_facade_support_return_type(facts, receiver_type, inner)
        );
    }
    if let Some(inner) = trimmed.strip_prefix('&') {
        let mut inner = inner.trim_start();
        let mutability = if let Some(rest) = inner.strip_prefix("mut ") {
            inner = rest.trim_start();
            "mut "
        } else {
            ""
        };
        return format!(
            "&{mutability}{}",
            normalize_external_facade_support_return_type(facts, receiver_type, inner)
        );
    }
    if trimmed.contains("::") || trimmed.contains('<') {
        return trimmed.to_string();
    }
    external_facade_crate_mapped_return_type(trimmed, &[], facts)
        .or_else(|| external_facade_receiver_crate_return_type(facts, receiver_type, trimmed))
        .unwrap_or_else(|| trimmed.to_string())
}

fn external_facade_receiver_crate_return_type(
    facts: &FactSet,
    receiver_type: Option<&str>,
    return_type: &str,
) -> Option<String> {
    if matches!(
        return_type,
        "Self"
            | "()"
            | "bool"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "usize"
            | "f32"
            | "f64"
            | "String"
            | "Path"
            | "Box"
            | "Vec"
    ) {
        return None;
    }
    if !valid_rust_type_ident(return_type) {
        return None;
    }
    let receiver_type = receiver_type?;
    let pack = facts.external_library_pack_for_type(receiver_type)?;
    if pack.mode != crate::facts::ExternalLibraryMode::RustCrateMapping {
        return None;
    }
    let crate_name = pack.rust_target.as_deref()?;
    Some(format!("{crate_name}::{return_type}"))
}

fn external_facade_return_type_with_facts_and_nullability(
    facts: &FactSet,
    java_type: &JavaType,
    nullability: NullabilityState,
) -> String {
    let rendered = external_facade_return_type_with_facts(java_type, facts);
    if rendered != "()"
        && matches!(
            nullability,
            NullabilityState::MaybeNull | NullabilityState::DefinitelyNull
        )
    {
        format!("Option<{rendered}>")
    } else {
        rendered
    }
}

fn external_facade_static_method(name: &str, arg_count: usize, return_type: &str) -> String {
    let rust_name = rust_raw_ident_if_needed(name);
    format!(
        "    pub fn {rust_name}{} -> {return_type} {{ {} }}",
        external_facade_args(arg_count),
        external_facade_default_expr(return_type, Some(name))
    )
}

fn external_facade_static_method_with_facts(
    facts: &FactSet,
    receiver_key: &str,
    java_method: &str,
    rust_method: &str,
    arg_count: usize,
    return_type: &str,
) -> String {
    let escaped_rust_method = rust_raw_ident_if_needed(rust_method);
    let body = if return_type == "String"
        && arg_count == 1
        && matches!(rust_method, "to_string" | "toString")
        && external_facade_method_uses_display_argument(receiver_key, java_method, 0)
    {
        "_arg0.to_string()".to_string()
    } else {
        external_facade_default_expr(return_type, Some(rust_method))
    };
    format!(
        "    pub fn {escaped_rust_method}{} -> {return_type} {{ {body} }}",
        external_facade_args_with_facts(facts, receiver_key, java_method, arg_count),
    )
}

fn external_facade_unmodelled_method_with_facts(
    facts: &FactSet,
    receiver_key: &str,
    java_method: &str,
    rust_method: &str,
    arg_count: usize,
    return_type: &str,
    is_static: bool,
) -> String {
    let escaped_rust_method = rust_raw_ident_if_needed(rust_method);
    let args = if is_static {
        external_facade_args_with_facts(facts, receiver_key, java_method, arg_count)
    } else {
        external_facade_self_args_with_facts(facts, receiver_key, java_method, arg_count)
    };
    let receiver = canonical_external_receiver_type_key(receiver_key);
    format!(
        "    pub fn {escaped_rust_method}{args} -> {return_type} {{ unimplemented!(\"{receiver}.{java_method} is not modelled\") }}"
    )
}

fn external_facade_constructor_name(arg_count: usize, overload_by_arity: bool) -> String {
    if overload_by_arity {
        format!("new_{arg_count}")
    } else {
        "new".to_string()
    }
}

fn external_facade_constructor_method(name: &str, arg_count: usize) -> String {
    let rust_name = rust_raw_ident_if_needed(name);
    format!(
        "    pub fn {rust_name}{} -> Self {{ Default::default() }}",
        external_facade_args(arg_count)
    )
}

fn external_facade_constructor_method_with_facts(
    facts: &FactSet,
    type_name: &str,
    name: &str,
    arg_count: usize,
) -> String {
    let Some(args) = external_facade_constructor_args_with_facts(facts, type_name, arg_count)
    else {
        return external_facade_constructor_method(name, arg_count);
    };
    let rust_name = rust_raw_ident_if_needed(name);
    format!("    pub fn {rust_name}{args} -> Self {{ Default::default() }}")
}

fn external_facade_constructor_args_with_facts(
    facts: &FactSet,
    type_name: &str,
    arg_count: usize,
) -> Option<String> {
    if arg_count == 0 {
        return Some("()".to_string());
    }
    let mut saw_fact = false;
    let mut generic_params = Vec::new();
    let mut args = Vec::new();
    for index in 0..arg_count {
        let arg_ty =
            external_facade_constructor_argument_rust_type(facts, type_name, arg_count, index)
                .unwrap_or_else(|| {
                    let generic = format!("T{index}");
                    generic_params.push(generic.clone());
                    generic
                });
        if arg_ty != format!("T{index}") {
            saw_fact = true;
        }
        args.push(format!("_arg{index}: {arg_ty}"));
    }
    if !saw_fact {
        return None;
    }
    if generic_params.is_empty() {
        Some(format!("({})", args.join(", ")))
    } else {
        Some(format!(
            "<{}>({})",
            generic_params.join(", "),
            args.join(", ")
        ))
    }
}

fn external_facade_constructor_argument_rust_type(
    facts: &FactSet,
    type_name: &str,
    arg_count: usize,
    index: usize,
) -> Option<String> {
    let effect = facts
        .external_constructor_argument_effect_for(type_name, arg_count, index)
        .cloned()
        .or_else(|| {
            crate::external_packs::java_platform_constructor_argument_effect_for(
                type_name, arg_count, index,
            )
        })?;
    let java_ty = effect.parameter_type?;
    Some(crate::emit::emit_type(&rust_type_for_java_type_with_facts(
        &java_ty, facts,
    )))
}

fn external_facade_eq_impl(ty: &str) -> String {
    format!(
        "impl PartialEq for {ty} {{\n    fn eq(&self, _other: &Self) -> bool {{ true }}\n}}\n\nimpl Eq for {ty} {{}}"
    )
}

fn external_facade_hash_impl(ty: &str) -> String {
    format!(
        "impl std::hash::Hash for {ty} {{\n    fn hash<H: std::hash::Hasher>(&self, _state: &mut H) {{}}\n}}"
    )
}

fn external_facade_display_impl(ty: &str) -> String {
    format!(
        "impl std::fmt::Display for {ty} {{\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{ write!(f, \"\") }}\n}}"
    )
}

fn external_facade_imported_static_method(name: &str, arg_count: usize) -> String {
    if imported_static_method_is_side_effect(name) {
        external_facade_static_method(name, arg_count, "()")
    } else {
        let rust_name = rust_raw_ident_if_needed(name);
        format!(
            "    pub fn {rust_name}{} -> R {{ Default::default() }}",
            external_facade_generic_args("R: Default", arg_count)
        )
    }
}

fn external_facade_imported_static_method_with_facts(
    facts: &FactSet,
    owner_key: &str,
    name: &str,
    arg_count: usize,
) -> String {
    if imported_static_method_returns_first_arg(owner_key, name, arg_count) {
        return external_facade_imported_static_first_arg_method(name, arg_count);
    }
    if let Some(effect) = facts
        .external_api_effect_for_receiver_with_arity(Some(owner_key), name, Some(arg_count))
        .cloned()
        .or_else(|| {
            crate::external_packs::external_api_effect_for_receiver_with_arity(
                Some(owner_key),
                name,
                Some(arg_count),
            )
        })
    {
        if let Some(return_type) = &effect.return_type {
            let return_type = external_facade_method_return_type_with_facts(
                facts,
                Some(owner_key),
                &effect.method_name,
                Some(arg_count),
                return_type,
                effect.return_nullability,
            );
            return external_facade_static_method_with_facts(
                facts,
                owner_key,
                &effect.method_name,
                name,
                arg_count,
                &return_type,
            );
        }
        if effect.provenance != FactProvenance::ConservativeFallback
            && matches!(
                effect.role,
                ExternalApiRole::SideEffect
                    | ExternalApiRole::Mutator
                    | ExternalApiRole::Setter
                    | ExternalApiRole::FluentMutator
                    | ExternalApiRole::IndexedSetter
            )
        {
            return external_facade_static_method_with_facts(
                facts,
                owner_key,
                &effect.method_name,
                name,
                arg_count,
                "()",
            );
        }
        if let Some(return_type) =
            imported_static_unmodelled_result_type(facts, owner_key, name, arg_count)
        {
            return external_facade_unmodelled_method_with_facts(
                facts,
                owner_key,
                &effect.method_name,
                name,
                arg_count,
                &return_type,
                true,
            );
        }
    }
    if let Some(return_type) = known_imported_static_method_return_type(owner_key, name, arg_count)
    {
        let return_type = external_facade_method_return_type_with_facts(
            facts,
            Some(owner_key),
            name,
            Some(arg_count),
            &return_type,
            NullabilityState::DefinitelyNonNull,
        );
        return external_facade_static_method_with_facts(
            facts,
            owner_key,
            name,
            name,
            arg_count,
            &return_type,
        );
    }
    external_facade_imported_static_method(name, arg_count)
}

fn imported_static_unmodelled_result_type(
    facts: &FactSet,
    owner_key: &str,
    name: &str,
    arg_count: usize,
) -> Option<String> {
    let effect = facts
        .external_api_effect_for_receiver_with_arity(Some(owner_key), name, Some(arg_count))
        .cloned()
        .or_else(|| {
            crate::external_packs::external_api_effect_for_receiver_with_arity(
                Some(owner_key),
                name,
                Some(arg_count),
            )
        })?;
    if effect.return_type.is_some()
        || effect.provenance == FactProvenance::ConservativeFallback
        || !external_api_role_returns_value(effect.role)
    {
        return None;
    }
    let owner_rust = rust_name_for_java_named_type(owner_key);
    Some(external_facade_unmodelled_result_type(
        &owner_rust,
        name,
        arg_count,
    ))
}

fn known_imported_static_method_return_type(
    owner_key: &str,
    name: &str,
    arg_count: usize,
) -> Option<JavaType> {
    let java_name = lower_camel_from_snake(name);
    known_external_method_return_type(Some(owner_key), &java_name, arg_count).or_else(|| {
        owner_key.rsplit('.').next().and_then(|simple| {
            known_external_method_return_type(Some(simple), &java_name, arg_count)
        })
    })
}

fn imported_static_method_returns_first_arg(owner_key: &str, name: &str, arg_count: usize) -> bool {
    crate::javalibs::imported_static_method_returns_broad_utils_first_arg(
        owner_key, name, arg_count,
    )
}

fn external_facade_imported_static_first_arg_method(name: &str, arg_count: usize) -> String {
    let rust_name = rust_raw_ident_if_needed(name);
    let generics = (0..arg_count)
        .map(|idx| format!("T{idx}"))
        .collect::<Vec<_>>()
        .join(", ");
    let args = (0..arg_count)
        .map(|idx| format!("_arg{idx}: T{idx}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("    pub fn {rust_name}<{generics}>({args}) -> T0 {{ _arg0 }}")
}

fn imported_static_method_is_side_effect(name: &str) -> bool {
    name.starts_with("set_")
        || name.starts_with("use_")
        || name.starts_with("assert_")
        || name.starts_with("validate_")
        || name.starts_with("update_")
}

fn external_facade_instance_method(name: &str, arg_count: usize, return_type: &str) -> String {
    let rust_name = rust_raw_ident_if_needed(name);
    format!(
        "    pub fn {rust_name}{} -> {return_type} {{ {} }}",
        external_facade_self_args(arg_count),
        external_facade_default_expr(return_type, Some(name))
    )
}

fn query_backed_external_inherited_methods_for_type(
    facts: &FactSet,
    ty: &TypeDecl,
    program: &Program,
    used_calls: &BTreeMap<String, BTreeSet<usize>>,
) -> Vec<crate::inherited_members::ExternalInheritedInstanceMethod> {
    let mut methods = Vec::new();
    let mut seen = BTreeSet::new();
    for super_name in resolved_external_super_names_for_method_queries(facts, ty, program) {
        for (rust_name, arities) in used_calls {
            for arg_count in arities {
                let Some(fact) = crate::external_packs::external_api_effect_for_receiver_with_arity(
                    Some(&super_name),
                    rust_name,
                    Some(*arg_count),
                ) else {
                    continue;
                };
                let rust_name = crate::parser::to_snake_case(&fact.method_name);
                if !seen.insert((rust_name.clone(), *arg_count)) {
                    continue;
                }
                methods.push(crate::inherited_members::ExternalInheritedInstanceMethod {
                    declaring_type: fact
                        .receiver_type
                        .clone()
                        .unwrap_or_else(|| super_name.clone()),
                    method_name: fact.method_name.clone(),
                    rust_name,
                    arg_count: *arg_count,
                    fact,
                });
            }
        }
    }
    methods
}

pub(crate) fn resolved_external_super_names_for_method_queries(
    facts: &FactSet,
    ty: &TypeDecl,
    program: &Program,
) -> Vec<String> {
    let mut super_names = Vec::new();
    if let Some(extends) = ty.extends.as_deref() {
        super_names.extend(resolved_external_type_names_for_simple_or_qualified(
            extends, program,
        ));
    }
    super_names.extend(
        facts
            .reachable_hierarchy_edges_for_type(&JavaType::Named {
                name: ty.name.clone(),
                args: Vec::new(),
            })
            .into_iter()
            .flat_map(|edge| {
                resolved_external_type_names_for_simple_or_qualified(&edge.super_name, program)
            }),
    );
    super_names.sort();
    super_names.dedup();
    super_names
}

fn resolved_external_type_names_for_simple_or_qualified(
    type_name: &str,
    program: &Program,
) -> Vec<String> {
    let type_name = type_name.trim();
    if type_name.contains('.') {
        return vec![type_name.to_string()];
    }

    let mut out = Vec::new();
    for import in program.imports.iter().filter(|import| !import.is_static) {
        if import.is_wildcard {
            let qualified = format!("{}.{}", import.path.trim_end_matches(".*"), type_name);
            if is_java_platform_facade_type_name(&qualified)
                || is_known_java_platform_stream_type_name(&qualified)
            {
                out.push(qualified);
            }
            continue;
        }
        if import.path.rsplit('.').next() == Some(type_name) {
            out.push(import.path.clone());
        }
    }
    out.sort();
    out.dedup();
    out
}

fn is_known_java_platform_stream_type_name(name: &str) -> bool {
    let canonical = canonical_external_receiver_type_key(name);
    let simple = name
        .rsplit('.')
        .next()
        .unwrap_or(name)
        .trim()
        .trim_start_matches("JavaIo");
    matches!(
        canonical.as_str(),
        "InputStream"
            | "OutputStream"
            | "FileInputStream"
            | "FileOutputStream"
            | "ByteArrayInputStream"
            | "ByteArrayOutputStream"
            | "BufferedInputStream"
            | "BufferedOutputStream"
            | "DataInputStream"
            | "DataOutputStream"
            | "FilterInputStream"
            | "FilterOutputStream"
            | "ObjectInputStream"
            | "ObjectOutputStream"
            | "GZIPInputStream"
            | "GZIPOutputStream"
            | "ZipInputStream"
            | "ZipOutputStream"
            | "PushbackInputStream"
            | "SequenceInputStream"
    ) || matches!(
        simple,
        "InputStream"
            | "OutputStream"
            | "FileInputStream"
            | "FileOutputStream"
            | "ByteArrayInputStream"
            | "ByteArrayOutputStream"
            | "BufferedInputStream"
            | "BufferedOutputStream"
            | "DataInputStream"
            | "DataOutputStream"
            | "FilterInputStream"
            | "FilterOutputStream"
            | "ObjectInputStream"
            | "ObjectOutputStream"
            | "GZIPInputStream"
            | "GZIPOutputStream"
            | "ZipInputStream"
            | "ZipOutputStream"
            | "PushbackInputStream"
            | "SequenceInputStream"
    )
}

fn external_facade_instance_method_with_facts(
    facts: &FactSet,
    receiver_key: &str,
    java_method: &str,
    rust_method: &str,
    arg_count: usize,
    return_type: &str,
) -> String {
    let escaped_rust_method = rust_raw_ident_if_needed(rust_method);
    if external_facade_method_is_self_returning_fluent(facts, receiver_key, java_method, arg_count)
    {
        let return_type = if facts
            .external_api_effect_for_receiver_with_arity(
                Some(receiver_key),
                java_method,
                Some(arg_count),
            )
            .is_some_and(|effect| !effect.thrown.is_empty())
        {
            "Result<Self, JavaException>"
        } else {
            "Self"
        };
        let return_expr = if raw_rust_type_is_result(return_type) {
            "Ok(self.clone())"
        } else {
            "self.clone()"
        };
        return format!(
            "    pub fn {escaped_rust_method}{} -> {return_type} {{ {return_expr} }}",
            external_facade_self_args_with_facts(facts, receiver_key, java_method, arg_count),
        );
    }
    format!(
        "    pub fn {escaped_rust_method}{} -> {return_type} {{ {} }}",
        external_facade_self_args_with_facts(facts, receiver_key, java_method, arg_count),
        external_facade_default_expr(return_type, Some(rust_method))
    )
}

fn external_facade_method_is_self_returning_fluent(
    facts: &FactSet,
    receiver_key: &str,
    java_method: &str,
    arg_count: usize,
) -> bool {
    facts
        .external_api_effect_for_receiver_with_arity(
            Some(receiver_key),
            java_method,
            Some(arg_count),
        )
        .is_some_and(|effect| {
            effect.role == ExternalApiRole::FluentMutator
                && effect.return_type.as_ref().is_some_and(|return_type| {
                    java_type_name_for_external_facade(return_type).is_some_and(|return_name| {
                        canonical_external_receiver_type_key(return_name)
                            == canonical_external_receiver_type_key(receiver_key)
                    })
                })
        })
}

fn external_facade_trait_method(name: &str, arg_count: usize, return_type: &str) -> String {
    let rust_name = rust_raw_ident_if_needed(name);
    format!(
        "    fn {rust_name}{} -> {return_type} {{ {} }}",
        external_facade_self_args(arg_count),
        external_facade_default_expr(return_type, Some(name))
    )
}

fn external_facade_trait_item(trait_surface: &ExternalFacadeTrait) -> String {
    let methods = trait_surface
        .methods
        .iter()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    if methods.is_empty() {
        format!(
            "pub trait {}: std::fmt::Display {{}}",
            trait_surface.trait_name
        )
    } else {
        format!(
            "pub trait {}: std::fmt::Display {{\n{}\n}}",
            trait_surface.trait_name, methods
        )
    }
}

fn external_facade_builder_fluent_method(name: &str, arg_count: usize) -> String {
    let rust_name = rust_raw_ident_if_needed(name);
    format!(
        "    pub fn {rust_name}{} -> Self {{ self.clone() }}",
        external_facade_self_args(arg_count)
    )
}

fn external_facade_builder_unmodelled_getter(
    owner: &str,
    rust_name: &str,
    java_name: &str,
    arg_count: usize,
    return_type: &str,
) -> String {
    let rust_name = rust_raw_ident_if_needed(rust_name);
    format!(
        "    pub fn {rust_name}{} -> {return_type} {{ unimplemented!(\"{owner}.{java_name} is not modelled\") }}",
        external_facade_self_args(arg_count)
    )
}

fn facade_method_name(method_body: &str) -> Option<&str> {
    let rest = method_body.split("pub fn ").nth(1)?;
    let end = rest.find(['<', '('])?;
    Some(&rest[..end])
}

fn rename_facade_method(method_body: &str, old_name: &str, new_name: &str) -> Option<String> {
    let current = facade_method_name(method_body)?;
    if current != old_name {
        return None;
    }
    Some(method_body.replacen(
        &format!("pub fn {old_name}"),
        &format!("pub fn {new_name}"),
        1,
    ))
}

fn facade_const_name(method_body: &str) -> Option<&str> {
    let rest = method_body.split("pub const ").nth(1)?;
    let end = rest.find(':')?;
    Some(rest[..end].trim())
}

fn facade_method_has_name(method_body: &str, method: &str) -> bool {
    method_body.contains(&format!(" pub fn {method}<"))
        || method_body.contains(&format!(" pub fn {method}("))
}

fn facade_method_matches_signature(method_body: &str, method: &str, arg_count: usize) -> bool {
    facade_method_has_name(method_body, method) && facade_method_arg_count(method_body) == arg_count
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FacadeMethodReceiverKind {
    Static,
    SharedSelf,
    MutableSelf,
    OwnedSelf,
}

fn facade_method_receiver_kind(method_body: &str) -> FacadeMethodReceiverKind {
    let Some(params) = method_body
        .split_once('(')
        .and_then(|(_, rest)| rest.split_once(')').map(|(params, _)| params))
    else {
        return FacadeMethodReceiverKind::Static;
    };
    let Some(first_param) = params
        .split(',')
        .map(str::trim)
        .find(|param| !param.is_empty())
    else {
        return FacadeMethodReceiverKind::Static;
    };
    match first_param {
        "&self" => FacadeMethodReceiverKind::SharedSelf,
        "&mut self" => FacadeMethodReceiverKind::MutableSelf,
        "self" => FacadeMethodReceiverKind::OwnedSelf,
        _ => FacadeMethodReceiverKind::Static,
    }
}

fn facade_method_arg_count(method_body: &str) -> usize {
    let Some(params) = method_body
        .split_once('(')
        .and_then(|(_, rest)| rest.split_once(')').map(|(params, _)| params))
    else {
        return 0;
    };
    params
        .split(',')
        .map(str::trim)
        .filter(|param| !param.is_empty() && *param != "&self" && param.contains(':'))
        .count()
}

fn facade_method_return_type<'a>(method_body: &'a str, method: &str) -> Option<&'a str> {
    if !facade_method_has_name(method_body, method) {
        return None;
    }
    method_body
        .split(" -> ")
        .nth(1)
        .and_then(|rest| rest.split(" {").next())
        .map(str::trim)
}

fn weak_external_facade_return_type(return_type: &str) -> bool {
    matches!(return_type, "()" | "Box<dyn std::any::Any>" | "R" | "T")
}

fn external_api_role_returns_value(role: ExternalApiRole) -> bool {
    matches!(
        role,
        ExternalApiRole::Getter | ExternalApiRole::BooleanGetter | ExternalApiRole::IndexedGetter
    )
}

fn external_facade_unmodelled_result_type(
    receiver_rust: &str,
    rust_method: &str,
    arg_count: usize,
) -> String {
    let suffix = if arg_count == 0 {
        "Result".to_string()
    } else {
        format!("{}Result", arg_count)
    };
    format!(
        "{}{}{}",
        receiver_rust,
        upper_camel_from_snake(rust_method),
        suffix
    )
}

fn external_facade_new_builder_result_type(receiver_rust: &str) -> String {
    format!("{receiver_rust}Builder")
}

fn external_erased_object_trait_method(method: &ExternalErasedObjectMethod) -> String {
    let name = rust_raw_ident_if_needed(&method.name);
    format!(
        "    fn {}{} -> {};",
        name,
        external_facade_self_args(method.arg_count),
        method.return_type
    )
}

fn external_erased_object_impl_method(method: &ExternalErasedObjectMethod) -> String {
    let name = rust_raw_ident_if_needed(&method.name);
    format!(
        "    fn {}{} -> {} {{ {} }}",
        name,
        external_facade_self_args(method.arg_count),
        method.return_type,
        external_facade_default_expr(&method.return_type, Some(&method.name))
    )
}

fn external_facade_default_expr(return_type: &str, method_name: Option<&str>) -> String {
    if let Some(args) = raw_rust_result_args(return_type)
        && args.len() == 2
        && args[1] == "JavaException"
    {
        let ok_type = args[0];
        if ok_type == "()" {
            "Ok(())".to_string()
        } else if ok_type == "Box<dyn std::any::Any>" {
            "Ok(Box::new(()))".to_string()
        } else {
            "Ok(Default::default())".to_string()
        }
    } else if raw_rust_type_is_option(return_type) {
        "None".to_string()
    } else if return_type == "JavaClass<()>" {
        "JavaClass::<()>::for_name(\"java.lang.Object\")".to_string()
    } else if return_type == "()" {
        "()".to_string()
    } else if return_type == "Box<dyn std::any::Any>" {
        "Box::new(())".to_string()
    } else if external_facade_return_type_requires_explicit_provider(return_type) {
        format!(
            "panic!(\"external facade method {} has no fallback value\")",
            method_name.unwrap_or("<unknown>")
        )
    } else {
        "Default::default()".to_string()
    }
}

fn external_facade_return_type_requires_explicit_provider(return_type: &str) -> bool {
    return_type == "Self" || return_type.starts_with('&') || return_type.contains("::")
}

fn external_facade_args(arg_count: usize) -> String {
    if arg_count == 0 {
        "()".to_string()
    } else {
        let generics = (0..arg_count)
            .map(|idx| format!("T{idx}"))
            .collect::<Vec<_>>()
            .join(", ");
        let args = (0..arg_count)
            .map(|idx| format!("_arg{idx}: T{idx}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!("<{generics}>({args})")
    }
}

fn external_facade_args_with_facts(
    facts: &FactSet,
    receiver_key: &str,
    method: &str,
    arg_count: usize,
) -> String {
    external_facade_args_with_fact_prefix(facts, receiver_key, method, arg_count, "")
}

fn external_facade_generic_args(first_generic: &str, arg_count: usize) -> String {
    let mut generics = vec![first_generic.to_string()];
    generics.extend((0..arg_count).map(|idx| format!("T{idx}")));
    if arg_count == 0 {
        format!("<{}>()", generics.join(", "))
    } else {
        let args = (0..arg_count)
            .map(|idx| format!("_arg{idx}: T{idx}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!("<{}>({args})", generics.join(", "))
    }
}

fn external_facade_self_args(arg_count: usize) -> String {
    if arg_count == 0 {
        "(&self)".to_string()
    } else {
        let generics = (0..arg_count)
            .map(|idx| format!("T{idx}"))
            .collect::<Vec<_>>()
            .join(", ");
        let args = (0..arg_count)
            .map(|idx| format!("_arg{idx}: T{idx}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!("<{generics}>(&self, {args})")
    }
}

fn external_facade_self_args_with_facts(
    facts: &FactSet,
    receiver_key: &str,
    method: &str,
    arg_count: usize,
) -> String {
    let key = CallableKey::method(
        receiver_key.to_string(),
        method.to_string(),
        Staticness::Unknown,
        arg_count,
    );
    let prefix = match facts.facade_abi_for_key(&key).map(|abi| abi.receiver) {
        Some(ReceiverAbi::MutBorrowedSelf) => "&mut self",
        Some(ReceiverAbi::OwnedSelf) => "self",
        _ => "&self",
    };
    external_facade_args_with_fact_prefix(facts, receiver_key, method, arg_count, prefix)
}

fn external_facade_args_with_fact_prefix(
    facts: &FactSet,
    receiver_key: &str,
    method: &str,
    arg_count: usize,
    prefix: &str,
) -> String {
    if arg_count == 0 {
        return if prefix.is_empty() {
            "()".to_string()
        } else {
            format!("({prefix})")
        };
    }

    let mut generic_params = Vec::new();
    let mut args = Vec::new();
    if !prefix.is_empty() {
        args.push(prefix.to_string());
    }
    for index in 0..arg_count {
        let arg_ty =
            if external_facade_method_uses_nullable_generic_handoff(receiver_key, method, index) {
                let generic = format!("T{index}");
                generic_params.push(generic.clone());
                format!("Option<{generic}>")
            } else {
                external_facade_argument_rust_type(facts, receiver_key, method, arg_count, index)
                    .unwrap_or_else(|| {
                        let generic = format!("T{index}");
                        generic_params.push(generic.clone());
                        generic
                    })
            };
        args.push(format!("_arg{index}: {arg_ty}"));
    }

    if generic_params.is_empty() {
        format!("({})", args.join(", "))
    } else {
        format!("<{}>({})", generic_params.join(", "), args.join(", "))
    }
}

fn external_facade_method_uses_nullable_generic_handoff(
    receiver_key: &str,
    method: &str,
    index: usize,
) -> bool {
    index == 0
        && matches!(method, "writeObject" | "write_object")
        && crate::stdlib::canonical_external_receiver_type_key(receiver_key)
            .rsplit('.')
            .next()
            .is_some_and(|simple| simple == "ObjectOutputStream")
}

fn external_facade_method_uses_generic_append_handoff(
    receiver_key: &str,
    method: &str,
    index: usize,
) -> bool {
    if index != 0 || !matches!(method, "append") {
        return false;
    }
    let receiver_simple = crate::stdlib::canonical_external_receiver_type_key(receiver_key)
        .rsplit('.')
        .next()
        .unwrap_or(receiver_key)
        .to_string();
    matches!(receiver_simple.as_str(), "StringBuilder" | "StringBuffer")
}

fn external_facade_method_uses_display_argument(
    receiver_key: &str,
    method: &str,
    index: usize,
) -> bool {
    if index != 0 {
        return false;
    }
    let receiver_simple = crate::stdlib::canonical_external_receiver_type_key(receiver_key)
        .rsplit('.')
        .next()
        .unwrap_or(receiver_key)
        .to_string();
    matches!(method, "toString" | "to_string")
        && matches!(receiver_simple.as_str(), "Double" | "Float" | "Integer")
}

fn external_facade_argument_rust_type(
    facts: &FactSet,
    receiver_key: &str,
    method: &str,
    arg_count: usize,
    index: usize,
) -> Option<String> {
    if external_facade_method_uses_generic_append_handoff(receiver_key, method, index) {
        return None;
    }
    if external_facade_method_uses_display_argument(receiver_key, method, index) {
        return Some("impl std::fmt::Display".to_string());
    }
    if let Some(ty) =
        external_facade_argument_rust_type_from_abi(facts, receiver_key, method, arg_count, index)
    {
        return Some(ty);
    }
    let effect =
        external_facade_argument_effect(facts, receiver_key, method, Some(arg_count), index)?;
    if let Some(ty) = external_facade_dyn_trait_argument_rust_type(facts, effect) {
        return Some(ty);
    }
    let shape = external_facade_argument_abi_shape(facts, receiver_key, method, index, effect)
        .or_else(|| external_facade_argument_shape(effect))?;
    Some(crate::emit::emit_type(&rust_type_for_slot_with_facts(
        &shape, facts,
    )))
}

fn external_facade_argument_rust_type_from_abi(
    facts: &FactSet,
    receiver_key: &str,
    method: &str,
    arg_count: usize,
    index: usize,
) -> Option<String> {
    let abi =
        external_facade_argument_abi_for_method(facts, receiver_key, method, arg_count, index)?;
    let pattern = abi.key.params.get(index)?;
    let java_ty = java_type_from_callable_type_pattern(pattern)?;
    let shape = external_facade_argument_shape_from_abi_slot(abi.args.get(index)?, java_ty);
    Some(crate::emit::emit_type(&rust_type_for_slot_with_facts(
        &shape, facts,
    )))
}

fn external_facade_argument_abi_for_method<'a>(
    facts: &'a FactSet,
    receiver_key: &str,
    method: &str,
    arg_count: usize,
    index: usize,
) -> Option<&'a FacadeAbiFact> {
    facts
        .facade_abi_facts
        .iter()
        .filter(|abi| {
            crate::stdlib::canonical_external_receiver_type_key(&abi.key.owner.type_name)
                == crate::stdlib::canonical_external_receiver_type_key(receiver_key)
                && abi.args.len() == arg_count
                && abi.args.get(index).is_some()
                && (abi.key.member.0 == method
                    || abi.rust_path.rsplit("::").next().unwrap_or(&abi.rust_path) == method)
        })
        .max_by_key(|abi| {
            let first_param = abi.key.params.first().and_then(|param| match param {
                TypePattern::Exact(type_key) | TypePattern::Erased(type_key) => {
                    Some(type_key.type_name.as_str())
                }
                TypePattern::TypeParam(_) | TypePattern::Any => None,
            });
            (
                abi.rust_path.rsplit("::").next().unwrap_or(&abi.rust_path) == method,
                first_param == Some("int"),
                abi.key
                    .params
                    .iter()
                    .filter(|param| !matches!(param, TypePattern::Any))
                    .count(),
            )
        })
}

fn java_type_from_callable_type_pattern(pattern: &TypePattern) -> Option<JavaType> {
    match pattern {
        TypePattern::Exact(type_key) | TypePattern::Erased(type_key) => {
            java_type_from_callable_type_name(&type_key.type_name)
        }
        TypePattern::TypeParam(name) => Some(JavaType::Unknown(name.clone())),
        TypePattern::Any => None,
    }
}

fn java_type_from_callable_type_name(type_name: &str) -> Option<JavaType> {
    let name = type_name.trim();
    if let Some(element_name) = name.strip_suffix("[]") {
        return java_type_from_callable_type_name(element_name)
            .map(|element| JavaType::Array(Box::new(element)));
    }
    let primitive = match name {
        "boolean" => Some(PrimitiveType::Boolean),
        "byte" => Some(PrimitiveType::Byte),
        "short" => Some(PrimitiveType::Short),
        "char" => Some(PrimitiveType::Char),
        "int" => Some(PrimitiveType::Int),
        "long" => Some(PrimitiveType::Long),
        "float" => Some(PrimitiveType::Float),
        "double" => Some(PrimitiveType::Double),
        _ => None,
    };
    if let Some(primitive) = primitive {
        return Some(JavaType::Primitive(primitive));
    }
    if matches!(name, "String" | "java.lang.String") {
        return Some(JavaType::String);
    }
    Some(JavaType::Named {
        name: name.to_string(),
        args: Vec::new(),
    })
}

fn external_facade_argument_abi_shape(
    facts: &FactSet,
    receiver_key: &str,
    method: &str,
    index: usize,
    effect: &crate::facts::ExternalApiArgumentEffectFact,
) -> Option<RustSlotShape> {
    let abi = facts.facade_abi_facts.iter().find(|abi| {
        crate::stdlib::canonical_external_receiver_type_key(&abi.key.owner.type_name)
            == crate::stdlib::canonical_external_receiver_type_key(receiver_key)
            && abi.key.member.0 == method
            && abi.args.get(index).is_some()
    })?;
    let java_ty = effect.parameter_type.clone()?;
    Some(external_facade_argument_shape_from_abi_slot(
        abi.args.get(index)?,
        java_ty,
    ))
}

fn external_facade_argument_shape_from_abi_slot(
    slot: &FacadeSlot,
    java_ty: JavaType,
) -> RustSlotShape {
    match slot {
        FacadeSlot::Borrowed => {
            let mut shape = RustSlotShape::conservative(java_ty.clone());
            if java_ty == JavaType::String {
                apply_borrowed_string_content_projection(&mut shape);
            } else if matches!(java_ty, JavaType::Array(_)) {
                apply_borrowed_sequence_content_projection(&mut shape);
            } else if !matches!(java_ty, JavaType::Primitive(_) | JavaType::Void) {
                shape.ownership = OwnershipShape::SharedBorrow;
            }
            shape
        }
        FacadeSlot::MutBorrowed => {
            let mut shape = RustSlotShape::conservative(java_ty);
            if !matches!(shape.java_ty, JavaType::Primitive(_) | JavaType::Void) {
                shape.ownership = OwnershipShape::MutableBorrow;
                shape.mutability = MutabilityShape::Mutable;
            }
            shape
        }
        FacadeSlot::Slice(inner) => {
            let mut shape = RustSlotShape::conservative(java_ty);
            apply_borrowed_sequence_content_projection(&mut shape);
            if external_facade_abi_slot_contains_mut_borrow(inner) {
                shape.ownership = OwnershipShape::MutableBorrow;
                shape.mutability = MutabilityShape::Mutable;
            }
            shape
        }
        FacadeSlot::Optional(inner) => external_facade_argument_shape_from_abi_slot(
            inner,
            crate::types::optional_java_type(java_ty),
        ),
        FacadeSlot::Owned
        | FacadeSlot::Index
        | FacadeSlot::DynTrait(_)
        | FacadeSlot::CrateHandle(_) => RustSlotShape::conservative(java_ty),
    }
}

fn external_facade_abi_slot_contains_mut_borrow(slot: &FacadeSlot) -> bool {
    match slot {
        FacadeSlot::MutBorrowed => true,
        FacadeSlot::Optional(inner) | FacadeSlot::Slice(inner) => {
            external_facade_abi_slot_contains_mut_borrow(inner)
        }
        FacadeSlot::Owned
        | FacadeSlot::Borrowed
        | FacadeSlot::Index
        | FacadeSlot::DynTrait(_)
        | FacadeSlot::CrateHandle(_) => false,
    }
}

fn external_facade_dyn_trait_argument_rust_type(
    facts: &FactSet,
    effect: &crate::facts::ExternalApiArgumentEffectFact,
) -> Option<String> {
    if effect.alias_policy != ExternalArgumentAliasPolicy::Borrowed {
        return None;
    }
    if matches!(
        crate::facts::external_parameter_nullability_policy(effect.parameter_nullability),
        NullabilityState::MaybeNull | NullabilityState::DefinitelyNull
    ) {
        return None;
    }
    let JavaType::Named { name, .. } = effect.parameter_type.as_ref()? else {
        return None;
    };
    let trait_path = external_facade_dyn_trait_path(facts, name)?;
    Some(format!("&impl {trait_path}"))
}

fn external_facade_receiver_is_dyn_trait(facts: &FactSet, receiver_key: &str) -> bool {
    let effective_facts = EffectiveFacts::new(facts);
    matches!(
        effective_facts.object_representation_override_strategy_for(&JavaType::Named {
            name: receiver_key.to_string(),
            args: Vec::new(),
        }),
        Some(HierarchyStrategy::DynTrait { .. })
    )
}

fn external_facade_dyn_trait_path(facts: &FactSet, type_name: &str) -> Option<String> {
    let effective_facts = EffectiveFacts::new(facts);
    let java_ty = JavaType::Named {
        name: type_name.to_string(),
        args: Vec::new(),
    };
    let Some(HierarchyStrategy::DynTrait { trait_name }) =
        effective_facts.object_representation_override_strategy_for(&java_ty)
    else {
        return None;
    };
    Some(dyn_trait_rust_path(trait_name, facts))
}

fn external_facade_argument_java_type(
    facts: &FactSet,
    receiver_key: &str,
    method: &str,
    arg_count: usize,
    index: usize,
) -> Option<JavaType> {
    external_facade_argument_effect(facts, receiver_key, method, Some(arg_count), index)
        .and_then(|effect| effect.parameter_type.clone())
}

fn external_facade_argument_nullability(
    effect: &crate::facts::ExternalApiArgumentEffectFact,
) -> NullabilityState {
    crate::facts::external_parameter_nullability_policy(effect.parameter_nullability)
}

fn external_facade_argument_shape(
    effect: &crate::facts::ExternalApiArgumentEffectFact,
) -> Option<RustSlotShape> {
    let base_java_ty = effect.parameter_type.clone()?;
    let mut java_ty = base_java_ty.clone();
    if matches!(
        external_facade_argument_nullability(effect),
        NullabilityState::MaybeNull | NullabilityState::DefinitelyNull
    ) {
        java_ty = crate::types::optional_java_type(java_ty);
    }
    let mut shape = RustSlotShape::conservative(java_ty);
    let nullable_argument = crate::types::optional_element_type(&shape.java_ty).is_some();
    let allow_alias_borrow =
        !nullable_argument || external_facade_effect_allows_nullable_borrowed_slot(effect);
    match effect.alias_policy {
        ExternalArgumentAliasPolicy::Borrowed
            if allow_alias_borrow && base_java_ty == JavaType::String =>
        {
            apply_borrowed_string_content_projection(&mut shape);
        }
        ExternalArgumentAliasPolicy::Borrowed
            if allow_alias_borrow && matches!(base_java_ty, JavaType::Array(_)) =>
        {
            apply_borrowed_sequence_content_projection(&mut shape);
        }
        ExternalArgumentAliasPolicy::Borrowed
            if allow_alias_borrow
                && !matches!(base_java_ty, JavaType::Primitive(_) | JavaType::Void) =>
        {
            shape.ownership = OwnershipShape::SharedBorrow;
        }
        ExternalArgumentAliasPolicy::NoAlias | ExternalArgumentAliasPolicy::Escapes => {
            if !matches!(base_java_ty, JavaType::Primitive(_) | JavaType::Void) {
                shape.ownership = OwnershipShape::Owned;
            }
        }
        ExternalArgumentAliasPolicy::Unknown | ExternalArgumentAliasPolicy::Borrowed => {}
    }
    if effect.argument_mutability == ReceiverMutabilityFact::MutatesReceiver
        && !matches!(shape.java_ty, JavaType::Primitive(_))
    {
        shape.ownership = OwnershipShape::MutableBorrow;
        shape.mutability = MutabilityShape::Mutable;
    }
    Some(shape)
}

fn external_facade_effect_allows_nullable_borrowed_slot(
    effect: &crate::facts::ExternalApiArgumentEffectFact,
) -> bool {
    effect.parameter_index == 0
        && matches!(effect.method_name.as_str(), "writeObject" | "write_object")
        && effect
            .receiver_type
            .as_deref()
            .map(crate::stdlib::canonical_external_receiver_type_key)
            .and_then(|receiver| receiver.rsplit('.').next().map(str::to_string))
            .is_some_and(|simple| simple == "ObjectOutputStream")
}

fn external_facade_argument_effect<'a>(
    facts: &'a FactSet,
    receiver_key: &str,
    method: &str,
    arg_count: Option<usize>,
    index: usize,
) -> Option<&'a crate::facts::ExternalApiArgumentEffectFact> {
    facts
        .external_api_argument_effect_for_receiver_with_arity(
            Some(receiver_key),
            method,
            arg_count,
            index,
        )
        .or_else(|| {
            let java_method = lower_camel_from_snake(method);
            facts.external_api_argument_effect_for_receiver_with_arity(
                Some(receiver_key),
                &java_method,
                arg_count,
                index,
            )
        })
}

fn is_external_facade_builder_fluent_method(method: &str) -> bool {
    method.starts_with("set_")
        || method.starts_with("add_")
        || method.starts_with("clear_")
        || method.starts_with("merge_")
        || method.starts_with("put_")
}

fn is_builtin_external_facade_type(name: &str) -> bool {
    matches!(
        name,
        "String"
            | "Vec"
            | "Option"
            | "Result"
            | "Box"
            | "Arrays"
            | "Collections"
            | "Collectors"
            | "HashMap"
            | "HashSet"
            | "Iterator"
            | "List"
            | "Map"
            | "Set"
            | "Stream"
            | "Class"
            | "JavaClass"
            | "File"
            | "Path"
            | "URI"
            | "URL"
            | "URLConnection"
            | "JarURLConnection"
            | "JarFile"
            | "JarEntry"
            | "Manifest"
            | "Attributes"
            | "AttributesName"
            | "Enumeration"
            | "AtomicInteger"
            | "ClassLoader"
            | "Locale"
            | "BufferedImage"
            | "Graphics"
            | "Graphics2D"
            | "InputStream"
            | "logger"
            | "bool"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "f32"
            | "f64"
            | "char"
            | "()"
    ) || crate::javalibs::is_builtin_external_facade_type(name)
}

fn is_allowed_builtin_external_from_impl(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        ("InputStream", "JavaIoInputStream") | ("JavaIoInputStream", "InputStream")
    ) || (to == "InputStream"
        && matches!(
            from,
            "FileInputStream"
                | "ByteArrayInputStream"
                | "BufferedInputStream"
                | "DataInputStream"
                | "GZIPInputStream"
                | "ObjectInputStream"
                | "FilterInputStream"
                | "PushbackInputStream"
                | "SequenceInputStream"
        ))
}

fn valid_external_facade_type_ident(name: &str) -> bool {
    valid_rust_type_ident(name)
        && name
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_uppercase())
        && !is_builtin_external_facade_type(name)
}

fn rust_name_for_exact_jdk_stream_adapter_endpoint(name: &str) -> String {
    match name {
        "java.io.InputStream" | "java.io.OutputStream" => rust_name_for_java_named_type(name),
        _ => rust_name_for_java_named_type(&canonical_external_receiver_type_key(name)),
    }
}

fn java_type_name_for_external_facade(java_ty: &JavaType) -> Option<&str> {
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
        _ => None,
    }
}

fn local_type_set_matches_external_name(local_types: &BTreeSet<String>, type_name: &str) -> bool {
    local_types
        .iter()
        .any(|local| crate::facts::external_type_name_matches(local, type_name))
}

fn external_trait_impl_rust_type_candidates(type_name: &str) -> BTreeSet<String> {
    let mut candidates = BTreeSet::new();
    let canonical = canonical_external_receiver_type_key(type_name);
    for candidate in [
        rust_name_for_java_named_type(type_name),
        rust_name_for_java_named_type(&canonical),
    ] {
        if valid_external_facade_type_ident(&candidate)
            && !is_builtin_external_facade_type(&candidate)
        {
            candidates.insert(candidate);
        }
    }
    if let Some(simple) = type_name.rsplit(['.', '$']).next() {
        for candidate in [
            rust_name_for_java_named_type(simple),
            rust_name_for_java_named_type(&canonical_external_receiver_type_key(simple)),
        ] {
            if valid_external_facade_type_ident(&candidate)
                && !is_builtin_external_facade_type(&candidate)
            {
                candidates.insert(candidate);
            }
        }
    }
    candidates
}

fn external_trait_impl_trait_name_candidates(
    implemented_name: &str,
    trait_type_to_name: &BTreeMap<String, String>,
) -> BTreeSet<String> {
    let mut candidates = BTreeSet::new();
    let canonical = canonical_external_receiver_type_key(implemented_name);
    for key in [
        implemented_name.to_string(),
        canonical.clone(),
        implemented_name
            .rsplit(['.', '$'])
            .next()
            .unwrap_or(implemented_name)
            .to_string(),
    ] {
        let key = canonical_external_receiver_type_key(&key);
        if let Some(trait_name) = trait_type_to_name.get(&key)
            && valid_external_facade_type_ident(trait_name)
        {
            candidates.insert(trait_name.clone());
        }
    }
    for candidate in [
        rust_name_for_java_named_type(implemented_name),
        rust_name_for_java_named_type(&canonical),
    ] {
        if valid_external_facade_type_ident(&candidate) {
            candidates.insert(candidate);
        }
    }
    candidates
}

pub fn external_static_string_constant_support_items(
    program: &Program,
    facts: &FactSet,
) -> Vec<crate::rust_ir::RustItem> {
    external_static_string_constant_support_items_with_local_type_program(program, facts, program)
}

pub fn external_static_string_constant_support_items_with_local_type_program(
    program: &Program,
    facts: &FactSet,
    local_type_program: &Program,
) -> Vec<crate::rust_ir::RustItem> {
    external_static_string_constant_support_items_with_context_programs_and_declared_owners(
        program,
        facts,
        local_type_program,
        &Program::default(),
        &BTreeSet::new(),
    )
}

pub fn external_static_string_constant_support_items_with_local_type_program_and_declared_owners(
    program: &Program,
    facts: &FactSet,
    local_type_program: &Program,
    declared_owners: &BTreeSet<String>,
) -> Vec<crate::rust_ir::RustItem> {
    external_static_string_constant_support_items_with_context_programs_and_declared_owners(
        program,
        facts,
        local_type_program,
        &Program::default(),
        declared_owners,
    )
}

pub fn external_static_string_constant_support_items_with_context_programs_and_declared_owners(
    program: &Program,
    facts: &FactSet,
    local_type_program: &Program,
    source_summary_type_program: &Program,
    declared_owners: &BTreeSet<String>,
) -> Vec<crate::rust_ir::RustItem> {
    let source_summary_types = local_type_keys_for_program(source_summary_type_program);
    let local_types = local_type_keys_for_program(local_type_program)
        .into_iter()
        .filter(|name| !source_summary_types.contains(name))
        .collect::<BTreeSet<_>>();
    let local_static_string_values = static_string_constant_values_for_program(local_type_program);
    let mut constants_by_owner: BTreeMap<String, BTreeMap<String, StaticConstantFact>> =
        BTreeMap::new();
    collect_external_static_string_constants_program(
        program,
        facts,
        &local_types,
        &mut constants_by_owner,
    );
    let mapped_types = rust_crate_mapped_external_facade_type_names(facts);
    let mut support_items = Vec::new();
    let mut support_type_names = BTreeSet::new();
    for (owner, fields) in constants_by_owner {
        if mapped_types.contains(&owner) {
            continue;
        }
        collect_static_constant_field_support_types(
            fields.values(),
            &local_types,
            declared_owners,
            &mut support_type_names,
        );
        if let Some(item) = external_static_string_constant_support_item(
            facts,
            &owner,
            fields,
            &local_static_string_values,
            declared_owners,
        ) {
            support_items.push(item);
        }
    }
    for type_name in support_type_names {
        support_items.push(crate::rust_ir::RustItem::Raw(format!(
            "#[derive(Clone, Debug, Default)]\npub struct {type_name};"
        )));
    }
    support_items
}

fn collect_static_constant_field_support_types<'a>(
    constants: impl Iterator<Item = &'a StaticConstantFact>,
    local_types: &BTreeSet<String>,
    declared_owners: &BTreeSet<String>,
    support_type_names: &mut BTreeSet<String>,
) {
    for constant in constants {
        if constant.field_kind == ExternalFieldKind::EnumVariant {
            continue;
        }
        collect_static_constant_java_type_support(
            &constant.field_type,
            local_types,
            declared_owners,
            support_type_names,
        );
    }
}

fn collect_static_constant_java_type_support(
    java_ty: &JavaType,
    local_types: &BTreeSet<String>,
    declared_owners: &BTreeSet<String>,
    support_type_names: &mut BTreeSet<String>,
) {
    match java_ty {
        JavaType::Named { name, args } => {
            let key = canonical_external_receiver_type_key(name);
            let rust_name = rust_name_for_java_named_type(&key);
            if valid_external_facade_type_ident(&rust_name)
                && !local_types.contains(&key)
                && !declared_owners.contains(&rust_name)
                && !is_builtin_external_facade_type(&rust_name)
            {
                support_type_names.insert(rust_name);
            }
            for arg in args {
                collect_static_constant_java_type_support(
                    arg,
                    local_types,
                    declared_owners,
                    support_type_names,
                );
            }
        }
        JavaType::Unknown(name) => {
            let key = canonical_external_receiver_type_key(name);
            let rust_name = rust_name_for_java_named_type(&key);
            if valid_external_facade_type_ident(&rust_name)
                && !local_types.contains(&key)
                && !declared_owners.contains(&rust_name)
                && !is_builtin_external_facade_type(&rust_name)
            {
                support_type_names.insert(rust_name);
            }
        }
        JavaType::Array(inner) => collect_static_constant_java_type_support(
            inner,
            local_types,
            declared_owners,
            support_type_names,
        ),
        JavaType::Wildcard(Some(bound)) => match bound {
            WildcardBound::Extends(inner) | WildcardBound::Super(inner) => {
                collect_static_constant_java_type_support(
                    inner,
                    local_types,
                    declared_owners,
                    support_type_names,
                )
            }
        },
        JavaType::Primitive(_) | JavaType::Void | JavaType::String | JavaType::Wildcard(None) => {}
    }
}

pub fn support_item_declared_type_names(items: &[crate::rust_ir::RustItem]) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for item in items {
        match item {
            crate::rust_ir::RustItem::Struct(strukt) => {
                names.insert(strukt.name.clone());
            }
            crate::rust_ir::RustItem::Raw(raw) => {
                collect_raw_support_declared_type_names(raw, &mut names);
            }
            crate::rust_ir::RustItem::Impl(_)
            | crate::rust_ir::RustItem::Function(_)
            | crate::rust_ir::RustItem::Comment(_) => {}
        }
    }
    names
}

fn collect_raw_support_declared_type_names(raw: &str, names: &mut BTreeSet<String>) {
    for line in raw.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("pub ") else {
            continue;
        };
        let Some(name) = rest
            .strip_prefix("struct ")
            .or_else(|| rest.strip_prefix("enum "))
            .or_else(|| rest.strip_prefix("type "))
        else {
            continue;
        };
        let name = name
            .split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
            .next()
            .unwrap_or_default();
        if valid_external_facade_type_ident(name) {
            names.insert(name.to_string());
        }
    }
}

fn static_string_constant_values_for_program(
    program: &Program,
) -> BTreeMap<(String, String), String> {
    let mut values = BTreeMap::new();
    for ty in &program.types {
        collect_static_string_constant_values_for_type(ty, &mut values);
    }
    values
}

fn collect_static_string_constant_values_for_type(
    ty: &crate::java_ir::TypeDecl,
    values: &mut BTreeMap<(String, String), String>,
) {
    for member in &ty.members {
        match member {
            Member::Field(field)
                if field.modifiers.contains(&crate::java_ir::Modifier::Static)
                    && field.modifiers.contains(&crate::java_ir::Modifier::Final)
                    && field.ty == JavaType::String =>
            {
                if let Some(Expr::Literal(crate::java_ir::Literal::String(value))) =
                    &field.initializer
                {
                    values.insert((ty.name.clone(), field.name.clone()), value.clone());
                }
            }
            Member::Type(nested) => collect_static_string_constant_values_for_type(nested, values),
            _ => {}
        }
    }
}

fn collect_external_static_string_constants_program(
    program: &Program,
    facts: &FactSet,
    local_types: &BTreeSet<String>,
    constants_by_owner: &mut BTreeMap<String, BTreeMap<String, StaticConstantFact>>,
) {
    let imported_owner_aliases = imported_static_owner_aliases(program);
    for ty in &program.types {
        collect_external_static_string_constants_type(
            ty,
            facts,
            local_types,
            &imported_owner_aliases,
            constants_by_owner,
        );
    }
}

fn collect_external_static_string_constants_type(
    ty: &crate::java_ir::TypeDecl,
    facts: &FactSet,
    local_types: &BTreeSet<String>,
    imported_owner_aliases: &BTreeMap<String, String>,
    constants_by_owner: &mut BTreeMap<String, BTreeMap<String, StaticConstantFact>>,
) {
    for member in &ty.members {
        match member {
            Member::Field(field) => {
                if let Some(initializer) = &field.initializer {
                    collect_external_static_string_constants_expr(
                        initializer,
                        facts,
                        local_types,
                        imported_owner_aliases,
                        constants_by_owner,
                    );
                }
            }
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    collect_external_static_string_constants_block(
                        body,
                        facts,
                        local_types,
                        imported_owner_aliases,
                        constants_by_owner,
                    );
                }
            }
            Member::Constructor(constructor) => {
                if let Some(body) = &constructor.body {
                    collect_external_static_string_constants_block(
                        body,
                        facts,
                        local_types,
                        imported_owner_aliases,
                        constants_by_owner,
                    );
                }
            }
            Member::Type(nested) => collect_external_static_string_constants_type(
                nested,
                facts,
                local_types,
                imported_owner_aliases,
                constants_by_owner,
            ),
            Member::Unsupported { .. } => {}
        }
    }
}

fn collect_external_static_string_constants_block(
    block: &Block,
    facts: &FactSet,
    local_types: &BTreeSet<String>,
    imported_owner_aliases: &BTreeMap<String, String>,
    constants_by_owner: &mut BTreeMap<String, BTreeMap<String, StaticConstantFact>>,
) {
    for stmt in &block.statements {
        collect_external_static_string_constants_stmt(
            stmt,
            facts,
            local_types,
            imported_owner_aliases,
            constants_by_owner,
        );
    }
}

fn collect_external_static_string_constants_stmt(
    stmt: &Stmt,
    facts: &FactSet,
    local_types: &BTreeSet<String>,
    imported_owner_aliases: &BTreeMap<String, String>,
    constants_by_owner: &mut BTreeMap<String, BTreeMap<String, StaticConstantFact>>,
) {
    match stmt {
        Stmt::VarDecl { initializer, .. } | Stmt::Return(initializer) => {
            if let Some(expr) = initializer {
                collect_external_static_string_constants_expr(
                    expr,
                    facts,
                    local_types,
                    imported_owner_aliases,
                    constants_by_owner,
                );
            }
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => collect_external_static_string_constants_expr(
            expr,
            facts,
            local_types,
            imported_owner_aliases,
            constants_by_owner,
        ),
        Stmt::Assign { target, value } => {
            collect_external_static_string_constants_expr(
                target,
                facts,
                local_types,
                imported_owner_aliases,
                constants_by_owner,
            );
            collect_external_static_string_constants_expr(
                value,
                facts,
                local_types,
                imported_owner_aliases,
                constants_by_owner,
            );
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_external_static_string_constants_expr(
                condition,
                facts,
                local_types,
                imported_owner_aliases,
                constants_by_owner,
            );
            collect_external_static_string_constants_block(
                then_block,
                facts,
                local_types,
                imported_owner_aliases,
                constants_by_owner,
            );
            if let Some(else_block) = else_block {
                collect_external_static_string_constants_block(
                    else_block,
                    facts,
                    local_types,
                    imported_owner_aliases,
                    constants_by_owner,
                );
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            collect_external_static_string_constants_expr(
                condition,
                facts,
                local_types,
                imported_owner_aliases,
                constants_by_owner,
            );
            collect_external_static_string_constants_block(
                body,
                facts,
                local_types,
                imported_owner_aliases,
                constants_by_owner,
            );
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for init in init {
                collect_external_static_string_constants_stmt(
                    init,
                    facts,
                    local_types,
                    imported_owner_aliases,
                    constants_by_owner,
                );
            }
            if let Some(condition) = condition {
                collect_external_static_string_constants_expr(
                    condition,
                    facts,
                    local_types,
                    imported_owner_aliases,
                    constants_by_owner,
                );
            }
            for update in update {
                collect_external_static_string_constants_expr(
                    update,
                    facts,
                    local_types,
                    imported_owner_aliases,
                    constants_by_owner,
                );
            }
            collect_external_static_string_constants_block(
                body,
                facts,
                local_types,
                imported_owner_aliases,
                constants_by_owner,
            );
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            collect_external_static_string_constants_expr(
                iterable,
                facts,
                local_types,
                imported_owner_aliases,
                constants_by_owner,
            );
            collect_external_static_string_constants_block(
                body,
                facts,
                local_types,
                imported_owner_aliases,
                constants_by_owner,
            );
        }
        Stmt::Switch { selector, cases } => {
            collect_external_static_string_constants_expr(
                selector,
                facts,
                local_types,
                imported_owner_aliases,
                constants_by_owner,
            );
            for case in cases {
                for stmt in &case.statements {
                    collect_external_static_string_constants_stmt(
                        stmt,
                        facts,
                        local_types,
                        imported_owner_aliases,
                        constants_by_owner,
                    );
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
                collect_external_static_string_constants_stmt(
                    resource,
                    facts,
                    local_types,
                    imported_owner_aliases,
                    constants_by_owner,
                );
            }
            collect_external_static_string_constants_block(
                body,
                facts,
                local_types,
                imported_owner_aliases,
                constants_by_owner,
            );
            for catch in catches {
                collect_external_static_string_constants_block(
                    &catch.body,
                    facts,
                    local_types,
                    imported_owner_aliases,
                    constants_by_owner,
                );
            }
            if let Some(finally_block) = finally_block {
                collect_external_static_string_constants_block(
                    finally_block,
                    facts,
                    local_types,
                    imported_owner_aliases,
                    constants_by_owner,
                );
            }
        }
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                collect_external_static_string_constants_expr(
                    arg,
                    facts,
                    local_types,
                    imported_owner_aliases,
                    constants_by_owner,
                );
            }
        }
        Stmt::LocalType(ty) => {
            collect_external_static_string_constants_type(
                ty,
                facts,
                local_types,
                imported_owner_aliases,
                constants_by_owner,
            );
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            collect_external_static_string_constants_block(
                block,
                facts,
                local_types,
                imported_owner_aliases,
                constants_by_owner,
            );
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
    }
}

fn collect_external_static_string_constants_expr(
    expr: &Expr,
    facts: &FactSet,
    local_types: &BTreeSet<String>,
    imported_owner_aliases: &BTreeMap<String, String>,
    constants_by_owner: &mut BTreeMap<String, BTreeMap<String, StaticConstantFact>>,
) {
    match expr {
        Expr::FieldAccess(receiver, field) => {
            if let Some((owner, constant)) =
                external_static_constant_owner(receiver, field, facts, imported_owner_aliases)
                && !local_types.contains(&owner)
            {
                constants_by_owner
                    .entry(owner)
                    .or_default()
                    .insert(canonical_external_field_name(field), constant);
            }
            collect_external_static_string_constants_expr(
                receiver,
                facts,
                local_types,
                imported_owner_aliases,
                constants_by_owner,
            );
        }
        Expr::ArrayAccess(array, index) => {
            collect_external_static_string_constants_expr(
                array,
                facts,
                local_types,
                imported_owner_aliases,
                constants_by_owner,
            );
            collect_external_static_string_constants_expr(
                index,
                facts,
                local_types,
                imported_owner_aliases,
                constants_by_owner,
            );
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for dimension in dimensions {
                collect_external_static_string_constants_expr(
                    dimension,
                    facts,
                    local_types,
                    imported_owner_aliases,
                    constants_by_owner,
                );
            }
            for value in initializer.iter().flatten() {
                collect_external_static_string_constants_expr(
                    value,
                    facts,
                    local_types,
                    imported_owner_aliases,
                    constants_by_owner,
                );
            }
        }
        Expr::ArrayInitializer(values) => {
            for value in values {
                collect_external_static_string_constants_expr(
                    value,
                    facts,
                    local_types,
                    imported_owner_aliases,
                    constants_by_owner,
                );
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_external_static_string_constants_expr(
                left,
                facts,
                local_types,
                imported_owner_aliases,
                constants_by_owner,
            );
            collect_external_static_string_constants_expr(
                right,
                facts,
                local_types,
                imported_owner_aliases,
                constants_by_owner,
            );
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_external_static_string_constants_expr(
                expr,
                facts,
                local_types,
                imported_owner_aliases,
                constants_by_owner,
            );
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_external_static_string_constants_expr(
                condition,
                facts,
                local_types,
                imported_owner_aliases,
                constants_by_owner,
            );
            collect_external_static_string_constants_expr(
                then_expr,
                facts,
                local_types,
                imported_owner_aliases,
                constants_by_owner,
            );
            collect_external_static_string_constants_expr(
                else_expr,
                facts,
                local_types,
                imported_owner_aliases,
                constants_by_owner,
            );
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_external_static_string_constants_expr(
                    receiver,
                    facts,
                    local_types,
                    imported_owner_aliases,
                    constants_by_owner,
                );
            }
            for arg in args {
                collect_external_static_string_constants_expr(
                    arg,
                    facts,
                    local_types,
                    imported_owner_aliases,
                    constants_by_owner,
                );
            }
        }
        Expr::New { args, body, .. } => {
            for arg in args {
                collect_external_static_string_constants_expr(
                    arg,
                    facts,
                    local_types,
                    imported_owner_aliases,
                    constants_by_owner,
                );
            }
            for member in body.iter().flatten() {
                match member {
                    Member::Field(field) => {
                        if let Some(initializer) = &field.initializer {
                            collect_external_static_string_constants_expr(
                                initializer,
                                facts,
                                local_types,
                                imported_owner_aliases,
                                constants_by_owner,
                            );
                        }
                    }
                    Member::Method(method) => {
                        if let Some(body) = &method.body {
                            collect_external_static_string_constants_block(
                                body,
                                facts,
                                local_types,
                                imported_owner_aliases,
                                constants_by_owner,
                            );
                        }
                    }
                    Member::Constructor(constructor) => {
                        if let Some(body) = &constructor.body {
                            collect_external_static_string_constants_block(
                                body,
                                facts,
                                local_types,
                                imported_owner_aliases,
                                constants_by_owner,
                            );
                        }
                    }
                    Member::Type(nested) => collect_external_static_string_constants_type(
                        nested,
                        facts,
                        local_types,
                        imported_owner_aliases,
                        constants_by_owner,
                    ),
                    Member::Unsupported { .. } => {}
                }
            }
        }
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => {
                collect_external_static_string_constants_expr(
                    expr,
                    facts,
                    local_types,
                    imported_owner_aliases,
                    constants_by_owner,
                )
            }
            crate::java_ir::LambdaBody::Block(block) => {
                collect_external_static_string_constants_block(
                    block,
                    facts,
                    local_types,
                    imported_owner_aliases,
                    constants_by_owner,
                )
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

#[derive(Clone)]
struct StaticConstantFact {
    field_type: JavaType,
    field_kind: ExternalFieldKind,
    alias_owner: Option<String>,
}

fn external_static_constant_owner(
    receiver: &Expr,
    field: &str,
    facts: &FactSet,
    imported_owner_aliases: &BTreeMap<String, String>,
) -> Option<(String, StaticConstantFact)> {
    let receiver_name = static_constant_receiver_java_name(receiver)?;
    let owner_name = imported_owner_aliases
        .get(receiver_name)
        .map(String::as_str)
        .unwrap_or(receiver_name);
    let lookup_owner = canonical_external_receiver_type_key(owner_name);
    let local_fact = facts.external_field_for(&lookup_owner, field).cloned();
    let builtin_fact = crate::external_packs::external_field_for(&lookup_owner, field);
    let explicit_fact = match (local_fact, builtin_fact) {
        (Some(local), Some(builtin))
            if local.provenance != FactProvenance::UserOverride
                && builtin.provenance == FactProvenance::ExternalLibraryPack =>
        {
            Some(builtin)
        }
        (Some(local), _) => Some(local),
        (None, builtin) => builtin,
    };
    if explicit_fact
        .as_ref()
        .is_some_and(|fact| fact.field_kind == ExternalFieldKind::NestedType)
    {
        return None;
    }
    let field_type = explicit_fact
        .as_ref()
        .and_then(|fact| fact.field_type.clone())
        .or_else(|| {
            let receiver_type = JavaType::Named {
                name: lookup_owner.clone(),
                args: Vec::new(),
            };
            known_external_field_type_for_receiver(&receiver_type, field)
        })?;
    static_constant_rust_type(&field_type)?;
    let owner_name = explicit_fact
        .as_ref()
        .map(|fact| fact.field.type_name.as_str())
        .unwrap_or(owner_name);
    let rust_owner = external_static_constant_rust_owner(owner_name)?;
    if !valid_rust_type_ident(&rust_owner) {
        return None;
    }
    let field_kind = explicit_fact
        .as_ref()
        .map(|fact| fact.field_kind)
        .unwrap_or(ExternalFieldKind::StaticFinal);
    Some((
        rust_owner,
        StaticConstantFact {
            field_type,
            field_kind,
            alias_owner: (owner_name != receiver_name)
                .then(|| external_static_constant_rust_owner(receiver_name))
                .flatten(),
        },
    ))
}

fn static_constant_receiver_java_name(receiver: &Expr) -> Option<&str> {
    match receiver {
        Expr::Var(receiver_name) => Some(receiver_name),
        Expr::FieldAccess(_, field) => Some(field),
        _ => None,
    }
}

fn external_static_constant_rust_owner(owner_name: &str) -> Option<String> {
    if owner_name.contains('.') {
        return Some(rust_name_for_java_named_type(
            &canonical_external_receiver_type_key(owner_name),
        ));
    }
    if owner_name
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
    {
        Some(owner_name.to_string())
    } else if owner_name.contains('_') || owner_name.chars().all(|ch| ch.is_ascii_lowercase()) {
        Some(upper_camel_from_snake(owner_name))
    } else {
        None
    }
}

fn external_static_string_constant_support_item(
    facts: &FactSet,
    owner: &str,
    fields: BTreeMap<String, StaticConstantFact>,
    local_static_string_values: &BTreeMap<(String, String), String>,
    declared_owners: &BTreeSet<String>,
) -> Option<crate::rust_ir::RustItem> {
    if is_awt_platform_facade_rust_ident(owner) {
        return None;
    }
    let alias_owners = fields
        .values()
        .filter_map(|constant| constant.alias_owner.as_deref())
        .filter(|alias| *alias != owner && valid_rust_type_ident(alias))
        .filter(|alias| !is_awt_platform_facade_rust_ident(alias))
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let consts = static_constant_impl_lines(owner, &fields, local_static_string_values);
    if consts.is_empty() {
        return None;
    }
    let impl_block = format!("impl {owner} {{\n{}\n}}", consts.join("\n"));
    let aliases = alias_owners
        .iter()
        .filter(|alias| !declared_owners.contains(*alias))
        .map(|alias| format!("pub type {alias} = {owner};"))
        .collect::<Vec<_>>()
        .join("\n");
    let alias_impls = alias_owners
        .iter()
        .filter(|alias| declared_owners.contains(*alias))
        .filter_map(|alias| {
            let consts = static_constant_impl_lines(alias, &fields, local_static_string_values);
            (!consts.is_empty()).then(|| format!("impl {alias} {{\n{}\n}}", consts.join("\n")))
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let external_trait_impls =
        external_static_enum_constant_trait_impls(facts, owner, &fields, declared_owners);
    let raw = if declared_owners.contains(owner) {
        [aliases, impl_block, alias_impls, external_trait_impls]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
    } else {
        let owner_block =
            format!("#[derive(Clone, Debug, Default)]\npub struct {owner};\n\n{impl_block}");
        [owner_block, aliases, alias_impls, external_trait_impls]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
    };
    Some(crate::rust_ir::RustItem::Raw(raw))
}

fn external_static_enum_constant_trait_impls(
    facts: &FactSet,
    owner: &str,
    fields: &BTreeMap<String, StaticConstantFact>,
    declared_owners: &BTreeSet<String>,
) -> String {
    if declared_owners.contains(owner)
        || !valid_external_facade_type_ident(owner)
        || !fields
            .values()
            .any(|constant| constant.field_kind == ExternalFieldKind::EnumVariant)
    {
        return String::new();
    }

    let mut lines = Vec::new();
    let mut trait_impls = BTreeSet::new();
    for capability in facts
        .type_capability_overrides
        .iter()
        .filter(|capability| crate::facts::external_type_name_matches(&capability.type_name, owner))
    {
        for implemented in &capability.implements {
            let Some(implemented_name) = java_type_name_for_external_facade(implemented) else {
                continue;
            };
            if facts.object_safe_trait_fact_for_name(implemented_name) != Some(true) {
                continue;
            }
            for trait_name in
                external_trait_impl_trait_name_candidates(implemented_name, &BTreeMap::new())
            {
                trait_impls.insert(trait_name);
            }
        }
    }
    if !trait_impls.is_empty() {
        lines.push(format!(
            "impl std::fmt::Display for {owner} {{\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{ write!(f, \"\") }}\n}}"
        ));
        for trait_name in trait_impls {
            lines.push(format!("impl {trait_name} for {owner} {{}}"));
        }
    }
    lines.join("\n\n")
}

fn static_constant_impl_lines(
    owner: &str,
    fields: &BTreeMap<String, StaticConstantFact>,
    local_static_string_values: &BTreeMap<(String, String), String>,
) -> Vec<String> {
    if canonical_external_receiver_type_key(owner) == "Level" {
        return Vec::new();
    }
    fields
        .iter()
        .filter(|(field, _)| valid_rust_const_ident(field))
        .filter_map(|(field, constant)| {
            if constant.field_kind == ExternalFieldKind::NestedType {
                return None;
            }
            let (rust_type, value) = if constant.field_kind == ExternalFieldKind::EnumVariant {
                (owner.to_string(), owner.to_string())
            } else {
                let rust_type = static_constant_rust_type(&constant.field_type)?;
                let value = static_constant_default_value(
                    owner,
                    field,
                    &constant.field_type,
                    local_static_string_values,
                )?;
                (rust_type, value)
            };
            let mut lines = vec![format!("    pub const {field}: {rust_type} = {value};")];
            if owner == "System" && field == "IN" {
                lines.push(format!("    pub const r#in: {rust_type} = {value};"));
            }
            Some(lines.join("\n"))
        })
        .collect()
}

fn static_constant_rust_type(java_ty: &JavaType) -> Option<String> {
    match java_ty {
        JavaType::String => Some("&'static str".to_string()),
        JavaType::Array(inner) if matches!(inner.as_ref(), JavaType::String) => {
            Some("&'static [&'static str]".to_string())
        }
        JavaType::Array(inner) => primitive_array_static_constant_rust_type(inner.as_ref()),
        JavaType::Primitive(PrimitiveType::Boolean) => Some("bool".to_string()),
        JavaType::Primitive(PrimitiveType::Byte) => Some("i8".to_string()),
        JavaType::Primitive(PrimitiveType::Short) => Some("i16".to_string()),
        JavaType::Primitive(PrimitiveType::Int) => Some("i32".to_string()),
        JavaType::Primitive(PrimitiveType::Long) => Some("i64".to_string()),
        JavaType::Primitive(PrimitiveType::Float) => Some("f32".to_string()),
        JavaType::Primitive(PrimitiveType::Double) => Some("f64".to_string()),
        JavaType::Primitive(PrimitiveType::Char) => Some("char".to_string()),
        JavaType::Named { name, args }
            if name.rsplit('.').next().unwrap_or(name) == "List"
                && matches!(args.as_slice(), [JavaType::String]) =>
        {
            Some("&'static [&'static str]".to_string())
        }
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            static_constant_named_value_type(name)
        }
        _ => None,
    }
}

fn static_constant_named_value_type(name: &str) -> Option<String> {
    let canonical = canonical_external_receiver_type_key(name);
    let rust_type = rust_name_for_java_named_type(&canonical);
    valid_rust_type_ident(&rust_type).then_some(rust_type)
}

fn static_constant_default_value(
    owner: &str,
    field: &str,
    java_ty: &JavaType,
    local_static_string_values: &BTreeMap<(String, String), String>,
) -> Option<String> {
    if let Some(value) = known_jdk_static_constant_value(owner, field) {
        return Some(value.to_string());
    }
    if owner.rsplit('.').next().unwrap_or(owner) == "BigDecimal"
        && let Some(value) = big_decimal_rounding_constant_value(field)
    {
        return Some(value.to_string());
    }
    match java_ty {
        JavaType::String => {
            let value = local_static_string_values
                .get(&(owner.to_string(), field.to_string()))
                .cloned()
                .unwrap_or_else(|| field.to_string());
            Some(format!("{value:?}"))
        }
        JavaType::Array(inner) if matches!(inner.as_ref(), JavaType::String) => {
            Some(static_string_slice_default_value(owner, field))
        }
        JavaType::Array(inner) if primitive_array_static_constant_rust_type(inner).is_some() => {
            Some("Vec::new()".to_string())
        }
        JavaType::Primitive(PrimitiveType::Boolean) => Some("false".to_string()),
        JavaType::Primitive(PrimitiveType::Byte)
        | JavaType::Primitive(PrimitiveType::Short)
        | JavaType::Primitive(PrimitiveType::Int) => Some("0".to_string()),
        JavaType::Primitive(PrimitiveType::Long) => Some("0i64".to_string()),
        JavaType::Primitive(PrimitiveType::Float) => Some("0.0f32".to_string()),
        JavaType::Primitive(PrimitiveType::Double) => Some("0.0f64".to_string()),
        JavaType::Primitive(PrimitiveType::Char) => Some("'\\0'".to_string()),
        JavaType::Named { name, args }
            if name.rsplit('.').next().unwrap_or(name) == "List"
                && matches!(args.as_slice(), [JavaType::String]) =>
        {
            Some(static_string_slice_default_value(owner, field))
        }
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            let rust_type =
                rust_name_for_java_named_type(&canonical_external_receiver_type_key(name));
            valid_rust_type_ident(&rust_type).then_some(rust_type)
        }
        _ => None,
    }
}

fn known_jdk_static_constant_value(owner: &str, field: &str) -> Option<&'static str> {
    let owner = owner.rsplit('.').next().unwrap_or(owner);
    let field = canonical_external_field_name(field);
    match (owner, field.as_str()) {
        ("Boolean", "TRUE") => Some("true"),
        ("Boolean", "FALSE") => Some("false"),
        ("Byte", "MAX_VALUE") => Some("i8::MAX"),
        ("Byte", "MIN_VALUE") => Some("i8::MIN"),
        ("Short", "MAX_VALUE") => Some("i16::MAX"),
        ("Short", "MIN_VALUE") => Some("i16::MIN"),
        ("Integer", "MAX_VALUE") => Some("i32::MAX"),
        ("Integer", "MIN_VALUE") => Some("i32::MIN"),
        ("Long", "MAX_VALUE") => Some("i64::MAX"),
        ("Long", "MIN_VALUE") => Some("i64::MIN"),
        ("Character", "MAX_VALUE") => Some("'\\u{ffff}'"),
        ("Character", "MIN_VALUE") => Some("'\\0'"),
        ("Float", "MAX_VALUE") => Some("f32::MAX"),
        ("Float", "MIN_VALUE") => Some("f32::from_bits(1)"),
        ("Float", "NaN") => Some("f32::NAN"),
        ("Float", "POSITIVE_INFINITY") => Some("f32::INFINITY"),
        ("Float", "NEGATIVE_INFINITY") => Some("f32::NEG_INFINITY"),
        ("Double", "MAX_VALUE") => Some("f64::MAX"),
        ("Double", "MIN_VALUE") => Some("f64::from_bits(1)"),
        ("Double", "NaN") => Some("f64::NAN"),
        ("Double", "POSITIVE_INFINITY") => Some("f64::INFINITY"),
        ("Double", "NEGATIVE_INFINITY") => Some("f64::NEG_INFINITY"),
        ("Math", "PI") => Some("std::f64::consts::PI"),
        ("Math", "E") => Some("std::f64::consts::E"),
        (owner, "SIZE") => wrapper_size_constant_value(owner),
        (owner, "BYTES") => wrapper_bytes_constant_value(owner),
        _ => None,
    }
}

fn wrapper_size_constant_value(owner: &str) -> Option<&'static str> {
    match owner {
        "Byte" => Some("8"),
        "Short" | "Character" => Some("16"),
        "Integer" | "Float" => Some("32"),
        "Long" | "Double" => Some("64"),
        _ => None,
    }
}

fn wrapper_bytes_constant_value(owner: &str) -> Option<&'static str> {
    match owner {
        "Byte" => Some("1"),
        "Short" | "Character" => Some("2"),
        "Integer" | "Float" => Some("4"),
        "Long" | "Double" => Some("8"),
        _ => None,
    }
}

fn big_decimal_rounding_constant_value(field: &str) -> Option<i32> {
    match canonical_external_field_name(field).as_str() {
        "ROUND_UP" => Some(0),
        "ROUND_DOWN" => Some(1),
        "ROUND_CEILING" => Some(2),
        "ROUND_FLOOR" => Some(3),
        "ROUND_HALF_UP" => Some(4),
        "ROUND_HALF_DOWN" => Some(5),
        "ROUND_HALF_EVEN" => Some(6),
        "ROUND_UNNECESSARY" => Some(7),
        _ => None,
    }
}

fn source_summary_static_constant_value(
    owner: &str,
    field: &str,
    java_ty: &JavaType,
    initializer: Option<&Expr>,
) -> Option<String> {
    initializer
        .and_then(|expr| source_summary_static_literal_value(java_ty, expr))
        .or_else(|| static_constant_default_value(owner, field, java_ty, &BTreeMap::new()))
}

fn source_summary_static_literal_value(java_ty: &JavaType, initializer: &Expr) -> Option<String> {
    let Expr::Literal(literal) = initializer else {
        return None;
    };
    match (java_ty, literal) {
        (JavaType::String, Literal::String(value)) => Some(format!("{value:?}")),
        (JavaType::Primitive(PrimitiveType::Boolean), Literal::Bool(value)) => {
            Some(value.to_string())
        }
        (JavaType::Primitive(PrimitiveType::Byte), Literal::Int(value))
        | (JavaType::Primitive(PrimitiveType::Short), Literal::Int(value))
        | (JavaType::Primitive(PrimitiveType::Int), Literal::Int(value)) => Some(value.to_string()),
        (JavaType::Primitive(PrimitiveType::Long), Literal::Long(value)) => {
            Some(format!("{value}i64"))
        }
        (JavaType::Primitive(PrimitiveType::Float), Literal::Float(value)) => {
            Some(format!("{value}f32"))
        }
        (JavaType::Primitive(PrimitiveType::Double), Literal::Double(value)) => {
            Some(format!("{value}f64"))
        }
        (JavaType::Primitive(PrimitiveType::Char), Literal::Char(value)) => {
            Some(format!("{value:?}"))
        }
        _ => None,
    }
}

fn auto_external_static_constant_surface_type_supported(java_ty: &JavaType) -> bool {
    matches!(
        java_ty,
        JavaType::String
            | JavaType::Array(_)
            | JavaType::Primitive(PrimitiveType::Boolean)
            | JavaType::Primitive(PrimitiveType::Byte)
            | JavaType::Primitive(PrimitiveType::Short)
            | JavaType::Primitive(PrimitiveType::Int)
            | JavaType::Primitive(PrimitiveType::Long)
            | JavaType::Primitive(PrimitiveType::Float)
            | JavaType::Primitive(PrimitiveType::Double)
            | JavaType::Primitive(PrimitiveType::Char)
    ) && static_constant_rust_type(java_ty).is_some()
}

fn primitive_array_static_constant_rust_type(java_ty: &JavaType) -> Option<String> {
    let element = match java_ty {
        JavaType::Primitive(PrimitiveType::Boolean) => "bool",
        JavaType::Primitive(PrimitiveType::Byte) => "i8",
        JavaType::Primitive(PrimitiveType::Short) => "i16",
        JavaType::Primitive(PrimitiveType::Int) => "i32",
        JavaType::Primitive(PrimitiveType::Long) => "i64",
        JavaType::Primitive(PrimitiveType::Float) => "f32",
        JavaType::Primitive(PrimitiveType::Double) => "f64",
        JavaType::Primitive(PrimitiveType::Char) => "char",
        _ => return None,
    };
    Some(format!("Vec<{element}>"))
}

fn static_string_slice_default_value(_owner: &str, _field: &str) -> String {
    "&[]".to_string()
}

fn valid_rust_type_ident(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn valid_rust_const_ident(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn valid_rust_field_ident(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn rust_raw_ident_if_needed(name: &str) -> String {
    if name.starts_with("r#")
        || name.contains("::")
        || name.contains('<')
        || name.contains('>')
        || name.contains(' ')
        || name.is_empty()
    {
        return name.to_string();
    }
    if is_rust_raw_identifier_keyword(name) {
        format!("r#{name}")
    } else {
        name.to_string()
    }
}

fn is_rust_raw_identifier_keyword(name: &str) -> bool {
    matches!(
        name,
        "as" | "async"
            | "await"
            | "break"
            | "const"
            | "continue"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "static"
            | "struct"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "macro"
            | "override"
            | "priv"
            | "try"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
    )
}

fn valid_rust_method_ident(name: &str) -> bool {
    valid_rust_field_ident(name)
}

pub fn program_uses_java_platform_facades(program: &Program) -> bool {
    if program.types.iter().any(|ty| {
        matches!(
            crate::stdlib::canonical_external_receiver_type_key(&ty.name).as_str(),
            "Path"
                | "URI"
                | "File"
                | "URL"
                | "URLConnection"
                | "JarURLConnection"
                | "JarFile"
                | "JarEntry"
                | "Manifest"
                | "Attributes"
                | "AttributesName"
                | "Enumeration"
                | "Iterator"
                | "AtomicInteger"
                | "ClassLoader"
                | "BufferedImage"
                | "Graphics"
                | "Graphics2D"
        )
    }) {
        return false;
    }
    program.imports.iter().any(|import| {
        matches!(
            import.path.as_str(),
            "java.nio.file.Path"
                | "java.net.URI"
                | "java.io.File"
                | "java.net.URL"
                | "java.net.URLConnection"
                | "java.net.JarURLConnection"
                | "java.util.jar.JarFile"
                | "java.util.jar.JarEntry"
                | "java.util.jar.Manifest"
                | "java.util.jar.Attributes"
                | "java.util.jar.Attributes.Name"
                | "java.util.Enumeration"
                | "java.util.Iterator"
                | "java.util.concurrent.atomic.AtomicInteger"
                | "java.lang.ClassLoader"
                | "java.awt.image.BufferedImage"
                | "java.awt.Graphics"
                | "java.awt.Graphics2D"
        )
    }) || program.types.iter().any(type_uses_java_platform_facades)
}

fn type_uses_java_platform_facades(ty: &crate::java_ir::TypeDecl) -> bool {
    ty.extends
        .as_deref()
        .is_some_and(is_java_platform_facade_type_name)
        || ty
            .implements
            .iter()
            .any(|name| is_java_platform_facade_type_name(name))
        || ty
            .type_param_bounds
            .iter()
            .flatten()
            .any(java_type_uses_java_platform_facade)
        || ty.members.iter().any(member_uses_java_platform_facades)
}

fn member_uses_java_platform_facades(member: &crate::java_ir::Member) -> bool {
    match member {
        crate::java_ir::Member::Field(field) => {
            java_type_uses_java_platform_facade(&field.ty)
                || field
                    .initializer
                    .as_ref()
                    .is_some_and(expr_uses_java_platform_facades)
        }
        crate::java_ir::Member::Method(method) => {
            java_type_uses_java_platform_facade(&method.return_type)
                || method
                    .throws
                    .iter()
                    .any(java_type_uses_java_platform_facade)
                || method
                    .params
                    .iter()
                    .any(|param| java_type_uses_java_platform_facade(&param.ty))
                || method
                    .type_param_bounds
                    .iter()
                    .flatten()
                    .any(java_type_uses_java_platform_facade)
                || method
                    .body
                    .as_ref()
                    .is_some_and(block_uses_java_platform_facades)
        }
        crate::java_ir::Member::Constructor(constructor) => {
            constructor
                .throws
                .iter()
                .any(java_type_uses_java_platform_facade)
                || constructor
                    .params
                    .iter()
                    .any(|param| java_type_uses_java_platform_facade(&param.ty))
                || constructor
                    .type_param_bounds
                    .iter()
                    .flatten()
                    .any(java_type_uses_java_platform_facade)
                || constructor
                    .body
                    .as_ref()
                    .is_some_and(block_uses_java_platform_facades)
        }
        crate::java_ir::Member::Type(nested) => type_uses_java_platform_facades(nested),
        crate::java_ir::Member::Unsupported { .. } => false,
    }
}

fn block_uses_java_platform_facades(block: &crate::java_ir::Block) -> bool {
    block.statements.iter().any(stmt_uses_java_platform_facades)
}

fn stmt_uses_java_platform_facades(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::VarDecl {
            ty, initializer, ..
        } => {
            java_type_uses_java_platform_facade(ty)
                || initializer
                    .as_ref()
                    .is_some_and(expr_uses_java_platform_facades)
        }
        Stmt::Return(initializer) => initializer
            .as_ref()
            .is_some_and(expr_uses_java_platform_facades),
        Stmt::Expr(expr) | Stmt::Throw(expr) => expr_uses_java_platform_facades(expr),
        Stmt::Assign { target, value } => {
            expr_uses_java_platform_facades(target) || expr_uses_java_platform_facades(value)
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_uses_java_platform_facades(condition)
                || block_uses_java_platform_facades(then_block)
                || else_block
                    .as_ref()
                    .is_some_and(block_uses_java_platform_facades)
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => expr_uses_java_platform_facades(condition) || block_uses_java_platform_facades(body),
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter().any(stmt_uses_java_platform_facades)
                || condition
                    .as_ref()
                    .is_some_and(expr_uses_java_platform_facades)
                || update.iter().any(expr_uses_java_platform_facades)
                || block_uses_java_platform_facades(body)
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            expr_uses_java_platform_facades(iterable) || block_uses_java_platform_facades(body)
        }
        Stmt::Switch { selector, cases } => {
            expr_uses_java_platform_facades(selector)
                || cases
                    .iter()
                    .any(|case| case.statements.iter().any(stmt_uses_java_platform_facades))
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources.iter().any(stmt_uses_java_platform_facades)
                || block_uses_java_platform_facades(body)
                || catches
                    .iter()
                    .any(|catch| block_uses_java_platform_facades(&catch.body))
                || finally_block
                    .as_ref()
                    .is_some_and(block_uses_java_platform_facades)
        }
        Stmt::ConstructorInvocation { args, .. } => {
            args.iter().any(expr_uses_java_platform_facades)
        }
        Stmt::LocalType(ty) => type_uses_java_platform_facades(ty),
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_uses_java_platform_facades(block)
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => false,
    }
}

fn expr_uses_java_platform_facades(expr: &Expr) -> bool {
    match expr {
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } => {
            receiver
                .as_deref()
                .is_some_and(|receiver| expr_is_java_logging_receiver_call(receiver, name, args))
                || receiver
                    .as_deref()
                    .is_some_and(|receiver| expr_is_java_platform_static_call(receiver, name, args))
                || receiver
                    .as_deref()
                    .is_some_and(expr_uses_java_platform_facades)
                || args.iter().any(expr_uses_java_platform_facades)
        }
        Expr::FieldAccess(receiver, _) => expr_uses_java_platform_facades(receiver),
        Expr::ArrayAccess(array, index) => {
            expr_uses_java_platform_facades(array) || expr_uses_java_platform_facades(index)
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions.iter().any(expr_uses_java_platform_facades)
                || initializer
                    .iter()
                    .flatten()
                    .any(expr_uses_java_platform_facades)
        }
        Expr::ArrayInitializer(values) => values.iter().any(expr_uses_java_platform_facades),
        Expr::Cast { expr, .. } | Expr::Unary { expr, .. } => expr_uses_java_platform_facades(expr),
        Expr::Binary { left, right, .. } => {
            expr_uses_java_platform_facades(left) || expr_uses_java_platform_facades(right)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_uses_java_platform_facades(condition)
                || expr_uses_java_platform_facades(then_expr)
                || expr_uses_java_platform_facades(else_expr)
        }
        Expr::InstanceOf { expr, ty } => {
            expr_uses_java_platform_facades(expr) || java_type_uses_java_platform_facade(ty)
        }
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => expr_uses_java_platform_facades(expr),
            crate::java_ir::LambdaBody::Block(block) => block_uses_java_platform_facades(block),
        },
        Expr::New { ty, args, body, .. } => {
            java_type_uses_java_platform_facade(ty)
                || args.iter().any(expr_uses_java_platform_facades)
                || body.iter().flatten().any(member_uses_java_platform_facades)
        }
        Expr::ClassLiteral(_)
        | Expr::MethodReference { .. }
        | Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => false,
    }
}

fn expr_is_java_platform_static_call(receiver: &Expr, name: &str, args: &[Expr]) -> bool {
    let Expr::Var(receiver_name) = receiver else {
        return false;
    };
    matches!(
        (
            crate::stdlib::canonical_external_receiver_type_key(receiver_name).as_str(),
            name,
            args.len()
        ),
        (
            "ClassLoader",
            "getSystemResource" | "get_system_resource",
            1
        )
    )
}

fn expr_is_java_logging_receiver_call(receiver: &Expr, name: &str, args: &[Expr]) -> bool {
    let Expr::Var(receiver_name) = receiver else {
        return false;
    };
    is_java_logging_receiver_name(receiver_name) && is_java_logging_method(name, args.len())
}

fn is_java_logging_receiver_name(name: &str) -> bool {
    matches!(
        crate::stdlib::canonical_external_receiver_type_key(name).as_str(),
        "logger" | "Logger"
    )
}

fn is_java_logging_method(name: &str, arg_count: usize) -> bool {
    arg_count >= 1 && matches!(name, "info" | "warn" | "debug" | "error" | "trace")
}

fn java_type_uses_java_platform_facade(ty: &JavaType) -> bool {
    match ty {
        JavaType::Named { name, args } => {
            is_java_platform_facade_type_name(name)
                || args.iter().any(java_type_uses_java_platform_facade)
        }
        JavaType::Array(inner) => java_type_uses_java_platform_facade(inner),
        JavaType::Wildcard(Some(WildcardBound::Extends(bound)))
        | JavaType::Wildcard(Some(WildcardBound::Super(bound))) => {
            java_type_uses_java_platform_facade(bound)
        }
        JavaType::Unknown(name) => is_java_platform_facade_type_name(name),
        JavaType::Void | JavaType::Primitive(_) | JavaType::String | JavaType::Wildcard(None) => {
            false
        }
    }
}

pub fn is_java_platform_facade_type_name(name: &str) -> bool {
    matches!(
        crate::stdlib::canonical_external_receiver_type_key(name).as_str(),
        "Path"
            | "URI"
            | "File"
            | "URL"
            | "URLConnection"
            | "JarURLConnection"
            | "JarFile"
            | "JarEntry"
            | "Manifest"
            | "Attributes"
            | "AttributesName"
            | "Enumeration"
            | "Iterator"
            | "AtomicInteger"
            | "ClassLoader"
            | "Serializable"
            | "BufferedImage"
            | "Graphics"
            | "Graphics2D"
    )
}

pub fn program_uses_java_class_tokens(program: &Program) -> bool {
    program
        .types
        .iter()
        .any(|ty| ty.members.iter().any(member_uses_java_class_tokens))
}

pub fn program_uses_java_string_split(program: &Program) -> bool {
    program
        .types
        .iter()
        .any(|ty| ty.members.iter().any(member_uses_java_string_split))
}

pub fn program_uses_java_string_runtime_helpers(program: &Program) -> bool {
    program.types.iter().any(|ty| {
        ty.members
            .iter()
            .any(member_uses_java_string_runtime_helpers)
    })
}

pub fn program_uses_java_erased_object_runtime_support(program: &Program) -> bool {
    program.imports.iter().any(|import| {
        matches!(
            import.path.as_str(),
            "java.io.ObjectOutputStream" | "java.io.Serializable" | "java.text.MessageFormat"
        )
    }) || program
        .types
        .iter()
        .any(type_uses_java_erased_object_runtime_support)
}

pub fn program_uses_java_string_format(program: &Program) -> bool {
    program
        .types
        .iter()
        .any(|ty| ty.members.iter().any(member_uses_java_string_format))
}

fn type_uses_java_erased_object_runtime_support(ty: &crate::java_ir::TypeDecl) -> bool {
    ty.implements.iter().any(|name| {
        crate::stdlib::canonical_external_receiver_type_key(name)
            .rsplit('.')
            .next()
            .is_some_and(|simple| simple == "Serializable")
    }) || ty
        .members
        .iter()
        .any(member_uses_java_erased_object_runtime_support)
}

fn member_uses_java_erased_object_runtime_support(member: &crate::java_ir::Member) -> bool {
    match member {
        crate::java_ir::Member::Field(field) => field
            .initializer
            .as_ref()
            .is_some_and(expr_uses_java_erased_object_runtime_support),
        crate::java_ir::Member::Method(method) => {
            java_type_uses_java_erased_object_runtime_support(&method.return_type)
                || method
                    .params
                    .iter()
                    .any(|param| java_type_uses_java_erased_object_runtime_support(&param.ty))
                || method
                    .body
                    .as_ref()
                    .is_some_and(block_uses_java_erased_object_runtime_support)
        }
        crate::java_ir::Member::Constructor(constructor) => {
            constructor
                .params
                .iter()
                .any(|param| java_type_uses_java_erased_object_runtime_support(&param.ty))
                || constructor
                    .body
                    .as_ref()
                    .is_some_and(block_uses_java_erased_object_runtime_support)
        }
        crate::java_ir::Member::Type(nested) => {
            type_uses_java_erased_object_runtime_support(nested)
        }
        crate::java_ir::Member::Unsupported { .. } => false,
    }
}

fn block_uses_java_erased_object_runtime_support(block: &crate::java_ir::Block) -> bool {
    block
        .statements
        .iter()
        .any(stmt_uses_java_erased_object_runtime_support)
}

fn stmt_uses_java_erased_object_runtime_support(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Expr(expr)
        | Stmt::Return(Some(expr))
        | Stmt::Throw(expr)
        | Stmt::VarDecl {
            initializer: Some(expr),
            ..
        } => expr_uses_java_erased_object_runtime_support(expr),
        Stmt::Assign { target, value } => {
            expr_uses_java_erased_object_runtime_support(target)
                || expr_uses_java_erased_object_runtime_support(value)
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_uses_java_erased_object_runtime_support(condition)
                || block_uses_java_erased_object_runtime_support(then_block)
                || else_block
                    .as_ref()
                    .is_some_and(block_uses_java_erased_object_runtime_support)
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            expr_uses_java_erased_object_runtime_support(condition)
                || block_uses_java_erased_object_runtime_support(body)
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter()
                .any(stmt_uses_java_erased_object_runtime_support)
                || condition
                    .as_ref()
                    .is_some_and(expr_uses_java_erased_object_runtime_support)
                || update
                    .iter()
                    .any(expr_uses_java_erased_object_runtime_support)
                || block_uses_java_erased_object_runtime_support(body)
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            expr_uses_java_erased_object_runtime_support(iterable)
                || block_uses_java_erased_object_runtime_support(body)
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources
                .iter()
                .any(stmt_uses_java_erased_object_runtime_support)
                || block_uses_java_erased_object_runtime_support(body)
                || catches
                    .iter()
                    .any(|catch| block_uses_java_erased_object_runtime_support(&catch.body))
                || finally_block
                    .as_ref()
                    .is_some_and(block_uses_java_erased_object_runtime_support)
        }
        Stmt::Switch { selector, cases } => {
            expr_uses_java_erased_object_runtime_support(selector)
                || cases.iter().any(|case| {
                    case.statements
                        .iter()
                        .any(stmt_uses_java_erased_object_runtime_support)
                })
        }
        Stmt::ConstructorInvocation { args, .. } => args
            .iter()
            .any(expr_uses_java_erased_object_runtime_support),
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_uses_java_erased_object_runtime_support(block)
        }
        Stmt::LocalType(ty) => type_uses_java_erased_object_runtime_support(ty),
        Stmt::VarDecl {
            initializer: None, ..
        } => false,
        Stmt::Return(None) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => false,
    }
}

fn expr_uses_java_erased_object_runtime_support(expr: &Expr) -> bool {
    match expr {
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } => {
            let string_value_of = matches!(name.as_str(), "valueOf" | "value_of")
                && args.len() == 1
                && receiver
                    .as_deref()
                    .and_then(|receiver| match receiver {
                        Expr::Var(name) => Some(canonical_external_receiver_type_key(name)),
                        _ => None,
                    })
                    .is_some_and(|receiver| receiver == "String");
            let receiver_to_string =
                matches!(name.as_str(), "toString" | "to_string") && args.is_empty();
            string_value_of
                || receiver_to_string
                || receiver
                    .as_deref()
                    .is_some_and(expr_uses_java_erased_object_runtime_support)
                || args
                    .iter()
                    .any(expr_uses_java_erased_object_runtime_support)
        }
        Expr::FieldAccess(receiver, _) => expr_uses_java_erased_object_runtime_support(receiver),
        Expr::ArrayAccess(array, index) => {
            expr_uses_java_erased_object_runtime_support(array)
                || expr_uses_java_erased_object_runtime_support(index)
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions
                .iter()
                .any(expr_uses_java_erased_object_runtime_support)
                || initializer.as_ref().is_some_and(|values| {
                    values
                        .iter()
                        .any(expr_uses_java_erased_object_runtime_support)
                })
        }
        Expr::ArrayInitializer(values) => values
            .iter()
            .any(expr_uses_java_erased_object_runtime_support),
        Expr::Cast { expr, .. } | Expr::Unary { expr, .. } => {
            expr_uses_java_erased_object_runtime_support(expr)
        }
        Expr::Binary { op, left, right } => {
            (matches!(op, BinaryOp::Add)
                && (expr_is_string_literal(left) || expr_is_string_literal(right)))
                || expr_uses_java_erased_object_runtime_support(left)
                || expr_uses_java_erased_object_runtime_support(right)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_uses_java_erased_object_runtime_support(condition)
                || expr_uses_java_erased_object_runtime_support(then_expr)
                || expr_uses_java_erased_object_runtime_support(else_expr)
        }
        Expr::InstanceOf { expr, ty } => {
            expr_uses_java_erased_object_runtime_support(expr)
                || java_type_uses_java_erased_object_runtime_support(ty)
        }
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => {
                expr_uses_java_erased_object_runtime_support(expr)
            }
            crate::java_ir::LambdaBody::Block(block) => {
                block_uses_java_erased_object_runtime_support(block)
            }
        },
        Expr::New { args, body, .. } => {
            args.iter()
                .any(expr_uses_java_erased_object_runtime_support)
                || body
                    .iter()
                    .flatten()
                    .any(member_uses_java_erased_object_runtime_support)
        }
        Expr::ClassLiteral(_)
        | Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::Raw(_)
        | Expr::MethodReference { .. }
        | Expr::Unsupported { .. } => false,
    }
}

fn expr_is_string_literal(expr: &Expr) -> bool {
    matches!(expr, Expr::Literal(Literal::String(_)))
}

fn java_type_uses_java_erased_object_runtime_support(ty: &JavaType) -> bool {
    match ty {
        JavaType::Named { name, args } => {
            let canonical = canonical_external_receiver_type_key(name);
            let simple = canonical.rsplit('.').next().unwrap_or(&canonical);
            matches!(simple, "Object" | "Serializable")
                || args
                    .iter()
                    .any(java_type_uses_java_erased_object_runtime_support)
        }
        JavaType::Array(inner) => java_type_uses_java_erased_object_runtime_support(inner),
        JavaType::Wildcard(Some(WildcardBound::Extends(bound)))
        | JavaType::Wildcard(Some(WildcardBound::Super(bound))) => {
            java_type_uses_java_erased_object_runtime_support(bound)
        }
        JavaType::Void | JavaType::Primitive(_) | JavaType::String | JavaType::Wildcard(None) => {
            false
        }
        JavaType::Unknown(name) => {
            let simple = canonical_external_receiver_type_key(name);
            let simple = simple.rsplit('.').next().unwrap_or(&simple);
            matches!(simple, "Object" | "Serializable")
        }
    }
}

fn member_uses_java_string_format(member: &crate::java_ir::Member) -> bool {
    match member {
        crate::java_ir::Member::Field(field) => field
            .initializer
            .as_ref()
            .is_some_and(expr_uses_java_string_format),
        crate::java_ir::Member::Method(method) => method
            .body
            .as_ref()
            .is_some_and(block_uses_java_string_format),
        crate::java_ir::Member::Constructor(constructor) => constructor
            .body
            .as_ref()
            .is_some_and(block_uses_java_string_format),
        crate::java_ir::Member::Type(ty) => ty.members.iter().any(member_uses_java_string_format),
        crate::java_ir::Member::Unsupported { .. } => false,
    }
}

fn block_uses_java_string_format(block: &crate::java_ir::Block) -> bool {
    block.statements.iter().any(stmt_uses_java_string_format)
}

fn stmt_uses_java_string_format(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Expr(expr)
        | Stmt::Return(Some(expr))
        | Stmt::Throw(expr)
        | Stmt::VarDecl {
            initializer: Some(expr),
            ..
        } => expr_uses_java_string_format(expr),
        Stmt::Assign { target, value } => {
            expr_uses_java_string_format(target) || expr_uses_java_string_format(value)
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_uses_java_string_format(condition)
                || block_uses_java_string_format(then_block)
                || else_block
                    .as_ref()
                    .is_some_and(block_uses_java_string_format)
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => expr_uses_java_string_format(condition) || block_uses_java_string_format(body),
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter().any(stmt_uses_java_string_format)
                || condition.as_ref().is_some_and(expr_uses_java_string_format)
                || update.iter().any(expr_uses_java_string_format)
                || block_uses_java_string_format(body)
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            expr_uses_java_string_format(iterable) || block_uses_java_string_format(body)
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources.iter().any(stmt_uses_java_string_format)
                || block_uses_java_string_format(body)
                || catches
                    .iter()
                    .any(|catch| block_uses_java_string_format(&catch.body))
                || finally_block
                    .as_ref()
                    .is_some_and(block_uses_java_string_format)
        }
        Stmt::Switch { selector, cases } => {
            expr_uses_java_string_format(selector)
                || cases
                    .iter()
                    .any(|case| case.statements.iter().any(stmt_uses_java_string_format))
        }
        Stmt::ConstructorInvocation { args, .. } => args.iter().any(expr_uses_java_string_format),
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_uses_java_string_format(block)
        }
        Stmt::LocalType(ty) => ty.members.iter().any(member_uses_java_string_format),
        Stmt::VarDecl {
            initializer: None, ..
        } => false,
        Stmt::Return(None) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => false,
    }
}

fn expr_uses_java_string_format(expr: &Expr) -> bool {
    match expr {
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } => {
            receiver
                .as_deref()
                .and_then(|receiver| match receiver {
                    Expr::Var(name) => Some(canonical_external_receiver_type_key(name)),
                    _ => None,
                })
                .is_some_and(|receiver| receiver == "String" && name == "format")
                || receiver
                    .as_deref()
                    .is_some_and(expr_uses_java_string_format)
                || args.iter().any(expr_uses_java_string_format)
        }
        Expr::FieldAccess(receiver, _) => expr_uses_java_string_format(receiver),
        Expr::ArrayAccess(array, index) => {
            expr_uses_java_string_format(array) || expr_uses_java_string_format(index)
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions.iter().any(expr_uses_java_string_format)
                || initializer
                    .as_ref()
                    .is_some_and(|values| values.iter().any(expr_uses_java_string_format))
        }
        Expr::ArrayInitializer(values) => values.iter().any(expr_uses_java_string_format),
        Expr::Cast { expr, .. } | Expr::Unary { expr, .. } => expr_uses_java_string_format(expr),
        Expr::Binary { left, right, .. } => {
            expr_uses_java_string_format(left) || expr_uses_java_string_format(right)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_uses_java_string_format(condition)
                || expr_uses_java_string_format(then_expr)
                || expr_uses_java_string_format(else_expr)
        }
        Expr::InstanceOf { expr, .. } => expr_uses_java_string_format(expr),
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => expr_uses_java_string_format(expr),
            crate::java_ir::LambdaBody::Block(block) => block_uses_java_string_format(block),
        },
        Expr::New { args, body, .. } => {
            args.iter().any(expr_uses_java_string_format)
                || body.iter().flatten().any(member_uses_java_string_format)
        }
        Expr::ClassLiteral(_)
        | Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::Raw(_)
        | Expr::MethodReference { .. }
        | Expr::Unsupported { .. } => false,
    }
}

fn member_uses_java_string_split(member: &crate::java_ir::Member) -> bool {
    match member {
        crate::java_ir::Member::Field(field) => field
            .initializer
            .as_ref()
            .is_some_and(expr_uses_java_string_split),
        crate::java_ir::Member::Method(method) => method
            .body
            .as_ref()
            .is_some_and(block_uses_java_string_split),
        crate::java_ir::Member::Constructor(constructor) => constructor
            .body
            .as_ref()
            .is_some_and(block_uses_java_string_split),
        crate::java_ir::Member::Type(ty) => ty.members.iter().any(member_uses_java_string_split),
        crate::java_ir::Member::Unsupported { .. } => false,
    }
}

fn member_uses_java_string_runtime_helpers(member: &crate::java_ir::Member) -> bool {
    member_uses_java_string_split(member)
        || match member {
            crate::java_ir::Member::Field(field) => field
                .initializer
                .as_ref()
                .is_some_and(expr_uses_java_string_runtime_helpers),
            crate::java_ir::Member::Method(method) => method
                .body
                .as_ref()
                .is_some_and(block_uses_java_string_runtime_helpers),
            crate::java_ir::Member::Constructor(constructor) => constructor
                .body
                .as_ref()
                .is_some_and(block_uses_java_string_runtime_helpers),
            crate::java_ir::Member::Type(ty) => ty
                .members
                .iter()
                .any(member_uses_java_string_runtime_helpers),
            crate::java_ir::Member::Unsupported { .. } => false,
        }
}

fn block_uses_java_string_runtime_helpers(block: &crate::java_ir::Block) -> bool {
    block
        .statements
        .iter()
        .any(stmt_uses_java_string_runtime_helpers)
}

fn stmt_uses_java_string_runtime_helpers(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Expr(expr)
        | Stmt::Return(Some(expr))
        | Stmt::Throw(expr)
        | Stmt::VarDecl {
            initializer: Some(expr),
            ..
        } => expr_uses_java_string_runtime_helpers(expr),
        Stmt::Assign { target, value } => {
            expr_uses_java_string_runtime_helpers(target)
                || expr_uses_java_string_runtime_helpers(value)
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_uses_java_string_runtime_helpers(condition)
                || block_uses_java_string_runtime_helpers(then_block)
                || else_block
                    .as_ref()
                    .is_some_and(block_uses_java_string_runtime_helpers)
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            expr_uses_java_string_runtime_helpers(condition)
                || block_uses_java_string_runtime_helpers(body)
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter().any(stmt_uses_java_string_runtime_helpers)
                || condition
                    .as_ref()
                    .is_some_and(expr_uses_java_string_runtime_helpers)
                || update.iter().any(expr_uses_java_string_runtime_helpers)
                || block_uses_java_string_runtime_helpers(body)
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            expr_uses_java_string_runtime_helpers(iterable)
                || block_uses_java_string_runtime_helpers(body)
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources.iter().any(stmt_uses_java_string_runtime_helpers)
                || block_uses_java_string_runtime_helpers(body)
                || catches
                    .iter()
                    .any(|catch| block_uses_java_string_runtime_helpers(&catch.body))
                || finally_block
                    .as_ref()
                    .is_some_and(block_uses_java_string_runtime_helpers)
        }
        Stmt::Switch { selector, cases } => {
            expr_uses_java_string_runtime_helpers(selector)
                || cases.iter().any(|case| {
                    case.statements
                        .iter()
                        .any(stmt_uses_java_string_runtime_helpers)
                })
        }
        Stmt::ConstructorInvocation { args, .. } => {
            args.iter().any(expr_uses_java_string_runtime_helpers)
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_uses_java_string_runtime_helpers(block)
        }
        Stmt::LocalType(ty) => ty
            .members
            .iter()
            .any(member_uses_java_string_runtime_helpers),
        Stmt::VarDecl {
            initializer: None, ..
        } => false,
        Stmt::Return(None) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => false,
    }
}

fn expr_uses_java_string_runtime_helpers(expr: &Expr) -> bool {
    match expr {
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } => {
            let uses_runtime_helper = matches!(
                (name.as_str(), args.len()),
                ("split", 1 | 2)
                    | ("substring", 1 | 2)
                    | ("hashCode", 0)
                    | ("indexOf", 1 | 2)
                    | ("lastIndexOf", 1 | 2)
                    | ("last_index_of", 1 | 2)
                    | ("replaceFirst", 2)
                    | ("replaceAll", 2)
                    | ("compareTo", 1)
                    | ("compare_to", 1)
                    | ("compareToIgnoreCase", 1)
                    | ("compare_to_ignore_case", 1)
                    | ("equalsIgnoreCase", 1)
                    | ("equals_ignore_case", 1)
                    | ("matches", 1)
            ) || matches!(
                (receiver.as_deref(), name.as_str(), args.len()),
                (Some(Expr::Var(owner)), "floorDiv" | "floor_div" | "floorMod" | "floor_mod", 2)
                    if canonical_external_receiver_type_key(owner) == "Math"
            ) || matches!(
                (receiver.as_deref(), name.as_str(), args.len()),
                (Some(Expr::Var(owner)), "round" | "abs", 1)
                    if canonical_external_receiver_type_key(owner) == "Math"
            ) || matches!(
                (receiver.as_deref(), name.as_str(), args.len()),
                (Some(Expr::Var(owner)), "max" | "min", 2)
                    if canonical_external_receiver_type_key(owner) == "Math"
            );
            uses_runtime_helper
                || receiver
                    .as_deref()
                    .is_some_and(expr_uses_java_string_runtime_helpers)
                || args.iter().any(expr_uses_java_string_runtime_helpers)
        }
        Expr::FieldAccess(receiver, _) => expr_uses_java_string_runtime_helpers(receiver),
        Expr::ArrayAccess(array, index) => {
            expr_uses_java_string_runtime_helpers(array)
                || expr_uses_java_string_runtime_helpers(index)
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions.iter().any(expr_uses_java_string_runtime_helpers)
                || initializer
                    .as_ref()
                    .is_some_and(|values| values.iter().any(expr_uses_java_string_runtime_helpers))
        }
        Expr::ArrayInitializer(values) => values.iter().any(expr_uses_java_string_runtime_helpers),
        Expr::Cast { expr, .. } | Expr::Unary { expr, .. } => {
            expr_uses_java_string_runtime_helpers(expr)
        }
        Expr::Binary { left, right, .. } => {
            expr_uses_java_string_runtime_helpers(left)
                || expr_uses_java_string_runtime_helpers(right)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_uses_java_string_runtime_helpers(condition)
                || expr_uses_java_string_runtime_helpers(then_expr)
                || expr_uses_java_string_runtime_helpers(else_expr)
        }
        Expr::InstanceOf { expr, .. } => expr_uses_java_string_runtime_helpers(expr),
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => expr_uses_java_string_runtime_helpers(expr),
            crate::java_ir::LambdaBody::Block(block) => {
                block_uses_java_string_runtime_helpers(block)
            }
        },
        Expr::New { ty, args, body, .. } => {
            (java_type_is_string(ty) && matches!(args.len(), 1 | 3))
                || (args.len() == 1 && java_type_is_boxed_primitive_name(ty))
                || args.iter().any(expr_uses_java_string_runtime_helpers)
                || body
                    .iter()
                    .flatten()
                    .any(member_uses_java_string_runtime_helpers)
        }
        Expr::ClassLiteral(_)
        | Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::Raw(_)
        | Expr::MethodReference { .. }
        | Expr::Unsupported { .. } => false,
    }
}

fn java_type_is_string(ty: &JavaType) -> bool {
    matches!(ty, JavaType::String)
        || matches!(ty, JavaType::Named { name, .. } | JavaType::Unknown(name) if name == "String" || name == "java.lang.String")
}

fn java_type_is_boxed_primitive_name(ty: &JavaType) -> bool {
    match ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => boxed_primitive_type(
            canonical_external_receiver_type_key(name)
                .rsplit('.')
                .next()
                .unwrap_or(name),
        )
        .is_some(),
        JavaType::String
        | JavaType::Primitive(_)
        | JavaType::Void
        | JavaType::Array(_)
        | JavaType::Wildcard(_) => false,
    }
}

fn block_uses_java_string_split(block: &crate::java_ir::Block) -> bool {
    block.statements.iter().any(stmt_uses_java_string_split)
}

fn stmt_uses_java_string_split(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Expr(expr)
        | Stmt::Return(Some(expr))
        | Stmt::Throw(expr)
        | Stmt::VarDecl {
            initializer: Some(expr),
            ..
        } => expr_uses_java_string_split(expr),
        Stmt::Assign { target, value } => {
            expr_uses_java_string_split(target) || expr_uses_java_string_split(value)
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_uses_java_string_split(condition)
                || block_uses_java_string_split(then_block)
                || else_block
                    .as_ref()
                    .is_some_and(block_uses_java_string_split)
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => expr_uses_java_string_split(condition) || block_uses_java_string_split(body),
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter().any(stmt_uses_java_string_split)
                || condition.as_ref().is_some_and(expr_uses_java_string_split)
                || update.iter().any(expr_uses_java_string_split)
                || block_uses_java_string_split(body)
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            expr_uses_java_string_split(iterable) || block_uses_java_string_split(body)
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources.iter().any(stmt_uses_java_string_split)
                || block_uses_java_string_split(body)
                || catches
                    .iter()
                    .any(|catch| block_uses_java_string_split(&catch.body))
                || finally_block
                    .as_ref()
                    .is_some_and(block_uses_java_string_split)
        }
        Stmt::Switch { selector, cases } => {
            expr_uses_java_string_split(selector)
                || cases
                    .iter()
                    .any(|case| case.statements.iter().any(stmt_uses_java_string_split))
        }
        Stmt::ConstructorInvocation { args, .. } => args.iter().any(expr_uses_java_string_split),
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_uses_java_string_split(block)
        }
        Stmt::LocalType(ty) => ty.members.iter().any(member_uses_java_string_split),
        Stmt::VarDecl {
            initializer: None, ..
        } => false,
        Stmt::Return(None) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => false,
    }
}

fn expr_uses_java_string_split(expr: &Expr) -> bool {
    match expr {
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } => {
            (receiver.is_some() && name == "split" && matches!(args.len(), 1 | 2))
                || receiver.as_deref().is_some_and(expr_uses_java_string_split)
                || args.iter().any(expr_uses_java_string_split)
        }
        Expr::FieldAccess(receiver, _) => expr_uses_java_string_split(receiver),
        Expr::ArrayAccess(array, index) => {
            expr_uses_java_string_split(array) || expr_uses_java_string_split(index)
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions.iter().any(expr_uses_java_string_split)
                || initializer
                    .as_ref()
                    .is_some_and(|values| values.iter().any(expr_uses_java_string_split))
        }
        Expr::ArrayInitializer(values) => values.iter().any(expr_uses_java_string_split),
        Expr::Cast { expr, .. } | Expr::Unary { expr, .. } => expr_uses_java_string_split(expr),
        Expr::Binary { left, right, .. } => {
            expr_uses_java_string_split(left) || expr_uses_java_string_split(right)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_uses_java_string_split(condition)
                || expr_uses_java_string_split(then_expr)
                || expr_uses_java_string_split(else_expr)
        }
        Expr::InstanceOf { expr, .. } => expr_uses_java_string_split(expr),
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => expr_uses_java_string_split(expr),
            crate::java_ir::LambdaBody::Block(block) => block_uses_java_string_split(block),
        },
        Expr::New { args, body, .. } => {
            args.iter().any(expr_uses_java_string_split)
                || body.iter().flatten().any(member_uses_java_string_split)
        }
        Expr::ClassLiteral(_)
        | Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::Raw(_)
        | Expr::MethodReference { .. }
        | Expr::Unsupported { .. } => false,
    }
}

fn member_uses_java_class_tokens(member: &crate::java_ir::Member) -> bool {
    match member {
        crate::java_ir::Member::Field(field) => field
            .initializer
            .as_ref()
            .is_some_and(expr_uses_java_class_tokens),
        crate::java_ir::Member::Method(method) => method
            .body
            .as_ref()
            .is_some_and(block_uses_java_class_tokens),
        crate::java_ir::Member::Constructor(constructor) => constructor
            .body
            .as_ref()
            .is_some_and(block_uses_java_class_tokens),
        crate::java_ir::Member::Type(ty) => ty.members.iter().any(member_uses_java_class_tokens),
        crate::java_ir::Member::Unsupported { .. } => false,
    }
}

fn block_uses_java_class_tokens(block: &crate::java_ir::Block) -> bool {
    block.statements.iter().any(stmt_uses_java_class_tokens)
}

fn stmt_uses_java_class_tokens(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Expr(expr)
        | Stmt::Return(Some(expr))
        | Stmt::Throw(expr)
        | Stmt::VarDecl {
            initializer: Some(expr),
            ..
        } => expr_uses_java_class_tokens(expr),
        Stmt::Assign { target, value } => {
            expr_uses_java_class_tokens(target) || expr_uses_java_class_tokens(value)
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_uses_java_class_tokens(condition)
                || block_uses_java_class_tokens(then_block)
                || else_block
                    .as_ref()
                    .is_some_and(block_uses_java_class_tokens)
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => expr_uses_java_class_tokens(condition) || block_uses_java_class_tokens(body),
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter().any(stmt_uses_java_class_tokens)
                || condition.as_ref().is_some_and(expr_uses_java_class_tokens)
                || update.iter().any(expr_uses_java_class_tokens)
                || block_uses_java_class_tokens(body)
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            expr_uses_java_class_tokens(iterable) || block_uses_java_class_tokens(body)
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources.iter().any(stmt_uses_java_class_tokens)
                || block_uses_java_class_tokens(body)
                || catches
                    .iter()
                    .any(|catch| block_uses_java_class_tokens(&catch.body))
                || finally_block
                    .as_ref()
                    .is_some_and(block_uses_java_class_tokens)
        }
        Stmt::Switch { selector, cases } => {
            expr_uses_java_class_tokens(selector)
                || cases
                    .iter()
                    .any(|case| case.statements.iter().any(stmt_uses_java_class_tokens))
        }
        Stmt::ConstructorInvocation { args, .. } => args.iter().any(expr_uses_java_class_tokens),
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_uses_java_class_tokens(block)
        }
        Stmt::LocalType(ty) => ty.members.iter().any(member_uses_java_class_tokens),
        Stmt::VarDecl {
            initializer: None, ..
        } => false,
        Stmt::Return(None) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => false,
    }
}

fn expr_uses_java_class_tokens(expr: &Expr) -> bool {
    match expr {
        Expr::ClassLiteral(_) => true,
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } => {
            let calls_class_for_name = receiver.as_deref().is_some_and(|receiver| {
                matches!(receiver, Expr::Var(receiver_name) if canonical_external_receiver_type_key(receiver_name) == "Class")
            }) && name == "forName";
            calls_class_for_name
                || matches!(name.as_str(), "getClass" | "get_class")
                || receiver.as_deref().is_some_and(expr_uses_java_class_tokens)
                || args.iter().any(expr_uses_java_class_tokens)
        }
        Expr::FieldAccess(receiver, _)
        | Expr::ArrayAccess(receiver, _)
        | Expr::Unary { expr: receiver, .. }
        | Expr::Cast { expr: receiver, .. }
        | Expr::InstanceOf { expr: receiver, .. } => expr_uses_java_class_tokens(receiver),
        Expr::Binary { left, right, .. } => {
            expr_uses_java_class_tokens(left) || expr_uses_java_class_tokens(right)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_uses_java_class_tokens(condition)
                || expr_uses_java_class_tokens(then_expr)
                || expr_uses_java_class_tokens(else_expr)
        }
        Expr::New { args, body, .. } => {
            args.iter().any(expr_uses_java_class_tokens)
                || body
                    .as_ref()
                    .is_some_and(|members| members.iter().any(member_uses_java_class_tokens))
        }
        Expr::ArrayInitializer(args) => args.iter().any(expr_uses_java_class_tokens),
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions.iter().any(expr_uses_java_class_tokens)
                || initializer
                    .as_ref()
                    .is_some_and(|elements| elements.iter().any(expr_uses_java_class_tokens))
        }
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => expr_uses_java_class_tokens(expr),
            crate::java_ir::LambdaBody::Block(block) => block_uses_java_class_tokens(block),
        },
        Expr::MethodReference { .. }
        | Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => false,
    }
}

fn strip_java_type_modifiers(mut name: &str) -> &str {
    loop {
        let trimmed = name.trim_start();
        if let Some(rest) = trimmed.strip_prefix("final ") {
            name = rest;
        } else if let Some(rest) = trimmed.strip_prefix("volatile ") {
            name = rest;
        } else if let Some(rest) = trimmed.strip_prefix("transient ") {
            name = rest;
        } else {
            return trimmed;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalApiMethodPolicy {
    pub role: ExternalApiRole,
    pub receiver_mutability: ReceiverMutabilityFact,
    pub return_nullability: NullabilityState,
    pub provenance: FactProvenance,
}

pub fn external_api_method_policy(
    receiver_type: Option<&str>,
    name: &str,
    arg_count: usize,
) -> ExternalApiMethodPolicy {
    external_api_method_policy_with_builtin_packs(receiver_type, name, arg_count, true)
}

pub fn external_api_method_policy_with_builtin_packs(
    receiver_type: Option<&str>,
    name: &str,
    arg_count: usize,
    enable_builtin_external_packs: bool,
) -> ExternalApiMethodPolicy {
    if let Some(receiver_type) = receiver_type
        && enable_builtin_external_packs
        && let Some(effect) = crate::external_packs::external_api_effect_for_receiver_with_arity(
            Some(receiver_type),
            name,
            Some(arg_count),
        )
    {
        return ExternalApiMethodPolicy {
            role: effect.role,
            receiver_mutability: effect.receiver_mutability,
            return_nullability: effect.return_nullability,
            provenance: effect.provenance,
        };
    }
    let role = external_api_method_role(receiver_type, name, arg_count);
    ExternalApiMethodPolicy {
        role,
        receiver_mutability: external_api_receiver_mutability(role),
        return_nullability: external_api_return_nullability(receiver_type, name, arg_count, role),
        provenance: external_api_method_policy_provenance_with_builtin_packs(
            receiver_type,
            name,
            arg_count,
            enable_builtin_external_packs,
        ),
    }
}

pub fn external_constructor_provenance(type_name: &str, arg_types: &[JavaType]) -> FactProvenance {
    external_constructor_provenance_with_builtin_packs(type_name, arg_types, true)
}

pub fn external_constructor_provenance_with_builtin_packs(
    type_name: &str,
    arg_types: &[JavaType],
    enable_builtin_external_packs: bool,
) -> FactProvenance {
    if enable_builtin_external_packs
        && let Some(constructor) =
            crate::external_packs::external_constructor_for(type_name, arg_types)
    {
        return constructor.provenance;
    }
    if is_known_collection_constructor(type_name, arg_types)
        || is_known_map_constructor(type_name, arg_types)
        || is_known_throwable_constructor(type_name, arg_types)
        || is_known_io_wrapper_constructor(type_name, arg_types)
        || is_known_jdk_utility_constructor(type_name, arg_types)
        || is_known_file_constructor(type_name, arg_types)
        || is_known_format_constructor(type_name, arg_types)
        || is_known_string_builder_constructor(type_name, arg_types)
        || is_known_tuple_constructor(type_name, arg_types)
    {
        FactProvenance::BuiltinExternalModel
    } else {
        FactProvenance::ConservativeFallback
    }
}

pub fn external_field_provenance(type_name: &str, field_name: &str) -> FactProvenance {
    external_field_provenance_with_builtin_packs(type_name, field_name, true)
}

pub fn external_field_provenance_with_builtin_packs(
    type_name: &str,
    field_name: &str,
    enable_builtin_external_packs: bool,
) -> FactProvenance {
    if enable_builtin_external_packs
        && let Some(field) = crate::external_packs::external_field_for(type_name, field_name)
    {
        return field.provenance;
    }
    let receiver_type = JavaType::Named {
        name: type_name.to_string(),
        args: Vec::new(),
    };
    if known_external_field_type_for_receiver(&receiver_type, field_name).is_some() {
        FactProvenance::BuiltinExternalModel
    } else {
        FactProvenance::ConservativeFallback
    }
}

fn is_known_collection_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    is_known_collection_type(type_name) && arg_types.len() <= 3
}

fn is_known_map_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    is_known_map_type(type_name) && arg_types.len() <= 3
}

fn is_known_throwable_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    is_known_throwable_type(type_name) && arg_types.len() <= 2
}

fn is_known_io_wrapper_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    (is_known_input_stream_type(type_name)
        || is_known_output_stream_type(type_name)
        || is_known_reader_type(type_name)
        || is_known_writer_type(type_name))
        && arg_types.len() <= 2
}

fn is_known_jdk_utility_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    match type_name {
        "BigInteger" => matches!(arg_types.len(), 1 | 2),
        "BitSet" => arg_types.len() <= 1,
        "Date" => arg_types.len() <= 1,
        "Random" => arg_types.len() <= 1,
        "ConsoleHandler" => arg_types.is_empty(),
        "ForkJoinPool" => arg_types.len() <= 1,
        "Manifest" => arg_types.len() == 1,
        "Properties" => arg_types.is_empty(),
        "ProcessBuilder" => arg_types.len() == 1,
        "SimpleEntry" => arg_types.len() == 2,
        "StringJoiner" => matches!(arg_types.len(), 1 | 2 | 3),
        "StringTokenizer" => matches!(arg_types.len(), 1 | 2 | 3),
        "URL" => arg_types.len() == 1,
        "ZipEntry" => arg_types.len() == 1,
        "ZipFile" => arg_types.len() == 1,
        "NullPointerException" => arg_types.len() <= 1,
        "AtomicBoolean" => arg_types.len() <= 1,
        "AtomicInteger" => arg_types.len() <= 1,
        "AtomicLong" => arg_types.len() <= 1,
        _ => false,
    }
}

fn is_known_file_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    matches!(type_name, "File" | "Path" | "URI") && matches!(arg_types.len(), 1 | 2)
}

fn is_known_format_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    matches!(
        type_name,
        "DecimalFormat"
            | "NumberFormat"
            | "DateFormat"
            | "SimpleDateFormat"
            | "MessageFormat"
            | "Format"
    ) && arg_types.len() <= 2
}

fn is_known_string_builder_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    matches!(type_name, "StringBuilder" | "StringBuffer") && arg_types.len() <= 1
}

fn is_known_tuple_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    let _ = (type_name, arg_types);
    false
}

fn external_api_method_policy_provenance_with_builtin_packs(
    receiver_type: Option<&str>,
    name: &str,
    arg_count: usize,
    enable_builtin_external_packs: bool,
) -> FactProvenance {
    if receiver_type.is_some() && is_known_object_inherited_method(name, arg_count) {
        return FactProvenance::BuiltinExternalModel;
    }
    if enable_builtin_external_packs
        && receiver_type.is_some_and(|receiver_type| {
            crate::external_packs::external_api_effect_for_receiver_with_arity(
                Some(receiver_type),
                name,
                Some(arg_count),
            )
            .is_some()
        })
    {
        return FactProvenance::ConservativeFallback;
    }
    if receiver_type.is_some() && name == "length" && arg_count == 0 {
        return FactProvenance::BuiltinExternalModel;
    }
    if receiver_type.is_some() && arg_count == 0 && is_known_zero_arg_domain_return_getter(name) {
        return FactProvenance::BuiltinExternalModel;
    }
    if receiver_type.is_some_and(is_known_external_enum_type)
        && matches!((name, arg_count), ("name" | "values", 0) | ("valueOf", 1))
    {
        return FactProvenance::BuiltinExternalModel;
    }
    if receiver_type.is_some_and(is_collection_api_builtin_method)
        && !matches!(
            collection_api_method_role(name, arg_count),
            ExternalApiRole::Unknown
        )
    {
        return FactProvenance::BuiltinExternalModel;
    }
    if receiver_type.is_some_and(is_map_api_builtin_method)
        && !matches!(
            map_api_method_role(name, arg_count),
            ExternalApiRole::Unknown
        )
    {
        return FactProvenance::BuiltinExternalModel;
    }

    match (receiver_type, name, arg_count) {
        (Some("Arrays"), "asList" | "stream", _) => FactProvenance::BuiltinExternalModel,
        (Some("Arrays"), "equals", 2) => FactProvenance::BuiltinExternalModel,
        (Some("Arrays"), "copyOf", 2) => FactProvenance::BuiltinExternalModel,
        (Some("Arrays"), "copyOfRange", 3) => FactProvenance::BuiltinExternalModel,
        (_, "compareTo", 0 | 1) => FactProvenance::BuiltinExternalModel,
        (_, "getCanonicalPath", 0) => FactProvenance::BuiltinExternalModel,
        (_, name, _) if is_known_output_side_effect_method(name) => {
            FactProvenance::BuiltinExternalModel
        }
        (Some(owner), "apply", 1 | 2) if is_known_apply_functional_interface(owner) => {
            FactProvenance::BuiltinExternalModel
        }
        (Some("Stream"), "mapToInt" | "mapToLong" | "mapToDouble", 1) => {
            FactProvenance::BuiltinExternalModel
        }
        (Some("IntStream" | "LongStream" | "DoubleStream"), "sum", 0) => {
            FactProvenance::BuiltinExternalModel
        }
        _ => FactProvenance::ConservativeFallback,
    }
}

fn is_collection_api_builtin_method(receiver_type: &str) -> bool {
    is_collection_like_receiver(Some(receiver_type))
}

fn is_map_api_builtin_method(receiver_type: &str) -> bool {
    is_map_like_receiver(Some(receiver_type))
}

pub fn external_api_method_role(
    receiver_type: Option<&str>,
    name: &str,
    arg_count: usize,
) -> ExternalApiRole {
    if receiver_type.is_none() && is_known_receiverless_static_utility_method(name, arg_count) {
        return ExternalApiRole::BooleanGetter;
    }
    if is_static_utility_owner(receiver_type) {
        return static_utility_method_role(receiver_type, name, arg_count);
    }
    if receiver_type.is_some_and(is_string_formatting_receiver) && name == "format" {
        return ExternalApiRole::Getter;
    }
    if receiver_type == Some("Array") && name == "clone" && arg_count == 0 {
        return ExternalApiRole::Getter;
    }
    if receiver_type == Some("BufferedImage")
        && matches!(
            name,
            "createGraphics" | "create_graphics" | "getGraphics" | "get_graphics"
        )
        && arg_count == 0
    {
        return ExternalApiRole::Getter;
    }
    if receiver_type == Some("Component")
        && matches!(name, "getGraphics" | "get_graphics")
        && arg_count == 0
    {
        return ExternalApiRole::Getter;
    }
    if receiver_type == Some("Component")
        && matches!(name, "getWidth" | "get_width" | "getHeight" | "get_height")
        && arg_count == 0
    {
        return ExternalApiRole::Getter;
    }
    if receiver_type == Some("Color")
        && matches!(
            name,
            "getRed" | "get_red" | "getGreen" | "get_green" | "getBlue" | "get_blue"
        )
        && arg_count == 0
    {
        return ExternalApiRole::Getter;
    }
    if receiver_type == Some("Font")
        && matches!(name, "getSize" | "get_size" | "isBold" | "is_bold")
        && arg_count == 0
    {
        return ExternalApiRole::Getter;
    }
    if receiver_type == Some("Graphics") && name == "dispose" && arg_count == 0 {
        return ExternalApiRole::Mutator;
    }
    if receiver_type == Some("Graphics")
        && matches!(
            name,
            "getClip"
                | "get_clip"
                | "getClipBounds"
                | "get_clip_bounds"
                | "getFontMetrics"
                | "get_font_metrics"
        )
    {
        return ExternalApiRole::Getter;
    }
    if receiver_type == Some("Graphics") && matches!(name, "drawImage" | "draw_image") {
        return ExternalApiRole::Getter;
    }
    if receiver_type == Some("TableModel")
        && matches!(
            name,
            "getRowCount"
                | "get_row_count"
                | "getColumnCount"
                | "get_column_count"
                | "getColumnName"
                | "get_column_name"
                | "getValueAt"
                | "get_value_at"
        )
    {
        return ExternalApiRole::Getter;
    }
    if receiver_type.is_some_and(is_known_external_enum_type) && name == "name" && arg_count == 0 {
        return ExternalApiRole::Getter;
    }
    if receiver_type
        .and_then(|receiver| boxed_number_value_method_return_type(receiver, name, arg_count))
        .is_some()
        || number_value_method_return_type(name, arg_count).is_some()
    {
        return ExternalApiRole::Getter;
    }
    if receiver_type
        .and_then(external_builder_product_type)
        .is_some()
        && name == "make"
        && arg_count == 0
    {
        return ExternalApiRole::Getter;
    }
    if receiver_type
        .is_some_and(|receiver| is_external_builder_fluent_method(receiver, name, arg_count))
    {
        return ExternalApiRole::FluentMutator;
    }
    if receiver_type == Some("Stream")
        && matches!(name, "mapToInt" | "mapToLong" | "mapToDouble")
        && arg_count == 1
    {
        return ExternalApiRole::Getter;
    }
    if matches!(
        receiver_type,
        Some("IntStream" | "LongStream" | "DoubleStream")
    ) && name == "sum"
        && arg_count == 0
    {
        return ExternalApiRole::Getter;
    }
    if receiver_type.is_some() && is_known_object_inherited_method(name, arg_count) {
        return ExternalApiRole::Getter;
    }
    if name == "fastqToPhred" {
        return ExternalApiRole::Getter;
    }
    if name == "getAttributeAsString" && matches!(arg_count, 1 | 2) {
        return ExternalApiRole::Getter;
    }
    if receiver_type.is_some_and(is_known_apply_functional_interface)
        && name == "apply"
        && matches!(arg_count, 1 | 2)
    {
        return ExternalApiRole::Getter;
    }
    if is_collection_like_receiver(receiver_type) {
        return collection_api_method_role(name, arg_count);
    }
    if is_map_like_receiver(receiver_type) {
        return map_api_method_role(name, arg_count);
    }
    if name == "compareTo" && matches!(arg_count, 0 | 1) {
        ExternalApiRole::Getter
    } else if (name == "get" && arg_count > 1) || name.starts_with("get") {
        ExternalApiRole::Getter
    } else if is_java_bean_boolean_getter(name, arg_count) || is_contains_predicate(name, arg_count)
    {
        ExternalApiRole::BooleanGetter
    } else if name == "size"
        || name == "length"
        || name == "equals"
        || name == "getClass"
        || name == "toString"
        || name == "toPath"
        || name == "split"
        || name == "ordinal"
        || name == "logBeta"
    {
        ExternalApiRole::Getter
    } else if is_side_effect_output_method(name) {
        ExternalApiRole::SideEffect
    } else if name == "set" && arg_count > 1 {
        ExternalApiRole::IndexedSetter
    } else if is_java_setter_name(name) {
        ExternalApiRole::Setter
    } else if matches!(
        name,
        "add" | "remove" | "put" | "clear" | "advance" | "read" | "write" | "copy"
    ) {
        ExternalApiRole::Mutator
    } else {
        ExternalApiRole::Unknown
    }
}

fn is_java_setter_name(name: &str) -> bool {
    name.starts_with("set_")
        || name.strip_prefix("set").is_some_and(|suffix| {
            suffix
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_uppercase())
        })
}

fn is_side_effect_output_method(name: &str) -> bool {
    matches!(
        name,
        "info"
            | "warn"
            | "error"
            | "debug"
            | "trace"
            | "append"
            | "print"
            | "printf"
            | "println"
            | "newLine"
            | "flush"
            | "close"
    ) || is_known_output_side_effect_method(name)
}

fn is_known_output_side_effect_method(name: &str) -> bool {
    matches!(
        name,
        "addAlignment" | "addAndFlush" | "addEvent" | "addMetaDataLine"
    )
}

fn is_known_apply_functional_interface(receiver: &str) -> bool {
    matches!(receiver, "Function" | "BiFunction")
}

pub fn external_api_receiver_mutability(role: ExternalApiRole) -> ReceiverMutabilityFact {
    match role {
        ExternalApiRole::Getter
        | ExternalApiRole::BooleanGetter
        | ExternalApiRole::SideEffect
        | ExternalApiRole::IndexedGetter => ReceiverMutabilityFact::ReadOnly,
        ExternalApiRole::Setter
        | ExternalApiRole::Mutator
        | ExternalApiRole::FluentMutator
        | ExternalApiRole::IndexedSetter => ReceiverMutabilityFact::MutatesReceiver,
        ExternalApiRole::Unknown => ReceiverMutabilityFact::Unknown,
    }
}

pub fn external_api_return_nullability(
    receiver_type: Option<&str>,
    name: &str,
    arg_count: usize,
    role: ExternalApiRole,
) -> NullabilityState {
    if matches!(role, ExternalApiRole::BooleanGetter) {
        return NullabilityState::DefinitelyNonNull;
    }
    if is_collection_like_receiver(receiver_type)
        && matches!(
            name,
            "stream" | "filter" | "distinct" | "sorted" | "collect"
        )
    {
        return NullabilityState::DefinitelyNonNull;
    }
    if (receiver_type == Some("Array") && name == "clone" && arg_count == 0)
        || (is_collection_like_receiver(receiver_type) && name == "toArray")
    {
        return NullabilityState::DefinitelyNonNull;
    }
    match (receiver_type, name, arg_count) {
        (
            _,
            "size" | "length" | "hashCode" | "toString" | "getBytes" | "toByteArray" | "ordinal",
            0,
        )
        | (
            Some(
                "Boolean" | "Byte" | "Short" | "Character" | "Integer" | "Long" | "Float"
                | "Double",
            ),
            "toString",
            1,
        )
        | (_, "getClass", 0)
        | (_, "logBeta", 2) => NullabilityState::DefinitelyNonNull,
        (_, "compareTo", 0 | 1) => NullabilityState::DefinitelyNonNull,
        (_, name, arg_count) if number_value_method_return_type(name, arg_count).is_some() => {
            NullabilityState::DefinitelyNonNull
        }
        (_, "isEmpty", 0) => NullabilityState::DefinitelyNonNull,
        (_, "equals" | "containsAll", 1) => NullabilityState::DefinitelyNonNull,
        (Some("Class" | "JavaClass"), "getResourceAsStream" | "get_resource_as_stream", 1) => {
            NullabilityState::MaybeNull
        }
        (Some("Class" | "JavaClass"), "getClassLoader" | "get_class_loader", 0) => {
            NullabilityState::DefinitelyNonNull
        }
        (Some("Class" | "JavaClass"), "isArray" | "is_array", 0) => {
            NullabilityState::DefinitelyNonNull
        }
        (Some("ClassLoader"), "getResource" | "get_resource", 1) => NullabilityState::MaybeNull,
        (Some("ClassLoader"), "getResourceAsStream" | "get_resource_as_stream", 1) => {
            NullabilityState::MaybeNull
        }
        (Some("URL"), "getFile" | "get_file", 0) => NullabilityState::DefinitelyNonNull,
        (Some("URL"), "openConnection" | "open_connection", 0) => {
            NullabilityState::DefinitelyNonNull
        }
        (Some("JarURLConnection"), "getJarFile" | "get_jar_file", 0) => {
            NullabilityState::DefinitelyNonNull
        }
        (Some("JarFile"), "entries" | "getManifest" | "get_manifest", 0) => {
            NullabilityState::DefinitelyNonNull
        }
        (Some("Manifest"), "getMainAttributes" | "get_main_attributes", 0) => {
            NullabilityState::DefinitelyNonNull
        }
        (Some("Attributes"), "getValue" | "get_value", 1) => NullabilityState::DefinitelyNonNull,
        (
            Some("BufferedImage"),
            "createGraphics" | "create_graphics" | "getGraphics" | "get_graphics",
            0,
        ) => NullabilityState::DefinitelyNonNull,
        (Some("Component"), "getGraphics" | "get_graphics", 0) => NullabilityState::MaybeNull,
        (Some("Component"), "getWidth" | "get_width" | "getHeight" | "get_height", 0) => {
            NullabilityState::DefinitelyNonNull
        }
        (
            Some("Color"),
            "getRed" | "get_red" | "getGreen" | "get_green" | "getBlue" | "get_blue",
            0,
        ) => NullabilityState::DefinitelyNonNull,
        (Some("Font"), "getSize" | "get_size" | "isBold" | "is_bold", 0) => {
            NullabilityState::DefinitelyNonNull
        }
        (Some("Graphics"), "dispose", 0) => NullabilityState::DefinitelyNonNull,
        (Some("Graphics"), "getClip" | "get_clip", 0) => NullabilityState::DefinitelyNonNull,
        (Some("Graphics"), "getClipBounds" | "get_clip_bounds", 0) => {
            NullabilityState::DefinitelyNonNull
        }
        (Some("Graphics"), "getFontMetrics" | "get_font_metrics", 1) => {
            NullabilityState::DefinitelyNonNull
        }
        (Some("Graphics"), "drawImage" | "draw_image", 4) => NullabilityState::DefinitelyNonNull,
        (
            Some("TableModel"),
            "getRowCount" | "get_row_count" | "getColumnCount" | "get_column_count",
            0,
        ) => NullabilityState::DefinitelyNonNull,
        (Some("TableModel"), "getColumnName" | "get_column_name", 1) => {
            NullabilityState::DefinitelyNonNull
        }
        (Some("TableModel"), "getValueAt" | "get_value_at", 2) => {
            NullabilityState::DefinitelyNonNull
        }
        (
            Some(owner),
            "getMessage" | "get_message" | "getLocalizedMessage" | "get_localized_message",
            0,
        ) if is_known_throwable_type(owner) => NullabilityState::MaybeNull,
        (Some(owner), "name", 0) if is_known_external_enum_type(owner) => {
            NullabilityState::DefinitelyNonNull
        }
        (Some(owner), name, arg_count)
            if boxed_number_value_method_return_type(owner, name, arg_count).is_some() =>
        {
            NullabilityState::DefinitelyNonNull
        }
        (Some(owner), "make", 0) if external_builder_product_type(owner).is_some() => {
            NullabilityState::Unknown
        }
        (Some("String"), "format", _) => NullabilityState::DefinitelyNonNull,
        (Some("String"), "split", 1 | 2) => NullabilityState::DefinitelyNonNull,
        (Some("String"), "substring", 1 | 2) => NullabilityState::DefinitelyNonNull,
        (Some(owner), "decode", _) if known_external_decoder_return_type(owner).is_some() => {
            NullabilityState::DefinitelyNonNull
        }
        (Some(owner), "format", _) if is_string_formatting_receiver(owner) => {
            NullabilityState::DefinitelyNonNull
        }
        (Some(owner), "join", _) if is_string_join_receiver(owner) => {
            NullabilityState::DefinitelyNonNull
        }
        (Some("Long"), "toBinaryString", 1) => NullabilityState::DefinitelyNonNull,
        (_, "getCanonicalPath", 0) => NullabilityState::DefinitelyNonNull,
        (Some("StringUtils"), "isEmpty", 1) => NullabilityState::DefinitelyNonNull,
        (Some("StringUtils"), "split", 2) => NullabilityState::DefinitelyNonNull,
        (Some("Integer"), "parseInt", 1 | 2) => NullabilityState::DefinitelyNonNull,
        (Some("Objects"), "equals", 2) => NullabilityState::DefinitelyNonNull,
        (Some("Objects"), "nonNull" | "isNull", 1) => NullabilityState::DefinitelyNonNull,
        (Some("Collections"), "emptyList" | "emptySet" | "emptyMap", 0) => {
            NullabilityState::DefinitelyNonNull
        }
        (Some("Collections"), "singletonList" | "singleton", 1) => {
            NullabilityState::DefinitelyNonNull
        }
        (Some("Collections"), "singletonMap", 2) => NullabilityState::DefinitelyNonNull,
        (Some("Iterators"), "forEnumeration", 1) => NullabilityState::DefinitelyNonNull,
        (Some(owner), "writeString", 2 | 3) if is_java_nio_files_receiver(owner) => {
            NullabilityState::DefinitelyNonNull
        }
        (Some("Files"), "newDirectoryStream", 1 | 2) => NullabilityState::DefinitelyNonNull,
        (Some("System"), "getProperty" | "get_property", 1 | 2) => NullabilityState::MaybeNull,
        (Some("ImmutableMap" | "ImmutableSortedMap"), "of", _) => {
            NullabilityState::DefinitelyNonNull
        }
        (Some("ListUtils" | "Sets"), "union", 2) => NullabilityState::DefinitelyNonNull,
        (Some("Collectors"), "toList" | "toSet" | "joining", 0)
        | (Some("Collectors"), "joining", 1 | 3) => NullabilityState::DefinitelyNonNull,
        (Some("Stream"), "of", _) => NullabilityState::DefinitelyNonNull,
        (Some("Stream"), "filter" | "distinct" | "sorted", 0 | 1) => {
            NullabilityState::DefinitelyNonNull
        }
        (Some("Stream"), "mapToInt" | "mapToLong" | "mapToDouble", 1) => {
            NullabilityState::DefinitelyNonNull
        }
        (Some("IntStream" | "LongStream" | "DoubleStream"), "sum", 0) => {
            NullabilityState::DefinitelyNonNull
        }
        (Some("Stream"), "collect", 1) | (Some("Stream"), "iterator", 0) => {
            NullabilityState::DefinitelyNonNull
        }
        (Some("Arrays"), "asList" | "stream", _) => NullabilityState::DefinitelyNonNull,
        (Some("Arrays"), "equals", 2) => NullabilityState::DefinitelyNonNull,
        (Some("Arrays"), "copyOf", 2) => NullabilityState::DefinitelyNonNull,
        (Some("Arrays"), "copyOfRange", 3) => NullabilityState::DefinitelyNonNull,
        (Some("ArrayUtils"), "addAll", 2) => NullabilityState::DefinitelyNonNull,
        (Some("MathArrays"), "ebeMultiply", 2) => NullabilityState::DefinitelyNonNull,
        (Some(owner), "toArray", 1) if primitive_array_utility_element_type(owner).is_some() => {
            NullabilityState::DefinitelyNonNull
        }
        (Some(_), "fastqToPhred", 1) => NullabilityState::DefinitelyNonNull,
        (Some(_), "phredToFastq", 1) => NullabilityState::DefinitelyNonNull,
        (Some(_), "getBasesReverseComplement", 1) => NullabilityState::DefinitelyNonNull,
        (Some(_), "getAttributeAsString", 1 | 2) => NullabilityState::MaybeNull,
        _ => NullabilityState::Unknown,
    }
}

fn collection_api_method_role(name: &str, _arg_count: usize) -> ExternalApiRole {
    match name {
        "get" => ExternalApiRole::IndexedGetter,
        "size" | "isEmpty" | "contains" | "containsAll" | "iterator" | "stream" | "filter"
        | "distinct" | "sorted" | "collect" | "toArray" => ExternalApiRole::Getter,
        "add" | "addAll" | "remove" | "removeAll" | "removeIf" | "clear" => {
            ExternalApiRole::Mutator
        }
        _ => external_api_method_role(None, name, _arg_count),
    }
}

pub fn is_content_read_argument_method(name: &str, arg_index: usize) -> bool {
    matches!(
        (name, arg_index),
        ("addAll", 0)
            | ("containsAll", 0)
            | ("contains", 0)
            | ("startsWith", 0)
            | ("starts_with", 0)
            | ("endsWith", 0)
            | ("ends_with", 0)
            | ("replace", 0)
            | ("replace", 1)
            | ("replaceFirst", 0)
            | ("replaceFirst", 1)
            | ("replace_first", 0)
            | ("replace_first", 1)
            | ("replaceAll", 0)
            | ("replaceAll", 1)
            | ("replace_all", 0)
            | ("replace_all", 1)
            | ("split", 0)
            | ("getResource", 0)
            | ("get_resource", 0)
            | ("getResourceAsStream", 0)
            | ("get_resource_as_stream", 0)
    )
}

fn map_api_method_role(name: &str, _arg_count: usize) -> ExternalApiRole {
    match name {
        "get" | "getOrDefault" | "containsKey" | "containsValue" | "keySet" | "values"
        | "entrySet" | "size" | "isEmpty" => ExternalApiRole::Getter,
        "put" | "putAll" | "remove" | "clear" => ExternalApiRole::Mutator,
        _ => external_api_method_role(None, name, _arg_count),
    }
}

fn static_utility_method_role(
    receiver_type: Option<&str>,
    name: &str,
    arg_count: usize,
) -> ExternalApiRole {
    match (receiver_type, name, arg_count) {
        (Some("String"), "format", _) => ExternalApiRole::Getter,
        (Some("String"), "valueOf" | "value_of", 1) => ExternalApiRole::Getter,
        (Some("String"), "split", 1 | 2) => ExternalApiRole::Getter,
        (Some("String"), "substring", 1 | 2) => ExternalApiRole::Getter,
        (Some("String" | "StringUtils" | "StringUtil"), "join", _) => ExternalApiRole::Getter,
        (Some("Long"), "toBinaryString", 1) => ExternalApiRole::Getter,
        (Some("StringUtils"), "isEmpty", 1) => ExternalApiRole::BooleanGetter,
        (Some("StringUtils"), "split", 2) => ExternalApiRole::Getter,
        (
            Some("Integer"),
            "parseInt" | "parse_int" | "valueOf" | "value_of" | "toHexString" | "to_hex_string",
            1 | 2,
        ) => ExternalApiRole::Getter,
        (Some("Float"), "compare", 2) => ExternalApiRole::Getter,
        (
            Some("Float"),
            "isNaN"
            | "is_na_n"
            | "isInfinite"
            | "is_infinite"
            | "parseFloat"
            | "parse_float"
            | "floatToIntBits"
            | "float_to_int_bits"
            | "floatToRawIntBits"
            | "float_to_raw_int_bits"
            | "intBitsToFloat"
            | "int_bits_to_float",
            1,
        ) => ExternalApiRole::Getter,
        (Some("Double"), "compare", 2) => ExternalApiRole::Getter,
        (
            Some("Double"),
            "parseDouble"
            | "parse_double"
            | "isNaN"
            | "is_na_n"
            | "isInfinite"
            | "is_infinite"
            | "doubleToLongBits"
            | "double_to_long_bits"
            | "doubleToRawLongBits"
            | "double_to_raw_long_bits"
            | "longBitsToDouble"
            | "long_bits_to_double"
            | "toString"
            | "to_string",
            1,
        ) => ExternalApiRole::Getter,
        (
            Some("Math"),
            "abs" | "ceil" | "floor" | "log" | "log10" | "exp" | "sqrt" | "round",
            1,
        ) => ExternalApiRole::Getter,
        (Some("Math"), "pow", 2) => ExternalApiRole::Getter,
        (Some("Math"), "random", 0) => ExternalApiRole::Getter,
        (Some("StrictMath"), "IEEEremainder" | "i_e_e_eremainder", 2) => ExternalApiRole::Getter,
        (Some("Beta"), "logBeta", 2) => ExternalApiRole::Getter,
        (Some("Objects"), "nonNull" | "isNull", 1) => ExternalApiRole::BooleanGetter,
        (Some("Objects"), "equals", 2) => ExternalApiRole::Getter,
        (Some("Collections"), "emptyList" | "emptySet" | "emptyMap", 0) => ExternalApiRole::Getter,
        (Some("Collections"), "singletonList" | "singleton", 1) => ExternalApiRole::Getter,
        (Some("Collections"), "singletonMap", 2) => ExternalApiRole::Getter,
        (Some("Collections"), "sort" | "shuffle" | "reverse", _) => ExternalApiRole::Mutator,
        (Some("Iterators"), "forEnumeration", 1) => ExternalApiRole::Getter,
        (Some(owner), "writeString", 2 | 3) if is_java_nio_files_receiver(owner) => {
            ExternalApiRole::Mutator
        }
        (Some("Files"), "newDirectoryStream", 1 | 2) => ExternalApiRole::Getter,
        (Some("System"), "getProperty" | "get_property", 1 | 2) => ExternalApiRole::Getter,
        (Some("System"), "lineSeparator" | "line_separator", 0) => ExternalApiRole::Getter,
        (Some("System"), "currentTimeMillis" | "current_time_millis", 0) => ExternalApiRole::Getter,
        (Some("System"), "identityHashCode" | "identity_hash_code", 1) => ExternalApiRole::Getter,
        (Some("System"), "gc", 0) => ExternalApiRole::SideEffect,
        (Some("System"), "arraycopy", 5) => ExternalApiRole::Mutator,
        (Some("System"), "setProperty" | "set_property", 2) => ExternalApiRole::Mutator,
        (Some("ImmutableMap" | "ImmutableSortedMap"), "of", _) => ExternalApiRole::Getter,
        (Some("ListUtils" | "Sets"), "union", 2) => ExternalApiRole::Getter,
        (Some("Collectors"), "toList" | "toSet" | "joining", 0)
        | (Some("Collectors"), "joining", 1 | 3) => ExternalApiRole::Getter,
        (Some("Stream"), "of", _) => ExternalApiRole::Getter,
        (
            Some("Stream"),
            "filter" | "distinct" | "sorted" | "mapToInt" | "mapToLong" | "mapToDouble" | "collect"
            | "iterator",
            _,
        ) => ExternalApiRole::Getter,
        (Some("IntStream" | "LongStream" | "DoubleStream"), "sum", 0) => ExternalApiRole::Getter,
        (Some("Arrays"), "asList" | "stream", _) => ExternalApiRole::Getter,
        (Some("Arrays"), "equals", 2) => ExternalApiRole::BooleanGetter,
        (Some("Arrays"), "copyOf", 2) => ExternalApiRole::Getter,
        (Some("Arrays"), "copyOfRange", 3) => ExternalApiRole::Getter,
        (Some("ArrayUtils"), "addAll", 2) => ExternalApiRole::Getter,
        (Some("MathArrays"), "ebeMultiply", 2) => ExternalApiRole::Getter,
        (Some(owner), "decode", _) if known_external_decoder_return_type(owner).is_some() => {
            ExternalApiRole::Getter
        }
        (Some(owner), "toArray", 1) if primitive_array_utility_element_type(owner).is_some() => {
            ExternalApiRole::Getter
        }
        _ => external_api_method_role(None, name, arg_count),
    }
}

fn is_static_utility_owner(receiver_type: Option<&str>) -> bool {
    matches!(
        receiver_type,
        Some("String")
            | Some("Long")
            | Some("Integer")
            | Some("Float")
            | Some("Double")
            | Some("Math")
            | Some("StrictMath")
            | Some("System")
            | Some("StringUtils")
            | Some("StringUtil")
            | Some("Beta")
            | Some("Objects")
            | Some("Collections")
            | Some("ImmutableMap")
            | Some("ImmutableSortedMap")
            | Some("Collectors")
            | Some("Stream")
            | Some("Iterators")
            | Some("Files")
            | Some("ListUtils")
            | Some("Sets")
            | Some("Arrays")
            | Some("MathArrays")
            | Some("TextCigarCodec")
            | Some("Booleans")
            | Some("Bytes")
            | Some("Shorts")
            | Some("Chars")
            | Some("Ints")
            | Some("Longs")
            | Some("Floats")
            | Some("Doubles")
    )
}

fn is_collection_like_receiver(receiver_type: Option<&str>) -> bool {
    matches!(
        receiver_type,
        Some("List")
            | Some("ArrayList")
            | Some("Set")
            | Some("HashSet")
            | Some("LinkedHashSet")
            | Some("TreeSet")
            | Some("Collection")
            | Some("Iterable")
            | Some("IntervalList")
            | Some("Stream")
    )
}

fn is_map_like_receiver(receiver_type: Option<&str>) -> bool {
    matches!(
        receiver_type,
        Some("Map") | Some("HashMap") | Some("LinkedHashMap")
    )
}
