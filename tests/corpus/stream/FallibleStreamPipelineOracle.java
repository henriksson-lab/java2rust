public class FallibleStreamPipelineOracle {
    static void fails(Class<? extends RuntimeException> type, Runnable operation) {
        try { operation.run(); }
        catch (RuntimeException error) {
            assert type.isInstance(error) : error;
            return;
        }
        throw new AssertionError("expected " + type.getName());
    }
    public static void main(String[] arguments) {
        FallibleStreamPipeline.reset();
        assert FallibleStreamPipeline.any(new int[] {0, 2, -1});
        assert FallibleStreamPipeline.visits() == 2;
        assert FallibleStreamPipeline.mapped() == 1;
        FallibleStreamPipeline.reset();
        fails(IllegalArgumentException.class, () -> FallibleStreamPipeline.any(new int[] {0, -1, 2}));
        assert FallibleStreamPipeline.visits() == 2;
        assert FallibleStreamPipeline.mapped() == 0;
        FallibleStreamPipeline.reset();
        fails(IllegalStateException.class, () -> FallibleStreamPipeline.any(new int[] {13, 2}));
        assert FallibleStreamPipeline.visits() == 1;
        assert FallibleStreamPipeline.mapped() == 1;
        assert !FallibleStreamPipeline.any(new int[] {});
        FallibleStreamPipeline.reset();
        assert FallibleStreamPipeline.bound(new int[] {0, 2, -1});
        assert FallibleStreamPipeline.visits() == 2;
        assert FallibleStreamPipeline.mapped() == 2;
        assert !FallibleStreamPipeline.bound(new int[] {});
        fails(IllegalArgumentException.class, () -> FallibleStreamPipeline.bound(new int[] {-1, 2}));
        assert FallibleStreamPipeline.all(new int[] {});
        assert !FallibleStreamPipeline.all(new int[] {2, -1});
        fails(IllegalArgumentException.class, () -> FallibleStreamPipeline.all(new int[] {-1, 2}));
        assert FallibleStreamPipeline.sum(new int[] {0, 1, 2}) == 6;
        fails(IllegalArgumentException.class, () -> FallibleStreamPipeline.sum(new int[] {1, -1, 2}));
        assert FallibleStreamPipeline.count(new int[] {0, 1, 2}) == 2;
        fails(IllegalArgumentException.class, () -> FallibleStreamPipeline.count(new int[] {1, -1, 2}));
        FallibleStreamPipeline.reset();
        assert FallibleStreamPipeline.first(new int[] {0, 2, -1}) == 4;
        assert FallibleStreamPipeline.visits() == 2;
        assert FallibleStreamPipeline.mapped() == 1;
        assert FallibleStreamPipeline.first(new int[] {}) == -1;
        fails(IllegalArgumentException.class, () -> FallibleStreamPipeline.first(new int[] {-1, 2}));
        assert FallibleStreamPipeline.collected(new int[] {0, 1, 2}).equals(java.util.Arrays.asList(2, 4));
        fails(IllegalArgumentException.class, () -> FallibleStreamPipeline.collected(new int[] {1, -1, 2}));
        for (int mode = 0; mode < 2; mode++) {
            FallibleStreamPipeline.reset();
            if (mode == 0) FallibleStreamPipeline.each(new int[] {0, 1, 2});
            else FallibleStreamPipeline.eachBlock(new int[] {0, 1, 2});
            assert FallibleStreamPipeline.consumed() == 6;
            FallibleStreamPipeline.reset();
            if (mode == 0) fails(IllegalStateException.class, () -> FallibleStreamPipeline.each(new int[] {1, 13, 2}));
            else fails(IllegalStateException.class, () -> FallibleStreamPipeline.eachBlock(new int[] {1, 13, 2}));
            assert FallibleStreamPipeline.visits() == 2;
            assert FallibleStreamPipeline.consumed() == 2;
        }
        assert FallibleStreamPipeline.nonNull(new int[] {0, 1, 0, 2}) == 2;
        assert FallibleStreamPipeline.nonNullPlain(new int[] {0, 1, 0, 2}) == 2;
        assert FallibleStreamPipeline.nonNullPlain(new int[] {0, 0}) == 0;
        fails(IllegalArgumentException.class, () -> FallibleStreamPipeline.nonNull(new int[] {0, -1, 2}));
        assert !FallibleStreamPipeline.caught(new int[] {-1});
        assert !FallibleStreamPipeline.mutatingNull(new int[] {});
        fails(NullPointerException.class, () -> FallibleStreamPipeline.mutatingNull(new int[] {1}));
        assert FallibleStreamPipeline.mutating(new int[] {1}, new java.util.HashSet<Integer>());
        assert !FallibleStreamPipeline.mutating(new int[] {1, 3}, new java.util.HashSet<Integer>(java.util.Arrays.asList(1, 3)));
        System.out.println("fallible-stream-pipeline: ok");
    }
}
