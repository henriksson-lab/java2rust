use new_java2rust::{
    facts::FactSet,
    java_ir::{JavaType, PrimitiveType},
    shape_projection::{
        indexed_assignment_shape_with_facts, indexed_read_shape_for_access_with_facts,
    },
    slot_shape::{DispatchShape, OwnershipShape, RustSlotShape},
};

#[test]
fn indexed_object_storage_keeps_erasure_and_null_carrier_on_reads_and_writes() {
    let facts = FactSet::default();
    for name in ["Object", "java.lang.Object"] {
        let object = JavaType::Named {
            name: name.into(),
            args: vec![],
        };
        let array = RustSlotShape::for_storage(JavaType::Array(Box::new(object.clone())));
        let read = indexed_read_shape_for_access_with_facts(&array, &facts).unwrap();
        let write = indexed_assignment_shape_with_facts(&array, &facts).unwrap();
        for shape in [&read, &write] {
            assert_eq!(shape.dispatch, DispatchShape::ErasedObject);
            assert_eq!(shape.optional_element_type(), Some(object.clone()));
        }
        assert_eq!(read.ownership, OwnershipShape::SharedBorrow);
        assert_eq!(write.ownership, OwnershipShape::Owned);
    }
    for element in [
        JavaType::Primitive(PrimitiveType::Int),
        JavaType::Named {
            name: "Payload".into(),
            args: vec![],
        },
    ] {
        let array = RustSlotShape::for_storage(JavaType::Array(Box::new(element)));
        for shape in [
            indexed_read_shape_for_access_with_facts(&array, &facts).unwrap(),
            indexed_assignment_shape_with_facts(&array, &facts).unwrap(),
        ] {
            assert_eq!(shape.dispatch, DispatchShape::Concrete);
        }
    }
}
