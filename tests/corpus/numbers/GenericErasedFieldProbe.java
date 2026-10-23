class GenericFieldValue {
    int count;
    void changed() { count++; }
}

class GenericErasedField<T> {
    T value;
    GenericErasedField(T value) { this.value = value; }
    public Object erased() { return value; }
}

class GenericErasedFieldProbe {
    static int run() {
        GenericFieldValue value = new GenericFieldValue();
        GenericErasedField<GenericFieldValue> holder = new GenericErasedField<>(value);
        Object first = holder.erased();
        if (first != value) return 1;
        ((GenericFieldValue) first).changed();
        if (value.count != 1 || holder.erased() != first) return 2;
        GenericFieldValue second = new GenericFieldValue();
        holder.value = second;
        Object replacement = holder.erased();
        if (replacement != second || replacement == first) return 3;
        ((GenericFieldValue) replacement).changed();
        if (second.count != 1 || value.count != 1) return 4;
        holder.value = null;
        if (holder.erased() != null) return 5;
        GenericErasedField<GenericFieldValue> empty = new GenericErasedField<>(null);
        if (empty.erased() != null) return 6;
        Object absent = empty.erased();
        GenericFieldValue castNull = (GenericFieldValue) absent;
        if (castNull != null || absent != null) return 7;
        try { ((GenericFieldValue) absent).changed(); return 8; }
        catch (NullPointerException error) { }
        Object wrong = "wrong";
        try { ((GenericFieldValue) wrong).changed(); return 9; }
        catch (ClassCastException error) { }
        if (!"wrong".equals(wrong)) return 10;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
