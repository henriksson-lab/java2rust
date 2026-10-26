interface AppendView<T> { int id(); }
class AppendLabel implements AppendView<String> {
    int calls;
    int mode;
    AppendHolder holder;
    AppendLabel(int mode) { this.mode = mode; }
    public int id() { return calls; }
    public String toString() {
        calls++;
        if (holder != null && holder.peek() != 7) throw new IllegalStateException("holder");
        if (mode == 2) throw new IllegalStateException("label");
        return mode == 1 ? null : "label:\uD800:" + calls;
    }
}
class AppendHolder {
    AppendView<String> value;
    int marker = 7;
    AppendHolder(AppendView<String> value) { this.value = value; }
    int peek() { return marker; }
    AppendView<String> get() { return value; }
    String text() { return "[" + get() + "]"; }
}
class AppendBox<T> {
    AppendView<T> value;
    AppendBox(AppendView<T> value) { this.value = value; }
    AppendView<T> get() { return value; }
}
class InterfaceAppendProbe {
    static String describe(AppendView<String> value) { return "[" + value + "]"; }
    static String valueOf(AppendView<String> value) { return String.valueOf(value); }
    static String call(AppendView<String> value) { return value.toString(); }
    static <T> String genericText(AppendBox<T> box) { return "[" + box.get() + "]"; }
    static String wildcardText(AppendBox<?> box) { return "[" + box.get() + "]"; }
    static int run() {
        AppendLabel label = new AppendLabel(0);
        AppendHolder holder = new AppendHolder(label);
        label.holder = holder;
        if (!holder.text().equals("[label:\uD800:1]")) return 1;
        if (label.calls != 1) return 2;
        if (!describe(label).equals("[label:\uD800:2]")) return 3;
        if (!describe(null).equals("[null]")) return 4;
        if (!describe(new AppendLabel(1)).equals("[null]")) return 5;
        if (!valueOf(null).equals("null")) return 6;
        try { describe(new AppendLabel(2)); return 7; }
        catch (IllegalStateException expected) {}
        if (call(new AppendLabel(1)) != null) return 8;
        try { call(null); return 9; }
        catch (NullPointerException expected) {}
        AppendBox<String> box = new AppendBox<String>(label);
        if (!genericText(box).equals("[label:\uD800:3]")) return 10;
        if (!wildcardText(box).equals("[label:\uD800:4]")) return 11;
        AppendBox<String> nullBox = new AppendBox<String>(null);
        if (!wildcardText(nullBox).equals("[null]")) return 12;
        if (!wildcardText(new AppendBox<String>(null)).equals("[null]")) return 13;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
