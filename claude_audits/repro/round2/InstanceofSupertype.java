/**
 * `instanceof` against a *supertype*. Java's `instanceof` is a subtype test;
 * the translation lowers it as `Box<dyn Any>::is::<T>()`, which is exact
 * runtime-type identity, so a `B` is not reported as an `A`.
 *
 * Java prints
 *
 *     true,true,false
 *
 * The translation compiles cleanly and prints
 *
 *     false,true,false
 *
 * A chain that tests the most-derived type first (`instanceof C` before
 * `instanceof B` before `instanceof A`) happens to agree, which is why this
 * needs its own fixture.
 */
public class InstanceofSupertype {
    static class A {
    }

    static class B extends A {
    }

    static boolean isA(Object o) {
        return o instanceof A;
    }

    public static String run() {
        return isA(new B()) + "," + isA(new A()) + "," + isA("x");
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
