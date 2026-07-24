use crate::java_ir::{JavaType, PrimitiveType};
use crate::method_reference_plan::{MethodReferencePlan, plan_method_reference};
use crate::slot_shape::RustSlotShape;

pub fn infer_single_input_method_reference_output_shape(
    target: &str,
    method: &str,
    input_shape: &RustSlotShape,
) -> Option<RustSlotShape> {
    let plan = plan_method_reference(target, method);
    infer_method_reference_output_shape_for_plan(&plan, input_shape)
}

pub fn infer_method_reference_output_shape_for_plan(
    plan: &MethodReferencePlan,
    input_shape: &RustSlotShape,
) -> Option<RustSlotShape> {
    match plan {
        MethodReferencePlan::AssociatedFunction { owner, method }
            if owner == "Integer" && method == "int_value" =>
        {
            Some(RustSlotShape::for_storage(JavaType::Primitive(
                PrimitiveType::Int,
            )))
        }
        MethodReferencePlan::AssociatedFunction { owner, method }
            if (owner == "String" && method == "value_of")
                || (owner == "Object" && method == "to_string") =>
        {
            Some(crate::shape_projection::owned_string_value_shape())
        }
        MethodReferencePlan::UnboundAccessor { method }
            if method == "get" || method.starts_with("get_") =>
        {
            let mut shape = input_shape.clone();
            shape.java_ty = JavaType::Unknown(method.clone());
            Some(shape)
        }
        _ => None,
    }
}
