/**
 * An enum with an abstract method and per-constant bodies. The bodies are
 * discarded and the abstract method is lowered to a panic.
 *
 * Java prints
 *
 *     5,6
 *
 * The translation compiles and panics:
 *
 *     abstract method EnumConstantBodies.Op.apply has no implementation
 *
 * Everything else about enums is correct: `values()`, `ordinal()`, `name()`,
 * `valueOf`, `switch`, `==`, fields with a constructor, and use as a map key.
 */
public class EnumConstantBodies {
    enum Op {
        ADD {
            int apply(int a, int b) {
                return a + b;
            }
        },
        MUL {
            int apply(int a, int b) {
                return a * b;
            }
        };

        abstract int apply(int a, int b);
    }

    public static String run() {
        return Op.ADD.apply(2, 3) + "," + Op.MUL.apply(2, 3);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
