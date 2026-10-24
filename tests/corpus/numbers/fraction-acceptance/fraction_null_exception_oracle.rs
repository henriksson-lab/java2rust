use java_math_runtime::BigInteger;
use translated::{BigFraction, Fraction, JavaException};

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
                error.is_instance(&["NullPointerException"])
            );
        }
    }
}
fn main() {
    let fraction = Fraction::of_int_int(2, 3).unwrap().unwrap();
    let big = BigFraction::of_int_int(2, 3).unwrap().unwrap();
    emit(0, Fraction::parse(None).map(|_| ()));
    emit(1, BigFraction::parse(None).map(|_| ()));
    emit(2, BigFraction::of_big_integer(None).map(|_| ()));
    emit(
        3,
        BigFraction::add_big_fraction_with_outer(big.clone(), None).map(|_| ()),
    );
    emit(
        4,
        Fraction::compare_to_with_outer(fraction.clone(), None).map(|_| ()),
    );
    emit(
        5,
        BigFraction::compare_to_with_outer(big.clone(), None).map(|_| ()),
    );
    emit(
        6,
        BigFraction::add_big_integer_with_outer(big.clone(), None).map(|_| ()),
    );
    emit(
        7,
        BigFraction::of_big_integer_big_integer(Some(BigInteger::value_of(1)), None).map(|_| ()),
    );
    emit(
        8,
        BigFraction::of_big_integer_big_integer(None, Some(BigInteger::value_of(1))).map(|_| ()),
    );
    emit(
        9,
        BigFraction::subtract_big_integer_with_outer(big.clone(), None).map(|_| ()),
    );
    emit(
        10,
        BigFraction::multiply_big_integer_with_outer(big.clone(), None).map(|_| ()),
    );
    emit(
        11,
        BigFraction::divide_big_integer_with_outer(big.clone(), None).map(|_| ()),
    );
    emit(
        12,
        BigFraction::subtract_big_fraction_with_outer(big.clone(), None).map(|_| ()),
    );
    emit(
        13,
        BigFraction::multiply_big_fraction_with_outer(big.clone(), None).map(|_| ()),
    );
    emit(
        14,
        BigFraction::divide_big_fraction_with_outer(big.clone(), None).map(|_| ()),
    );
    emit(
        15,
        Fraction::add_fraction_with_outer(fraction.clone(), None).map(|_| ()),
    );
    emit(
        16,
        Fraction::subtract_fraction_with_outer(fraction.clone(), None).map(|_| ()),
    );
    emit(
        17,
        Fraction::multiply_fraction_with_outer(fraction.clone(), None).map(|_| ()),
    );
    emit(
        18,
        Fraction::divide_fraction_with_outer(fraction, None).map(|_| ()),
    );
    emit(
        19,
        BigFraction::big_decimal_value_rounding_mode_with_outer(big.clone(), None).map(|_| ()),
    );
    emit(
        20,
        BigFraction::big_decimal_value_int_rounding_mode_with_outer(big, 3, None).map(|_| ()),
    );
    // The invoking-only FnMut ABI cannot express a null pointer directly.
    // Co-translated Java calls express those two nullable source arguments;
    // the bridge invokes the generated public algorithms without catch/repair.
    emit(
        21,
        translated::FractionNullBoundary::invoke_supplier(0).map(|_| ()),
    );
    emit(
        22,
        translated::FractionNullBoundary::invoke_supplier(1).map(|_| ()),
    );
}
