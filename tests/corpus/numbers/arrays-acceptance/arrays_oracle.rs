use std::{cell::RefCell, rc::Rc};
use translated::{
    BitIndexUpdatingInterval, HashIndexSet, IndexSupport, JavaException, KeyUpdatingInterval,
    MultidimensionalCounter, Selection, SortInPlace, Sorting, UpdatingInterval,
};

type ArrayHandle<T> = Rc<RefCell<Vec<T>>>;
fn array<T>(values: Vec<T>) -> ArrayHandle<T> {
    Rc::new(RefCell::new(values))
}
fn status(action: impl FnOnce() -> Result<(), JavaException>) -> String {
    match action() {
        Ok(()) => "OK".into(),
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
            format!("{}:{message}", error.get_class().get_name())
        }
    }
}
fn view<T: std::fmt::Debug>(values: &ArrayHandle<T>) -> String {
    format!("{:?}", &*values.borrow())
}
fn view_double(values: &ArrayHandle<f64>) -> String {
    format!(
        "[{}]",
        values
            .borrow()
            .iter()
            .map(|value| format!("{:x}", value.to_bits()))
            .collect::<Vec<_>>()
            .join(", ")
    )
}
fn data(n: usize, pattern: i32) -> Vec<i32> {
    let mut state = 0x12345678i32 + pattern * 0x123;
    (0..n)
        .map(|i| {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            match pattern {
                0 => (n - i) as i32,
                1 => (i * 17 % 11) as i32 - 5,
                2 => state,
                3 => i as i32,
                _ => {
                    if i % 2 == 0 {
                        i as i32
                    } else {
                        -(i as i32)
                    }
                }
            }
        })
        .collect()
}
fn longs(input: &[i32]) -> Vec<i64> {
    input
        .iter()
        .enumerate()
        .map(|(i, value)| ((*value as i64) << 32) ^ (0x9e3779b9 + i as i64))
        .collect()
}
fn doubles(input: &[i32], pattern: i32) -> Vec<f64> {
    input
        .iter()
        .enumerate()
        .map(|(i, value)| {
            if pattern == 4 {
                if i % 13 == 0 {
                    return -0.0;
                }
                if i % 29 == 0 {
                    return 0.0;
                }
                if i % 17 == 0 {
                    return f64::from_bits(0x7ff8000000000042 + (i % 3) as u64);
                }
                if i % 19 == 0 {
                    return if i % 2 == 0 {
                        f64::INFINITY
                    } else {
                        f64::NEG_INFINITY
                    };
                }
            }
            *value as f64 / 8.0
        })
        .collect()
}
fn selection() {
    for ty in 0..3 {
        for n in [0, 1, 2, 3, 5, 16, 31, 64, 129, 512, 2048, 5000] {
            for pattern in 0..5 {
                for variant in 0..4 {
                    for query in 0..6 {
                        let base = data(n, pattern);
                        let from = if variant >= 2 && n >= 2 { 1 } else { 0 };
                        let to = if variant >= 2 && n >= 2 {
                            n as i32 - 1
                        } else {
                            n as i32
                        };
                        let k = match query {
                            0 => from,
                            1 => from + (to - from) / 2,
                            2 => to - 1,
                            4 => from + (to - from) / 4,
                            5 => from + 3 * (to - from) / 4,
                            _ => to,
                        };
                        let indices = array(match query {
                            0 => vec![],
                            2 => vec![k, from, to - 1, k],
                            4 if to > from => (0..17)
                                .map(|i| from + (to - from - 1) * (16 - i) / 16)
                                .collect(),
                            _ => vec![k],
                        });
                        let (result, values) = match ty {
                            0 => {
                                let a = array(doubles(&base, pattern));
                                let result = status(|| match variant {
                                    0 => Selection::select_double_array_int(a.clone(), k),
                                    1 => Selection::select_double_array_int_array(
                                        a.clone(),
                                        indices.clone(),
                                    ),
                                    2 => Selection::select_double_array_int_int_int(
                                        a.clone(),
                                        from,
                                        to,
                                        k,
                                    ),
                                    _ => Selection::select_double_array_int_int_int_array(
                                        a.clone(),
                                        from,
                                        to,
                                        indices.clone(),
                                    ),
                                });
                                (result, view_double(&a))
                            }
                            1 => {
                                let a = array(base);
                                let result = status(|| match variant {
                                    0 => Selection::select_int_array_int(&a, k),
                                    1 => Selection::select_int_array_int_array(&a, &indices),
                                    2 => Selection::select_int_array_int_int_int(&a, from, to, k),
                                    _ => Selection::select_int_array_int_int_int_array(
                                        &a, from, to, &indices,
                                    ),
                                });
                                (result, view(&a))
                            }
                            _ => {
                                let a = array(longs(&base));
                                let result = status(|| match variant {
                                    0 => Selection::select_long_array_int(&a, k),
                                    1 => Selection::select_long_array_int_array(&a, &indices),
                                    2 => Selection::select_long_array_int_int_int(&a, from, to, k),
                                    _ => Selection::select_long_array_int_int_int_array(
                                        &a, from, to, &indices,
                                    ),
                                });
                                (result, view(&a))
                            }
                        };
                        let keys = if variant % 2 == 1 {
                            view(&indices)
                        } else {
                            "-".into()
                        };
                        println!("selection:{ty}:{n}:{pattern}:{variant}:{query}\t{result}\t{values}\t{keys}");
                    }
                }
            }
        }
    }
    for (case, values) in [[3, 0, 1, 2], [3, 0, 0, 2], [2, 2, 0, 3], [1, 1, 1, 1]]
        .iter()
        .enumerate()
    {
        for variant in 0..2 {
            let a = array(values.to_vec());
            let result = status(|| {
                if variant == 0 {
                    Selection::select_int_array_int_array(&a, &a)
                } else {
                    Selection::select_int_array_int_int_int_array(&a, 0, 4, &a)
                }
            });
            println!("selection-alias:{case}:{variant}\t{result}\t{}", view(&a));
        }
    }
}
fn sort() {
    for (id, name) in ["", "ascending", "ASCENDING ", "UNKNOWN"]
        .into_iter()
        .enumerate()
    {
        println!(
            "enum-invalid:{id}\t{}",
            status(|| {
                match std::panic::catch_unwind(|| {
                    SortInPlace::value_of(translated::JavaString::from_scalar(name))
                }) {
                    Ok(_) => Ok(()),
                    Err(error) => match error.downcast::<JavaException>() {
                        Ok(error) => Err(*error),
                        Err(other) => std::panic::resume_unwind(other),
                    },
                }
            })
        );
    }
    println!(
        "enum-null\t{}",
        status(|| SortInPlace::__java_value_of_nullable_result(None).map(|_| ()))
    );
    for order in SortInPlace::values() {
        println!(
            "enum:{}\t{}\t{}\t{}\t{}\t{}",
            order.ordinal(),
            order.name(),
            order,
            SortInPlace::value_of(order.name()) == order,
            order.compare_to(SortInPlace::ASCENDING),
            order.compare_to(SortInPlace::DESCENDING)
        );
        for n in [0, 1, 3, 8, 33] {
            for pattern in 0..5 {
                for alias in 0..8 {
                    let x = array(doubles(&data(n, pattern), pattern));
                    let y = array((0..n).map(|i| 10000.0 + i as f64).collect());
                    let mut rows = match alias {
                        0 => vec![],
                        1 => vec![Some(y.clone())],
                        2 => vec![Some(y.clone()), Some(y.clone())],
                        3 => vec![Some(x.clone())],
                        4 => vec![Some(x.clone()), Some(x.clone())],
                        5 => vec![Some(x.clone()), Some(y.clone()), Some(x.clone())],
                        6 => vec![Some(y.clone()), Some(array(vec![0.0; n + 1]))],
                        _ => vec![Some(y.clone()), None],
                    };
                    let result = status(|| order.apply(x.clone(), &mut rows));
                    println!(
                        "sort:{}:{n}:{pattern}:{alias}\t{result}\t{}\t{}",
                        order.ordinal(),
                        view_double(&x),
                        view_double(&y)
                    );
                }
            }
        }
    }
}
fn counters() {
    let dimensions = [
        vec![1],
        vec![2, 3],
        vec![3, 1, 2],
        vec![1, 1, 1, 1],
        vec![2, 1, 2, 1, 1, 2],
        vec![],
        vec![0, 2],
        vec![-1],
        vec![2, -3],
        vec![65536, 65536],
        vec![i32::MAX, 2],
    ];
    for (id, dimension) in dimensions.iter().enumerate() {
        let sizes = array(dimension.clone());
        let made = status(|| {
            let counter = MultidimensionalCounter::of(sizes.clone())?;
            println!(
                "counter:{id}:view\t{}\t{}\t{:?}\t{}",
                counter.get_dimension(),
                counter.get_size(),
                counter.get_sizes()?,
                counter.to_string()
            );
            sizes.borrow_mut()[0] = 99;
            let mut copy = counter.get_sizes()?;
            copy[0] = 77;
            println!("counter:{id}:copies\t{:?}", counter.get_sizes()?);
            for index in -1..=counter.get_size() {
                let result = status(|| {
                    let multi = counter.to_multi(index)?;
                    let linear = counter.to_uni(&multi.borrow())?;
                    println!("counter:{id}:index:{index}\t{}\t{linear}", view(&multi));
                    Ok(())
                });
                println!("counter:{id}:index-status:{index}\t{result}");
            }
            let actual = counter.get_sizes()?;
            for slot in 0..actual.len() {
                for upper in 0..2 {
                    let mut coordinate = vec![0; actual.len()];
                    coordinate[slot] = if upper == 0 { -1 } else { actual[slot] };
                    println!(
                        "counter:{id}:invalid:{slot}:{upper}\t{}",
                        status(|| counter.to_uni(&coordinate).map(|_| ()))
                    );
                }
            }
            println!(
                "counter:{id}:arity\t{}",
                status(|| counter.to_uni(&vec![0; actual.len() + 1]).map(|_| ()))
            );
            Ok(())
        });
        println!("counter:{id}:construct\t{made}");
    }
}
fn interval(value: &dyn UpdatingInterval, n: i32) -> String {
    format!(
        "{}:{}:{}",
        value.left().unwrap(),
        value.right().unwrap(),
        IndexSupport::count_indices(Some(value), n).unwrap()
    )
}
fn helpers() {
    for capacity in [-1, 0, 1, 2, 7, 16] {
        let set = HashIndexSet::create(capacity).unwrap();
        for (step, index) in [
            0,
            7,
            14,
            0,
            i32::MAX,
            21,
            28,
            35,
            42,
            49,
            56,
            63,
            70,
            77,
            84,
            91,
            98,
        ]
        .into_iter()
        .enumerate()
        {
            let mut added = false;
            let result = status(|| {
                added = set.borrow_mut().add(index)?;
                Ok(())
            });
            println!("hash:{capacity}:{step}:{index}\t{result}\t{added}");
            if result != "OK" {
                break;
            }
        }
        println!(
            "hash:{capacity}:negative\t{}",
            status(|| set.borrow_mut().add(-1).map(|_| ()))
        );
    }
    println!(
        "hash:capacity\t{}",
        status(|| HashIndexSet::create((1 << 29) + 1).map(|_| ()))
    );
    let inputs = [
        vec![5, 3, 1, 3, 5, 7],
        vec![1],
        vec![7, 7, 7],
        data(64, 3),
        data(64, 0),
        data(256, 3),
    ];
    for (case, input) in inputs.iter().enumerate() {
        let keys = array(input.clone());
        let n = input.len() as i32;
        let value = IndexSupport::create_updating_interval(keys.clone(), n).unwrap();
        println!("factory:{case}\t{}\t{}", interval(&*value, n), view(&keys));
    }
    for bit in 0..2 {
        let keys = [1, 3, 5, 7, 64, 128, 130, 511];
        let mut value: Box<dyn UpdatingInterval> = if bit == 0 {
            Rc::new(RefCell::new(KeyUpdatingInterval::new_int_array_int(
                Some(array(keys.to_vec())),
                keys.len() as i32,
            )))
            .into()
        } else {
            let mut bits = BitIndexUpdatingInterval::new_int_int(1, 511);
            for k in keys {
                bits.set(k).unwrap();
            }
            Rc::new(RefCell::new(bits)).into()
        };
        let n = keys.len() as i32;
        println!("interval:{bit}:initial\t{}", interval(&*value, n));
        value.update_left(3).unwrap();
        value.update_right(130).unwrap();
        let mut lower = value.split_left(7, 64).unwrap();
        println!(
            "interval:{bit}:split\t{}\t{}",
            interval(&*lower, n),
            interval(&*value, n)
        );
        lower.update_left(5).unwrap();
        value.update_right(128).unwrap();
        println!(
            "interval:{bit}:reuse\t{}\t{}",
            interval(&*lower, n),
            interval(&*value, n)
        );
    }
    for n in [1, 3, 20] {
        for operation in 0..2 {
            let keys = array(data(n, 1).iter().map(|x| x + 5).collect());
            let count = if operation == 0 {
                Sorting::insertion_sort_indices(keys.clone(), n as i32)
            } else {
                Sorting::sort_indices(keys.clone(), n as i32)
            }
            .unwrap();
            println!("indices:{n}:{operation}\t{count}\t{}", view(&keys));
        }
    }
}
fn sorting_helpers() {
    let many = array((0..1024).map(|i| i * 3 + 1).collect());
    let mut large: Box<dyn UpdatingInterval> = Rc::new(RefCell::new(
        KeyUpdatingInterval::new_int_array_int(Some(many), 1024),
    ))
    .into();
    let mut prefix = large.split_left(1000, 1500).unwrap();
    println!(
        "interval-large:split\t{}\t{}",
        interval(&*prefix, 1024),
        interval(&*large, 1024)
    );
    prefix.update_left(301).unwrap();
    large.update_right(2701).unwrap();
    println!(
        "interval-large:reuse\t{}\t{}",
        interval(&*prefix, 1024),
        interval(&*large, 1024)
    );
    for kind in 0..3 {
        for pattern in 0..5 {
            for operation in 0..5 {
                let base = data(9, pattern);
                let values = match kind {
                    0 => {
                        let a = array(doubles(&base, 0));
                        match operation {
                            0 => Sorting::sort_double_array_int_int(a.clone(), 1, 7),
                            1 => Sorting::sort3_double_array_int_int_int(a.clone(), 1, 4, 7),
                            2 => Sorting::sort5_double_array_int_int_int_int_int(
                                a.clone(),
                                0,
                                2,
                                4,
                                6,
                                8,
                            ),
                            3 => Sorting::lower_median4_double_array_int_int_int_int(
                                a.clone(),
                                1,
                                3,
                                5,
                                7,
                            ),
                            _ => Sorting::upper_median4_double_array_int_int_int_int(
                                a.clone(),
                                1,
                                3,
                                5,
                                7,
                            ),
                        }
                        .unwrap();
                        view_double(&a)
                    }
                    1 => {
                        let a = array(base);
                        match operation {
                            0 => Sorting::sort_int_array_int_int(a.clone(), 1, 7),
                            1 => Sorting::sort3_int_array_int_int_int(a.clone(), 1, 4, 7),
                            2 => Sorting::sort5_int_array_int_int_int_int_int(
                                a.clone(),
                                0,
                                2,
                                4,
                                6,
                                8,
                            ),
                            3 => Sorting::lower_median4_int_array_int_int_int_int(
                                a.clone(),
                                1,
                                3,
                                5,
                                7,
                            ),
                            _ => Sorting::upper_median4_int_array_int_int_int_int(
                                a.clone(),
                                1,
                                3,
                                5,
                                7,
                            ),
                        }
                        .unwrap();
                        view(&a)
                    }
                    _ => {
                        let a = array(longs(&base));
                        match operation {
                            0 => Sorting::sort_long_array_int_int(a.clone(), 1, 7),
                            1 => Sorting::sort3_long_array_int_int_int(a.clone(), 1, 4, 7),
                            2 => Sorting::sort5_long_array_int_int_int_int_int(
                                a.clone(),
                                0,
                                2,
                                4,
                                6,
                                8,
                            ),
                            3 => Sorting::lower_median4_long_array_int_int_int_int(
                                a.clone(),
                                1,
                                3,
                                5,
                                7,
                            ),
                            _ => Sorting::upper_median4_long_array_int_int_int_int(
                                a.clone(),
                                1,
                                3,
                                5,
                                7,
                            ),
                        }
                        .unwrap();
                        view(&a)
                    }
                };
                println!("sorting-helper:{kind}:{pattern}:{operation}\t{values}");
            }
        }
    }
}
fn ranges() {
    for kind in 0..3 {
        for (id, [from, to, k]) in [
            [-1, 4, 1],
            [0, 6, 2],
            [4, 2, 3],
            [0, 0, 0],
            [5, 5, 5],
            [2, 4, 1],
        ]
        .into_iter()
        .enumerate()
        {
            for variant in 0..2 {
                let indices = array(vec![k, k]);
                let (result, values) = match kind {
                    0 => {
                        let a = array(doubles(&data(5, 0), 0));
                        let result = status(|| {
                            if variant == 0 {
                                Selection::select_double_array_int_int_int(a.clone(), from, to, k)
                            } else {
                                Selection::select_double_array_int_int_int_array(
                                    a.clone(),
                                    from,
                                    to,
                                    indices.clone(),
                                )
                            }
                        });
                        (result, view_double(&a))
                    }
                    1 => {
                        let a = array(data(5, 0));
                        let result = status(|| {
                            if variant == 0 {
                                Selection::select_int_array_int_int_int(&a, from, to, k)
                            } else {
                                Selection::select_int_array_int_int_int_array(
                                    &a, from, to, &indices,
                                )
                            }
                        });
                        (result, view(&a))
                    }
                    _ => {
                        let a = array(longs(&data(5, 0)));
                        let result = status(|| {
                            if variant == 0 {
                                Selection::select_long_array_int_int_int(&a, from, to, k)
                            } else {
                                Selection::select_long_array_int_int_int_array(
                                    &a, from, to, &indices,
                                )
                            }
                        });
                        (result, view(&a))
                    }
                };
                println!(
                    "selection-range:{kind}:{id}:{variant}\t{result}\t{values}\t{}",
                    view(&indices)
                );
            }
        }
    }
}
fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("selection") => {
            selection();
            ranges();
        }
        Some("sort") => sort(),
        Some("counter") => counters(),
        Some("helpers") => {
            helpers();
            sorting_helpers();
        }
        _ => panic!("unknown oracle mode"),
    }
}
