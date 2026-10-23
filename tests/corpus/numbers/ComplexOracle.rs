//! Caller-only oracle: numeric algorithms are linked from untouched output.
use std::sync::{Arc, Mutex};
use translated::{Complex as C, JavaException, JavaString};
type Complex = Arc<Mutex<C>>;
// This caller's inputs contain only scalar text. Keep the output boundary
// explicitly checked; production strings must not acquire a lossy Display.
fn scalar(text: &JavaString) -> String { text.scalar_string().expect("scalar oracle output") }
fn bits(x: f64) -> String { format!("{:x}", x.to_bits()) }
fn pair(z: Complex) -> String {
    format!("{},{}", bits(C::get_real_with_outer(z.clone())), bits(C::get_imaginary_with_outer(z)))
}
fn out(key: &str, operation: impl FnOnce() -> Result<String, JavaException>) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation));
    let value = match result {
        Ok(Ok(value)) => value,
        Ok(Err(e)) => format!("ERR:{}:{}", e.class_name().rsplit(['.', '$']).next().unwrap(), scalar(e.message())),
        Err(e) => {
            if let Some(e) = e.downcast_ref::<JavaException>() {
                format!("ERR:{}:{}", e.class_name().rsplit(['.', '$']).next().unwrap(), scalar(e.message()))
            } else if let Some(e) = e.downcast_ref::<String>() { format!("PANIC:{e}") }
            else if let Some(e) = e.downcast_ref::<&str>() { format!("PANIC:{e}") }
            else { "PANIC:unknown payload".into() }
        }
    };
    println!("{key}={value}");
}
macro_rules! o { ($key:expr, $value:expr) => { out(&$key, || Ok(($value).to_string())); }; }
macro_rules! p { ($key:expr, $value:expr) => { out(&$key, || Ok(pair($value))); }; }
macro_rules! f { ($key:expr, $value:expr) => { out(&$key, || Ok(pair($value?))); }; }
fn pairs() -> Vec<(f64,f64)> {
    vec![(0.0,0.0),(0.0,-0.0),(-0.0,0.0),(-0.0,-0.0),
        (1.0,0.0),(1.0,-0.0),(-1.0,0.0),(-1.0,-0.0),
        (0.0,1.0),(-0.0,1.0),(0.0,-1.0),(-0.0,-1.0),
        (1.0,1.0),(-1.0,1.0),(1.0,-1.0),(-1.0,-1.0),
        (0.5,0.25),(0.1,-0.3),(2.0,3.0),(-2.0,-3.0),
        (1e-200,1e-200),(1e200,-1e200),(f64::MIN_POSITIVE,f64::from_bits(1)),
        (f64::MAX,f64::MAX),(f64::MAX,f64::from_bits(1)),
        (f64::INFINITY,1.0),(1.0,f64::NEG_INFINITY),
        (f64::INFINITY,f64::NEG_INFINITY),(f64::NAN,1.0),
        (1.0,f64::NAN),(f64::INFINITY,f64::NAN),
        (f64::from_bits(0xfff8000000001234),-0.0),
        (708.0,-708.0),(f64::from_bits(0x3ff0000000000001),1e-16)]
}
fn scalars() -> Vec<f64> {
    vec![0.0,-0.0,1.0,-1.0,0.5,-0.5,0.1,-0.1,1e-200,1e200,
        f64::from_bits(1),f64::MIN_POSITIVE,f64::MAX,f64::INFINITY,f64::NEG_INFINITY,f64::NAN]
}
fn numeric() {
    let inputs=pairs();
    for (i,&(re,im)) in inputs.iter().enumerate() {
        let z=C::of_cartesian(re,im); let k=format!("z.{i}");
        p!(format!("{k}.cartesian"), z.clone());
        o!(format!("{k}.getReal"), bits(C::get_real_with_outer(z.clone())));
        o!(format!("{k}.real"), bits(C::real_with_outer(z.clone())));
        o!(format!("{k}.getImaginary"), bits(C::get_imaginary_with_outer(z.clone())));
        o!(format!("{k}.imag"), bits(C::imag_with_outer(z.clone())));
        o!(format!("{k}.abs"), bits(C::abs_with_outer(z.clone())));
        o!(format!("{k}.arg"), bits(C::arg_with_outer(z.clone())));
        o!(format!("{k}.norm"), bits(C::norm_with_outer(z.clone())));
        o!(format!("{k}.isNaN"), C::is_na_n_with_outer(z.clone()));
        o!(format!("{k}.isInfinite"), C::is_infinite_with_outer(z.clone()));
        o!(format!("{k}.isFinite"), C::is_finite_with_outer(z.clone()));
        p!(format!("{k}.conj"), C::conj_with_outer(z.clone()));
        p!(format!("{k}.negate"), C::negate_with_outer(z.clone()));
        p!(format!("{k}.proj"), C::proj_with_outer(z.clone()));
        p!(format!("{k}.exp"), C::exp_with_outer(z.clone()));
        f!(format!("{k}.log"), C::log_with_outer(z.clone()));
        f!(format!("{k}.log10"), C::log10_with_outer(z.clone()));
        f!(format!("{k}.sqrt"), C::sqrt_with_outer(z.clone()));
        p!(format!("{k}.sin"), C::sin_with_outer(z.clone()));
        p!(format!("{k}.cos"), C::cos_with_outer(z.clone()));
        f!(format!("{k}.tan"), C::tan_with_outer(z.clone()));
        f!(format!("{k}.asin"), C::asin_with_outer(z.clone()));
        f!(format!("{k}.acos"), C::acos_with_outer(z.clone()));
        f!(format!("{k}.atan"), C::atan_with_outer(z.clone()));
        p!(format!("{k}.sinh"), C::sinh_with_outer(z.clone()));
        p!(format!("{k}.cosh"), C::cosh_with_outer(z.clone()));
        f!(format!("{k}.tanh"), C::tanh_with_outer(z.clone()));
        f!(format!("{k}.asinh"), C::asinh_with_outer(z.clone()));
        f!(format!("{k}.acosh"), C::acosh_with_outer(z.clone()));
        f!(format!("{k}.atanh"), C::atanh_with_outer(z.clone()));
        out(&format!("{k}.equals.self"), || Ok(C::equals_object_with_outer(z.clone(),Some(Box::new(z.clone())))?.to_string()));
        out(&format!("{k}.equals.fresh"), || Ok(C::equals_object_with_outer(z.clone(),Some(Box::new(C::of_cartesian(C::real_with_outer(z.clone()),C::imag_with_outer(z.clone())))))?.to_string()));
        out(&format!("{k}.equals.null"), || Ok(C::equals_object_with_outer(z.clone(),None)?.to_string()));
        out(&format!("{k}.equals.string"), || Ok(C::equals_object_with_outer(z.clone(),Some(Box::new(JavaString::fresh("other"))))?.to_string()));
        o!(format!("{k}.hash"), C::hash_code_with_outer(z.clone()));
        out(&format!("{k}.string"), || C::to_string_with_outer(z.clone()).map(|text| scalar(&text)));
        o!(format!("{k}.identity.proj"), Arc::ptr_eq(&C::proj_with_outer(z.clone()),&z));
        o!(format!("{k}.identity.conj"), Arc::ptr_eq(&C::conj_with_outer(z.clone()),&z));
        o!(format!("{k}.identity.negate"), Arc::ptr_eq(&C::negate_with_outer(z.clone()),&z));
        for (j,x) in scalars().into_iter().enumerate() {
            let s=format!("{k}.scalar.{j}");
            p!(format!("{s}.add"), C::add_double_with_outer(z.clone(),x));
            p!(format!("{s}.addImaginary"), C::add_imaginary_with_outer(z.clone(),x));
            p!(format!("{s}.subtract"), C::subtract_double_with_outer(z.clone(),x));
            p!(format!("{s}.subtractImaginary"), C::subtract_imaginary_with_outer(z.clone(),x));
            p!(format!("{s}.subtractFrom"), C::subtract_from_with_outer(z.clone(),x));
            p!(format!("{s}.subtractFromImaginary"), C::subtract_from_imaginary_with_outer(z.clone(),x));
            p!(format!("{s}.multiply"), C::multiply_double_with_outer(z.clone(),x));
            p!(format!("{s}.multiplyImaginary"), C::multiply_imaginary_with_outer(z.clone(),x));
            p!(format!("{s}.divide"), C::divide_double_with_outer(z.clone(),x));
            p!(format!("{s}.divideImaginary"), C::divide_imaginary_with_outer(z.clone(),x));
            f!(format!("{s}.pow"), C::pow_double_with_outer(z.clone(),x));
        }
        for (j,&(a,b)) in inputs.iter().enumerate() {
            let w=C::of_cartesian(a,b); let k=format!("{k}.binary.{j}");
            f!(format!("{k}.add"), C::add_complex_with_outer(z.clone(),Some(w.clone())));
            f!(format!("{k}.subtract"), C::subtract_complex_with_outer(z.clone(),Some(w.clone())));
            f!(format!("{k}.multiply"), C::multiply_complex_with_outer(z.clone(),Some(w.clone())));
            f!(format!("{k}.divide"), C::divide_complex_with_outer(z.clone(),Some(w.clone())));
            f!(format!("{k}.pow"), C::pow_complex_with_outer(z.clone(),Some(w.clone())));
            out(&format!("{k}.equals"), || Ok(C::equals_object_with_outer(z.clone(),Some(Box::new(w)))?.to_string()));
        }
        for n in [-3,-1,0,1,2,3,4] {
            out(&format!("{k}.roots.{n}"), || Ok(C::nth_root_with_outer(z.clone(),n)?.borrow().cloned_items().into_iter().map(pair).collect::<Vec<_>>().join(";")));
        }
        p!(format!("{k}.after"), z);
        p!(format!("{k}.polar"), C::of_polar(re,im));
        p!(format!("{k}.cis"), C::of_cis(re));
    }
}
fn parsing() {
    let inputs=["(1,2)","(0.0,-0.0)","(-0.0,0.0)","(NaN,Infinity)","(-Infinity,NaN)",
        "( 1 , 2 )","(\t1,2\t)","(0x1p2,-0x1p-4)","(1f,2D)","(+1e2,-2e-3)",
        "(1e9999,1e-9999)","", "(1)","[1,2)","(1,2]","(,2)","(1,)","(1,2,3)",
        "(abc,2)","(1,abc)","(inf,2)","(nan,2)","(1_0,2)","(😀,1)","(é,1)","(1,２)"];
    for (i,s) in inputs.into_iter().enumerate() { f!(format!("parse.{i}"), C::parse(Some(JavaString::fresh(s)))); }
}
fn main() {
    p!("constant.I", C::__static_I()); p!("constant.ONE", C::__static_ONE());
    p!("constant.ZERO", C::__static_ZERO());
    o!("constant.identity", Arc::ptr_eq(&C::__static_I(),&C::__static_I()) && Arc::ptr_eq(&C::__static_ONE(),&C::__static_ONE()) && Arc::ptr_eq(&C::__static_ZERO(),&C::__static_ZERO()));
    numeric(); parsing(); println!("oracle=ok");
}
