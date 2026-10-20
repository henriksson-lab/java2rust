// Compile with tests/fixtures/class_bound_writes.java.
public class ClassBoundWritesOracle {
    public static void main(String[] args) {
        int result = ClassBoundWrites.run();
        if (result != 0) throw new AssertionError(result);
        System.out.println(result);
    }
}
