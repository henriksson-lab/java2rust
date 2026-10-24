use translated::{ContinuedFractionExtension as Extension, JavaException};

fn exception(error: JavaException) -> String {
    let name = error.get_class().get_name();
    // Pinned exception identifiers have no literal '$'; Java uses getSimpleName.
    let simple = name.rsplit(['.', '$']).next().unwrap();
    let message = error
        .get_message()
        .map(|text| {
            text.units()
                .iter()
                .map(|unit| format!("{unit:04x}"))
                .collect::<String>()
        })
        .unwrap_or_else(|| "NULL".into());
    format!(
        "ERR\t{simple}\t{}\t{message}",
        error.is_instance(&["ArithmeticException"])
    )
}
fn main() {
    for mode in 0..6 {
        for defaults in 0..2 {
            if mode == 5 && defaults == 1 {
                continue;
            }
            for point in 0..2 {
                for precision in 0..2 {
                    for limit in [0, 1, 32] {
                        let value = Extension::create(mode);
                        let alias = value.clone();
                        let label = format!("{mode}:{defaults}:{point}:{precision}:{limit}");
                        println!(
                            "{label}:initial\t{}\t{}",
                            Extension::get_calls_with_outer(value.clone()),
                            Extension::get_trace_with_outer(value.clone())
                        );
                        for repeat in 0..2 {
                            Extension::set_factor_with_outer(
                                alias.clone(),
                                f64::from(repeat) + 1.0,
                            );
                            let output = Extension::run_with_outer(
                                value.clone(),
                                f64::from(point) * 2.0,
                                if precision == 0 { 1e-12 } else { f64::NAN },
                                limit,
                                defaults == 1,
                            );
                            let outcome = match output {
                                Ok(number) => format!("{:016x}", number.to_bits()),
                                Err(error) => exception(error),
                            };
                            println!(
                                "{label}:{repeat}\t{outcome}\t{}\t{}",
                                Extension::get_calls_with_outer(alias.clone()),
                                Extension::get_trace_with_outer(alias.clone())
                            );
                        }
                    }
                }
            }
        }
    }
}
