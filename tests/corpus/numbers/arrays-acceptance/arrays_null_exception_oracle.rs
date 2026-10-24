use std::{cell::RefCell, rc::Rc};
use translated::{JavaException, MultidimensionalCounter, Selection, SortInPlace};
fn array<T>(values: Vec<T>) -> Rc<RefCell<Vec<T>>> {
    Rc::new(RefCell::new(values))
}
fn status(result: Result<(), JavaException>) -> String {
    match result {
        Ok(()) => "OK\tfalse\tNULL".into(),
        Err(error) => {
            let message = error
                .get_message()
                .map(|s| {
                    s.units()
                        .iter()
                        .map(|u| format!("{u:04x}"))
                        .collect::<String>()
                })
                .unwrap_or_else(|| "NULL".into());
            format!(
                "{}\t{}\t{message}",
                error.get_class().get_name(),
                error.is_instance(&["NullPointerException"])
            )
        }
    }
}
fn main() {
    for which in 0..20 {
        let doubles = array(vec![3.0f64, 1.0, 2.0]);
        let ints = array(vec![3, 1, 2]);
        let longs = array(vec![3i64, 1, 2]);
        let k = array(vec![0]);
        let result = (|| -> Result<(), JavaException> {
            match which {
                0 => Selection::select_double_array_int(None, 0),
                1 => Selection::select_double_array_int_array(None, Some(k.clone())),
                2 => Selection::select_double_array_int_int_int(None, 0, 1, 0),
                3 => Selection::select_double_array_int_int_int_array(None, 0, 1, k.clone()),
                4 => Selection::select_int_array_int(None, 0),
                5 => Selection::select_int_array_int_array(None, Some(&k)),
                6 => Selection::select_int_array_int_int_int(None, 0, 1, 0),
                7 => Selection::select_int_array_int_int_int_array(None, 0, 1, &k),
                8 => Selection::select_long_array_int(None, 0),
                9 => Selection::select_long_array_int_array(None, Some(&k)),
                10 => Selection::select_long_array_int_int_int(None, 0, 1, 0),
                11 => Selection::select_long_array_int_int_int_array(None, 0, 1, &k),
                12 => Selection::select_double_array_int_array(Some(doubles.clone()), None),
                13 => Selection::select_int_array_int_array(Some(&ints), None),
                14 => Selection::select_long_array_int_array(Some(&longs), None),
                15 => MultidimensionalCounter::of(None).map(|_| ()),
                16 => MultidimensionalCounter::of(Some(array(vec![2, 3])))?
                    .to_uni(None)
                    .map(|_| ()),
                17 => SortInPlace::ASCENDING.apply(None, Some(&mut [])),
                18 => SortInPlace::DESCENDING.apply(Some(doubles.clone()), None),
                _ => SortInPlace::__java_value_of_nullable_result(None).map(|_| ()),
            }
        })();
        let bits = doubles
            .borrow()
            .iter()
            .map(|x| format!("{:x}", x.to_bits()))
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "{which}\t{}\t[{bits}]\t{:?}\t{:?}",
            status(result),
            ints.borrow(),
            longs.borrow()
        );
    }
}
