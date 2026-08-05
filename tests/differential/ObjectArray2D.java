/**
 * A two-dimensional **object** array, allocated both ways, written and read.
 *
 * `String[][]` used to be four sites and three carriers: declared
 * `Vec<Vec<String>>`, built by `ndarray::Array2::from_elem`, written with nested
 * indexing and read as `g[[i, j]]`. It did not compile, and the two Java
 * allocation forms of the same declared type disagreed with each other —
 * `new String[2][]` built a nested `Vec` while `new String[2][2]` built an
 * ndarray.
 *
 * `Cell` is a source type on purpose. An *external facade* element such as
 * `StringBuilder` still falls back to an empty `Vec` here — the two-dimensional
 * form of the residual claude_audits/done/0034 records — and a fixture that
 * cannot pass for a second reason pins neither.
 *
 * The policy was never in doubt: `ndarray_array2_element_type` requires a
 * **primitive** element. Construction never asked it, and reading asked a
 * different question ("is this two-dimensional?") from writing ("is the element
 * a primitive?"). Both allocation forms appear below so the four sites are
 * forced to agree.
 *
 * The last case is the evaluation-order half. `g[f()][h()] = v` on a ragged
 * array fell through to the plain assignment path, where Rust reads the value
 * before either index; Java evaluates the array reference, then the indices,
 * then the value.
 *
 * The primitive control — `int[][]`, which keeps the ndarray carrier and its own
 * ordered path — is **not** here: a `int[][]` local pulls in `ndarray`, which
 * this harness does not link, so it would fail for a reason unrelated to what
 * this fixture pins. It is covered by
 * `ndarray_backed_two_dimensional_assignment_keeps_its_own_ordered_path` in
 * `tests/slot_shape_design.rs` instead.
 *
 * Java prints "x,y,z,v2,2".
 *
 * See claude_audits/done/0038-two-dimensional-object-array-carrier-disagrees.md
 * and claude_audits/done/0026-assignment-evaluation-order-differs-from-java.md.
 */
public class ObjectArray2D {
    /** A source type, so the element is not an external facade. */
    static class Cell {
        private String v;

        Cell() {
            this.v = "";
        }

        Cell(String v) {
            this.v = v;
        }

        String value() {
            return v;
        }
    }

    /**
     * A static `int`, not a static `int[]`: a static array field drops its
     * initializer and its writes go through a cloning getter, so it neither
     * allocates nor records — claude_audits/done/0039, and a second reason for
     * this fixture to fail would pin nothing.
     */
    static int counter = 0;

    static int bump() {
        counter = counter + 1;
        return counter - 1;
    }

    public static String run() {
        // Rectangular allocation of a non-primitive element.
        String[][] a = new String[2][2];
        a[0][1] = "x";

        // Ragged allocation of the same declared type.
        String[][] b = new String[2][];
        b[0] = new String[2];
        b[1] = new String[2];
        b[1][0] = "y";

        // A source type as the element, so this is not special to String.
        Cell[][] c = new Cell[1][1];
        c[0][0] = new Cell("z");

        // Java order: the indices are evaluated before the value, so the value
        // sees counter == 2 and the write lands on b[0][1].
        counter = 0;
        b[bump()][bump()] = "v" + counter;

        return a[0][1] + "," + b[1][0] + "," + c[0][0].value()
                + "," + b[0][1] + "," + counter;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
