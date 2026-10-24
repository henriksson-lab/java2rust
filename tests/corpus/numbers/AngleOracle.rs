//! Independent caller: all algorithms are linked generated production methods.
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use translated::JavaReferenceIdentity;
use translated::{
    Angle, Angle_Deg as Deg, Angle_Rad as Rad, Angle_Turn as Turn, AngleContractsOracle as Client,
    AngleValue, CosAngle, JavaException, JavaString, Reduce,
};

fn bits(x: f64) -> String {
    format!("{:x}", x.to_bits())
}
fn angle(unit: usize, value: f64) -> AngleValue {
    match unit {
        0 => AngleValue::Angle_Turn(Turn::of(value)),
        1 => AngleValue::Angle_Rad(Rad::of(value)),
        _ => AngleValue::Angle_Deg(Deg::of(value)),
    }
}
fn boxed(value: &AngleValue) -> Box<dyn Any> {
    value.__java_clone_to_any()
}
fn units(text: Option<&JavaString>) -> String {
    text.map(|text| {
        text.units()
            .iter()
            .map(|unit| format!("{unit:x}"))
            .collect::<Vec<_>>()
            .join(",")
    })
    .unwrap_or_else(|| "null".into())
}
fn out(key: &str, operation: impl FnOnce() -> Result<String, JavaException>) {
    let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)) {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => format!(
            "ERR:{}:{}",
            error.class_name().rsplit(['.', '$']).next().unwrap(),
            units(error.get_message().as_ref())
        ),
        Err(error) => {
            if let Some(error) = error.downcast_ref::<JavaException>() {
                format!(
                    "ERR:{}:{}",
                    error.class_name().rsplit(['.', '$']).next().unwrap(),
                    units(error.get_message().as_ref())
                )
            } else {
                "PANIC:non-Java exception".into()
            }
        }
    };
    println!("{key}={result}");
}
macro_rules! o {
    ($key:expr, $value:expr) => {
        out(&$key, || Ok(($value).to_string()))
    };
}
fn angles() {
    let values = [
        0.0,
        -0.0,
        0.125,
        0.25,
        0.5,
        1.0,
        -1.0,
        45.0,
        -90.0,
        180.0,
        360.0,
        720.0,
        -1080.0,
        1e6,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::from_bits(0x7ff8000000000011),
    ];
    for unit in 0..3 {
        for (i, &value) in values.iter().enumerate() {
            let a = angle(unit, value);
            let k = format!("angle.{unit}.{i}");
            o!(
                format!("{k}.values"),
                format!(
                    "{},{},{},{}",
                    bits(a.get_as_double()),
                    bits(a.to_turn().get_as_double()),
                    bits(a.to_rad().get_as_double()),
                    bits(a.to_deg().get_as_double())
                )
            );
            o!(
                format!("{k}.cycles"),
                format!(
                    "{},{},{}",
                    bits(a.to_turn().to_rad().to_deg().get_as_double()),
                    bits(a.to_rad().to_deg().to_turn().get_as_double()),
                    bits(a.to_deg().to_turn().to_rad().get_as_double())
                )
            );
            o!(format!("{k}.hash"), a.hash_code());
            o!(
                format!("{k}.identity"),
                format!(
                    "{},{},{}",
                    a.java_reference_identity() == a.to_turn().java_reference_identity(),
                    a.java_reference_identity() == a.to_rad().java_reference_identity(),
                    a.java_reference_identity() == a.to_deg().java_reference_identity()
                )
            );
            o!(format!("{k}.self"), a.equals(Some(boxed(&a)))?);
            o!(format!("{k}.null"), a.equals(None)?);
            o!(
                format!("{k}.foreign"),
                a.equals(Some(Box::new("foreign".to_string())))?
            );
            o!(format!("{k}.class"), a.get_class().get_name());
            for other in 0..3 {
                for (j, &value) in values.iter().enumerate() {
                    let b = angle(other, value);
                    o!(
                        format!("{k}.equals.{other}.{j}"),
                        a.equals(Some(boxed(&b)))?
                    );
                }
            }
            o!(format!("{k}.after"), bits(a.get_as_double()));
        }
    }
    o!("constant.twoPi", bits(Angle::TWO_PI));
    o!("constant.halfPi", bits(Angle::PI_OVER_TWO));
    o!(
        "constant.turnZero",
        bits(Turn::__static_ZERO().get_as_double())
    );
    o!(
        "constant.radZero",
        bits(Rad::__static_ZERO().get_as_double())
    );
    o!(
        "constant.degZero",
        bits(Deg::__static_ZERO().get_as_double())
    );
    o!("constant.radPi", bits(Rad::__static_PI().get_as_double()));
    o!(
        "constant.radTwoPi",
        bits(Rad::__static_TWO_PI().get_as_double())
    );
}
fn reduction_inputs() -> [f64; 23] {
    let bound = std::f64::consts::PI / 3.0;
    [
        0.0,
        -0.0,
        -5e-17,
        5e-17,
        -1.0,
        1.0,
        -Angle::TWO_PI,
        -std::f64::consts::PI,
        std::f64::consts::PI,
        Angle::TWO_PI,
        45.0,
        90.0,
        180.0,
        360.0,
        720.0,
        -1080.0,
        1e6,
        f64::from_bits(bound.to_bits() - 1),
        bound,
        f64::from_bits(bound.to_bits() + 1),
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ]
}
fn reductions() {
    let values = reduction_inputs();
    let operators = [
        Turn::__static_WITHIN_0_AND_1(),
        Rad::__static_WITHIN_0_AND_2PI(),
        Rad::__static_WITHIN_MINUS_PI_AND_PI(),
        Deg::__static_WITHIN_0_AND_360(),
        Turn::normalizer(-0.5),
        Rad::normalizer(std::f64::consts::PI / 3.0),
        Deg::normalizer(-180.0),
    ];
    for (i, op) in operators.iter().enumerate() {
        observe_reduction(i, &values, |x| op.applyAsDouble(x));
    }
    // Local callbacks have their existing local carrier, not a forged Send
    // conversion. Both loops merely invoke generated production algorithms.
    for (i, (offset, period)) in [(0.0, 360.0), (10.0, -360.0), (-0.5, 1.0), (2.0, 0.0)]
        .into_iter()
        .enumerate()
    {
        let op = Client::operator(Arc::new(Reduce::new(offset, period)));
        observe_reduction(i + 7, &values, |x| op.applyAsDouble(x));
    }
    let direct = Reduce::new(10.0, -360.0);
    for (j, &x) in values.iter().enumerate() {
        o!(format!("direct.{j}"), bits(direct.apply_as_double(x)));
    }
}
fn observe_reduction(i: usize, values: &[f64], call: impl Fn(f64) -> f64) {
    for (j, &x) in values.iter().enumerate() {
        let k = format!("reduce.{i}.{j}");
        o!(format!("{k}.value"), bits(call(x)));
        o!(format!("{k}.repeat"), bits(call(x)));
        o!(format!("{k}.twice"), bits(call(call(x))));
    }
}
fn vectors() {
    let values = vec![
        vec![1.0, 0.0],
        vec![0.0, 1.0],
        vec![7.0, 7.0],
        vec![-5.0, 0.0],
        vec![-100.0, 100.0],
        vec![0.0, 0.0],
        vec![1e-200, 1e-200],
        vec![-1e200, -1e200],
        vec![1e200, -1e200],
        vec![1.0, 1.0, 0.0],
        vec![1.0, 1.0, 1.0],
        vec![],
        vec![1.0],
        vec![f64::NAN, 1.0],
        vec![f64::INFINITY, 1.0],
    ]
    .into_iter()
    .map(|value| Rc::new(RefCell::new(value)))
    .collect::<Vec<_>>();
    for (i, a) in values.iter().enumerate() {
        for (j, b) in values.iter().enumerate() {
            let k = format!("cos.{i}.{j}");
            o!(
                format!("{k}.value"),
                bits(CosAngle::value(Some(a.clone()), Some(b.clone()))?)
            );
            o!(
                format!("{k}.repeat"),
                bits(CosAngle::value(Some(a.clone()), Some(b.clone()))?)
            );
        }
    }
}
fn main() {
    angles();
    reductions();
    vectors();
    println!("oracle=ok");
}
