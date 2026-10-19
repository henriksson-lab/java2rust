class FieldArrayPayload {
    int value;
    FieldArrayPayload(int value) { this.value = value; }
}
class FieldArrayHolder {
    int[] values;
    String[] labels;
    FieldArrayPayload[] payloads;
    FieldArrayHolder() {
        values = new int[] { 1, 2 };
        labels = new String[] { "left", "right" };
        payloads = new FieldArrayPayload[] { new FieldArrayPayload(3) };
    }
    int read() { return values[0]; }
    void write(int value) { values[0] = value; }
    int replace() { values = new int[] { 99, 0 }; return 7; }
    int replaceDuringStore() { return (values[0] = replace()); }
    int readDuringStore() { return (values[0] = read() + 3); }
    int compoundRead() { return (values[0] += read()); }
    void clearLabels() { labels[0] = labels[1] = null; }
}
class SharedFieldArrays {
    static int trace;
    static int[] staticValues;
    static FieldArrayHolder receiver(FieldArrayHolder value) { trace = trace * 10 + 1; return value; }
    static int index(int value) { trace = trace * 10 + 2; return value; }
    static int rhs() { trace = trace * 10 + 3; return 9; }
    static int aliasWrite() {
        FieldArrayHolder owner = new FieldArrayHolder();
        FieldArrayHolder alias = owner;
        owner.write(8);
        return alias.read();
    }
    static int rhsRead() {
        FieldArrayHolder owner = new FieldArrayHolder();
        return owner.readDuringStore();
    }
    static int compoundRead() {
        FieldArrayHolder owner = new FieldArrayHolder();
        return owner.compoundRead();
    }
    static int rebinding() {
        FieldArrayHolder owner = new FieldArrayHolder();
        int[] original = owner.values;
        owner.values[0] = owner.replace();
        return original[0] * 100 + owner.values[0];
    }
    static int selfRebinding() {
        FieldArrayHolder owner = new FieldArrayHolder();
        owner.replaceDuringStore();
        return owner.values[0];
    }
    static boolean chainedNulls() {
        FieldArrayHolder owner = new FieldArrayHolder();
        String[] original = owner.labels;
        owner.clearLabels();
        return original[0] == null && original[1] == null;
    }
    static int order(boolean nullParent, boolean nullArray, int position) {
        trace = 0;
        FieldArrayHolder owner = nullParent ? null : new FieldArrayHolder();
        if (!nullParent && nullArray) owner.values = null;
        int tag = 0;
        try { receiver(owner).values[index(position)] = rhs(); }
        catch (NullPointerException error) { tag = 1; }
        catch (ArrayIndexOutOfBoundsException error) { tag = 2; }
        return trace * 10 + tag;
    }
    static int compoundOrder(boolean nullArray, int position) {
        trace = 0;
        FieldArrayHolder owner = new FieldArrayHolder();
        if (nullArray) owner.values = null;
        int tag = 0;
        try { receiver(owner).values[index(position)] += rhs(); }
        catch (NullPointerException error) { tag = 1; }
        catch (ArrayIndexOutOfBoundsException error) { tag = 2; }
        return trace * 10 + tag;
    }
    static int staticAlias() {
        staticValues = new int[] { 1 };
        int[] alias = staticValues;
        staticValues[0] = 8;
        return alias[0];
    }
    static int localOnly() {
        int[] values = new int[] { 1 };
        values[0] = 2;
        return values[0];
    }
    static int updates() {
        FieldArrayHolder owner = new FieldArrayHolder();
        int old = owner.values[0]++;
        int next = ++owner.values[0];
        return old * 100 + next * 10 + owner.values[0];
    }
    static int division(int denominator) {
        FieldArrayHolder owner = new FieldArrayHolder();
        owner.values[0] = 8;
        try { owner.values[0] /= denominator; }
        catch (ArithmeticException error) { return owner.values[0] + 100; }
        return owner.values[0];
    }
    static int clonedAllocation() {
        FieldArrayHolder owner = new FieldArrayHolder();
        int[] copy = owner.values.clone();
        copy[0] = 7;
        return owner.values[0] * 10 + copy[0];
    }
    static void fill(int[] array) { array[0] = 6; }
    static int argumentMutation() {
        FieldArrayHolder owner = new FieldArrayHolder();
        fill(owner.values);
        return owner.values[0];
    }
    static int shallowClone() {
        FieldArrayHolder owner = new FieldArrayHolder();
        FieldArrayPayload[] copy = owner.payloads.clone();
        copy[0].value = 7;
        copy[0] = new FieldArrayPayload(2);
        return owner.payloads[0].value * 10 + copy[0].value;
    }
}
