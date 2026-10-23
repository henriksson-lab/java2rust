//! Caller only: all quaternion/interpolation algorithms come from generated production.
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use translated::{
    JavaException, JavaString, Quaternion as Q, QuaternionContractsOracle, Slerp as S,
};
type Quaternion = Arc<Mutex<Q>>;

fn bits(value: f64) -> String {
    format!("{:x}", value.to_bits())
}
fn parts(value: Quaternion) -> String {
    format!(
        "{},{},{},{}",
        bits(Q::get_w_with_outer(value.clone())),
        bits(Q::get_x_with_outer(value.clone())),
        bits(Q::get_y_with_outer(value.clone())),
        bits(Q::get_z_with_outer(value))
    )
}
fn units(value: Option<&JavaString>) -> String {
    value
        .map(|text| {
            text.units()
                .iter()
                .map(|unit| format!("{unit:x}"))
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_else(|| "null".into())
}
fn error(value: &JavaException) -> String {
    let mut result = format!(
        "ERR:{}:{}",
        value.class_name().rsplit(['.', '$']).next().unwrap(),
        units(value.get_message().as_ref())
    );
    if let Some(cause) = value.cause() {
        result.push_str(&format!(":CAUSE:{}", error(cause)));
    }
    result
}
fn out(key: &str, operation: impl FnOnce() -> Result<String, JavaException>) {
    let value = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)) {
        Ok(Ok(value)) => value,
        Ok(Err(value)) => error(&value),
        Err(value) => {
            if let Some(value) = value.downcast_ref::<JavaException>() {
                error(value)
            } else if let Some(value) = value.downcast_ref::<String>() {
                format!("PANIC:{value}")
            } else if let Some(value) = value.downcast_ref::<&str>() {
                format!("PANIC:{value}")
            } else {
                "PANIC:unknown payload".into()
            }
        }
    };
    println!("{key}={value}");
}
macro_rules! o {
    ($key:expr, $value:expr) => {
        out(&$key, || Ok(($value).to_string()));
    };
}
macro_rules! p {
    ($key:expr, $value:expr) => {
        out(&$key, || Ok(parts($value)));
    };
}
macro_rules! f {
    ($key:expr, $value:expr) => {
        out(&$key, || Ok(parts($value?)));
    };
}
fn inputs() -> Vec<[f64; 4]> {
    vec![
        [0.0, 0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
        [1.0, 2.0, 3.0, 4.0],
        [-1.0, 2.0, -3.0, 4.0],
        [0.1, 0.2, -0.3, 0.4],
        [-0.0, 0.0, -0.0, 0.0],
        [1e-100, -1e-100, 0.0, 0.0],
        [1e100, 1e100, -1e100, 1e100],
        [f64::MIN_POSITIVE, 0.0, 0.0, 0.0],
        [f64::MAX, 1.0, 2.0, 3.0],
        [f64::NAN, 1.0, 2.0, 3.0],
        [1.0, f64::INFINITY, 0.0, -0.0],
    ]
}
fn make(value: [f64; 4]) -> Quaternion {
    Q::of_double_double_double_double(value[0], value[1], value[2], value[3])
}
fn numeric() {
    let values = inputs();
    let epsilons = [-1.0, 0.0, 1e-15, 1.0, f64::NAN];
    for (i, &v) in values.iter().enumerate() {
        let q = make(v);
        let k = format!("q.{i}");
        p!(format!("{k}.parts"), q.clone());
        o!(
            format!("{k}.scalar"),
            bits(Q::get_scalar_part_with_outer(q.clone()))
        );
        out(&format!("{k}.vector"), || {
            let a = Q::get_vector_part_with_outer(q.clone());
            let a = a.borrow();
            Ok(format!("{},{},{}", bits(a[0]), bits(a[1]), bits(a[2])))
        });
        p!(format!("{k}.conjugate"), Q::conjugate_with_outer(q.clone()));
        out(&format!("{k}.norm"), || {
            Ok(bits(Q::norm_with_outer(q.clone())?))
        });
        out(&format!("{k}.normSq"), || {
            Ok(bits(Q::norm_sq_with_outer(q.clone())?))
        });
        f!(format!("{k}.normalize"), Q::normalize_with_outer(q.clone()));
        f!(
            format!("{k}.polar"),
            Q::positive_polar_form_with_outer(q.clone())
        );
        f!(format!("{k}.negate"), Q::negate_with_outer(q.clone()));
        f!(format!("{k}.inverse"), Q::inverse_with_outer(q.clone()));
        o!(format!("{k}.hash"), Q::hash_code_with_outer(q.clone()));
        o!(
            format!("{k}.string"),
            units(Some(&Q::to_string_with_outer(q.clone())))
        );
        out(&format!("{k}.equals.self"), || {
            Ok(Q::equals_object_with_outer(q.clone(), Some(Box::new(q.clone())))?.to_string())
        });
        out(&format!("{k}.equals.fresh"), || {
            Ok(Q::equals_object_with_outer(q.clone(), Some(Box::new(make(v))))?.to_string())
        });
        out(&format!("{k}.equals.null"), || {
            Ok(Q::equals_object_with_outer(q.clone(), None)?.to_string())
        });
        out(&format!("{k}.equals.foreign"), || {
            Ok(
                Q::equals_object_with_outer(q.clone(), Some(Box::new(JavaString::fresh("other"))))?
                    .to_string(),
            )
        });
        out(&format!("{k}.normalized.state"), || {
            let n = Q::normalize_with_outer(q.clone())?;
            let p = Q::positive_polar_form_with_outer(n.clone())?;
            Ok(format!(
                "{},{},{},{},{},{},{},{}",
                Arc::ptr_eq(&q, &n),
                Arc::ptr_eq(&n, &Q::normalize_with_outer(n.clone())?),
                Arc::ptr_eq(&p, &Q::normalize_with_outer(p.clone())?),
                Arc::ptr_eq(&p, &Q::positive_polar_form_with_outer(p.clone())?),
                bits(Q::norm_with_outer(n.clone())?),
                bits(Q::norm_sq_with_outer(n.clone())?),
                Q::is_unit_with_outer(n.clone(), -1.0)?,
                Q::is_unit_with_outer(n, f64::NAN)?
            ))
        });
        for (e, &eps) in epsilons.iter().enumerate() {
            out(&format!("{k}.eps.{e}.unit"), || {
                Ok(Q::is_unit_with_outer(q.clone(), eps)?.to_string())
            });
            o!(
                format!("{k}.eps.{e}.pure"),
                Q::is_pure_with_outer(q.clone(), eps)
            );
        }
        for (j, &b) in values.iter().enumerate() {
            let r = make(b);
            let key = format!("{k}.binary.{j}");
            f!(
                format!("{key}.multiply.static"),
                Q::multiply_quaternion_quaternion(Some(q.clone()), Some(r.clone()))
            );
            f!(
                format!("{key}.multiply"),
                Q::multiply_quaternion_with_outer(q.clone(), Some(r.clone()))
            );
            f!(
                format!("{key}.add.static"),
                Q::add_quaternion_quaternion(Some(q.clone()), Some(r.clone()))
            );
            f!(
                format!("{key}.add"),
                Q::add_quaternion_with_outer(q.clone(), Some(r.clone()))
            );
            f!(
                format!("{key}.subtract.static"),
                Q::subtract_quaternion_quaternion(Some(q.clone()), Some(r.clone()))
            );
            f!(
                format!("{key}.subtract"),
                Q::subtract_quaternion_with_outer(q.clone(), Some(r.clone()))
            );
            out(&format!("{key}.dot.static"), || {
                Ok(bits(Q::dot_quaternion_quaternion(
                    Some(q.clone()),
                    Some(r.clone()),
                )?))
            });
            out(&format!("{key}.dot"), || {
                Ok(bits(Q::dot_quaternion_with_outer(
                    q.clone(),
                    Some(r.clone()),
                )?))
            });
            out(&format!("{key}.equals"), || {
                Ok(Q::equals_object_with_outer(q.clone(), Some(Box::new(r.clone())))?.to_string())
            });
            for (e, &eps) in epsilons.iter().enumerate() {
                out(&format!("{key}.equals.{e}"), || {
                    Ok(
                        Q::equals_quaternion_double_with_outer(q.clone(), Some(r.clone()), eps)?
                            .to_string(),
                    )
                });
            }
        }
        for (j, &scale) in [
            0.0,
            -0.0,
            1.0,
            -1.0,
            0.5,
            2.0,
            1e-100,
            1e100,
            f64::INFINITY,
            f64::NAN,
        ]
        .iter()
        .enumerate()
        {
            p!(
                format!("{k}.scalar.{j}.multiply"),
                Q::multiply_double_with_outer(q.clone(), scale)
            );
            p!(
                format!("{k}.scalar.{j}.divide"),
                Q::divide_with_outer(q.clone(), scale)
            );
        }
        p!(format!("{k}.after"), q);
    }
}
fn arrays_and_parsing() {
    for length in 0..=4 {
        let a = Rc::new(RefCell::new(vec![0.0; length]));
        let k = format!("factory.length.{length}");
        f!(format!("{k}.pure"), Q::of_double_array(Some(a.clone())));
        f!(
            format!("{k}.scalar"),
            Q::of_double_double_array(1.0, Some(&a))
        );
    }
    f!("factory.null.pure", Q::of_double_array(None));
    f!("factory.null.scalar", Q::of_double_double_array(1.0, None));
    out("factory.copy", || {
        let a = Rc::new(RefCell::new(vec![2.0, 3.0, 4.0]));
        let q = Q::of_double_double_array(1.0, Some(&a))?;
        let r = Q::of_double_array(Some(a.clone()))?;
        a.borrow_mut()[0] = 99.0;
        let b = Q::get_vector_part_with_outer(q.clone());
        b.borrow_mut()[1] = 99.0;
        Ok(format!(
            "{};{};{}",
            parts(q.clone()),
            parts(r),
            !Rc::ptr_eq(
                &Q::get_vector_part_with_outer(q.clone()),
                &Q::get_vector_part_with_outer(q)
            )
        ))
    });
    let mut strings = [
        "[1 2 3 4]",
        "[-0.0 0.0 -0.0 0.0]",
        "[NaN Infinity -Infinity NaN]",
        "[0x1p2 -0x1p-4 1f 2D]",
        "[1e9999 1e-9999 1 2]",
        "",
        "[1 2 3]",
        "[1 2 3 4 5]",
        "(1 2 3 4]",
        "[1 2 3 4)",
        "[ 1 2 3 4]",
        "[1  2 3 4]",
        "[1 2 3 4 ]",
        "[a 2 3 4]",
        "[1 a 3 4]",
        "[1 2 a 4]",
        "[1 2 3 a]",
    ]
    .into_iter()
    .map(|s| Some(JavaString::fresh(s)))
    .collect::<Vec<_>>();
    strings.push(Some(JavaString::from_units(vec![
        91, 0xd800, 32, 50, 32, 51, 32, 52, 93,
    ])));
    strings.push(None);
    for (i, s) in strings.into_iter().enumerate() {
        f!(format!("parse.{i}"), Q::parse(s));
    }
    let q = make([1.0, 2.0, 3.0, 4.0]);
    f!("null.add", Q::add_quaternion_with_outer(q.clone(), None));
    f!(
        "null.subtract",
        Q::subtract_quaternion_with_outer(q.clone(), None)
    );
    f!(
        "null.multiply",
        Q::multiply_quaternion_with_outer(q.clone(), None)
    );
    out("null.dot", || {
        Ok(bits(Q::dot_quaternion_with_outer(q.clone(), None)?))
    });
    out("null.equals.eps", || {
        Ok(Q::equals_quaternion_double_with_outer(q, None, 1e-10)?.to_string())
    });
}
fn interpolations() {
    let pairs = [
        (Q::__static_ONE(), Q::__static_I()),
        (Q::__static_ONE(), make([1.0, 1e-5, 0.0, 0.0])),
        (Q::__static_I(), make([0.0, -1.0, 0.0, 0.0])),
        (make([1.0, 2.0, 3.0, 4.0]), make([-2.0, 3.0, -4.0, 5.0])),
    ];
    let ts = [-0.5, -0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.5];
    for (i, (start, end)) in pairs.into_iter().enumerate() {
        let start = Q::positive_polar_form_with_outer(start).expect("valid interpolation start");
        let interpolation =
            S::new(Some(start.clone()), Some(end.clone())).expect("valid interpolation endpoints");
        // This adapter is itself translated Java. Do not reconstruct its
        // source-to-native SAM conversion by hand in the Rust caller.
        let callback = QuaternionContractsOracle::as_callback(interpolation.clone());
        for (j, &t) in ts.iter().enumerate() {
            let k = format!("slerp.{i}.{j}");
            out(&format!("{k}.apply"), || {
                Ok(parts(
                    S::apply_with_outer(interpolation.clone(), t)?
                        .expect("non-null production interpolation result"),
                ))
            });
            out(&format!("{k}.callback"), || {
                Ok(parts(
                    callback
                        .apply_result(t)?
                        .expect("non-null production callback result"),
                ))
            });
            out(&format!("{k}.repeat"), || {
                Ok(parts(
                    S::apply_with_outer(interpolation.clone(), t)?
                        .expect("non-null repeated interpolation result"),
                ))
            });
            out(&format!("{k}.startIdentity"), || {
                Ok(Arc::ptr_eq(
                    &S::apply_with_outer(interpolation.clone(), t)?
                        .expect("non-null identity observation"),
                    &start,
                )
                .to_string())
            });
        }
        o!(
            format!("slerp.{i}.after"),
            format!("{};{}", parts(start), parts(end))
        );
    }
    out("slerp.null.start", || {
        Ok(parts(
            S::apply_with_outer(S::new(None, Some(Q::__static_ONE()))?, 0.5)?
                .expect("non-null result"),
        ))
    });
    out("slerp.null.end", || {
        Ok(parts(
            S::apply_with_outer(S::new(Some(Q::__static_ONE()), None)?, 0.5)?
                .expect("non-null result"),
        ))
    });
    out("slerp.zero.start", || {
        Ok(parts(
            S::apply_with_outer(
                S::new(Some(Q::__static_ZERO()), Some(Q::__static_ONE()))?,
                0.5,
            )?
            .expect("non-null result"),
        ))
    });
    out("slerp.zero.end", || {
        Ok(parts(
            S::apply_with_outer(
                S::new(Some(Q::__static_ONE()), Some(Q::__static_ZERO()))?,
                0.5,
            )?
            .expect("non-null result"),
        ))
    });
}
fn main() {
    p!("constant.ZERO", Q::__static_ZERO());
    p!("constant.ONE", Q::__static_ONE());
    p!("constant.I", Q::__static_I());
    p!("constant.J", Q::__static_J());
    p!("constant.K", Q::__static_K());
    o!(
        "constant.identity",
        Arc::ptr_eq(&Q::__static_ZERO(), &Q::__static_ZERO())
            && Arc::ptr_eq(&Q::__static_ONE(), &Q::__static_ONE())
            && Arc::ptr_eq(&Q::__static_I(), &Q::__static_I())
            && Arc::ptr_eq(&Q::__static_J(), &Q::__static_J())
            && Arc::ptr_eq(&Q::__static_K(), &Q::__static_K())
    );
    numeric();
    arrays_and_parsing();
    interpolations();
    println!("oracle=ok");
}
