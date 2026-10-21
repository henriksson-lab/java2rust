//! Independent caller: sampling algorithms exist only in generated upstream code.
use std::{any::Any, cell::RefCell, rc::Rc};
use translated::{CollectionSampler, JavaException, ListSampler, PermutationSampler, UniformRandomProvider};

#[derive(Default)]
struct ScriptState { position: usize, bounds: Vec<i32> }
#[derive(Clone, Default)]
struct Scripted(Rc<RefCell<ScriptState>>);
impl Scripted {
    fn boxed(&self) -> Box<dyn UniformRandomProvider> { Box::new(self.clone()) }
    fn bounds(&self) -> Vec<i32> { self.0.borrow().bounds.clone() }
}
impl UniformRandomProvider for Scripted {
    fn next_int_int(&mut self, n: i32) -> Result<i32, JavaException> {
        assert!(n > 0);
        let values = [0, 7, 1, i32::MAX, 3, 0, 11, 2, 19];
        let mut state = self.0.borrow_mut();
        let value = values[state.position % values.len()] % n;
        state.position += 1;
        state.bounds.push(n);
        Ok(value)
    }
    fn next_int(&mut self) -> Result<i32, JavaException> { panic!("unexpected nextInt") }
    fn next_long(&mut self) -> Result<i64, JavaException> { panic!("unexpected nextLong") }
    fn next_long_long(&mut self, _: i64) -> Result<i64, JavaException> { panic!("unexpected boundedLong") }
    fn next_boolean(&mut self) -> Result<bool, JavaException> { panic!("unexpected boolean") }
    fn next_float(&mut self) -> Result<f32, JavaException> { panic!("unexpected float") }
    fn next_double(&mut self) -> Result<f64, JavaException> { panic!("unexpected double") }
    fn next_bytes_byte_array(&self, _: Rc<RefCell<Vec<i8>>>) -> Result<(), JavaException> { panic!("unexpected bytes") }
    fn next_bytes_byte_array_int_int(&self, _: Rc<RefCell<Vec<i8>>>, _: i32, _: i32) -> Result<(), JavaException> { panic!("unexpected bytes") }
    fn as_any(&self) -> &dyn Any { self }
    fn clone_any(&self) -> Box<dyn Any> { Box::new(self.clone()) }
    fn clone_box(&self) -> Box<dyn UniformRandomProvider> { self.boxed() }
}
fn exception<T>(label: &str, kind: &str, result: Result<T, JavaException>) {
    match result {
        Err(error) => { assert!(error.is_instance(&[kind]), "{label}: {error:?}"); println!("{label}={kind}"); }
        Ok(_) => panic!("{label} did not throw"),
    }
}
fn permutations() {
    for n in [1, 2, 5, 17] {
        for k in [1, n] {
            let rng = Scripted::default();
            let control_rng = Scripted::default();
            let mut sampler = PermutationSampler::new(rng.boxed(), n, k).unwrap();
            let mut control = PermutationSampler::new(control_rng.boxed(), n, k).unwrap();
            for repeat in 0..5 {
                let sample = sampler.sample().unwrap();
                assert_eq!(*sample.borrow(), *control.sample().unwrap().borrow());
                let mut seen = vec![false; n as usize];
                assert_eq!(sample.borrow().len(), k as usize);
                for &value in sample.borrow().iter() {
                    assert!(value >= 0 && value < n && !seen[value as usize]);
                    seen[value as usize] = true;
                }
                println!("permutation.{n}.{k}.{repeat}={:?}", *sample.borrow());
                sample.borrow_mut()[0] = -999;
            }
            println!("permutation.{n}.{k}.bounds={:?}", rng.bounds());
        }
    }
    for n in [0, 1, 5] { println!("natural.{n}={:?}", *PermutationSampler::natural(n).unwrap().borrow()); }
    exception("natural.negative", "NegativeArraySizeException", PermutationSampler::natural(-1));
    for (n, k) in [(-1, 1), (0, 1), (3, 0), (3, -1), (3, 4)] {
        exception(&format!("permutation.invalid.{n}.{k}"), "IllegalArgumentException", PermutationSampler::new(Scripted::default().boxed(), n, k));
    }
}
fn shuffles() {
    for length in [0, 1, 2, 7] {
        for start in [0, 0.max(length / 2), length] {
            for head in [false, true] {
                if head && start >= length { continue; }
                let mut array_rng = Scripted::default();
                let mut list_rng = Scripted::default();
                let array = PermutationSampler::natural(length).unwrap();
                let alias = array.clone();
                let mut list = (0..length).map(|i| format!("v{i}")).collect::<Vec<_>>();
                PermutationSampler::shuffle_uniform_random_provider_int_array_int_boolean(&mut array_rng, Some(array), start, head).unwrap();
                ListSampler::shuffle_uniform_random_provider_list_int_boolean(&mut list_rng, &mut list, start, head).unwrap();
                for i in 0..length as usize {
                    assert_eq!(list[i], format!("v{}", alias.borrow()[i]));
                    if (head && i as i32 > start) || (!head && (i as i32) < start) { assert_eq!(alias.borrow()[i], i as i32); }
                }
                assert_eq!(array_rng.bounds(), list_rng.bounds());
                let label = format!("shuffle.{length}.{start}.{head}");
                println!("{label}={:?}", *alias.borrow());
                println!("{label}.list=[{}]", list.join(", "));
                println!("{label}.bounds={:?}", array_rng.bounds());
            }
        }
        let mut rng = Scripted::default();
        let full = PermutationSampler::natural(length).unwrap();
        PermutationSampler::shuffle_uniform_random_provider_int_array(&mut rng, Some(full.clone())).unwrap();
        println!("shuffle.full.{length}={:?}", *full.borrow());
        let mut list_rng = Scripted::default();
        let mut list = (0..length).collect::<Vec<_>>();
        ListSampler::shuffle_uniform_random_provider_list(&mut list_rng, &mut list).unwrap();
        println!("shuffle.fullList.{length}={list:?}");
        assert_eq!(rng.bounds(), list_rng.bounds());
    }
    exception("shuffle.array.invalid", "ArrayIndexOutOfBoundsException", PermutationSampler::shuffle_uniform_random_provider_int_array_int_boolean(&mut Scripted::default(), Some(Rc::new(RefCell::new(vec![]))), 0, true));
    exception("shuffle.list.invalid", "ArrayIndexOutOfBoundsException", ListSampler::shuffle_uniform_random_provider_list_int_boolean(&mut Scripted::default(), &mut Vec::<String>::new(), 0, true));
}
struct Tag { identity: i32, value: i32 }
type Item = Option<Rc<RefCell<Tag>>>;
fn item_text(item: &Item) -> String { item.as_ref().map_or_else(|| "null".into(), |tag| { let tag = tag.borrow(); format!("{}:{}", tag.identity, tag.value) }) }
fn valid_item(item: &Item, a: &Rc<RefCell<Tag>>, b: &Rc<RefCell<Tag>>) -> bool { item.as_ref().is_none_or(|tag| Rc::ptr_eq(tag, a) || Rc::ptr_eq(tag, b)) }
fn collections() {
    let a = Rc::new(RefCell::new(Tag { identity: 1, value: 7 }));
    let b = Rc::new(RefCell::new(Tag { identity: 2, value: 7 }));
    let mut original = vec![Some(a.clone()), Some(b.clone()), Some(a.clone()), None];
    for k in [1, 2, 4] {
        let rng = Scripted::default();
        for repeat in 0..3 {
            let mut sample = ListSampler::sample(&rng, original.clone(), k).unwrap();
            assert_eq!(original.len(), 4);
            assert!(Rc::ptr_eq(original[0].as_ref().unwrap(), &a) && Rc::ptr_eq(original[1].as_ref().unwrap(), &b) && Rc::ptr_eq(original[2].as_ref().unwrap(), &a) && original[3].is_none());
            assert!(sample.iter().all(|item| valid_item(item, &a, &b)));
            println!("list.sample.{k}.{repeat}=[{}]", sample.iter().map(item_text).collect::<Vec<_>>().join(", "));
            sample.clear();
            assert_eq!(original.len(), 4);
        }
        println!("list.sample.{k}.bounds={:?}", rng.bounds());
    }
    let rng = Scripted::default();
    let mut sampler = CollectionSampler::new(rng.boxed(), original.clone()).unwrap();
    original.clear();
    original.push(Some(Rc::new(RefCell::new(Tag { identity: 99, value: 99 }))));
    a.borrow_mut().value = 42;
    for i in 0..20 {
        let sampled = sampler.sample().unwrap().expect("generic return slot");
        assert!(valid_item(&sampled, &a, &b));
        println!("collection.sample.{i}={}", item_text(&sampled));
    }
    println!("collection.bounds={:?}", rng.bounds());
    let mut other = PermutationSampler::new(rng.boxed(), 5, 3).unwrap();
    println!("sharedRng.permutation={:?}", *other.sample().unwrap().borrow());
    println!("sharedRng.collection={}", item_text(&sampler.sample().unwrap().expect("generic return slot")));
    println!("sharedRng.bounds={:?}", rng.bounds());
    exception("collection.empty", "IllegalArgumentException", CollectionSampler::<String>::new(Scripted::default().boxed(), vec![]));
    for k in [-1, 0, 2] { exception(&format!("list.sample.invalid.{k}"), "IllegalArgumentException", ListSampler::sample(&Scripted::default(), vec!["one".to_string()], k)); }
}
fn main() { permutations(); shuffles(); collections(); println!("oracle=ok"); }
