/**
 * `super.base()` inside an override. Java calls the superclass implementation;
 * the translation replaces the whole call with `Default::default()`, so the
 * override computes `0 + 5` instead of `10 + 5`.
 *
 * Java prints
 *
 *     15,10
 *
 * The translation compiles cleanly and prints
 *
 *     5,10
 *
 * The generated body is `return (0i32).wrapping_add(5);`.
 */
public class SuperMethodCall {
    static class Base {
        int base() {
            return 10;
        }
    }

    static class Derived extends Base {
        int base() {
            return super.base() + 5;
        }
    }

    public static String run() {
        Derived d = new Derived();
        return d.base() + "," + new Base().base();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
