use std::collections::{BTreeMap, BTreeSet};

use crate::effective_facts::EffectiveFacts;
use crate::facts::FactSet;
use crate::facts::{
    CallableKey, ExternalApiRole, ExternalArgumentAliasPolicy, FacadeAbiFact, FacadeSlot,
    FactProvenance, HierarchyStrategy, NullabilityState, ReceiverMutabilityFact, ResultAbi,
    Staticness,
};
use crate::java_ir::{
    Block, Expr, JavaType, Member, MethodDecl, PrimitiveType, Program, Stmt, TypeDecl,
    WildcardBound,
};
use crate::shape_projection::{
    apply_borrowed_sequence_content_projection, apply_borrowed_string_content_projection,
};
use crate::slot_render::{
    dyn_trait_rust_path, rust_name_for_java_named_type, rust_type_for_java_type_with_facts,
    rust_type_for_slot_with_facts,
};
use crate::slot_shape::{MutabilityShape, OwnershipShape, RustSlotShape};

pub fn is_known_jdk_shadow_sensitive_owner(name: &str) -> bool {
    matches!(name, "Objects" | "Collections" | "Integer" | "Paths")
}

pub fn is_content_read_method(name: &str) -> bool {
    matches!(name, "length" | "isEmpty" | "trim")
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
        (_, "length", 0) => Some(JavaType::Primitive(PrimitiveType::Int)),
        (_, "compareTo", 0 | 1) => Some(JavaType::Primitive(PrimitiveType::Int)),
        (_, "ordinal", 0) => Some(JavaType::Primitive(PrimitiveType::Int)),
        (_, "logBeta", 2) => Some(JavaType::Primitive(PrimitiveType::Double)),
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
        (Some(owner), "isPresent", 0) if is_optional_like_receiver(owner) => {
            Some(JavaType::Primitive(PrimitiveType::Boolean))
        }
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
        (Some(owner), "getMessage", 0) if is_known_throwable_type(owner) => Some(JavaType::String),
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
        (_, "getBytes", 0) | (_, "toByteArray", 0) => Some(JavaType::Array(Box::new(
            JavaType::Primitive(PrimitiveType::Byte),
        ))),
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
    match (name, arg_count) {
        ("toString" | "toLowerCase" | "toUpperCase" | "trim", 0) => Some(JavaType::String),
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
        (
            "Float",
            "MAX_VALUE" | "MIN_VALUE" | "NaN" | "POSITIVE_INFINITY" | "NEGATIVE_INFINITY",
        ) => PrimitiveType::Float,
        (
            "Double",
            "MAX_VALUE" | "MIN_VALUE" | "NaN" | "POSITIVE_INFINITY" | "NEGATIVE_INFINITY",
        ) => PrimitiveType::Double,
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
    if receiver == "ArrayUtils"
        && let Some(primitive) = array_utils_empty_array_element_type(&field_name)
    {
        return Some(JavaType::Array(Box::new(JavaType::Primitive(primitive))));
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
        ("Locale", "ENGLISH") => "Locale",
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
        || matches!(field_name, "NAME" | "DESCRIPTION" | "DOC" | "USAGE")
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
        "byte" => "Byte",
        "short" => "Short",
        "character" => "Character",
        "integer" => "Integer",
        "long" => "Long",
        "float" => "Float",
        "double" => "Double",
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
        "system_utils" => "SystemUtils",
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
        "locale" => "Locale",
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
    crate::rust_ir::RustItem::Raw(
        r#"#[derive(Debug, Clone, PartialEq, Eq)]
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

    pub fn equals<U>(&self, other: JavaClass<U>) -> bool {
        self.name == other.name
    }
}"#
        .to_string(),
    )
}

pub fn java_platform_facade_support_item() -> crate::rust_ir::RustItem {
    crate::rust_ir::RustItem::Raw(
        r#"#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
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

    pub fn to_string(&self) -> String {
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

    pub fn to_path(&self) -> Path {
        Path::new(self.value.clone())
    }

    pub fn get_path(&self) -> String {
        self.value.clone()
    }

    pub fn get_absolute_path(&self) -> String {
        self.value.clone()
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

pub fn java_string_split_support_item() -> crate::rust_ir::RustItem {
    crate::rust_ir::RustItem::Raw(
        r#"pub fn java_string_split<R: AsRef<str>>(value: &str, regex: R, limit: i32) -> Vec<String> {
    let regex = regex.as_ref();
    let delimiter = match regex {
        "\\t" | "\\\\t" => "\t",
        "\\n" | "\\\\n" => "\n",
        "\\r" | "\\\\r" => "\r",
        "\\s+" => " ",
        other => other,
    };
    let mut parts = if delimiter == "\\s+" {
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
    let source_summary_types = local_type_keys_for_program(source_summary_type_program);
    let used_calls = collect_program_call_arities(program);
    let used_constructors = collect_program_constructor_arities(program);
    let imported_static_owner_calls = collect_imported_static_owner_calls(program);

    let mut support = ExternalApiFacadeSupport::default();
    support.add_nested_type_aliases_from_fields(facts, &local_types);
    support.add_declared_external_types(program, &local_types);
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
    support.add_inherited_support_field_types(program, local_type_program, &emitted_local_types);
    support.add_used_external_constructors(facts, &used_constructors, &local_types);
    support.add_used_external_api_facts(
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
    support.add_imported_static_owner_calls(facts, &imported_static_owner_calls, &local_types);
    support.add_known_domain_getter_surfaces_from_used_calls(facts, &used_calls);
    support.add_builder_surface_from_used_calls(facts, &used_calls);
    support.add_external_field_surfaces(facts, &local_types);
    support.add_external_dyn_trait_surfaces(facts);
    support.suppress_rust_crate_mapped_external_types(facts);
    support.into_items()
}

fn local_type_keys_for_program(program: &Program) -> BTreeSet<String> {
    let mut types = BTreeSet::new();
    for ty in &program.types {
        collect_local_type_keys(ty, &mut types);
    }
    types
}

fn collect_local_type_keys(ty: &crate::java_ir::TypeDecl, types: &mut BTreeSet<String>) {
    types.insert(canonical_external_receiver_type_key(&ty.name));
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_local_type_keys(nested, types);
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

#[derive(Default)]
struct ExternalApiFacadeSupport {
    opaque_types: BTreeSet<String>,
    trait_type_names: BTreeSet<String>,
    traits: BTreeMap<String, ExternalFacadeTrait>,
    trait_impls: BTreeMap<String, BTreeSet<String>>,
    aliases: BTreeMap<String, String>,
    fields: BTreeMap<String, BTreeMap<String, String>>,
    impls: BTreeMap<String, BTreeSet<String>>,
    erased_object_methods: BTreeMap<String, ExternalErasedObjectMethod>,
    erased_object_method_conflicts: BTreeSet<String>,
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

impl ExternalApiFacadeSupport {
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
                    NullabilityState::DefinitelyNonNull,
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
                if valid_external_facade_type_ident(&return_type) {
                    self.add_opaque_type(return_type.clone());
                }
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
                    let return_type = external_facade_method_return_type_with_facts(
                        facts,
                        Some(&type_key),
                        &method.name,
                        Some(arity),
                        &method.return_type,
                        NullabilityState::DefinitelyNonNull,
                    );
                    self.add_java_type(&method.return_type);
                    for param in &method.params {
                        self.add_java_type(&param.ty);
                    }
                    if valid_external_facade_type_ident(&return_type) {
                        self.add_opaque_type(return_type.clone());
                    }
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
            Stmt::While { body, .. } | Stmt::EnhancedFor { body, .. } => {
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
            Stmt::Block(block) => self.add_declared_external_types_from_block(block, local_types),
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
                if self.has_method_named(&owner, &rust_name) {
                    continue;
                }
                self.add_method(
                    owner.clone(),
                    external_facade_constructor_method(&rust_name, *arg_count),
                );
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
            if external_facade_receiver_is_dyn_trait(facts, &receiver_key) {
                if let Some(return_type) = &fact.return_type {
                    self.add_java_type(return_type);
                }
                let arg_count = fact
                    .arg_count
                    .unwrap_or_else(|| arities.iter().next().copied().unwrap_or_default());
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
                let return_rust = fact
                    .return_type
                    .as_ref()
                    .map(|return_type| external_facade_return_type_with_facts(return_type, facts))
                    .unwrap_or_else(|| "Box<dyn std::any::Any>".to_string());
                self.add_method(
                    receiver_rust,
                    external_facade_static_method(&rust_base_method, 0, &return_rust),
                );
            } else if receiver_rust.ends_with("Builder") {
                continue;
            } else {
                let arg_count = fact
                    .arg_count
                    .unwrap_or_else(|| arities.iter().next().copied().unwrap_or_default());
                let rust_method = crate::call_plan::external_method_rust_name(
                    &receiver_key,
                    &fact.method_name,
                    arg_count,
                    facts,
                );
                if !valid_rust_method_ident(&rust_method) {
                    continue;
                }
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
                    .unwrap_or_else(|| "()".to_string());
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
                {
                    self.add_erased_object_method(
                        facts,
                        Some(&receiver_key),
                        rust_method.clone(),
                        arg_count,
                        return_rust.clone(),
                    );
                }
                self.add_method(
                    receiver_rust,
                    external_facade_instance_method_with_facts(
                        facts,
                        &receiver_key,
                        &fact.method_name,
                        &rust_method,
                        arg_count,
                        &return_rust,
                    ),
                );
            }
        }
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
                external_facade_argument_java_type(facts, receiver_key, method, index)
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
            let owner_rust = rust_name_for_java_named_type(&owner_key);
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
                let method_body = external_facade_imported_static_method_with_facts(
                    facts, &owner_key, method, *arg_count,
                );
                for method_owner in &method_owners {
                    if self.has_method_named(method_owner, method) {
                        continue;
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
            for (rust_method, arities) in used_calls {
                let java_method = lower_camel_from_snake(rust_method);
                for arg_count in arities {
                    if owner.ends_with("ExecutorService")
                        && rust_method == "submit"
                        && *arg_count == 1
                    {
                        let method_body =
                            external_facade_instance_method(rust_method, *arg_count, "Future");
                        if self.has_method_signature(&owner, rust_method, *arg_count) {
                            self.replace_method_signature(
                                &owner,
                                rust_method,
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
                        let method_body =
                            external_facade_builder_generic_getter(rust_method, *arg_count);
                        if self.has_method_signature(&owner, rust_method, *arg_count) {
                            self.replace_method_signature(
                                &owner,
                                rust_method,
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
                            external_facade_instance_method(rust_method, *arg_count, "()");
                        if !self.has_method_signature(&owner, rust_method, *arg_count) {
                            self.add_method(owner.clone(), method_body);
                        }
                        continue;
                    }
                    let Some(return_type) =
                        known_external_method_return_type(Some(&owner), &java_method, *arg_count)
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
                    let method_body =
                        external_facade_instance_method(rust_method, *arg_count, &return_rust);
                    if self.has_method_signature(&owner, rust_method, *arg_count)
                        && !self.replace_weak_method_return(
                            &owner,
                            rust_method,
                            *arg_count,
                            &method_body,
                        )
                    {
                        continue;
                    }

                    if !self.has_method_signature(&owner, rust_method, *arg_count)
                        && Self::known_missing_opaque_support_method_return_type(
                            &owner,
                            &java_method,
                            *arg_count,
                        )
                        .is_none()
                    {
                        continue;
                    }

                    if !self.has_method_named(&owner, rust_method) {
                        self.add_method(owner.clone(), method_body);
                    }
                }
            }
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
        for fact in &facts.external_field_facts {
            let receiver_key = canonical_external_receiver_type_key(&fact.field.type_name);
            if local_types.contains(&receiver_key) {
                continue;
            }
            let owner = rust_name_for_java_named_type(&receiver_key);
            if !valid_external_facade_type_ident(&owner)
                || !self.opaque_types.contains(&owner)
                || is_builtin_external_facade_type(&owner)
            {
                continue;
            }
            let Some(field_type) = &fact.field_type else {
                continue;
            };
            if is_self_typed_external_enum_constant_field(
                &receiver_key,
                &fact.field.field_name,
                field_type,
            ) {
                self.add_java_type(field_type);
                continue;
            }
            let rust_name = crate::parser::to_snake_case(&fact.field.field_name);
            if !valid_rust_field_ident(&rust_name) {
                continue;
            }
            let rust_type = external_facade_return_type_with_facts(field_type, facts);
            self.add_java_type(field_type);
            self.fields
                .entry(owner)
                .or_default()
                .insert(rust_name, rust_type);
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
                        self.add_method(
                            builder.clone(),
                            external_facade_builder_fluent_method(method, *arg_count),
                        );
                    } else if method.starts_with("get_") && method.ends_with("_builder") {
                        self.add_method(
                            builder.clone(),
                            external_facade_builder_generic_getter(method, *arg_count),
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
        if valid_external_facade_type_ident(&name) {
            self.opaque_types.insert(name);
        }
    }

    fn add_type_alias(&mut self, alias: String, target: String) {
        if alias != target
            && valid_external_facade_type_ident(&alias)
            && valid_external_facade_type_ident(&target)
        {
            self.add_opaque_type(target.clone());
            self.aliases.insert(alias, target);
        }
    }

    fn add_method(&mut self, owner: String, method: String) {
        if valid_external_facade_type_ident(&owner) {
            self.impls.entry(owner).or_default().insert(method);
        }
    }

    fn has_method_named(&self, owner: &str, method: &str) -> bool {
        self.impls.get(owner).is_some_and(|methods| {
            methods
                .iter()
                .any(|body| facade_method_has_name(body, method))
        })
    }

    fn has_method_signature(&self, owner: &str, method: &str, arg_count: usize) -> bool {
        self.impls.get(owner).is_some_and(|methods| {
            methods
                .iter()
                .any(|body| facade_method_matches_signature(body, method, arg_count))
        })
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

    fn add_external_dyn_trait_surfaces(&mut self, facts: &FactSet) {
        let mut trait_type_to_name = BTreeMap::new();
        for override_fact in &facts.hierarchy_overrides {
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

        for effect in &facts.external_api_effect_facts {
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
                facts,
            );
            if !valid_rust_method_ident(&rust_method) {
                continue;
            }
            let return_type = effect
                .return_type
                .as_ref()
                .map(|return_type| {
                    external_facade_method_return_type_with_facts(
                        facts,
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

        for capability in &facts.type_capability_overrides {
            let implementor = canonical_external_receiver_type_key(&capability.type_name);
            if !valid_external_facade_type_ident(&implementor)
                || is_builtin_external_facade_type(&implementor)
            {
                continue;
            }
            for implemented in &capability.implements {
                let Some(implemented_name) = java_type_name_for_external_facade(implemented) else {
                    continue;
                };
                let implemented_key = canonical_external_receiver_type_key(implemented_name);
                let Some(trait_name) = trait_type_to_name.get(&implemented_key) else {
                    continue;
                };
                self.trait_impls
                    .entry(trait_name.clone())
                    .or_default()
                    .insert(implementor.clone());
            }
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
    }

    fn into_items(mut self) -> Vec<crate::rust_ir::RustItem> {
        let alias_names = self.aliases.keys().cloned().collect::<BTreeSet<_>>();
        self.opaque_types.retain(|name| {
            !alias_names.contains(name)
                && !self.trait_type_names.contains(name)
                && !is_builtin_external_facade_type(name)
        });

        let display_and_eq_types = self.opaque_types.iter().cloned().collect::<Vec<_>>();
        let mut lines = Vec::new();
        for ty in self.opaque_types {
            if let Some(fields) = self.fields.remove(&ty)
                && !fields.is_empty()
            {
                let field_lines = fields
                    .into_iter()
                    .map(|(name, ty)| format!("    pub {name}: {ty},"))
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
                if self.trait_type_names.contains(&implementor) {
                    continue;
                }
                lines.push(format!("impl {trait_name} for {implementor} {{}}"));
            }
        }
        for (owner, methods) in self.impls {
            if is_builtin_external_facade_type(&owner)
                || alias_names.contains(&owner)
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
        Stmt::While { body, .. } | Stmt::Block(body) => {
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
        Stmt::While { condition, body } => {
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
        Stmt::Block(block) => collect_block_imported_static_owner_calls(
            block,
            aliases,
            static_members,
            shadows,
            calls,
        ),
        Stmt::VarDecl {
            initializer: None, ..
        }
        | Stmt::Return(None)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => {}
    }
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
        | Expr::Raw(_) => {}
    }
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
        Stmt::While { condition, body } => {
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
        Stmt::Block(block) => collect_block_constructor_arities(block, constructors),
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
        | Expr::Raw(_) => {}
    }
}

fn constructed_type_name(ty: &JavaType) -> Option<&str> {
    match ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
        _ => None,
    }
}

fn external_constructor_fact_arity_count(facts: &FactSet, type_name: &str) -> usize {
    facts
        .external_constructor_facts
        .iter()
        .filter(|fact| external_constructor_type_matches(&fact.type_name, type_name))
        .map(|fact| fact.arg_types.len())
        .collect::<BTreeSet<_>>()
        .len()
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
        Stmt::While { condition, body } => {
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
        Stmt::Block(block) => collect_block_call_arities(block, calls),
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
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_) => {}
    }
}

fn external_facade_return_type_with_facts(java_type: &JavaType, facts: &FactSet) -> String {
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
    if let Some(return_type) = external_facade_method_abi_return_type_with_facts(
        facts,
        receiver_type,
        method_name,
        arg_count,
        java_type,
    ) {
        return normalize_external_facade_return_type_text(&return_type);
    }
    if let Some(rust_return) =
        facts.external_rust_return_type_for_receiver(receiver_type, method_name, arg_count)
    {
        return normalize_external_facade_return_type_text(&rust_return.rust_type_name);
    }
    normalize_external_facade_return_type_text(
        &external_facade_return_type_with_facts_and_nullability(facts, java_type, nullability),
    )
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
        ResultAbi::OptionWrapped if rendered.starts_with("Option<") => rendered,
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
            if inner.starts_with("Option<") {
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
    if let Some(inner) = trimmed
        .strip_prefix("Option<")
        .and_then(|rest| rest.strip_suffix('>'))
    {
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
    format!(
        "    pub fn {name}{} -> {return_type} {{ {} }}",
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
    format!(
        "    pub fn {rust_method}{} -> {return_type} {{ {} }}",
        external_facade_args_with_facts(facts, receiver_key, java_method, arg_count),
        external_facade_default_expr(return_type, Some(rust_method))
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
    format!(
        "    pub fn {name}{} -> Self {{ Default::default() }}",
        external_facade_args(arg_count)
    )
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
        format!(
            "    pub fn {name}{} -> R {{ Default::default() }}",
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
    if let Some(effect) =
        facts.external_api_effect_for_receiver_with_arity(Some(owner_key), name, Some(arg_count))
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
    let generics = (0..arg_count)
        .map(|idx| format!("T{idx}"))
        .collect::<Vec<_>>()
        .join(", ");
    let args = (0..arg_count)
        .map(|idx| format!("_arg{idx}: T{idx}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("    pub fn {name}<{generics}>({args}) -> T0 {{ _arg0 }}")
}

fn imported_static_method_is_side_effect(name: &str) -> bool {
    name.starts_with("set_")
        || name.starts_with("use_")
        || name.starts_with("assert_")
        || name.starts_with("validate_")
        || name.starts_with("update_")
}

fn external_facade_instance_method(name: &str, arg_count: usize, return_type: &str) -> String {
    format!(
        "    pub fn {name}{} -> {return_type} {{ {} }}",
        external_facade_self_args(arg_count),
        external_facade_default_expr(return_type, Some(name))
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
    if external_facade_method_is_self_returning_fluent(facts, receiver_key, java_method, arg_count)
    {
        return format!(
            "    pub fn {rust_method}{} -> &Self {{ self }}",
            external_facade_self_args_with_facts(facts, receiver_key, java_method, arg_count),
        );
    }
    format!(
        "    pub fn {rust_method}{} -> {return_type} {{ {} }}",
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
    format!(
        "    fn {name}{} -> {return_type} {{ {} }}",
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
        format!("pub trait {} {{}}", trait_surface.trait_name)
    } else {
        format!("pub trait {} {{\n{}\n}}", trait_surface.trait_name, methods)
    }
}

fn external_facade_builder_fluent_method(name: &str, arg_count: usize) -> String {
    format!(
        "    pub fn {name}{} -> Self {{ self.clone() }}",
        external_facade_self_args(arg_count)
    )
}

fn external_facade_builder_generic_getter(name: &str, arg_count: usize) -> String {
    format!(
        "    pub fn {name}{} -> T {{ Default::default() }}",
        external_facade_generic_self_args("T: Default", arg_count)
    )
}

fn facade_method_has_name(method_body: &str, method: &str) -> bool {
    method_body.contains(&format!(" pub fn {method}<"))
        || method_body.contains(&format!(" pub fn {method}("))
}

fn facade_method_matches_signature(method_body: &str, method: &str, arg_count: usize) -> bool {
    facade_method_has_name(method_body, method) && facade_method_arg_count(method_body) == arg_count
}

fn facade_method_arg_count(method_body: &str) -> usize {
    method_body.matches("_arg").count()
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
    matches!(return_type, "()" | "Box<dyn std::any::Any>")
}

fn external_erased_object_trait_method(method: &ExternalErasedObjectMethod) -> String {
    format!(
        "    fn {}{} -> {};",
        method.name,
        external_facade_self_args(method.arg_count),
        method.return_type
    )
}

fn external_erased_object_impl_method(method: &ExternalErasedObjectMethod) -> String {
    format!(
        "    fn {}{} -> {} {{ {} }}",
        method.name,
        external_facade_self_args(method.arg_count),
        method.return_type,
        external_facade_default_expr(&method.return_type, Some(&method.name))
    )
}

fn external_facade_default_expr(return_type: &str, method_name: Option<&str>) -> String {
    if return_type.starts_with("Option<") {
        "None".to_string()
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
    external_facade_args_with_fact_prefix(facts, receiver_key, method, arg_count, "&self")
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
        let arg_ty = external_facade_argument_rust_type(facts, receiver_key, method, index)
            .unwrap_or_else(|| {
                let generic = format!("T{index}");
                generic_params.push(generic.clone());
                generic
            });
        args.push(format!("_arg{index}: {arg_ty}"));
    }

    if generic_params.is_empty() {
        format!("({})", args.join(", "))
    } else {
        format!("<{}>({})", generic_params.join(", "), args.join(", "))
    }
}

fn external_facade_argument_rust_type(
    facts: &FactSet,
    receiver_key: &str,
    method: &str,
    index: usize,
) -> Option<String> {
    let effect = external_facade_argument_effect(facts, receiver_key, method, index)?;
    if let Some(ty) = external_facade_dyn_trait_argument_rust_type(facts, effect) {
        return Some(ty);
    }
    let shape = external_facade_argument_shape(effect)?;
    Some(crate::emit::emit_type(&rust_type_for_slot_with_facts(
        &shape, facts,
    )))
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
    index: usize,
) -> Option<JavaType> {
    external_facade_argument_effect(facts, receiver_key, method, index)
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
    let mut java_ty = effect.parameter_type.clone()?;
    if matches!(
        external_facade_argument_nullability(effect),
        NullabilityState::MaybeNull | NullabilityState::DefinitelyNull
    ) {
        java_ty = crate::types::optional_java_type(java_ty);
    }
    let mut shape = RustSlotShape::conservative(java_ty);
    if crate::types::optional_element_type(&shape.java_ty).is_none() {
        match effect.alias_policy {
            ExternalArgumentAliasPolicy::Borrowed if shape.java_ty == JavaType::String => {
                apply_borrowed_string_content_projection(&mut shape);
            }
            ExternalArgumentAliasPolicy::Borrowed
                if matches!(shape.java_ty, JavaType::Array(_)) =>
            {
                apply_borrowed_sequence_content_projection(&mut shape);
            }
            ExternalArgumentAliasPolicy::Borrowed
                if !matches!(shape.java_ty, JavaType::Primitive(_)) =>
            {
                shape.ownership = OwnershipShape::SharedBorrow;
            }
            ExternalArgumentAliasPolicy::NoAlias | ExternalArgumentAliasPolicy::Escapes => {
                if !matches!(shape.java_ty, JavaType::Primitive(_)) {
                    shape.ownership = OwnershipShape::Owned;
                }
            }
            ExternalArgumentAliasPolicy::Unknown | ExternalArgumentAliasPolicy::Borrowed => {}
        }
    }
    if effect.argument_mutability == ReceiverMutabilityFact::MutatesReceiver
        && !matches!(shape.java_ty, JavaType::Primitive(_))
    {
        shape.ownership = OwnershipShape::MutableBorrow;
        shape.mutability = MutabilityShape::Mutable;
    }
    Some(shape)
}

fn external_facade_argument_effect<'a>(
    facts: &'a FactSet,
    receiver_key: &str,
    method: &str,
    index: usize,
) -> Option<&'a crate::facts::ExternalApiArgumentEffectFact> {
    facts
        .external_api_argument_effect_for_receiver(Some(receiver_key), method, index)
        .or_else(|| {
            let java_method = lower_camel_from_snake(method);
            facts.external_api_argument_effect_for_receiver(Some(receiver_key), &java_method, index)
        })
}

fn external_facade_generic_self_args(first_generic: &str, arg_count: usize) -> String {
    let mut generics = vec![first_generic.to_string()];
    generics.extend((0..arg_count).map(|idx| format!("T{idx}")));
    if arg_count == 0 {
        format!("<{}>(&self)", generics.join(", "))
    } else {
        let args = (0..arg_count)
            .map(|idx| format!("_arg{idx}: T{idx}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!("<{}>(&self, {args})", generics.join(", "))
    }
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
            | "Object"
            | "Set"
            | "Stream"
            | "File"
            | "Path"
            | "URI"
            | "URL"
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

fn valid_external_facade_type_ident(name: &str) -> bool {
    valid_rust_type_ident(name)
        && name
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_uppercase())
        && !is_builtin_external_facade_type(name)
}

fn java_type_name_for_external_facade(java_ty: &JavaType) -> Option<&str> {
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
        _ => None,
    }
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
    let mut constants_by_owner: BTreeMap<String, BTreeMap<String, JavaType>> = BTreeMap::new();
    collect_external_static_string_constants_program(
        program,
        facts,
        &local_types,
        &mut constants_by_owner,
    );
    let mapped_types = rust_crate_mapped_external_facade_type_names(facts);
    constants_by_owner
        .into_iter()
        .filter(|(owner, _)| !mapped_types.contains(owner))
        .filter_map(|(owner, fields)| {
            external_static_string_constant_support_item(
                &owner,
                fields,
                &local_static_string_values,
                declared_owners,
            )
        })
        .collect()
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
    constants_by_owner: &mut BTreeMap<String, BTreeMap<String, JavaType>>,
) {
    for ty in &program.types {
        collect_external_static_string_constants_type(ty, facts, local_types, constants_by_owner);
    }
}

fn collect_external_static_string_constants_type(
    ty: &crate::java_ir::TypeDecl,
    facts: &FactSet,
    local_types: &BTreeSet<String>,
    constants_by_owner: &mut BTreeMap<String, BTreeMap<String, JavaType>>,
) {
    for member in &ty.members {
        match member {
            Member::Field(field) => {
                if let Some(initializer) = &field.initializer {
                    collect_external_static_string_constants_expr(
                        initializer,
                        facts,
                        local_types,
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
                        constants_by_owner,
                    );
                }
            }
            Member::Type(nested) => collect_external_static_string_constants_type(
                nested,
                facts,
                local_types,
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
    constants_by_owner: &mut BTreeMap<String, BTreeMap<String, JavaType>>,
) {
    for stmt in &block.statements {
        collect_external_static_string_constants_stmt(stmt, facts, local_types, constants_by_owner);
    }
}

fn collect_external_static_string_constants_stmt(
    stmt: &Stmt,
    facts: &FactSet,
    local_types: &BTreeSet<String>,
    constants_by_owner: &mut BTreeMap<String, BTreeMap<String, JavaType>>,
) {
    match stmt {
        Stmt::VarDecl { initializer, .. } | Stmt::Return(initializer) => {
            if let Some(expr) = initializer {
                collect_external_static_string_constants_expr(
                    expr,
                    facts,
                    local_types,
                    constants_by_owner,
                );
            }
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => collect_external_static_string_constants_expr(
            expr,
            facts,
            local_types,
            constants_by_owner,
        ),
        Stmt::Assign { target, value } => {
            collect_external_static_string_constants_expr(
                target,
                facts,
                local_types,
                constants_by_owner,
            );
            collect_external_static_string_constants_expr(
                value,
                facts,
                local_types,
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
                constants_by_owner,
            );
            collect_external_static_string_constants_block(
                then_block,
                facts,
                local_types,
                constants_by_owner,
            );
            if let Some(else_block) = else_block {
                collect_external_static_string_constants_block(
                    else_block,
                    facts,
                    local_types,
                    constants_by_owner,
                );
            }
        }
        Stmt::While { condition, body } => {
            collect_external_static_string_constants_expr(
                condition,
                facts,
                local_types,
                constants_by_owner,
            );
            collect_external_static_string_constants_block(
                body,
                facts,
                local_types,
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
                    constants_by_owner,
                );
            }
            if let Some(condition) = condition {
                collect_external_static_string_constants_expr(
                    condition,
                    facts,
                    local_types,
                    constants_by_owner,
                );
            }
            for update in update {
                collect_external_static_string_constants_expr(
                    update,
                    facts,
                    local_types,
                    constants_by_owner,
                );
            }
            collect_external_static_string_constants_block(
                body,
                facts,
                local_types,
                constants_by_owner,
            );
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            collect_external_static_string_constants_expr(
                iterable,
                facts,
                local_types,
                constants_by_owner,
            );
            collect_external_static_string_constants_block(
                body,
                facts,
                local_types,
                constants_by_owner,
            );
        }
        Stmt::Switch { selector, cases } => {
            collect_external_static_string_constants_expr(
                selector,
                facts,
                local_types,
                constants_by_owner,
            );
            for case in cases {
                for stmt in &case.statements {
                    collect_external_static_string_constants_stmt(
                        stmt,
                        facts,
                        local_types,
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
                    constants_by_owner,
                );
            }
            collect_external_static_string_constants_block(
                body,
                facts,
                local_types,
                constants_by_owner,
            );
            for catch in catches {
                collect_external_static_string_constants_block(
                    &catch.body,
                    facts,
                    local_types,
                    constants_by_owner,
                );
            }
            if let Some(finally_block) = finally_block {
                collect_external_static_string_constants_block(
                    finally_block,
                    facts,
                    local_types,
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
                    constants_by_owner,
                );
            }
        }
        Stmt::LocalType(ty) => {
            collect_external_static_string_constants_type(
                ty,
                facts,
                local_types,
                constants_by_owner,
            );
        }
        Stmt::Block(block) => {
            collect_external_static_string_constants_block(
                block,
                facts,
                local_types,
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
    constants_by_owner: &mut BTreeMap<String, BTreeMap<String, JavaType>>,
) {
    match expr {
        Expr::FieldAccess(receiver, field) => {
            if let Some((owner, field_type)) =
                external_static_constant_owner(receiver, field, facts)
                && !local_types.contains(&owner)
            {
                constants_by_owner
                    .entry(owner)
                    .or_default()
                    .insert(canonical_external_field_name(field), field_type);
            }
            collect_external_static_string_constants_expr(
                receiver,
                facts,
                local_types,
                constants_by_owner,
            );
        }
        Expr::ArrayAccess(array, index) => {
            collect_external_static_string_constants_expr(
                array,
                facts,
                local_types,
                constants_by_owner,
            );
            collect_external_static_string_constants_expr(
                index,
                facts,
                local_types,
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
                    constants_by_owner,
                );
            }
            for value in initializer.iter().flatten() {
                collect_external_static_string_constants_expr(
                    value,
                    facts,
                    local_types,
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
                    constants_by_owner,
                );
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_external_static_string_constants_expr(
                left,
                facts,
                local_types,
                constants_by_owner,
            );
            collect_external_static_string_constants_expr(
                right,
                facts,
                local_types,
                constants_by_owner,
            );
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_external_static_string_constants_expr(
                expr,
                facts,
                local_types,
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
                constants_by_owner,
            );
            collect_external_static_string_constants_expr(
                then_expr,
                facts,
                local_types,
                constants_by_owner,
            );
            collect_external_static_string_constants_expr(
                else_expr,
                facts,
                local_types,
                constants_by_owner,
            );
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_external_static_string_constants_expr(
                    receiver,
                    facts,
                    local_types,
                    constants_by_owner,
                );
            }
            for arg in args {
                collect_external_static_string_constants_expr(
                    arg,
                    facts,
                    local_types,
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
                                constants_by_owner,
                            );
                        }
                    }
                    Member::Type(nested) => collect_external_static_string_constants_type(
                        nested,
                        facts,
                        local_types,
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
                    constants_by_owner,
                )
            }
            crate::java_ir::LambdaBody::Block(block) => {
                collect_external_static_string_constants_block(
                    block,
                    facts,
                    local_types,
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
        | Expr::Raw(_) => {}
    }
}

fn external_static_constant_owner(
    receiver: &Expr,
    field: &str,
    facts: &FactSet,
) -> Option<(String, JavaType)> {
    let receiver_name = static_constant_receiver_java_name(receiver)?;
    let lookup_owner = canonical_external_receiver_type_key(receiver_name);
    let rust_owner =
        static_constant_rust_owner(receiver_name.rsplit('.').next().unwrap_or(receiver_name))?;
    if !valid_rust_type_ident(&rust_owner) {
        return None;
    }
    let field_type = facts
        .external_field_for(&lookup_owner, field)
        .and_then(|fact| fact.field_type.clone())
        .or_else(|| {
            let receiver_type = JavaType::Named {
                name: lookup_owner.clone(),
                args: Vec::new(),
            };
            known_external_field_type_for_receiver(&receiver_type, field)
        })
        .or_else(|| {
            crate::external_packs::external_field_for(&lookup_owner, field)
                .and_then(|fact| fact.field_type)
        })?;
    static_constant_rust_type(&field_type)?;
    Some((rust_owner, field_type))
}

fn static_constant_receiver_java_name(receiver: &Expr) -> Option<&str> {
    match receiver {
        Expr::Var(receiver_name) => Some(receiver_name),
        Expr::FieldAccess(_, field) => Some(field),
        _ => None,
    }
}

fn static_constant_rust_owner(receiver_name: &str) -> Option<String> {
    if receiver_name
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
    {
        Some(receiver_name.to_string())
    } else if receiver_name.contains('_') || receiver_name.chars().all(|ch| ch.is_ascii_lowercase())
    {
        Some(upper_camel_from_snake(receiver_name))
    } else {
        None
    }
}

fn external_static_string_constant_support_item(
    owner: &str,
    fields: BTreeMap<String, JavaType>,
    local_static_string_values: &BTreeMap<(String, String), String>,
    declared_owners: &BTreeSet<String>,
) -> Option<crate::rust_ir::RustItem> {
    let consts = fields
        .into_iter()
        .filter(|(field, _)| valid_rust_const_ident(field))
        .filter_map(|(field, field_type)| {
            let rust_type = static_constant_rust_type(&field_type)?;
            let value = static_constant_default_value(
                owner,
                &field,
                &field_type,
                local_static_string_values,
            )?;
            Some(format!("    pub const {field}: {rust_type} = {value};"))
        })
        .collect::<Vec<_>>();
    if consts.is_empty() {
        return None;
    }
    let impl_block = format!("impl {owner} {{\n{}\n}}", consts.join("\n"));
    let raw = if declared_owners.contains(owner) {
        impl_block
    } else {
        format!("pub struct {owner};\n\n{impl_block}")
    };
    Some(crate::rust_ir::RustItem::Raw(raw))
}

fn static_constant_rust_type(java_ty: &JavaType) -> Option<String> {
    match java_ty {
        JavaType::String => Some("&'static str".to_string()),
        JavaType::Primitive(PrimitiveType::Boolean) => Some("bool".to_string()),
        JavaType::Primitive(PrimitiveType::Byte) => Some("i8".to_string()),
        JavaType::Primitive(PrimitiveType::Short) => Some("i16".to_string()),
        JavaType::Primitive(PrimitiveType::Int) => Some("i32".to_string()),
        JavaType::Primitive(PrimitiveType::Long) => Some("i64".to_string()),
        JavaType::Primitive(PrimitiveType::Float) => Some("f32".to_string()),
        JavaType::Primitive(PrimitiveType::Double) => Some("f64".to_string()),
        JavaType::Primitive(PrimitiveType::Char) => Some("char".to_string()),
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            let rust_type =
                rust_name_for_java_named_type(&canonical_external_receiver_type_key(name));
            valid_rust_type_ident(&rust_type).then_some(rust_type)
        }
        _ => None,
    }
}

fn static_constant_default_value(
    owner: &str,
    field: &str,
    java_ty: &JavaType,
    local_static_string_values: &BTreeMap<(String, String), String>,
) -> Option<String> {
    match java_ty {
        JavaType::String => {
            let value = local_static_string_values
                .get(&(owner.to_string(), field.to_string()))
                .cloned()
                .unwrap_or_else(|| field.to_string());
            Some(format!("{value:?}"))
        }
        JavaType::Primitive(PrimitiveType::Boolean) => Some("false".to_string()),
        JavaType::Primitive(PrimitiveType::Byte)
        | JavaType::Primitive(PrimitiveType::Short)
        | JavaType::Primitive(PrimitiveType::Int) => Some("0".to_string()),
        JavaType::Primitive(PrimitiveType::Long) => Some("0i64".to_string()),
        JavaType::Primitive(PrimitiveType::Float) => Some("0.0f32".to_string()),
        JavaType::Primitive(PrimitiveType::Double) => Some("0.0f64".to_string()),
        JavaType::Primitive(PrimitiveType::Char) => Some("'\\0'".to_string()),
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            let rust_type =
                rust_name_for_java_named_type(&canonical_external_receiver_type_key(name));
            (rust_type == owner).then_some(owner.to_string())
        }
        _ => None,
    }
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

fn valid_rust_method_ident(name: &str) -> bool {
    valid_rust_field_ident(name)
}

pub fn program_uses_java_platform_facades(program: &Program) -> bool {
    if program.types.iter().any(|ty| {
        matches!(
            crate::stdlib::canonical_external_receiver_type_key(&ty.name).as_str(),
            "Path" | "URI" | "File" | "URL"
        )
    }) {
        return false;
    }
    program.imports.iter().any(|import| {
        matches!(
            import.path.as_str(),
            "java.nio.file.Path" | "java.net.URI" | "java.io.File" | "java.net.URL"
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
        Stmt::VarDecl { initializer, .. } | Stmt::Return(initializer) => initializer
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
        Stmt::While { condition, body } => {
            expr_uses_java_platform_facades(condition) || block_uses_java_platform_facades(body)
        }
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
        Stmt::Block(block) => block_uses_java_platform_facades(block),
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
        Expr::InstanceOf { expr, .. } => expr_uses_java_platform_facades(expr),
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => expr_uses_java_platform_facades(expr),
            crate::java_ir::LambdaBody::Block(block) => block_uses_java_platform_facades(block),
        },
        Expr::New { args, body, .. } => {
            args.iter().any(expr_uses_java_platform_facades)
                || body.iter().flatten().any(member_uses_java_platform_facades)
        }
        Expr::ClassLiteral(_)
        | Expr::MethodReference { .. }
        | Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::Raw(_) => false,
    }
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
        "Path" | "URI" | "File" | "URL"
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

pub fn program_uses_java_string_format(program: &Program) -> bool {
    program
        .types
        .iter()
        .any(|ty| ty.members.iter().any(member_uses_java_string_format))
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
        Stmt::While { condition, body } => {
            expr_uses_java_string_format(condition) || block_uses_java_string_format(body)
        }
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
        Stmt::Block(block) => block_uses_java_string_format(block),
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
        | Expr::MethodReference { .. } => false,
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
        Stmt::While { condition, body } => {
            expr_uses_java_string_split(condition) || block_uses_java_string_split(body)
        }
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
        Stmt::Block(block) => block_uses_java_string_split(block),
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
        | Expr::MethodReference { .. } => false,
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
        Stmt::While { condition, body } => {
            expr_uses_java_class_tokens(condition) || block_uses_java_class_tokens(body)
        }
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
        Stmt::Block(block) => block_uses_java_class_tokens(block),
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
        | Expr::Raw(_) => false,
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
        (Some(owner), "getMessage", 0) if is_known_throwable_type(owner) => {
            NullabilityState::MaybeNull
        }
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
        (Some("System"), "getProperty", 1 | 2) => NullabilityState::MaybeNull,
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
    matches!((name, arg_index), ("addAll", 0) | ("containsAll", 0))
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
        (Some("String"), "split", 1 | 2) => ExternalApiRole::Getter,
        (Some("String"), "substring", 1 | 2) => ExternalApiRole::Getter,
        (Some("String" | "StringUtils" | "StringUtil"), "join", _) => ExternalApiRole::Getter,
        (Some("Long"), "toBinaryString", 1) => ExternalApiRole::Getter,
        (Some("StringUtils"), "isEmpty", 1) => ExternalApiRole::BooleanGetter,
        (Some("StringUtils"), "split", 2) => ExternalApiRole::Getter,
        (Some("Integer"), "parseInt", 1 | 2) => ExternalApiRole::Getter,
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
        (Some("System"), "getProperty", 1 | 2) => ExternalApiRole::Getter,
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
