use translated::{GeneralizedContinuedFraction as Fraction, GeneralizedContinuedFraction_Coefficient as Coefficient, JavaException};

// Independent Rust caller of generated production algorithms. No generated
// body or translated Java oracle is included here.
fn main() {
    for (index, (a, b)) in [(1.0, 2.0), (-0.0, 0.0), (-1.0, -2.0), (f64::MAX, f64::MIN_POSITIVE)].into_iter().enumerate() {
        let coefficient = Coefficient::of(a, b);
        println!("coefficient:{index}\t{:016x}\t{:016x}", coefficient.get_a().to_bits(), coefficient.get_b().to_bits());
    }
    for overload in 0..6 {
        for mode in 0..6 {
            if (2..=4).contains(&mode) && overload != 2 && overload != 5 { continue; }
            for (e, epsilon) in [1e-12, 0.0, f64::NAN].into_iter().enumerate() {
                for limit in [0, 1, 64] {
                    let mut calls = 0;
                    let mut generator = || {
                        calls += 1;
                        if calls > 256 { return Err(JavaException::new("IllegalStateException", "supplier budget")); }
                        if mode == 5 && calls == 3 { return Err(JavaException::new("IllegalStateException", "supplier stop")); }
                        let (a, b) = match mode {
                            1 => (f64::from(calls), f64::from(calls + 1)),
                            2 => (1.0, 0.0),
                            3 => (f64::NAN, 1.0),
                            4 => (1.0, f64::INFINITY),
                            _ => (1.0, 1.0),
                        };
                        Ok(Coefficient::of(a, b))
                    };
                    let result = match overload {
                        0 => Fraction::value_supplier(&mut generator),
                        1 => Fraction::value_supplier_double(&mut generator, epsilon),
                        2 => Fraction::value_supplier_double_int(&mut generator, epsilon, limit),
                        3 => Fraction::value_double_supplier(0.0, &mut generator),
                        4 => Fraction::value_double_supplier_double(0.0, &mut generator, epsilon),
                        _ => Fraction::value_double_supplier_double_int(0.0, &mut generator, epsilon, limit),
                    };
                    drop(generator);
                    let label = format!("{overload}:{mode}:{e}:{limit}");
                    match result {
                        Ok(value) => println!("{label}\t{:016x}\t{calls}", value.to_bits()),
                        Err(error) => {
                            let class = error.get_class().get_name();
                            // Pinned exception identifiers have no literal '$'; Java uses getSimpleName.
                            let simple = class.rsplit(['.', '$']).next().unwrap();
                            let message = error.get_message().map(|message| message.units().iter().map(|unit| format!("{unit:04x}")).collect::<String>()).unwrap_or_else(|| "NULL".into());
                            println!("{label}\tERR\t{simple}\t{}\t{message}\t{calls}", error.is_instance(&["ArithmeticException"]));
                        }
                    }
                }
            }
        }
    }
}
