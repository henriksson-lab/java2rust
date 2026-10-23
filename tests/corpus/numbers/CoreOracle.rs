//! Caller-only oracle. All numeric algorithms are linked from untouched output.
use translated::{ArithmeticUtils as A, Norm, Precision as P, Sum, DD, DDMath, JavaException};
use java_math_runtime::{BigDecimal, BigInteger, RoundingMode};
use std::{cell::RefCell, rc::Rc, sync::{Arc, Mutex}, fmt::Display};

fn bits(x: f64) -> String { format!("{:x}", x.to_bits()) }
fn arithmetic_bits(x: f64) -> String { format!("f64-arith({:016x})", x.to_bits()) }
fn arithmetic_dd(x: Arc<Mutex<DD>>) -> String {
    format!("{},{}", arithmetic_bits(DD::hi_with_outer(x.clone())), arithmetic_bits(DD::lo_with_outer(x)))
}
fn power_dd(x: Arc<Mutex<DD>>, exponent: i32) -> String {
    if exponent == 0 || exponent == 1 { dd(x) } else { arithmetic_dd(x) }
}
fn arr<T>(values: Vec<T>) -> Rc<RefCell<Vec<T>>> { Rc::new(RefCell::new(values)) }
fn dd(x: Arc<Mutex<DD>>) -> String {
    format!("{},{}", bits(DD::hi_with_outer(x.clone())), bits(DD::lo_with_outer(x)))
}
fn error(e: &JavaException) -> String {
    format!("ERR:{}:{}", e.class_name().rsplit(['.', '$']).next().unwrap(), e.message().scalar_string().expect("this numeric oracle emits scalar exception messages"))
}
fn out<T: Display>(key: &str, operation: impl FnOnce() -> Result<T, JavaException>) {
    let value = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)) {
        Ok(Ok(value)) => value.to_string(),
        Ok(Err(e)) => error(&e),
        Err(payload) => {
            if let Some(e) = payload.downcast_ref::<JavaException>() { error(e) }
            else if let Some(e) = payload.downcast_ref::<String>() { format!("PANIC:{e}") }
            else if let Some(e) = payload.downcast_ref::<&str>() { format!("PANIC:{e}") }
            else { "PANIC:unknown payload".to_string() }
        }
    };
    println!("{key}={value}");
}
macro_rules! o { ($key:expr, $value:expr) => { out(&$key, || Ok($value)); }; }
macro_rules! r { ($key:expr, $value:expr) => { out(&$key, || $value); }; }

fn arithmetic() {
    let pairs = [(0,0),(0,-7),(-7,0),(21,14),(-21,14),(i32::MIN,1),
        (i32::MIN,0),(i32::MIN,i32::MIN),(i32::MAX,2),(-1,-1),(1,0)];
    for (i,(a,b)) in pairs.into_iter().enumerate() {
        let k=format!("arith.i.{i}");
        r!(format!("{k}.gcd"), A::gcd_int_int(a,b));
        r!(format!("{k}.lcm"), A::lcm_int_int(a,b));
        r!(format!("{k}.rem"), A::remainder_unsigned_int_int(a,b));
        r!(format!("{k}.div"), A::divide_unsigned_int_int(a,b));
    }
    let pairs = [(0,0),(-7,0),(21,14),(-21,14),(i64::MIN,1),(i64::MIN,0),
        (i64::MIN,i64::MIN),(i64::MAX,2),(-1,-1),(1,0),(0x123456789abcdef,0x1234567),(-1,3)];
    for (i,(a,b)) in pairs.into_iter().enumerate() {
        let k=format!("arith.l.{i}");
        r!(format!("{k}.gcd"), A::gcd_long_long(a,b));
        r!(format!("{k}.lcm"), A::lcm_long_long(a,b));
        r!(format!("{k}.rem"), A::remainder_unsigned_long_long(a,b));
        r!(format!("{k}.div"), A::divide_unsigned_long_long(a,b));
    }
    for base in [0,1,-1,2,-2,7,i32::MIN,i32::MAX] {
        for e in [-1,0,1,2,3,8,31,32,63,64] {
            let k=format!("arith.pow.{base}.{e}");
            r!(format!("{k}.int"), A::pow_int_int(base,e));
            r!(format!("{k}.long"), A::pow_long_int(base as i64,e));
        }
    }
    for s in ["0","1","-1","2","-3","12345678901234567890"] {
        let base: BigInteger=s.parse().unwrap();
        for e in [-1,0,1,2,5,31] {
            let k=format!("arith.big.{s}.{e}");
            r!(format!("{k}.int"), A::pow_big_integer_int(Some(base.clone()),e));
            r!(format!("{k}.long"), A::pow_big_integer_long(Some(base.clone()),e as i64));
            r!(format!("{k}.big"), A::pow_big_integer_big_integer(Some(base.clone()),Some(BigInteger::value_of(e as i64))));
        }
    }
    for n in [i64::MIN,-8,-1,0,1,2,3,4,1024,i64::MAX] {
        o!(format!("arith.power2.{n}"), A::is_power_of_two(n));
    }
    // Match the Java caller's width boundaries; production owns every power.
    for s in ["0", "1", "-1"] {
        let base: BigInteger = s.parse().unwrap();
        for e in [i64::MIN, -1, 0, 1, 2, 63, 64, 2147483647, 2147483648, i64::MAX] {
            r!(format!("arith.wide.long.{s}.{e}"), A::pow_big_integer_long(Some(base.clone()), e));
        }
        for text in ["-18446744073709551617", "-1", "0", "1", "2", "63", "64",
                "2147483648", "9223372036854775808", "18446744073709551616",
                "18446744073709551617", "1267650600228229401496703205377"] {
            let e: BigInteger = text.parse().unwrap();
            r!(format!("arith.wide.big.{s}.{text}"), A::pow_big_integer_big_integer(Some(base.clone()), Some(e)));
        }
        o!(format!("arith.wide.unchanged.{s}"), base.clone());
    }
}

fn values() -> Vec<f64> {
    vec![0.0,-0.0,1.0,-1.0,0.1,-0.1,1e-200,1e200,f64::from_bits(1),f64::MIN_POSITIVE,
        f64::MAX,f64::INFINITY,f64::NEG_INFINITY,f64::NAN]
}

fn norms() {
    o!("norm.values", Norm::values().len());
    let vs=values();
    for n in Norm::values() {
        let k=format!("norm.{}", n.name());
        o!(format!("{k}.ordinal"), n.ordinal());
        o!(format!("{k}.lookup"), Norm::value_of(n.name()) == n);
        o!(format!("{k}.repeat"), Norm::values()[n.ordinal() as usize] == n);
        for i in 0..vs.len() {
            let (x,y,z)=(vs[i],vs[(i+3)%vs.len()],vs[(i+7)%vs.len()]);
            let p=format!("{k}.{i}");
            r!(format!("{p}.two"), n.of_double_double(x,y).map(bits));
            r!(format!("{p}.three"), n.of_double_double_double(x,y,z).map(bits));
            r!(format!("{p}.array"), n.of_double_array(Some(arr(vec![x,y,z,-x]))).map(bits));
        }
        r!(format!("{k}.empty"), n.of_double_array(Some(arr(vec![]))).map(bits));
        r!(format!("{k}.single"), n.of_double_array(Some(arr(vec![-3.0]))).map(bits));
    }
    o!("norm.badname", Norm::value_of("missing").name());
}

fn precision() {
    o!("precision.epsilon", bits(P::__static_EPSILON()));
    o!("precision.safe", bits(P::SAFE_MIN));
    let vs=values();
    for i in 0..vs.len() { for j in 0..vs.len() {
        let (x,y)=(vs[i],vs[j]); let (a,b)=(x as f32,y as f32);
        let k=format!("precision.{i}.{j}");
        o!(format!("{k}.cmp.eps"), P::compare_to_double_double_double(x,y,1e-12));
        o!(format!("{k}.cmp.ulps"), P::compare_to_double_double_int(x,y,2));
        o!(format!("{k}.f.eq"), P::equals_float_float(a,b));
        o!(format!("{k}.f.nan"), P::equals_including_na_n_float_float(a,b));
        o!(format!("{k}.f.eps"), P::equals_float_float_float(a,b,1e-6));
        o!(format!("{k}.f.nan.eps"), P::equals_including_na_n_float_float_float(a,b,1e-6));
        o!(format!("{k}.f.ulps"), P::equals_float_float_int(a,b,2));
        o!(format!("{k}.f.nan.ulps"), P::equals_including_na_n_float_float_int(a,b,2));
        o!(format!("{k}.d.eq"), P::equals_double_double(x,y));
        o!(format!("{k}.d.nan"), P::equals_including_na_n_double_double(x,y));
        o!(format!("{k}.d.eps"), P::equals_double_double_double(x,y,1e-12));
        o!(format!("{k}.d.rel"), P::equals_with_relative_tolerance(x,y,1e-12));
        o!(format!("{k}.d.nan.eps"), P::equals_including_na_n_double_double_double(x,y,1e-12));
        o!(format!("{k}.d.ulps"), P::equals_double_double_int(x,y,2));
        o!(format!("{k}.d.nan.ulps"), P::equals_including_na_n_double_double_int(x,y,2));
        o!(format!("{k}.delta"), bits(P::representable_delta(x,y)));
    }}
    let modes=[RoundingMode::UP,RoundingMode::DOWN,RoundingMode::CEILING,RoundingMode::FLOOR,
        RoundingMode::HALF_UP,RoundingMode::HALF_DOWN,RoundingMode::HALF_EVEN,RoundingMode::UNNECESSARY];
    for x in [-0.0,0.0,0.1,2.5,-2.5,1.235,-1.235,1e100,f64::NAN,f64::INFINITY] {
        for scale in [-2,0,2,15] {
            let k=format!("precision.round.{}.{scale}",bits(x));
            r!(format!("{k}.default"), P::round_double_int(x,scale).map(bits));
            for (i,mode) in modes.into_iter().enumerate() {
                r!(format!("{k}.{i}"), P::round_double_int_rounding_mode(x,scale,Some(mode)).map(bits));
            }
        }
    }
    for eps in [0.0,1e-12,-1.0,f64::NAN,f64::INFINITY] {
        let k=format!("precision.equiv.{}",bits(eps));
        r!(format!("{k}.create"), P::double_equivalence_of_epsilon(eps).map(|_| "ok"));
        if !(eps>=0.0) || eps.is_infinite() { continue; }
        let eq=P::double_equivalence_of_epsilon(eps).unwrap();
        for i in 0..vs.len() {
            let (x,y)=(vs[i],vs[(i+1)%vs.len()]); let p=format!("{k}.{i}");
            o!(format!("{p}.eq"), eq.eq(x,y)); o!(format!("{p}.zero"), eq.eq_zero(x));
            o!(format!("{p}.lt"), eq.lt(x,y)); o!(format!("{p}.lte"), eq.lte(x,y));
            o!(format!("{p}.gt"), eq.gt(x,y)); o!(format!("{p}.gte"), eq.gte(x,y));
            o!(format!("{p}.sign"), bits(eq.signum(x))); o!(format!("{p}.cmp"), eq.compare(x,y));
        }
    }
}

fn sums() {
    let arrays=vec![vec![],vec![1.0],vec![1e16,1.0,-1e16],vec![1e-200,1e-200,-1e-200],
        vec![f64::MAX,f64::MAX],vec![f64::INFINITY,1.0],vec![f64::INFINITY,f64::NEG_INFINITY],
        vec![f64::NAN,1.0],vec![-0.0,0.0]];
    for (i,v) in arrays.into_iter().enumerate() {
        let k=format!("sum.{i}"); let a=arr(v.clone());
        o!(format!("{k}.of"), bits(Sum::get_as_double_with_outer(Sum::of_double_array_varargs(Some(a.clone()))?)));
        let s=Sum::create(); let saved=s.clone();
        o!(format!("{k}.identity"), Rc::ptr_eq(&Sum::add_double_array_varargs_with_outer(s.clone(),Some(&a))?,&saved));
        o!(format!("{k}.array"), bits(Sum::get_as_double_with_outer(saved)));
        for x in v { Sum::accept_with_outer(s.clone(),x); }
        o!(format!("{k}.accept"), bits(Sum::get_as_double_with_outer(s.clone())));
        o!(format!("{k}.add"), { Sum::add_sum_with_outer(s.clone(),Some(Sum::of_double(3.0)))?; bits(Sum::get_as_double_with_outer(s.clone())) });
        o!(format!("{k}.subtract"), { Sum::subtract_with_outer(s.clone(),Some(Sum::of_double(3.0)))?; arithmetic_bits(Sum::get_as_double_with_outer(s.clone())) });
        o!(format!("{k}.selfadd"), { Sum::add_sum_with_outer(s.clone(),Some(s.clone()))?; bits(Sum::get_as_double_with_outer(s.clone())) });
        o!(format!("{k}.selfsubtract"), { Sum::subtract_with_outer(s.clone(),Some(s.clone()))?; arithmetic_bits(Sum::get_as_double_with_outer(s.clone())) });
        o!(format!("{k}.product"), bits(Sum::get_as_double_with_outer(Sum::add_product_with_outer(Sum::create(),0.1,0.2))));
        r!(format!("{k}.products"), Sum::of_products(Some(a.clone()),Some(a.clone())).map(|x| bits(Sum::get_as_double_with_outer(x))));
        r!(format!("{k}.addproducts"), Sum::add_products_with_outer(Sum::of_double(1.0),Some(&a),Some(&a)).map(|x| bits(Sum::get_as_double_with_outer(x))));
    }
    o!("sum.scalar", bits(Sum::get_as_double_with_outer(Sum::add_double_with_outer(Sum::of_double(2.0),3.0))));
    r!("sum.badproducts", Sum::of_products(Some(arr(vec![1.0])),Some(arr(vec![]))).map(|x| bits(Sum::get_as_double_with_outer(x))));
    r!("sum.badaddproducts", Sum::add_products_with_outer(Sum::create(),Some(&arr(vec![])),Some(&arr(vec![1.0]))).map(|x| bits(Sum::get_as_double_with_outer(x))));
    let object = Sum::create();
    let text = translated::JavaArraysDeepToStringElement::java_object_display_result(&object).unwrap();
    r!("sum.object.display", translated::JavaHashCode::java_hash_code_result(&object).map(|hash|
        text.scalar_string().expect("object display is scalar") == format!("{}@{:x}", object.borrow().get_class().get_name(), hash)));
    let alias = Rc::clone(&object);
    r!("sum.object.alias", translated::JavaArraysDeepToStringElement::java_object_display_result(&alias).map(|value| value == text));
    r!("sum.object.mutation", {
        Sum::add_double_with_outer(Rc::clone(&alias),3.0);
        translated::JavaArraysDeepToStringElement::java_object_display_result(&object).map(|value| value == text)
    });
    let erased: Box<dyn std::any::Any> = Box::new(Rc::clone(&object));
    r!("sum.object.erased", translated::java_erased_object_string_result(&erased).map(|value| value.as_ref().is_some_and(|value| value.java_equals(&text))));
}

fn doubles() {
    for (i,x) in values().into_iter().enumerate() {
        let a=DD::of_double(x); let k=format!("dd.{i}");
        o!(format!("{k}.of"), dd(a.clone()));
        o!(format!("{k}.finite"), DD::is_finite_with_outer(a.clone()));
        o!(format!("{k}.double"), bits(DD::double_value_with_outer(a.clone())));
        o!(format!("{k}.float"), format!("{:x}", DD::float_value_with_outer(a.clone()).to_bits()));
        o!(format!("{k}.int"), DD::int_value_with_outer(a.clone()));
        o!(format!("{k}.long"), DD::long_value_with_outer(a.clone()));
        r!(format!("{k}.decimal"), DD::big_decimal_value_with_outer(a.clone()).map(|d| d.to_string()));
        o!(format!("{k}.negate"), dd(DD::negate_with_outer(a.clone())?));
        o!(format!("{k}.abs"), dd(DD::abs_with_outer(a.clone())?));
        o!(format!("{k}.floor"), dd(DD::floor_with_outer(a.clone())));
        o!(format!("{k}.ceil"), dd(DD::ceil_with_outer(a.clone())));
        o!(format!("{k}.square"), dd(DD::square_with_outer(a.clone())));
        o!(format!("{k}.sqrt"), dd(DD::sqrt_with_outer(a.clone())));
        o!(format!("{k}.reciprocal"), arithmetic_dd(DD::reciprocal_with_outer(a.clone())?));
        o!(format!("{k}.identity.pow1"), Arc::ptr_eq(&DD::pow_int_with_outer(a.clone(),1)?,&a));
        o!(format!("{k}.identity.pow0"), Arc::ptr_eq(&DD::pow_int_with_outer(a.clone(),0)?,&DD::__static_ONE()));
        o!(format!("{k}.identity.pow2fresh"), !Arc::ptr_eq(&DD::pow_int_with_outer(a.clone(),2)?,&a));
        o!(format!("{k}.identity.abs"), Arc::ptr_eq(&DD::abs_with_outer(a.clone())?,&a));
        o!(format!("{k}.add.d"), dd(DD::add_double_with_outer(a.clone(),0.1)));
        o!(format!("{k}.add.dd"), dd(DD::add_d_d_with_outer(a.clone(),Some(DD::of_sum(0.1,1e-18)))?));
        o!(format!("{k}.sub.d"), dd(DD::subtract_double_with_outer(a.clone(),0.1)));
        o!(format!("{k}.sub.dd"), dd(DD::subtract_d_d_with_outer(a.clone(),Some(DD::of_double(0.1)))?));
        o!(format!("{k}.mul.d"), dd(DD::multiply_double_with_outer(a.clone(),0.1)));
        o!(format!("{k}.mul.dd"), dd(DD::multiply_d_d_with_outer(a.clone(),Some(DD::of_double(0.1)))?));
        o!(format!("{k}.mul.i"), dd(DD::multiply_int_with_outer(a.clone(),3)?));
        o!(format!("{k}.div.d"), arithmetic_dd(DD::divide_double_with_outer(a.clone(),3.0)));
        o!(format!("{k}.div.dd"), arithmetic_dd(DD::divide_d_d_with_outer(a.clone(),Some(DD::of_int(3)))?));
        o!(format!("{k}.zero"), Arc::ptr_eq(&DD::zero_with_outer(a.clone()),&DD::__static_ZERO()));
        r!(format!("{k}.iszero"), DD::is_zero_with_outer(a.clone()));
        o!(format!("{k}.one"), Arc::ptr_eq(&DD::one_with_outer(a.clone()),&DD::__static_ONE()));
        r!(format!("{k}.isone"), DD::is_one_with_outer(a.clone()));
        r!(format!("{k}.equals.self"), DD::equals_object_with_outer(a.clone(),Some(Box::new(a.clone()))));
        r!(format!("{k}.equals.copy"), DD::equals_object_with_outer(a.clone(),Some(Box::new(DD::of_double(x)))));
        r!(format!("{k}.equals.null"), DD::equals_object_with_outer(a.clone(),None));
        r!(format!("{k}.equals.other"), DD::equals_object_with_outer(a.clone(),Some(Box::new(translated::JavaString::intern_literal("x")))));
        o!(format!("{k}.hash"), DD::hash_code_with_outer(a.clone()));
        r!(format!("{k}.string"), DD::to_string_with_outer(a.clone()).map(|text| text.scalar_string().expect("numeric output is scalar")));
        o!(format!("{k}.scalb"), dd(DD::scalb_with_outer(a.clone(),17)));
        r!(format!("{k}.frexp"), { let e=arr(vec![123]); DD::frexp_with_outer(a.clone(),Some(e.clone())).map(|v| format!("{},{}",dd(v),e.borrow()[0])) });
        for n in [-3,-1,0,1,2,5,31] {
            let p=format!("{k}.pow.{n}");
            o!(format!("{p}.plain"), power_dd(DD::pow_int_with_outer(a.clone(),n)?,n));
            r!(format!("{p}.scaled"), { let e=arr(vec![123]); DD::pow_int_long_array_with_outer(a.clone(),n,Some(e.clone())).map(|v| format!("{},{}",dd(v),e.borrow()[0])) });
            r!(format!("{p}.math"), { let e=arr(vec![123]); DDMath::pow(a.clone(),n,Some(e.clone())).map(|v| format!("{},{}",dd(v),e.borrow()[0])) });
        }
        o!(format!("{k}.unchanged"), dd(a.clone()));
    }
    for n in [i64::MIN,-9007199254740993,-1,0,1,9007199254740993,i64::MAX] {
        o!(format!("dd.long.{n}"), dd(DD::of_long(n))); o!(format!("dd.unsignedlong.{n}"), dd(DD::of_unsigned_long(n)));
    }
    for n in [i32::MIN,-1,0,1,i32::MAX] {
        o!(format!("dd.int.{n}"), dd(DD::of_int(n))); o!(format!("dd.unsignedint.{n}"), dd(DD::of_unsigned_int(n)));
    }
    for s in ["0","0.1","1.0000000000000000000000000001","1e300","1e-300","-12345678901234567890.12345"] {
        r!(format!("dd.from.{s}"), DD::from(Some(BigDecimal::new(s).unwrap())).map(dd));
    }
    let pairs=[(1e16,1.0),(0.1,0.2),(f64::MAX,2.0),(f64::from_bits(1),0.5),(-0.0,0.0),(f64::NAN,1.0)];
    for (i,(x,y)) in pairs.into_iter().enumerate() {
        let k=format!("dd.factory.{i}");
        o!(format!("{k}.sum"), dd(DD::of_sum(x,y))); o!(format!("{k}.diff"), dd(DD::of_difference(x,y)));
        o!(format!("{k}.prod"), dd(DD::of_product(x,y))); o!(format!("{k}.square"), dd(DD::of_square(x)));
        o!(format!("{k}.quot"), dd(DD::from_quotient(x,y)));
    }
    use translated::{Addition,Multiplication,NativeOperators};
    let a=DD::of_sum(3.0,1e-17); let b=DD::of_int(2);
    let mut add=translated::__JavaSharedDDAsAddition(a.clone());
    let mut mul=translated::__JavaSharedDDAsMultiplication(a.clone());
    let mut native=translated::__JavaSharedDDAsNativeOperators(a.clone());
    o!("interfaces.add", dd(Addition::add(&mut add,Some(b.clone()))?)); o!("interfaces.zero", dd(Addition::zero(&mut add)));
    o!("interfaces.negate", dd(Addition::negate(&mut add)?)); r!("interfaces.iszero", Addition::is_zero(&mut add));
    o!("interfaces.multiply", dd(Multiplication::multiply(&mut mul,Some(b.clone()))?)); o!("interfaces.one", dd(Multiplication::one(&mut mul)));
    o!("interfaces.reciprocal", arithmetic_dd(Multiplication::reciprocal(&mut mul)?)); r!("interfaces.isone", Multiplication::is_one(&mut mul));
    o!("interfaces.subtract", dd(NativeOperators::subtract(&mut native,Some(b.clone()))?));
    o!("interfaces.divide", arithmetic_dd(NativeOperators::divide(&mut native,Some(b.clone()))?));
    o!("interfaces.multiply.int", dd(NativeOperators::multiply_int(&mut native,3)?));
    o!("interfaces.pow", power_dd(NativeOperators::pow(&mut native,5)?,5));
    r!("dd.frexp.empty", DD::frexp_with_outer(a.clone(),Some(arr(vec![]))).map(dd));
    r!("dd.pow.empty", DD::pow_int_long_array_with_outer(a.clone(),2,Some(arr(vec![]))).map(dd));
    r!("dd.math.empty", DDMath::pow(a.clone(),2,Some(arr(vec![]))).map(dd));
}

fn compensated() {
    let small = 2f64.powi(-55); let medium = 2f64.powi(-54); let tiny = 2f64.powi(-56);
    let xs = [DD::of_sum(1.0,small), DD::of_sum(1.0,-small),
        DD::of_sum(-1.0,small), DD::of_sum(-1.0,-small),
        DD::of_sum(2.0,medium), DD::of_sum(2.0,-medium),
        DD::of_sum(-2.0,medium), DD::of_sum(-2.0,-medium),
        DD::of_sum(0.5,tiny), DD::of_sum(-0.5,-tiny)];
    for (i,a) in xs.iter().enumerate() {
        let k = format!("compensated.{i}");
        o!(format!("{k}.input"), dd(a.clone()));
        o!(format!("{k}.floor"), dd(DD::floor_with_outer(a.clone())));
        o!(format!("{k}.ceil"), dd(DD::ceil_with_outer(a.clone())));
        o!(format!("{k}.negate"), dd(DD::negate_with_outer(a.clone())?));
        o!(format!("{k}.abs"), dd(DD::abs_with_outer(a.clone())?));
        o!(format!("{k}.square"), dd(DD::square_with_outer(a.clone())));
        o!(format!("{k}.sqrt"), dd(DD::sqrt_with_outer(a.clone())));
        o!(format!("{k}.reciprocal"), arithmetic_dd(DD::reciprocal_with_outer(a.clone())?));
        o!(format!("{k}.double"), bits(DD::double_value_with_outer(a.clone())));
        o!(format!("{k}.float"), format!("{:x}", DD::float_value_with_outer(a.clone()).to_bits()));
        o!(format!("{k}.int"), DD::int_value_with_outer(a.clone()));
        o!(format!("{k}.long"), DD::long_value_with_outer(a.clone()));
        r!(format!("{k}.decimal"), DD::big_decimal_value_with_outer(a.clone()).map(|v| v.to_string()));
        o!(format!("{k}.finite"), DD::is_finite_with_outer(a.clone()));
        r!(format!("{k}.equalscopy"), DD::equals_object_with_outer(a.clone(), Some(Box::new(
            DD::of_sum(DD::hi_with_outer(a.clone()), DD::lo_with_outer(a.clone()))))));
        o!(format!("{k}.hash"), DD::hash_code_with_outer(a.clone()));
        r!(format!("{k}.string"), DD::to_string_with_outer(a.clone()).map(|text| text.scalar_string().expect("numeric output is scalar")));
        r!(format!("{k}.iszero"), DD::is_zero_with_outer(a.clone()));
        r!(format!("{k}.isone"), DD::is_one_with_outer(a.clone()));
        for (j,b) in xs.iter().enumerate() {
            let p = format!("{k}.pair.{j}");
            o!(format!("{p}.add.d"), dd(DD::add_double_with_outer(a.clone(),DD::hi_with_outer(b.clone()))));
            o!(format!("{p}.add.dd"), dd(DD::add_d_d_with_outer(a.clone(),Some(b.clone()))?));
            o!(format!("{p}.sub.d"), dd(DD::subtract_double_with_outer(a.clone(),DD::hi_with_outer(b.clone()))));
            o!(format!("{p}.sub.dd"), dd(DD::subtract_d_d_with_outer(a.clone(),Some(b.clone()))?));
            o!(format!("{p}.mul.d"), dd(DD::multiply_double_with_outer(a.clone(),DD::hi_with_outer(b.clone()))));
            o!(format!("{p}.mul.dd"), dd(DD::multiply_d_d_with_outer(a.clone(),Some(b.clone()))?));
            o!(format!("{p}.mul.i"), dd(DD::multiply_int_with_outer(a.clone(),3)?));
            o!(format!("{p}.div.d"), arithmetic_dd(DD::divide_double_with_outer(a.clone(),DD::hi_with_outer(b.clone()))));
            o!(format!("{p}.div.dd"), arithmetic_dd(DD::divide_d_d_with_outer(a.clone(),Some(b.clone()))?));
        }
        o!(format!("{k}.unchanged"), dd(a.clone()));
    }
}

fn boundaries() {
    let encodings: [u64;18] = [0,1,2,1_u64<<63,(1_u64<<63)+1,
        0x000fffffffffffff,0x0010000000000000,0x3fefffffffffffff,
        0x3ff0000000000000,0x3ff0000000000001,0x3ff0000000000002,
        0xbff0000000000000,0xbff0000000000001,0x7fefffffffffffff,
        0x7ff0000000000000,0xfff0000000000000,0x7ff8000000000000,0xfff8000000000000];
    let float_encodings: [u32;18] = [0,1,2,1_u32<<31,(1_u32<<31)+1,
        0x007fffff,0x00800000,0x3f7fffff,0x3f800000,0x3f800001,0x3f800002,
        0xbf800000,0xbf800001,0x7f7fffff,0x7f800000,0xff800000,0x7fc00000,0xffc00000];
    for i in 0..encodings.len() {
        let x=f64::from_bits(encodings[i]);
        let y=f64::from_bits(encodings[(i+1)%encodings.len()]);
        let a=f32::from_bits(float_encodings[i]);
        let b=f32::from_bits(float_encodings[(i+1)%float_encodings.len()]);
        for ulps in [-1,0,1,2,3,4,i32::MAX] {
            let k=format!("boundary.ulps.{i}.{ulps}");
            o!(format!("{k}.d.eq"), P::equals_double_double_int(x,y,ulps));
            o!(format!("{k}.d.nan"), P::equals_including_na_n_double_double_int(x,y,ulps));
            o!(format!("{k}.d.cmp"), P::compare_to_double_double_int(x,y,ulps));
            o!(format!("{k}.f.eq"), P::equals_float_float_int(a,b,ulps));
            o!(format!("{k}.f.nan"), P::equals_including_na_n_float_float_int(a,b,ulps));
        }
    }
    let values = [DD::of_double(0.0),DD::of_double(-0.0),DD::of_int(1),DD::of_int(-1),
        DD::of_sum(1.5,f64::from_bits(0x3c90000000000000)),
        DD::of_sum(-0.75,f64::from_bits(0x3c70000000000000)),
        DD::of_double(f64::from_bits(1)),DD::of_double(f64::MIN_POSITIVE),DD::of_double(f64::MAX),
        DD::of_sum(1.0,f64::from_bits(0x3ca0000000000000))];
    for (i,value) in values.into_iter().enumerate() {
        for scale in [i32::MIN,i32::MIN+1,-4097,-2048,-1075,-1074,-1023,-1022,-1021,-53,-1,0,1,52,53,
            1023,1024,2047,4097,i32::MAX] {
            o!(format!("boundary.scalb.{i}.{scale}"), dd(DD::scalb_with_outer(value.clone(),scale)));
        }
        for n in [i32::MIN,i32::MAX,-1075,-1024,-100,-32,-2,-1,0,1,2,32,100,1024,1075] {
            let k=format!("boundary.pow.{i}.{n}");
            o!(format!("{k}.plain"), power_dd(DD::pow_int_with_outer(value.clone(),n)?,n));
            r!(format!("{k}.scaled"), {
                let e=arr(vec![123]); DD::pow_int_long_array_with_outer(value.clone(),n,Some(e.clone()))
                    .map(|v| format!("{},{}",dd(v),e.borrow()[0]))
            });
            r!(format!("{k}.math"), {
                let e=arr(vec![123]); DDMath::pow(value.clone(),n,Some(e.clone()))
                    .map(|v| format!("{},{}",dd(v),e.borrow()[0]))
            });
        }
        o!(format!("boundary.unchanged.{i}"), dd(value.clone()));
    }
    let copies: [u64;12] = [0,1_u64<<63,1,(1_u64<<63)+1,
        0x3ff0000000000000,0xbff0000000000000,
        0x7ff0000000000000,0xfff0000000000000,
        0x7ff8000000000000,0xfff8000000000000,
        0x7ff8000000000042,0xfff8000000000042];
    for (i, raw) in copies.into_iter().enumerate() {
        o!(format!("boundary.copy.{i}"), dd(DD::of_double(f64::from_bits(raw))));
    }
}

fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    arithmetic(); norms(); precision(); sums(); doubles();
    boundaries(); compensated();
    println!("oracle=ok");
}
