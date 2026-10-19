class NullObjectString {
    public String toString() { return null; }
}
class ThrowingObjectString {
    public String toString() { throw new IllegalArgumentException(); }
}
class CountingObjectString {
    int calls;
    public String toString() { calls++; return "n" + calls; }
}
class MaybeObjectString {
    private int mode;
    MaybeObjectString(int value) { mode = value; }
    public String toString() {
        if (mode == 1) throw new IllegalStateException();
        if (mode == 2) return null;
        return "ok";
    }
}
class GenericStringRenderer<T> {
    String render(T value) { return value.toString(); }
    String key(java.util.Map<T, Integer> values) {
        String text = "";
        for (java.util.Map.Entry<T, Integer> entry : values.entrySet()) {
            text += entry.getKey().toString();
        }
        return text;
    }
}
class VirtualObjectStrings {
    static String typedText(CountingObjectString value) { return "typed=" + value; }
    static String typedDisplay() {
        CountingObjectString original = new CountingObjectString();
        String text = "" + original + ":" + original;
        return text + ":" + original.calls;
    }
    static String typedParameter() {
        CountingObjectString original = new CountingObjectString();
        String text = typedText(original);
        return text + ":" + original.calls;
    }
    static String objectText(Object value) { return value.toString(); }
    static boolean objectParameterNullResult() { return objectText(new NullObjectString()) == null; }
    static String mutableDispatch() {
        CountingObjectString original = new CountingObjectString();
        Object value = original;
        String first = value.toString();
        String second = value.toString();
        return first + ":" + second + ":" + original.calls;
    }
    static boolean nullableCheckedResult() {
        Object value = new MaybeObjectString(2);
        return value.toString() == null;
    }
    static int checkedNullableDispatch() {
        Object value = new MaybeObjectString(1);
        try { value.toString(); }
        catch (IllegalStateException ex) { return 11; }
        return -1;
    }
    static <T> String render(T value) { return value.toString(); }
    static <T> String forward(T value) { return render(value); }
    static String genericText() {
        GenericStringRenderer<String> renderer = new GenericStringRenderer<String>();
        return renderer.render("ok");
    }
    static String staticText() { return forward("generic"); }
    static String mapKey() {
        GenericStringRenderer<String> renderer = new GenericStringRenderer<String>();
        java.util.Map<String, Integer> values = new java.util.LinkedHashMap<String, Integer>();
        values.put("key", 1);
        return renderer.key(values);
    }
    static boolean genericNullResult() {
        GenericStringRenderer<NullObjectString> renderer = new GenericStringRenderer<NullObjectString>();
        return renderer.render(new NullObjectString()) == null;
    }
    static int genericCaught() {
        GenericStringRenderer<ThrowingObjectString> renderer = new GenericStringRenderer<ThrowingObjectString>();
        try { renderer.render(new ThrowingObjectString()); }
        catch (IllegalArgumentException ex) { return 9; }
        return -1;
    }
    static boolean nullableResult() {
        Object value = new NullObjectString();
        String text = value.toString();
        return text == null;
    }
    static String displayNullResult() {
        Object value = new NullObjectString();
        return "value=" + value;
    }
    static int checkedDispatch() {
        Object value = new ThrowingObjectString();
        try {
            value.toString();
        } catch (IllegalArgumentException ex) { return 7; }
        return -1;
    }
}
