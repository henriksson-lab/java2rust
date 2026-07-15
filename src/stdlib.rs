use crate::facts::{ExternalApiRole, FactProvenance, NullabilityState, ReceiverMutabilityFact};
use crate::java_ir::{JavaType, PrimitiveType};

pub fn is_known_jdk_shadow_sensitive_owner(name: &str) -> bool {
    matches!(name, "Objects" | "Collections" | "Integer" | "Paths")
}

pub fn is_content_read_method(name: &str) -> bool {
    matches!(name, "length" | "isEmpty" | "trim")
}

pub fn is_borrowed_string_view_method(name: &str, arg_count: usize) -> bool {
    name == "trim" && arg_count == 0
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
        "Exception" => matches!(
            actual,
            "Exception"
                | "RuntimeException"
                | "IOException"
                | "FileNotFoundException"
                | "EOFException"
                | "IllegalArgumentException"
                | "IllegalStateException"
                | "NotImplementedException"
                | "NumberFormatException"
                | "ArrayIndexOutOfBoundsException"
                | "IndexOutOfBoundsException"
                | "UnsupportedOperationException"
                | "SAMException"
                | "StorageException"
                | "UncheckedIOException"
                | "RuntimeIOException"
        ),
        "Error" => matches!(
            actual,
            "Error" | "OutOfMemoryError" | "StackOverflowError" | "AssertionError"
        ),
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
                | "NotImplementedException"
                | "NumberFormatException"
                | "ArrayIndexOutOfBoundsException"
                | "IndexOutOfBoundsException"
                | "ConcurrentModificationException"
                | "UnsupportedOperationException"
                | "SAMException"
                | "StorageException"
                | "UncheckedIOException"
                | "RuntimeIOException"
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
        "Locatable" => is_known_locatable_type(actual),
        "Feature" => is_known_feature_type(actual),
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
    matches!(
        name,
        "Locatable" | "Feature" | "Iterable" | "Comparable" | "Serializable"
    )
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

fn is_known_locatable_type(name: &str) -> bool {
    matches!(
        name,
        "Locatable"
            | "VariantContext"
            | "GATKVariant"
            | "VariantContextVariantAdapter"
            | "SimpleInterval"
            | "Interval"
            | "GenomeLoc"
            | "SAMRecord"
            | "SAMSequenceRecord"
            | "Feature"
            | "SimpleFeature"
            | "NamedFeature"
            | "BEDFeature"
            | "TableFeature"
            | "GencodeGtfFeature"
            | "GencodeGtfGeneFeature"
            | "GencodeGtfTranscriptFeature"
            | "GencodeGtfExonFeature"
            | "GencodeGtfCDSFeature"
            | "GencodeGtfStartCodonFeature"
            | "GencodeGtfStopCodonFeature"
            | "GencodeGtfUTRFeature"
            | "GencodeGtfSelenocysteineFeature"
    )
}

fn is_known_feature_type(name: &str) -> bool {
    matches!(
        name,
        "Feature"
            | "VariantContext"
            | "GATKVariant"
            | "VariantContextVariantAdapter"
            | "SimpleFeature"
            | "NamedFeature"
            | "BEDFeature"
            | "TableFeature"
            | "GencodeGtfFeature"
            | "GencodeGtfGeneFeature"
            | "GencodeGtfTranscriptFeature"
            | "GencodeGtfExonFeature"
            | "GencodeGtfCDSFeature"
            | "GencodeGtfStartCodonFeature"
            | "GencodeGtfStopCodonFeature"
            | "GencodeGtfUTRFeature"
            | "GencodeGtfSelenocysteineFeature"
    )
}

fn is_known_write_option_type(name: &str) -> bool {
    matches!(
        name.rsplit('.').next().unwrap_or(name),
        "WriteOption"
            | "ReadsFormatWriteOption"
            | "FileCardinalityWriteOption"
            | "TempPartsDirectoryWriteOption"
            | "BaiWriteOption"
            | "SbiWriteOption"
    )
}

fn is_known_reference_type(name: &str) -> bool {
    !matches!(
        name,
        "boolean" | "byte" | "short" | "char" | "int" | "long" | "float" | "double" | "void"
    )
}

fn is_known_throwable_type(name: &str) -> bool {
    matches!(
        name,
        "Throwable"
            | "Exception"
            | "RuntimeException"
            | "IOException"
            | "FileNotFoundException"
            | "EOFException"
            | "IllegalArgumentException"
            | "IllegalStateException"
            | "NoSuchElementException"
            | "NotImplementedException"
            | "NumberFormatException"
            | "ArrayIndexOutOfBoundsException"
            | "IndexOutOfBoundsException"
            | "ConcurrentModificationException"
            | "UnsupportedOperationException"
            | "SAMException"
            | "StorageException"
            | "UncheckedIOException"
            | "Error"
            | "OutOfMemoryError"
            | "StackOverflowError"
            | "AssertionError"
    )
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
        (Some("Allele"), "create", 1 | 2) => Some(JavaType::Named {
            name: "Allele".to_string(),
            args: Vec::new(),
        }),
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
        (Some("StringUtils"), "isEmpty", 1) => Some(JavaType::Primitive(PrimitiveType::Boolean)),
        (Some("StringUtils"), "split", 2) => Some(JavaType::Array(Box::new(JavaType::String))),
        (Some("Utils"), "split", 2 | 3) => Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::String],
        }),
        (Some("IOUtils"), "replaceExtension", 2) => Some(JavaType::String),
        (Some("Integer"), "parseInt", 1 | 2) => Some(JavaType::Primitive(PrimitiveType::Int)),
        (Some("QualityUtils"), "qualToProb" | "qualToProbLog10", 1) => {
            Some(JavaType::Primitive(PrimitiveType::Double))
        }
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
        (_, "getPhredScaledQual", 0) => Some(JavaType::Primitive(PrimitiveType::Double)),
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
        known_argument_sensitive_external_method_return_type(receiver_type, name, arg_types)
    {
        return Some(java_ty);
    }
    known_external_method_return_type_for_receiver(receiver_type, name, arg_types.len())
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
        && let Some(java_ty) = known_external_nested_enum_type(&receiver, field_name)
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

fn canonical_external_field_name(field_name: &str) -> String {
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
    let receiver = receiver.rsplit('.').next().unwrap_or(receiver);
    if receiver == "ReadEnds"
        && matches!(field_name.as_str(), "FF" | "FR" | "RF" | "RR" | "F" | "R")
    {
        return Some(JavaType::Primitive(PrimitiveType::Byte));
    }
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
    if receiver == "SAMRecord"
        && let Some(java_ty) = sam_record_static_field_type(&field_name)
    {
        return Some(java_ty);
    }
    if receiver == "BlockCompressedStreamConstants"
        && let Some(java_ty) = block_compressed_stream_constant_field_type(&field_name)
    {
        return Some(java_ty);
    }
    if receiver == "VCFConstants"
        && let Some(java_ty) = vcf_constant_field_type(&field_name)
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
    if receiver == "Allele"
        && matches!(
            field_name.as_str(),
            "NON_REF_STRING" | "UNSPECIFIED_ALTERNATE_ALLELE_STRING"
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
    if receiver == "GenotypeLikelihoods"
        && matches!(
            field_name.as_str(),
            "MAX_DIPLOID_ALT_ALLELES_THAT_CAN_BE_GENOTYPED"
        )
    {
        return Some(JavaType::Primitive(PrimitiveType::Int));
    }
    if receiver == "CommonInfo" && matches!(field_name.as_str(), "NO_LOG10_PERROR") {
        return Some(JavaType::Primitive(PrimitiveType::Double));
    }
    if receiver == "VariantContext" && matches!(field_name.as_str(), "NO_LOG10_PERROR") {
        return Some(JavaType::Primitive(PrimitiveType::Double));
    }
    if receiver == "SpecialArgumentsCollection" && matches!(field_name.as_str(), "SHOW_HIDDEN") {
        return Some(JavaType::Primitive(PrimitiveType::Boolean));
    }
    if receiver == "TarArchiveOutputStream"
        && matches!(field_name.as_str(), "BIGNUMBER_STAR" | "LONGFILE_GNU")
    {
        return Some(JavaType::Primitive(PrimitiveType::Int));
    }
    if receiver == "SBIIndexWriter" && matches!(field_name.as_str(), "DEFAULT_GRANULARITY") {
        return Some(JavaType::Primitive(PrimitiveType::Long));
    }
    if receiver == "Math" && matches!(field_name.as_str(), "PI" | "E") {
        return Some(JavaType::Primitive(PrimitiveType::Double));
    }
    if receiver == "SystemUtils" {
        match field_name.as_str() {
            "OS_ARCH" | "OS_NAME" => return Some(JavaType::String),
            "IS_OS_MAC" | "IS_OS_LINUX" | "IS_OS_WINDOWS" => {
                return Some(JavaType::Primitive(PrimitiveType::Boolean));
            }
            _ => {}
        }
    }
    if receiver == "Defaults" {
        match field_name.as_str() {
            "COMPRESSION_LEVEL" => return Some(JavaType::Primitive(PrimitiveType::Int)),
            "USE_ASYNC_IO_READ_FOR_SAMTOOLS"
            | "USE_ASYNC_IO_WRITE_FOR_SAMTOOLS"
            | "USE_ASYNC_IO_WRITE_FOR_TRIBBLE" => {
                return Some(JavaType::Primitive(PrimitiveType::Boolean));
            }
            _ => {}
        }
    }
    if receiver == "RetryHandler"
        && matches!(
            field_name.as_str(),
            "DEFAULT_RETRYABLE_HTTP_CODES"
                | "DEFAULT_RETRYABLE_EXCEPTIONS"
                | "DEFALT_RETRYABLE_MESSAGES"
        )
    {
        return Some(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        });
    }
    if receiver == "BwaMemAligner" && matches!(field_name.as_str(), "MEM_F_ALL") {
        return Some(JavaType::Primitive(PrimitiveType::Int));
    }
    if receiver == "FeatureCodecHeader" && matches!(field_name.as_str(), "NO_HEADER_END") {
        return Some(JavaType::Primitive(PrimitiveType::Long));
    }
    if receiver == "MessageDigestAlgorithms" && matches!(field_name.as_str(), "SHA_256") {
        return Some(JavaType::String);
    }
    if receiver == "SAMReadGroupRecord"
        && matches!(
            field_name.as_str(),
            "READ_GROUP_ID_TAG" | "READ_GROUP_SAMPLE_TAG"
        )
    {
        return Some(JavaType::String);
    }
    if receiver == "WDLWorkUnitHandler" && matches!(field_name.as_str(), "POSITIONAL_ARGS") {
        return Some(JavaType::String);
    }
    if receiver == "Genotype" && matches!(field_name.as_str(), "UNPHASED_ALLELE_SEPARATOR") {
        return Some(JavaType::Primitive(PrimitiveType::Char));
    }
    if receiver == "Spliterator"
        && matches!(
            field_name.as_str(),
            "IMMUTABLE" | "NONNULL" | "SIZED" | "SUBSIZED" | "ORDERED" | "SORTED" | "DISTINCT"
        )
    {
        return Some(JavaType::Primitive(PrimitiveType::Int));
    }
    if receiver == "SAMUtils" && matches!(field_name.as_str(), "MAX_PHRED_SCORE") {
        return Some(JavaType::Primitive(PrimitiveType::Int));
    }
    if receiver == "SAMSequenceRecord" && matches!(field_name.as_str(), "UNKNOWN_SEQUENCE_LENGTH") {
        return Some(JavaType::Primitive(PrimitiveType::Int));
    }
    if receiver == "TemplateProperties"
        && matches!(
            field_name.as_str(),
            "ARGUMENTS_POSITIONAL" | "ARGUMENT_DEFAULT_VALUE" | "ARGUMENT_TYPE"
        )
    {
        return Some(JavaType::String);
    }
    if receiver == "GenotypeConcordanceSummaryMetrics"
        && matches!(
            field_name.as_str(),
            "GENOTYPE_CONCORDANCE"
                | "NON_REF_GENOTYPE_CONCORDANCE"
                | "HET_PPV"
                | "HET_SENSITIVITY"
                | "HOMVAR_PPV"
                | "HOMVAR_SENSITIVITY"
                | "VAR_PPV"
                | "VAR_SENSITIVITY"
                | "VAR_SPECIFICITY"
        )
    {
        return Some(JavaType::String);
    }
    if receiver == "VCFHeader" {
        match field_name.as_str() {
            "REFERENCE_KEY" => return Some(JavaType::String),
            _ => {}
        }
    }
    if receiver == "ReservedTagConstants" && matches!(field_name.as_str(), "XT") {
        return Some(JavaType::String);
    }
    if receiver == "SequenceUtil" && matches!(field_name.as_str(), "VALID_BASES_UPPER") {
        return Some(JavaType::Array(Box::new(JavaType::Primitive(
            PrimitiveType::Byte,
        ))));
    }
    if receiver == "BAMFileConstants" && matches!(field_name.as_str(), "FIXED_BLOCK_SIZE") {
        return Some(JavaType::Primitive(PrimitiveType::Int));
    }
    if receiver == "GATKConfig" && matches!(field_name.as_str(), "CONFIG_FILE_VARIABLE_FILE_NAME") {
        return Some(JavaType::String);
    }
    if receiver == "CloudStorageFileSystem" && matches!(field_name.as_str(), "URI_SCHEME") {
        return Some(JavaType::String);
    }
    if matches!(
        receiver,
        "HttpFileSystemProvider" | "HttpsFileSystemProvider"
    ) && matches!(field_name.as_str(), "SCHEME")
    {
        return Some(JavaType::String);
    }
    if receiver == "ReferenceContext" && matches!(field_name.as_str(), "UNINITIALIZED_REFERENCE_ID")
    {
        return Some(JavaType::Primitive(PrimitiveType::Int));
    }
    if receiver == "Slice" && matches!(field_name.as_str(), "EMBEDDED_REFERENCE_ABSENT_CONTENT_ID")
    {
        return Some(JavaType::Primitive(PrimitiveType::Int));
    }
    if receiver == "TabixFormat" && matches!(field_name.as_str(), "GENERIC_FLAGS" | "ZERO_BASED") {
        return Some(JavaType::Primitive(PrimitiveType::Int));
    }
    if receiver == "GenotypeConcordance" && matches!(field_name.as_str(), "CONTINGENCY_STATE_TAG") {
        return Some(JavaType::String);
    }
    if receiver == "ReadLikelihoodCalculationEngine" {
        match field_name.as_str() {
            "MAX_REPEAT_LENGTH" => return Some(JavaType::Primitive(PrimitiveType::Int)),
            "DEFAULT_EXPECTED_ERROR_RATE_PER_BASE" => {
                return Some(JavaType::Primitive(PrimitiveType::Double));
            }
            _ => {}
        }
    }
    if receiver == "VariantContextWriterBuilder" && matches!(field_name.as_str(), "DEFAULT_OPTIONS")
    {
        return Some(JavaType::Named {
            name: "EnumSet".to_string(),
            args: vec![JavaType::Named {
                name: "Options".to_string(),
                args: Vec::new(),
            }],
        });
    }
    if receiver == "HttpFileSystemProviderSettings"
        && matches!(field_name.as_str(), "DEFAULT_SETTINGS")
    {
        return Some(JavaType::Named {
            name: "HttpFileSystemProviderSettings".to_string(),
            args: Vec::new(),
        });
    }
    if matches!(receiver, "FastqConstants" | "ReservedTagConstants")
        && matches!(
            field_name.as_str(),
            "FIRST_OF_PAIR" | "SECOND_OF_PAIR" | "PROGRAM_GROUP_ID" | "READ_GROUP_ID"
        )
    {
        return Some(JavaType::String);
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
        ("ReadsFormatWriteOption", "BAM") => "ReadsFormatWriteOption",
        ("Option", "CACHE_FILE_BASED_INDEXES" | "INCLUDE_SOURCE_IN_RECORDS") => "Option",
        ("SupportedHeaderLineType", "FORMAT" | "INFO") => "SupportedHeaderLineType",
        ("Level", "DEBUG" | "INFO" | "ERROR") => "Level",
        ("Locale", "ENGLISH") => "Locale",
        ("ElementKind", "FIELD") => "ElementKind",
        ("IndexType", "TABIX") => "IndexType",
        ("IndexBalanceApproach", "FOR_SEEK_TIME") => "IndexBalanceApproach",
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
        ("MapperFeature", "ACCEPT_CASE_INSENSITIVE_PROPERTIES") => "MapperFeature",
        ("DataFormat", "AVRO" | "ARROW") => "DataFormat",
        ("DataSeriesType", "BYTE_ARRAY") => "DataSeriesType",
        ("CloudStorageConfiguration", "DEFAULT") => "CloudStorageConfiguration",
        ("NullOutputStream", "INSTANCE" | "NULL_OUTPUT_STREAM") => "OutputStream",
        ("NullWriter", "INSTANCE") => "Writer",
        ("ValidationStringency", "LENIENT" | "SILENT" | "STRICT") => "ValidationStringency",
        ("FileCardinalityWriteOption", "MULTIPLE" | "SINGLE") => "FileCardinalityWriteOption",
        ("GenotypesContext", "NO_GENOTYPES") => "GenotypesContext",
        ("DeserializationFeature", "UNWRAP_ROOT_VALUE") => "DeserializationFeature",
        ("JexlMissingValueTreatment", "TREAT_AS_MATCH" | "TREAT_AS_MISMATCH") => {
            "JexlMissingValueTreatment"
        }
        ("TimeUnit", "SECONDS" | "MILLISECONDS" | "MICROSECONDS" | "NANOSECONDS") => "TimeUnit",
        _ => return None,
    };
    Some(JavaType::Named {
        name: named.to_string(),
        args: Vec::new(),
    })
}

fn block_compressed_stream_constant_field_type(field_name: &str) -> Option<JavaType> {
    match field_name {
        "BLOCK_HEADER_LENGTH" | "BLOCK_LENGTH_OFFSET" | "DEFAULT_COMPRESSION_LEVEL" => {
            Some(JavaType::Primitive(PrimitiveType::Int))
        }
        "EMPTY_GZIP_BLOCK" => Some(JavaType::Array(Box::new(JavaType::Primitive(
            PrimitiveType::Byte,
        )))),
        _ => None,
    }
}

fn vcf_constant_field_type(field_name: &str) -> Option<JavaType> {
    match field_name {
        "PASSES_FILTERS_V4"
        | "MISSING_VALUE_V4"
        | "GENOTYPE_ALLELE_DEPTHS"
        | "SVTYPE"
        | "NULL_ALLELE"
        | "UNFILTERED" => Some(JavaType::String),
        "INFO_FIELD_ARRAY_SEPARATOR_CHAR" => Some(JavaType::Primitive(PrimitiveType::Char)),
        "MAX_GENOTYPE_QUAL" => Some(JavaType::Primitive(PrimitiveType::Int)),
        _ => None,
    }
}

fn sam_record_static_field_type(field_name: &str) -> Option<JavaType> {
    match field_name {
        "NO_ALIGNMENT_CIGAR" => Some(JavaType::Named {
            name: "Cigar".to_string(),
            args: Vec::new(),
        }),
        "NO_ALIGNMENT_REFERENCE_NAME" | "NULL_QUALS_STRING" => Some(JavaType::String),
        "NULL_QUALS" => Some(JavaType::Array(Box::new(JavaType::Primitive(
            PrimitiveType::Byte,
        )))),
        "NULL_SEQUENCE" => Some(JavaType::Array(Box::new(JavaType::Primitive(
            PrimitiveType::Byte,
        )))),
        "NO_ALIGNMENT_START"
        | "NO_ALIGNMENT_REFERENCE_INDEX"
        | "UNKNOWN_MAPPING_QUALITY"
        | "NO_MAPPING_QUALITY" => Some(JavaType::Primitive(PrimitiveType::Int)),
        _ => None,
    }
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

fn known_external_nested_enum_type(receiver: &str, field_name: &str) -> Option<JavaType> {
    let nested_name = match (receiver.rsplit('.').next().unwrap_or(receiver), field_name) {
        ("Log", "LogLevel") => "LogLevel",
        ("HDF5File", "OpenMode") => "OpenMode",
        ("SAMFileHeader", "SortOrder") => "SortOrder",
        ("SAMFileHeader", "GroupOrder") => "GroupOrder",
        ("FieldValue", "Attribute") => "Attribute",
        ("QueryJobConfiguration", "Priority") => "Priority",
        ("BlockCompressedInputStream", "FileTermination") => "FileTermination",
        ("Percentile", "EstimationType") => "EstimationType",
        ("VariantContextWriterBuilder", "OutputType") => "OutputType",
        ("VCFHeader", "HEADER_FIELDS") => "HEADER_FIELDS",
        ("Coordinates", "GenomicsDBColumn") => "GenomicsDBColumn",
        ("Coordinates", "ContigPosition") => "ContigPosition",
        ("GenotypeConcordanceStates", "CallState") => "CallState",
        ("GenotypeConcordanceStates", "TruthState") => "TruthState",
        ("VariantContext", "Type") => "Type",
        ("MarkDuplicates", "DuplicateTaggingPolicy") => "DuplicateTaggingPolicy",
        ("SamPairUtil", "PairOrientation") => "PairOrientation",
        ("GenomicsDBExportConfiguration", "ExportConfiguration") => "ExportConfiguration",
        ("Storage", "SignUrlOption") => "SignUrlOption",
        ("IndexFactory", "IndexType") => "IndexType",
        ("IndexFactory", "IndexBalanceApproach") => "IndexBalanceApproach",
        ("SamReaderFactory", "Option") => "Option",
        ("VCFCompoundHeaderLine", "SupportedHeaderLineType") => "SupportedHeaderLineType",
        ("GenomicsDBImportConfiguration", "ImportConfiguration") => "ImportConfiguration",
        ("GenomicsDBImportConfiguration", "Partition") => "Partition",
        ("GenomicsDBVidMapProto", "VidMappingPB") => "VidMappingPB",
        ("GenomicsDBVidMapProto", "FieldLengthDescriptorComponentPB") => {
            "FieldLengthDescriptorComponentPB"
        }
        ("ReadSession", "TableReadOptions") => "TableReadOptions",
        _ => return None,
    };
    Some(JavaType::Named {
        name: nested_name.to_string(),
        args: Vec::new(),
    })
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
        ("DuplicationMetrics", "LIBRARY") => Some(JavaType::String),
        (
            "DuplicationMetrics",
            "UNPAIRED_READS_EXAMINED"
            | "READ_PAIRS_EXAMINED"
            | "SECONDARY_OR_SUPPLEMENTARY_RDS"
            | "UNMAPPED_READS"
            | "UNPAIRED_READ_DUPLICATES"
            | "READ_PAIR_DUPLICATES"
            | "READ_PAIR_OPTICAL_DUPLICATES"
            | "ESTIMATED_LIBRARY_SIZE",
        ) => Some(JavaType::Primitive(PrimitiveType::Long)),
        ("DuplicationMetrics", "PERCENT_DUPLICATION") => {
            Some(JavaType::Primitive(PrimitiveType::Double))
        }
        ("JexlVCMatchExp", "name") => Some(JavaType::String),
        ("JexlVCMatchExp", "exp") => Some(JavaType::Named {
            name: "Expression".to_string(),
            args: Vec::new(),
        }),
        ("SAMTagAndValue", "tag") => Some(JavaType::String),
        ("SAMTagAndValue", "value") => Some(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        }),
        ("SAMBinaryTagAndValue", "value") => Some(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        }),
        ("SAMBinaryTagAndValue", "tag") => Some(JavaType::String),
        (
            "ReadDataHolder",
            "readBases" | "readQuals" | "insertionGOP" | "deletionGOP" | "overallGCP",
        ) => Some(JavaType::Array(Box::new(JavaType::Primitive(
            PrimitiveType::Byte,
        )))),
        ("HaplotypeDataHolder", "haplotypeBases" | "haplotypePDBases") => Some(JavaType::Array(
            Box::new(JavaType::Primitive(PrimitiveType::Byte)),
        )),
        ("PDHMMNativeArguments", "avxLevel") => Some(JavaType::Named {
            name: "AVXLevel".to_string(),
            args: Vec::new(),
        }),
        ("PDHMMNativeArguments", "openMPSetting") => Some(JavaType::Named {
            name: "OpenMPSetting".to_string(),
            args: Vec::new(),
        }),
        ("PDHMMNativeArguments", "maxNumberOfThreads")
        | ("PairHMMNativeArguments", "maxNumberOfThreads") => {
            Some(JavaType::Primitive(PrimitiveType::Int))
        }
        ("PairHMMNativeArguments", "useDoublePrecision") => {
            Some(JavaType::Primitive(PrimitiveType::Boolean))
        }
        ("GenotypeLikelihoodsAllelePair", "alleleIndex1" | "alleleIndex2") => {
            Some(JavaType::Primitive(PrimitiveType::Int))
        }
        ("TruthAndCallStates", "callState") => Some(JavaType::Named {
            name: "CallState".to_string(),
            args: Vec::new(),
        }),
        ("TruthAndCallStates", "truthState") => Some(JavaType::Named {
            name: "TruthState".to_string(),
            args: Vec::new(),
        }),
        ("SWNativeAlignerResult", "alignment_offset") => {
            Some(JavaType::Primitive(PrimitiveType::Int))
        }
        ("SWNativeAlignerResult", "cigar") => Some(JavaType::String),
        ("BatchCompletionCallbackFunctionArgument", "batchCount" | "totalBatchCount") => {
            Some(JavaType::Primitive(PrimitiveType::Int))
        }
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
        "VCFConstants" | "GATKVCFConstants" => {
            field_name.ends_with("_KEY")
                || field_name.ends_with("_FIELD")
                || field_name.ends_with("_SEPARATOR")
        }
        "FileExtensions" => true,
        _ => false,
    }
}

fn known_argument_sensitive_external_method_return_type(
    receiver_type: Option<&JavaType>,
    name: &str,
    arg_types: &[JavaType],
) -> Option<JavaType> {
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

    let receiver_key = receiver_type.and_then(external_receiver_type_key);
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
        (Some("Utils"), "stream", [source]) => {
            collection_element_type(source).map(|element| stream_return_type(element.clone()))
        }
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

fn is_enum_like_external_constant(receiver: &str, field_name: &str) -> bool {
    let original_field_name = field_name;
    let field_name = canonical_external_field_name(field_name);
    match receiver.rsplit('.').next().unwrap_or(receiver) {
        "Allele" => is_allele_constant(&field_name) || is_enum_constant_name(&field_name),
        "CigarOperator" => is_cigar_operator_constant(&field_name),
        "SAMTag" => is_sam_tag_constant(&field_name),
        "OpenMode"
        | "LogLevel"
        | "VCFHeaderLineCount"
        | "GenotypeType"
        | "Strand"
        | "CallState"
        | "TruthState"
        | "Type"
        | "SAMFlag"
        | "VCFHeaderVersion"
        | "PairOrientation"
        | "Options"
        | "SWOverhangStrategy"
        | "Attribute"
        | "Priority"
        | "FileTermination"
        | "GroupOrder"
        | "StructuralVariantType"
        | "GoalType"
        | "EstimationType"
        | "Characteristics"
        | "HEADER_FIELDS"
        | "ReadsFormatWriteOption"
        | "TimeUnit"
        | "OutputType"
        | "Level"
        | "Locale"
        | "ElementKind"
        | "IndexType"
        | "IndexBalanceApproach" => is_enum_constant_name(&field_name),
        "DuplicateTaggingPolicy" => {
            is_duplicate_tagging_policy_constant(original_field_name)
                || is_duplicate_tagging_policy_constant(&field_name)
        }
        "VCFHeaderLineType" => is_vcf_header_line_type_constant(original_field_name),
        "SortOrder" => {
            is_sort_order_constant(original_field_name) || is_sort_order_constant(&field_name)
        }
        _ => false,
    }
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
    matches!(
        receiver.rsplit('.').next().unwrap_or(receiver),
        "Allele"
            | "CigarOperator"
            | "SAMTag"
            | "OpenMode"
            | "LogLevel"
            | "SortOrder"
            | "VCFHeaderLineType"
            | "VCFHeaderLineCount"
            | "GenotypeType"
            | "Strand"
            | "Characteristics"
            | "HEADER_FIELDS"
            | "CallState"
            | "TruthState"
            | "Type"
            | "SAMFlag"
            | "VCFHeaderVersion"
            | "DuplicateTaggingPolicy"
            | "PairOrientation"
            | "Options"
            | "SWOverhangStrategy"
            | "Attribute"
            | "Priority"
            | "FileTermination"
            | "GroupOrder"
            | "StructuralVariantType"
            | "GoalType"
            | "EstimationType"
            | "ReadsFormatWriteOption"
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

fn is_duplicate_tagging_policy_constant(field_name: &str) -> bool {
    matches!(field_name, "DontTag" | "OpticalOnly" | "All")
}

fn is_sort_order_constant(field_name: &str) -> bool {
    matches!(
        field_name,
        "coordinate" | "queryname" | "unsorted" | "unknown" | "duplicate"
    )
}

fn is_allele_constant(field_name: &str) -> bool {
    matches!(
        field_name,
        "UNSPECIFIED_ALTERNATE_ALLELE" | "NO_CALL" | "NON_REF_ALLELE" | "SPAN_DEL"
    )
}

fn is_cigar_operator_constant(field_name: &str) -> bool {
    matches!(
        field_name,
        "M" | "I"
            | "D"
            | "N"
            | "S"
            | "H"
            | "P"
            | "EQ"
            | "X"
            | "MATCH"
            | "MATCH_OR_MISMATCH"
            | "INSERTION"
            | "DELETION"
            | "SKIPPED_REGION"
            | "SOFT_CLIP"
            | "HARD_CLIP"
            | "PADDING"
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
        "String" | "Utils" | "StringUtils" | "StringUtil"
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
            | "getBaseString"
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
        "utils" => "Utils",
        "quality_utils" => "QualityUtils",
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
        "g_a_t_k_v_c_f_constants" => "GATKVCFConstants",
        "constants" => "Constants",
        "file_extensions" => "FileExtensions",
        "i_o_utils" => "IOUtils",
        "h_d_f5_file" => "HDF5File",
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
        "variant_context" => "VariantContext",
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
        "genomics_d_b_column" => "GenomicsDBColumn",
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
        "variant_context_writer_builder" => "VariantContextWriterBuilder",
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
        "genomics_d_b_export_configuration" => "GenomicsDBExportConfiguration",
        "export_configuration" => "ExportConfiguration",
        "read_likelihood_calculation_engine" => "ReadLikelihoodCalculationEngine",
        "special_arguments_collection" => "SpecialArgumentsCollection",
        "http_method" => "HttpMethod",
        "mapper_feature" => "MapperFeature",
        "data_format" => "DataFormat",
        "data_series_type" => "DataSeriesType",
        "cloud_storage_configuration" => "CloudStorageConfiguration",
        "null_output_stream" => "NullOutputStream",
        "null_writer" => "NullWriter",
        "validation_stringency" => "ValidationStringency",
        "index_factory" => "IndexFactory",
        "file_cardinality_write_option" => "FileCardinalityWriteOption",
        "genotypes_context" => "GenotypesContext",
        "feature_codec_header" => "FeatureCodecHeader",
        "message_digest_algorithms" => "MessageDigestAlgorithms",
        "s_a_m_read_group_record" => "SAMReadGroupRecord",
        "deserialization_feature" => "DeserializationFeature",
        "jexl_missing_value_treatment" => "JexlMissingValueTreatment",
        "w_d_l_work_unit_handler" => "WDLWorkUnitHandler",
        "genotype" => "Genotype",
        "genomics_d_b_import_configuration" => "GenomicsDBImportConfiguration",
        "genomics_d_b_vid_map_proto" => "GenomicsDBVidMapProto",
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
    let role = external_api_method_role(receiver_type, name, arg_count);
    ExternalApiMethodPolicy {
        role,
        receiver_mutability: external_api_receiver_mutability(role),
        return_nullability: external_api_return_nullability(receiver_type, name, arg_count, role),
        provenance: external_api_method_policy_provenance(receiver_type, name, arg_count),
    }
}

pub fn external_constructor_provenance(type_name: &str, arg_types: &[JavaType]) -> FactProvenance {
    if is_known_collection_constructor(type_name, arg_types)
        || is_known_map_constructor(type_name, arg_types)
        || is_known_throwable_constructor(type_name, arg_types)
        || is_known_io_wrapper_constructor(type_name, arg_types)
        || is_known_jdk_utility_constructor(type_name, arg_types)
        || is_known_file_constructor(type_name, arg_types)
        || is_known_format_constructor(type_name, arg_types)
        || is_known_htsjdk_constructor(type_name, arg_types)
        || is_known_apache_commons_constructor(type_name, arg_types)
        || is_known_apache_commons_collections_constructor(type_name, arg_types)
        || is_known_apache_commons_compress_constructor(type_name, arg_types)
        || is_known_apache_commons_lang_constructor(type_name, arg_types)
        || is_known_apache_commons_configuration_constructor(type_name, arg_types)
        || is_known_apache_http_constructor(type_name, arg_types)
        || is_known_jama_constructor(type_name, arg_types)
        || is_known_hipparchus_constructor(type_name, arg_types)
        || is_known_guava_constructor(type_name, arg_types)
        || is_known_reflections_constructor(type_name, arg_types)
        || is_known_barclay_constructor(type_name, arg_types)
        || is_known_fastutil_constructor(type_name, arg_types)
        || is_known_picard_constructor(type_name, arg_types)
        || is_known_disq_constructor(type_name, arg_types)
        || is_known_broad_http_constructor(type_name, arg_types)
        || is_known_json_constructor(type_name, arg_types)
        || is_known_intel_gkl_constructor(type_name, arg_types)
        || is_known_hadoop_constructor(type_name, arg_types)
        || is_known_opencsv_constructor(type_name, arg_types)
        || is_known_genomicsdb_constructor(type_name, arg_types)
        || is_known_avro_constructor(type_name, arg_types)
        || is_known_hdf5_constructor(type_name, arg_types)
        || is_known_kryo_constructor(type_name, arg_types)
        || is_known_bwa_constructor(type_name, arg_types)
        || is_known_jgrapht_constructor(type_name, arg_types)
        || is_known_spark_constructor(type_name, arg_types)
        || is_known_sqlite_constructor(type_name, arg_types)
        || is_known_string_builder_constructor(type_name, arg_types)
        || is_known_tuple_constructor(type_name, arg_types)
    {
        FactProvenance::BuiltinExternalModel
    } else {
        FactProvenance::ConservativeFallback
    }
}

pub fn external_field_provenance(type_name: &str, field_name: &str) -> FactProvenance {
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

fn is_known_htsjdk_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    match type_name {
        "VCFInfoHeaderLine" | "VCFFormatHeaderLine" => {
            matches!(arg_types.len(), 3 | 4)
        }
        "VCFFilterHeaderLine" => arg_types.len() == 2,
        "VCFHeaderLine" => arg_types.len() == 2,
        "VCFHeader" => arg_types.len() <= 2,
        "VCFCodec" => arg_types.is_empty(),
        "VCFRecordCodec" => arg_types.len() == 2,
        "CigarElement" => arg_types.len() == 2,
        "Cigar" => arg_types.len() <= 1,
        "GenotypeBuilder" => arg_types.len() <= 2,
        "VariantContextComparator" => arg_types.len() == 1,
        "VariantContextBuilder" => matches!(arg_types.len(), 0 | 1 | 5),
        "SeekableBufferedStream" => arg_types.len() == 1,
        "SeekablePathStream" => arg_types.len() == 1,
        "BAMRecordCodec" => arg_types.len() == 1,
        "BlockCompressedInputStream" => matches!(arg_types.len(), 1 | 2),
        "BlockCompressedOutputStream" => matches!(arg_types.len(), 2 | 3),
        "BAMIndexer" => arg_types.len() == 2,
        "BCF2Codec" => arg_types.is_empty(),
        "FeatureCodecHeader" => arg_types.len() == 2,
        "BinaryCodec" => arg_types.len() <= 1,
        "SAMTextHeaderCodec" => arg_types.is_empty(),
        "MetricsFile" => arg_types.is_empty(),
        "BufferedLineReader" => arg_types.len() == 1,
        "AsciiLineReaderIterator" => arg_types.len() == 1,
        "LineIteratorImpl" => arg_types.len() == 1,
        "SynchronousLineReader" => arg_types.len() == 1,
        "DefaultSAMRecordFactory" => arg_types.is_empty(),
        "ComponentMethods" => arg_types.is_empty(),
        "FilteringSamIterator" => arg_types.len() == 2,
        "FastaSequenceFile" => arg_types.len() == 2,
        "CramContainerIterator" => arg_types.len() == 1,
        "Container" => arg_types.len() == 3,
        "SAMRecordCoordinateComparator" | "SAMRecordQueryNameComparator" => arg_types.is_empty(),
        "SAMSequenceDictionary" => arg_types.len() <= 1,
        "SAMSequenceRecord" => arg_types.len() == 2,
        "SAMProgramRecord" => arg_types.len() == 1,
        "SAMFormatException" => arg_types.len() == 1,
        "SBIIndexWriter" => arg_types.len() == 2,
        "CRAMReferenceRegion" => arg_types.len() == 2,
        "Histogram" => matches!(arg_types.len(), 0 | 2),
        "Interval" => matches!(arg_types.len(), 1 | 2 | 3 | 5),
        "PeekableIterator" => arg_types.len() == 1,
        "MergingIterator" => arg_types.len() == 2,
        "DefaultingMap" => arg_types.len() == 2,
        "SamFileHeaderMerger" => arg_types.len() == 3,
        "MergingSamRecordIterator" => arg_types.len() == 3,
        "Lazy" => arg_types.len() == 1,
        "ReferenceSequence" => arg_types.len() == 3,
        "CodecLineParsingException" => arg_types.len() == 1,
        "RuntimeIOException" => matches!(arg_types.len(), 1 | 2),
        "SAMFileHeader" => arg_types.len() <= 1,
        "SAMReadGroupRecord" => arg_types.len() == 1,
        "SAMRecord" => arg_types.len() == 1,
        "SAMFileWriterFactory" => arg_types.is_empty(),
        "IndexedFastaSequenceFile" => arg_types.len() == 1,
        "IntervalTree" => arg_types.is_empty(),
        "IntervalMergerIterator" => arg_types.len() == 4,
        "FastaReferenceWriterBuilder" => arg_types.is_empty(),
        "Gff3BaseData" => arg_types.len() == 9,
        "SimpleFeature" => arg_types.len() == 3,
        _ => false,
    }
}

fn is_known_apache_commons_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    match type_name {
        "Array2DRowRealMatrix" => matches!(arg_types.len(), 1 | 2),
        "ArrayRealVector" => matches!(arg_types.len(), 0 | 1 | 2),
        "DefaultRealMatrixChangingVisitor" => arg_types.is_empty(),
        "MutableDouble" => matches!(arg_types.len(), 0 | 1),
        "Mean" => arg_types.is_empty(),
        "BinomialDistribution" => matches!(arg_types.len(), 2 | 3),
        "BetaDistribution" => arg_types.len() == 3,
        "BrentOptimizer" => arg_types.len() == 2,
        "AbstractIntegerDistribution" => arg_types.len() == 1,
        "DescriptiveStatistics" => arg_types.is_empty(),
        "DimensionMismatchException" => arg_types.len() == 2,
        "EnumeratedDistribution" => arg_types.len() == 2,
        "ExponentialDistribution" => arg_types.len() == 2,
        "HypergeometricDistribution" => arg_types.len() == 4,
        "JDKRandomGenerator" => arg_types.is_empty(),
        "MaxEval" => arg_types.len() == 1,
        "Median" => arg_types.is_empty(),
        "NormalDistribution" => matches!(arg_types.len(), 2 | 3),
        "Percentile" => matches!(arg_types.len(), 0 | 1),
        "ChiSquaredDistribution" => matches!(arg_types.len(), 1 | 2),
        "PoissonDistribution" => arg_types.len() == 1,
        "SingularValueDecomposition" => arg_types.len() == 1,
        "StandardDeviation" => arg_types.is_empty(),
        "TDistribution" => matches!(arg_types.len(), 1 | 2),
        _ => false,
    }
}

fn is_known_apache_commons_collections_constructor(
    type_name: &str,
    arg_types: &[JavaType],
) -> bool {
    match type_name {
        "DefaultedMap" => arg_types.len() == 1,
        "FilterIterator" => arg_types.len() == 2,
        "SingletonIterator" => arg_types.len() == 1,
        "MultiMap" => arg_types.is_empty(),
        _ => false,
    }
}

fn is_known_apache_commons_compress_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    (matches!(
        type_name,
        "GzipCompressorInputStream"
            | "GzipCompressorOutputStream"
            | "TarArchiveInputStream"
            | "TarArchiveOutputStream"
    ) && arg_types.len() == 1)
        || (matches!(type_name, "NullInputStream" | "NullOutputStream") && arg_types.len() <= 1)
        || (matches!(type_name, "TarArchiveEntry") && matches!(arg_types.len(), 1 | 2))
}

fn is_known_apache_commons_lang_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    match type_name {
        "HashCodeBuilder" => arg_types.is_empty(),
        "MutableInt" => arg_types.len() <= 1,
        "MutableLong" => arg_types.len() <= 1,
        "NotImplementedException" => matches!(arg_types.len(), 1 | 2),
        _ => false,
    }
}

fn is_known_apache_commons_configuration_constructor(
    type_name: &str,
    arg_types: &[JavaType],
) -> bool {
    matches!(type_name, "Configurations") && arg_types.is_empty()
}

fn is_known_apache_http_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    match type_name {
        "HttpGet" => arg_types.len() == 1,
        "PoolingHttpClientConnectionManager" => arg_types.is_empty(),
        _ => false,
    }
}

fn is_known_jama_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    matches!(type_name, "Matrix") && matches!(arg_types.len(), 1 | 2)
}

fn is_known_hipparchus_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    matches!(type_name, "EmpiricalDistribution") && arg_types.is_empty()
}

fn is_known_guava_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    matches!(type_name, "ThreadFactoryBuilder") && arg_types.is_empty()
}

fn is_known_reflections_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    matches!(type_name, "Reflections") && arg_types.len() == 1
}

fn is_known_barclay_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    match type_name {
        "CommandLineException" => matches!(arg_types.len(), 1 | 2),
        "BadArgumentValue" => matches!(arg_types.len(), 1 | 2 | 3),
        "MissingArgument" => matches!(arg_types.len(), 1 | 2),
        "ClassFinder" => arg_types.is_empty(),
        "CommandLineArgumentParser" => arg_types.len() == 3,
        _ => false,
    }
}

fn is_known_fastutil_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    match type_name {
        "Int2ObjectArrayMap" | "Int2ObjectOpenHashMap" | "IntArrayList" => arg_types.len() <= 1,
        "Int2IntArrayMap" => arg_types.len() == 2,
        "Object2IntOpenHashMap" => arg_types.len() <= 1,
        _ => false,
    }
}

fn is_known_picard_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    match type_name {
        "OpticalDuplicateFinder" => matches!(arg_types.len(), 0 | 3),
        "GenotypeConcordanceCounts" => arg_types.is_empty(),
        "GenotypeConcordanceSummaryMetrics" => arg_types.len() == 5,
        "DiagnosticsAndQCProgramGroup" => arg_types.is_empty(),
        "IntervalsManipulationProgramGroup" => arg_types.is_empty(),
        "OtherProgramGroup" => arg_types.is_empty(),
        "ReadDataManipulationProgramGroup" | "ReferenceProgramGroup" => arg_types.is_empty(),
        _ => false,
    }
}

fn is_known_disq_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    matches!(
        type_name,
        "HtsjdkReadsRdd" | "HtsjdkVariantsRdd" | "HtsjdkReadsTraversalParameters"
    ) && arg_types.len() == 2
}

fn is_known_broad_http_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    matches!(type_name, "HttpFileSystemProviderSettings") && arg_types.len() == 3
}

fn is_known_json_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    match type_name {
        "JSONObject" => arg_types.len() <= 1,
        "JSONArray" => arg_types.len() <= 1,
        "JSONTokener" => arg_types.len() == 1,
        _ => false,
    }
}

fn is_known_intel_gkl_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    matches!(
        type_name,
        "IntelDeflaterFactory"
            | "IntelInflaterFactory"
            | "IntelPDHMM"
            | "IntelPairHmm"
            | "IntelPairHmmOMP"
    ) && arg_types.is_empty()
}

fn is_known_hadoop_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    matches!(type_name, "Configuration") && arg_types.is_empty()
}

fn is_known_opencsv_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    match type_name {
        "CSVReader" => matches!(arg_types.len(), 1 | 4),
        "CSVWriter" => matches!(arg_types.len(), 2 | 4),
        _ => false,
    }
}

fn is_known_genomicsdb_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    match type_name {
        "GenomicsDBFeatureReader" => arg_types.len() == 3,
        "GenomicsDBImporter" => arg_types.len() == 1,
        "ImportConfig" => arg_types.len() == 8,
        _ => false,
    }
}

fn is_known_avro_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    matches!(type_name, "GenericDatumReader") && arg_types.len() == 1
}

fn is_known_hdf5_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    match type_name {
        "HDF5Library" => arg_types.is_empty(),
        "HDF5File" => matches!(arg_types.len(), 1 | 2),
        _ => false,
    }
}

fn is_known_kryo_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    match type_name {
        "Kryo" => arg_types.is_empty(),
        "Input" | "Output" => arg_types.len() == 1,
        "FieldSerializer" => arg_types.len() == 2,
        "ImmutableMapSerializer" => arg_types.is_empty(),
        _ => false,
    }
}

fn is_known_bwa_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    matches!(type_name, "BwaMemIndex" | "BwaMemAligner") && arg_types.len() == 1
}

fn is_known_jgrapht_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    match type_name {
        "IntegerComponentNameProvider" => arg_types.is_empty(),
        "SimpleGraph" => arg_types.len() == 1,
        "ConnectivityInspector" | "CycleDetector" | "DefaultDirectedWeightedGraph" => {
            arg_types.len() == 1
        }
        "DOTExporter" => arg_types.len() == 5,
        _ => false,
    }
}

fn is_known_spark_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    match type_name {
        "SparkConf" => arg_types.is_empty(),
        "HashPartitioner" | "JavaSparkContext" => arg_types.len() == 1,
        "JavaRDD" => arg_types.len() == 2,
        "PartitionGroup" => arg_types.len() == 1,
        "RowMatrix" => arg_types.len() == 1,
        _ => false,
    }
}

fn is_known_sqlite_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    matches!(type_name, "SQLiteConfig") && arg_types.is_empty()
}

fn is_known_string_builder_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    matches!(type_name, "StringBuilder" | "StringBuffer") && arg_types.len() <= 1
}

fn is_known_tuple_constructor(type_name: &str, arg_types: &[JavaType]) -> bool {
    matches!(
        type_name,
        "Tuple2" | "Pair" | "ImmutablePair" | "MutablePair"
    ) && arg_types.len() == 2
        || matches!(
            type_name,
            "Tuple3" | "Triple" | "ImmutableTriple" | "MutableTriple"
        ) && arg_types.len() == 3
}

fn external_api_method_policy_provenance(
    receiver_type: Option<&str>,
    name: &str,
    arg_count: usize,
) -> FactProvenance {
    if receiver_type.is_some() && is_known_object_inherited_method(name, arg_count) {
        return FactProvenance::BuiltinExternalModel;
    }
    if receiver_type.is_some() && name == "length" && arg_count == 0 {
        return FactProvenance::BuiltinExternalModel;
    }
    if receiver_type.is_some() && arg_count == 0 && is_known_zero_arg_domain_return_getter(name) {
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
        (Some("Allele"), "create", 1 | 2) => FactProvenance::BuiltinExternalModel,
        (Some("Allele"), "getBaseString", 0) => FactProvenance::BuiltinExternalModel,
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
    if receiver_type == Some("Allele") && name == "create" && matches!(arg_count, 1 | 2) {
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
    } else if name.starts_with("set") {
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
        (Some("Allele"), "create", 1 | 2) => NullabilityState::DefinitelyNonNull,
        (Some("Allele"), "getBaseString", 0) => NullabilityState::DefinitelyNonNull,
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
        (Some("Utils"), "split", 2 | 3) => NullabilityState::DefinitelyNonNull,
        (Some("Utils"), "stream", 1) => NullabilityState::DefinitelyNonNull,
        (Some("IOUtils"), "replaceExtension", 2) => NullabilityState::DefinitelyNonNull,
        (Some("Integer"), "parseInt", 1 | 2) => NullabilityState::DefinitelyNonNull,
        (Some("QualityUtils"), "qualToProb" | "qualToProbLog10", 1) => {
            NullabilityState::DefinitelyNonNull
        }
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
        (Some("Files"), "newDirectoryStream", 1 | 2) => NullabilityState::DefinitelyNonNull,
        (Some("System"), "getProperty", 1 | 2) => NullabilityState::MaybeNull,
        (Some("ImmutablePair" | "Pair"), "of", 2) => NullabilityState::DefinitelyNonNull,
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
        "add" | "addAll" | "remove" | "removeAll" | "clear" => ExternalApiRole::Mutator,
        _ => external_api_method_role(None, name, _arg_count),
    }
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
        (Some("String" | "Utils" | "StringUtils" | "StringUtil"), "join", _) => {
            ExternalApiRole::Getter
        }
        (Some("Long"), "toBinaryString", 1) => ExternalApiRole::Getter,
        (Some("StringUtils"), "isEmpty", 1) => ExternalApiRole::BooleanGetter,
        (Some("StringUtils"), "split", 2) => ExternalApiRole::Getter,
        (Some("Utils"), "split", 2 | 3) | (Some("Utils"), "stream", 1) => ExternalApiRole::Getter,
        (Some("IOUtils"), "replaceExtension", 2) => ExternalApiRole::Getter,
        (Some("Integer"), "parseInt", 1 | 2) => ExternalApiRole::Getter,
        (Some("QualityUtils"), "qualToProb" | "qualToProbLog10", 1) => ExternalApiRole::Getter,
        (Some("Beta"), "logBeta", 2) => ExternalApiRole::Getter,
        (Some("Objects"), "nonNull" | "isNull", 1) => ExternalApiRole::BooleanGetter,
        (Some("Objects"), "equals", 2) => ExternalApiRole::Getter,
        (Some("Collections"), "emptyList" | "emptySet" | "emptyMap", 0) => ExternalApiRole::Getter,
        (Some("Collections"), "singletonList" | "singleton", 1) => ExternalApiRole::Getter,
        (Some("Collections"), "singletonMap", 2) => ExternalApiRole::Getter,
        (Some("Collections"), "sort" | "shuffle" | "reverse", _) => ExternalApiRole::Mutator,
        (Some("Iterators"), "forEnumeration", 1) => ExternalApiRole::Getter,
        (Some("Files"), "newDirectoryStream", 1 | 2) => ExternalApiRole::Getter,
        (Some("System"), "getProperty", 1 | 2) => ExternalApiRole::Getter,
        (Some("ImmutablePair" | "Pair"), "of", 2) => ExternalApiRole::Getter,
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
            | Some("Utils")
            | Some("QualityUtils")
            | Some("Beta")
            | Some("IOUtils")
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
