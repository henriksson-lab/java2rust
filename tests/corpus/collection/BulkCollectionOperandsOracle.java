import java.util.Arrays;

public class BulkCollectionOperandsOracle {
    private static void expects(Class<? extends RuntimeException> kind, Runnable operation, int trace) {
        BulkCollectionOperands.reset();
        try {
            operation.run();
            throw new AssertionError("expected " + kind.getName());
        } catch (RuntimeException error) {
            assert kind.isInstance(error) : error;
        }
        assert BulkCollectionOperands.trace() == trace : BulkCollectionOperands.trace();
    }

    public static void main(String[] arguments) {
        BulkCollectionOperands.reset();
        assert !BulkCollectionOperands.contains(0, 0);
        assert BulkCollectionOperands.trace() == 12;
        BulkCollectionOperands.reset();
        assert BulkCollectionOperands.contains(0, 3);
        assert BulkCollectionOperands.trace() == 12;
        expects(IllegalArgumentException.class, () -> BulkCollectionOperands.contains(1, 0), 1);
        expects(NullPointerException.class, () -> BulkCollectionOperands.contains(2, 0), 12);
        expects(NullPointerException.class, () -> BulkCollectionOperands.contains(2, 3), 12);
        expects(IllegalStateException.class, () -> BulkCollectionOperands.contains(2, 1), 12);
        expects(NullPointerException.class, () -> BulkCollectionOperands.contains(0, 2), 12);
        BulkCollectionOperands.reset();
        assert BulkCollectionOperands.adds(0, 0);
        assert BulkCollectionOperands.trace() == 12;
        BulkCollectionOperands.reset();
        assert BulkCollectionOperands.removes(0, 0);
        assert BulkCollectionOperands.trace() == 12;
        BulkCollectionOperands.reset();
        assert BulkCollectionOperands.retains(0, 0);
        assert BulkCollectionOperands.trace() == 12;
        expects(IllegalStateException.class, () -> BulkCollectionOperands.adds(2, 1), 12);
        expects(IllegalStateException.class, () -> BulkCollectionOperands.removes(2, 1), 12);
        expects(IllegalStateException.class, () -> BulkCollectionOperands.retains(2, 1), 12);
        assert BulkCollectionOperands.nameCollision(Arrays.asList(1, 2), Arrays.asList(2));
        assert !BulkCollectionOperands.nameCollision(Arrays.asList(1), Arrays.asList(2));
        assert BulkCollectionOperands.stagingNames(Arrays.asList(1, 2), Arrays.asList(2));
        assert BulkCollectionOperands.crossKinds() == 321;
        assert new BulkCollectionOperands().fieldSelf(0) == 40;
        expects(NullPointerException.class, () -> new BulkCollectionOperands().fieldSelf(1), 0);
        BulkCollectionOperands.reset();
        assert BulkCollectionOperands.sameCallContains();
        assert BulkCollectionOperands.trace() == 11;
        BulkCollectionOperands.reset();
        assert BulkCollectionOperands.sameCallRemoves();
        assert BulkCollectionOperands.trace() == 11;
        BulkCollectionOperands.reset();
        assert !BulkCollectionOperands.sameCallRetains();
        assert BulkCollectionOperands.trace() == 11;
        java.util.List<Integer> values = new java.util.ArrayList<>(Arrays.asList(1, 2));
        assert BulkCollectionOperands.selfContains(values);
        assert !BulkCollectionOperands.selfRetains(values);
        assert values.equals(Arrays.asList(1, 2));
        assert BulkCollectionOperands.selfAdds(values);
        assert values.equals(Arrays.asList(1, 2, 1, 2));
        assert BulkCollectionOperands.selfRemoves(values);
        assert values.isEmpty();
        assert !BulkCollectionOperands.selfRemoves(values);
        expects(IllegalStateException.class, () -> BulkCollectionOperands.addStatement(2, 1), 12);
        expects(IllegalStateException.class, () -> BulkCollectionOperands.retainStatement(2, 1), 12);
        System.out.println("bulk-collection-operands: ok");
    }
}
