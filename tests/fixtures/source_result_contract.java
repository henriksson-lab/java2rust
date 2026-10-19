class CheckedResults {
    static <T> T required(T value) {
        if (value == null) { throw new IllegalArgumentException(); }
        return value;
    }
    static String direct() {
        String value = null;
        value = "ok";
        return required(value);
    }
    static String nullable(String value) { return value; }
    static <T> T maybe(T value) {
        if (value == null) { return null; }
        return value;
    }
    static <T> T forward(T value) {
        T actual = required(value);
        return actual;
    }
    static <T> T select(ResultHolder<T> source, T ignored) {
        if (ignored == null) { }
        return source.get();
    }
    static <T> T selected(ResultHolder<T> source, T ignored) {
        T result = select(source, ignored);
        return result;
    }
    static <T> T reject(T value) {
        if (value == null) { }
        throw new IllegalStateException();
    }
    static <T> T rejected(T value) {
        T result = reject(value);
        return result;
    }
    static String rejection() {
        try { return rejected((String) null); }
        catch (IllegalStateException failure) { return "caught"; }
    }
    static String run() {
        String value = forward(direct());
        String missing = nullable(null);
        ResultHolder<String> holder = new ResultHolder<>("chosen");
        String ignored = null;
        String chosen = selected(holder, ignored);
        String genericMissing = maybe((String) null);
        return value + ":" + missing + ":" + chosen + ":" + rejection() + ":" + genericMissing + ":" + BoundedResults.run();
    }
}

class ResultHolder<T> {
    T value;
    ResultHolder(T value) { this.value = value; }
    T get() { return value; }
}

class BoundedResults {
    static <M extends java.util.Map<String, Integer>> M maybeMap(M value) {
        if (value == null) { return null; }
        return value;
    }
    static int run() {
        java.util.Map<String, Integer> result = maybeMap(new java.util.HashMap());
        return result.size();
    }
}
