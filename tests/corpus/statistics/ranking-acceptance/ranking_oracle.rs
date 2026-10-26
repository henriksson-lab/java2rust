use std::{cell::RefCell, rc::Rc};
use translated::{IntUnaryOperator, NaNStrategy, NaturalRanking, TiesStrategy};

fn data(k: usize) -> Vec<f64> {
    match k {
        0 => vec![],
        1 => vec![4.0],
        2 => vec![3.0, 1.0, 2.0, 1.0],
        3 => vec![-0.0, 0.0, -0.0, 0.0],
        4 => vec![
            f64::from_bits(0x7ff8000000000042),
            f64::from_bits(0xfff8000000000043),
        ],
        5 => vec![f64::NAN, 3.0, 1.0, 3.0, f64::NAN],
        6 => vec![f64::NEG_INFINITY, f64::INFINITY, 2.0, 2.0, f64::NAN],
        _ => (0..40).map(|i| (i % 3) as f64).collect(),
    }
}
fn bits(values: &[f64]) -> String {
    values
        .iter()
        .map(|v| format!("{:x}", v.to_bits()))
        .collect::<Vec<_>>()
        .join(",")
}
fn row(label: &str, rank: &mut NaturalRanking, values: Vec<f64>) {
    let input = Rc::new(RefCell::new(values));
    let before = bits(&input.borrow());
    let (status, output, alias) = match rank.apply(Rc::clone(&input)) {
        Ok(result) => (
            "OK".to_string(),
            bits(&result.borrow()),
            Rc::ptr_eq(&result, &input),
        ),
        Err(error) => (
            format!(
                "{}:{}",
                error.get_class().get_name(),
                error
                    .get_message()
                    .map(|text| text
                        .units()
                        .iter()
                        .map(|unit| format!("{unit:04x}"))
                        .collect::<String>())
                    .unwrap_or_else(|| "NULL".into())
            ),
            "-".to_string(),
            false,
        ),
    };
    println!(
        "{label}\t{status}\t{output}\t{}\t{alias}\t{}\t{}",
        before == bits(&input.borrow()),
        rank.get_nan_strategy().unwrap().name(),
        rank.get_ties_strategy().unwrap().name()
    );
}
fn deterministic() -> IntUnaryOperator {
    IntUnaryOperator::from_fn(|_| 0)
}
fn main() {
    for nan in NaNStrategy::values() {
        for ties in TiesStrategy::values() {
            let mut rank = if ties == TiesStrategy::RANDOM {
                NaturalRanking::new_na_n_strategy_int_unary_operator(nan, deterministic())
            } else {
                NaturalRanking::new_na_n_strategy_ties_strategy(nan, ties)
            };
            for k in 0..8 {
                row(
                    &format!("{}:{}:{k}", nan.name(), ties.name()),
                    &mut rank,
                    data(k),
                );
            }
        }
    }
    let mut constructors = [
        NaturalRanking::new(),
        NaturalRanking::new_na_n_strategy(NaNStrategy::MINIMAL),
        NaturalRanking::new_ties_strategy(TiesStrategy::MINIMUM),
        NaturalRanking::new_na_n_strategy_ties_strategy(
            NaNStrategy::FIXED,
            TiesStrategy::SEQUENTIAL,
        ),
        NaturalRanking::new_int_unary_operator(deterministic()),
        NaturalRanking::new_na_n_strategy_int_unary_operator(NaNStrategy::MINIMAL, deterministic()),
    ];
    for (i, rank) in constructors.iter_mut().enumerate() {
        row(&format!("ctor:{i}"), rank, vec![5.0, 2.0, 2.0]);
    }
    let mut random = NaturalRanking::new_ties_strategy(TiesStrategy::RANDOM);
    let mut valid = true;
    for _ in 0..16 {
        let input = Rc::new(RefCell::new(vec![3.0; 8]));
        let output = random.apply(Rc::clone(&input)).unwrap();
        let mut ranks = output.borrow().clone();
        ranks.sort_by(f64::total_cmp);
        for i in 0..8 {
            valid &= ranks[i] == (i + 1) as f64 && input.borrow()[i] == 3.0;
        }
    }
    println!("native-random\t{valid}");
}
