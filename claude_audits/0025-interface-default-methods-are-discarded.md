# 0025 — Interface default method bodies are discarded, then panic at runtime

Status: **validated** (runtime-confirmed)
Severity: high — default methods are ubiquitous in modern Java
Kind: missing member handling; the Rust target supports it directly

## Symptom

A `default` method's body is dropped. The trait declares the method with no
body, and each implementing class gets a `panic!` stub.

```java
interface Greeter {
    String name();
    default String greet() { return "hello " + name(); }
}
public class Q5 implements Greeter {
    public String name() { return "world"; }
    public String run() { return greet(); }
}
```

Generated:

```rust
fn greet(&self) -> String;                 // trait: declaration only, body lost

// impl for Q5:
fn greet(&self) -> String {
    panic!("missing Java interface method implementation: Q5.greet");
}
```

Running it:

```
thread 'main' panicked at q5m.rs:1697:33:
missing Java interface method implementation: Q5.greet
```

Java prints `"hello world"`. The output compiles cleanly, so this is invisible
to the corpus error count ([0018](0018-the-compile-error-metric-cannot-see-silent-failures.md)).

## Why this one should be cheap

Rust traits support default method bodies natively, and the mapping is direct:

```rust
trait Greeter {
    fn name(&self) -> String;
    fn greet(&self) -> String { format!("hello {}", self.name()) }   // default body
}
```

The default body calls other trait methods through `&self`, exactly as the Java
version calls them through `this`. No new machinery is needed — the body simply
needs to be lowered into the trait declaration instead of being dropped, and the
`panic!` stub must not be generated when a default exists.

## Points to get right

1. **A class that overrides the default must win.** Emit the override in the
   `impl` and rely on Rust's normal precedence.
2. **`Iface.super.method()`** — Java's explicit qualified super-call to a
   specific interface default. In Rust this is `<dyn Iface>::method(self)` or
   `Iface::method(self)`. Needed for the diamond-resolution case.
3. **Conflicting defaults from two interfaces.** Java requires the class to
   override and disambiguate; Rust likewise will not pick one. If the source
   compiles, an override exists, so this should fall out — but it is worth a test.
4. **`static` and `private` interface methods** (Java 8/9) are separate cases;
   probe them rather than assuming they share the path.
5. Default methods on interfaces that are also used as **`dyn` trait objects**
   must stay object-safe. A default body that is generic or takes `self` by value
   would break that; diagnose rather than emit.

## Relationship to other findings

The `panic!`-as-placeholder pattern here is the same as
[0023](0023-inner-class-construction-always-panics.md) and 0009's
`panic!("…parameter count does not match interface")`. In all three, a
*translator* gap is encoded as a *runtime* failure in generated code, which
means it compiles and the corpus metric scores it as success. Whatever emits
these should raise a translator diagnostic as well — see
[0018](0018-the-compile-error-metric-cannot-see-silent-failures.md).

## Regression tests worth adding

```java
interface G { String name(); default String greet(){ return "hi " + name(); } }
class A implements G { public String name(){ return "a"; } }              // greet() -> "hi a"
class B implements G { public String name(){ return "b"; }
                       public String greet(){ return "yo"; } }            // override wins -> "yo"
interface H { default String greet(){ return "h"; } }
class C implements G, H { public String name(){ return "c"; }
                          public String greet(){ return G.super.greet(); } }  // -> "hi c"
```

The third case pins `Iface.super`, which is the part most likely to be missed.
