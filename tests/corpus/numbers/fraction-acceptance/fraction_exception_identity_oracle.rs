use translated::{
    ArithmeticUtils, BigFraction, Fraction, GeneralizedContinuedFraction as Gcf,
    GeneralizedContinuedFraction_Coefficient as Coefficient, JavaException, JavaString,
};

fn emit(request: i32, result: Result<(), JavaException>) {
    match result {
        Ok(()) => println!("{request}\tOK"),
        Err(error) => {
            let message = error
                .get_message()
                .map(|text| {
                    text.units()
                        .iter()
                        .map(|unit| format!("{unit:04x}"))
                        .collect::<String>()
                })
                .unwrap_or_else(|| "NULL".into());
            println!(
                "{request}\t{}\t{}\t{message}",
                error.get_class().get_name(),
                error.is_instance(&["ArithmeticException"])
            );
        }
    }
}
fn main() {
    emit(0, Fraction::of_int_int(1, 0).map(|_| ()));
    emit(1, Fraction::from_double(f64::NAN).map(|_| ()));
    emit(2, BigFraction::parse(&JavaString::from("x")).map(|_| ()));
    emit(3, ArithmeticUtils::gcd_int_int(i32::MIN, 0).map(|_| ()));
    emit(
        4,
        Gcf::value_supplier_double_int(&mut || Ok(Coefficient::of(f64::NAN, 1.0)), 1e-12, 2)
            .map(|_| ()),
    );
}
