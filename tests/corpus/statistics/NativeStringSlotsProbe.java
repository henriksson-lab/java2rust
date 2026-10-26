public final class NativeStringSlotsProbe {
    enum Label { VALUE }
    static String echo(String value) { return value; }
    static String maybe(boolean present) {
        return present ? String.valueOf(Label.VALUE) : null;
    }
    static int message() {
        try { throw new IllegalStateException(String.valueOf(Label.VALUE)); }
        catch (IllegalStateException error) {
            return "VALUE".equals(error.getMessage()) ? 0 : 1;
        }
    }
    static final class SourceFactory {
        static String valueOf(int ignored) { return "source"; }
    }
    public static int run() {
        if (message() != 0) return 1;
        if (!"VALUE".equals(echo(String.valueOf(Label.VALUE)))) return 2;
        if (!"VALUE".equals(maybe(true)) || maybe(false) != null) return 3;
        if (!"null".equals(echo(String.valueOf((Object) null)))) return 4;
        if (!"source".equals(echo(SourceFactory.valueOf(1)))) return 5;
        String logical = "\uD800x\uDC00";
        String result = echo(String.valueOf(logical));
        if (result.length() != 3 || result.charAt(0) != '\uD800' || result.charAt(2) != '\uDC00') return 6;
        if (result != logical) return 7;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
