class RetainedPrimitiveArrayIntrinsics {
    int[] values = new int[] { 1, 2, 3, 4, 5 };
    int calls = 0;
    int[] source() { calls++; return values; }
    int position() { calls++; values[0] = 7; return 0; }
    int[] destination() { calls++; return values; }
    int[] expose() { return values; }
    static int replaceOrMutate(int[] seed) {
        if (seed.length == 0) seed = new int[1];
        seed[0] = 42;
        return seed[0];
    }
    int check() {
        int[] alias = expose();
        System.arraycopy(source(), position(), destination(), 1, 4);
        if (calls != 3) return 1;
        if (alias[0] != 7 || alias[1] != 7 || alias[2] != 2
                || alias[3] != 3 || alias[4] != 4) return 2;
        System.arraycopy(values, 1, values, 0, 4);
        if (alias[0] != 7 || alias[1] != 2 || alias[2] != 3
                || alias[3] != 4 || alias[4] != 4) return 3;
        try { System.arraycopy(values, -1, values, 0, 1); return 4; }
        catch (IndexOutOfBoundsException error) {}
        try { System.arraycopy(values, 0, values, 0, -1); return 5; }
        catch (IndexOutOfBoundsException error) {}
        if (alias[0] != 7 || alias[1] != 2) return 6;
        if (replaceOrMutate(alias) != 42 || values[0] != 42) return 7;
        int[] empty = new int[0];
        int[] unchanged = empty;
        if (replaceOrMutate(empty) != 42 || unchanged.length != 0) return 8;
        return 0;
    }
    static int run() { return new RetainedPrimitiveArrayIntrinsics().check(); }
}
