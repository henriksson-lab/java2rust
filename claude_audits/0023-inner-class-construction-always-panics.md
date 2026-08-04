# 0023 — Constructing a non-static inner class always panics at runtime

Status: **validated** (runtime-confirmed)
Severity: high — any Java program using a non-static inner class dies on first use
Kind: blocked by the missing shared representation ([0002](0002-object-aliasing-has-no-representation.md))

## Symptom

Non-static inner classes compile, but their constructor is a `panic!`. Every
`new Inner()` aborts at runtime.

```java
public class InnerCapture {
    private int base = 10;
    class Adder {
        int add(int x) { return base + x; }
    }
    public int run() {
        Adder a = new Adder();
        base = 20;
        return a.add(1);       // Java: 21
    }
}
```

Generated:

```rust
pub struct InnerCaptureAdder {
    __outer: std::rc::Rc<std::cell::RefCell<InnerCapture>>,
}

impl InnerCaptureAdder {
    pub fn add(&self, x: i32) -> i32 {
        return self.__outer.borrow().base + x;          // correct
    }

    pub fn new() -> InnerCaptureAdder {
        return InnerCaptureAdder {
            __outer: panic!("unsupported Java inner-class outer capture construction \
                             for InnerCapture.Adder; pass an outer InnerCapture handle"),
        };
    }
}

impl InnerCapture {
    pub fn run(&mut self) -> i32 {
        let a: InnerCaptureAdder = InnerCaptureAdder::new();   // no outer handle passed
        self.base = 20;
        return a.add(1);
    }
}
```

Running it:

```
thread 'main' panicked at ic.rs:1676:45:
unsupported Java inner-class outer capture construction for InnerCapture.Adder;
pass an outer InnerCapture handle
```

## What is and is not in place

The *access* side is done and correct: the `__outer` field exists with the right
`Rc<RefCell<Outer>>` type, and `add` reads `self.__outer.borrow().base`, which
would give Java's by-reference capture semantics — `base` mutated after
construction would be visible, returning 21.

Only the *construction* side is missing, and it is missing for a structural
reason.

## Root cause: this is 0002 with the consequence made visible

To construct `InnerCaptureAdder`, `run` must hand it an
`Rc<RefCell<InnerCapture>>`. But `run`'s receiver is `&mut InnerCapture` — the
outer object is represented **by value**, and there is no way to obtain a shared
handle to it.

That is exactly the gap in
[0002](0002-object-aliasing-has-no-representation.md): `ObjectRepresentation`
(`src/object_model.rs:151`) has `ConcreteStruct | DynTrait | JavaEnum |
ClosedEnum | ErasedObject` and **no shared variant**. The `Rc<RefCell<…>>` used
for `__outer` (`src/item_assembly.rs:525`) is hand-written for this one field —
noted in 0002 as "a one-off, not a representation the planner can choose". This
issue is what that one-off costs: the field type can be written, but no
representation can produce a value for it.

So 0002 is not only an aliasing-correctness issue. It **blocks a language
feature outright**. That materially raises its priority: `SUGGESTIONS.md`
places 0002b last partly because `E0382` is rare in the corpora, but inner
classes are common in Java and this failure is invisible to compile-error counts
(it compiles; it panics).

## Fix direction

Requires the shared representation from 0002b. Once a type can be planned as
`Rc<RefCell<T>>`:

1. Mark any class that is the outer type of a non-static inner class as
   requiring a shared representation.
2. Give the inner class a constructor taking the outer handle
   (`InnerCaptureAdder::new(outer: Rc<RefCell<InnerCapture>>)`), and lower
   `new Adder()` inside an instance method to pass the enclosing handle.
3. `outer.new Inner()` (qualified creation from outside) uses the same path.

A narrower interim option, if 0002b is far off: restrict the shared
representation to *only* those classes that are outer types of inner classes.
That is a much smaller blast radius than the general aliasing analysis and would
unblock this feature on its own — worth considering as the first concrete
consumer of the new representation, since it has a clear, checkable success
criterion.

## Interim requirement

Until then, this should be a **translator diagnostic**, not a runtime `panic!`
embedded in generated code. As written, a corpus reports zero compile errors for
a program guaranteed to abort. Same principle as
[0018](0018-the-compile-error-metric-cannot-see-silent-failures.md).

## Regression tests worth adding

```java
class Outer { int base = 10; class In { int get(){ return base; } }
              int run(){ In i = new In(); base = 20; return i.get(); } }   // 20, not 10
```

The mutate-after-construction step is the point: it distinguishes by-reference
capture from a copy, and a copy-based implementation would return 10.
