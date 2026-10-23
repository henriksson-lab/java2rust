public class FormatTextProbe {
    static int constructed;
    static class Unused {
        Unused() { constructed++; }
        public String toString() { throw new IllegalStateException("must not display"); }
    }
    static class Failing {
        public String toString() { throw new IllegalArgumentException("display\ud800"); }
    }
    static class FailingError {
        public String toString() { throw new AssertionError("display error"); }
    }
    static String format(String template, Object[] values) { return String.format(template, values); }
    static int run() {
        String raw = "\ud800A\udc00";
        String pattern = "\ud800:%s:\udc00";
        if (!format(pattern, new Object[]{raw, new Unused()}).equals("\ud800:" + raw + ":\udc00")) return 1;
        if (constructed != 1) return 2;
        if (!String.format("\ud800%s\udc00", "é😀").equals("\ud800é😀\udc00")) return 3;
        if (!String.format("plain\ud800", new Unused()).equals("plain\ud800")) return 4;
        if (constructed != 2) return 5;
        if (!String.format("constant\ud800").equals("constant\ud800")) return 6;
        if (!format("%S", new Object[]{"a\ud800ß"}).equals("A\ud800SS")) return 7;
        if (!format("%s|%.2f|%x", new Object[]{raw, 1.25, 31}).equals(raw + "|1.25|1f")) return 8;
        if (!format("%%:%s%n", new Object[]{raw}).equals("%:" + raw + "\n")) return 9;
        try { String.format("%s", new Failing()); return 10; }
        catch (IllegalArgumentException error) {
            if (!error.getMessage().equals("display\ud800")) return 11;
        }
        try { String.format("%s"); return 12; }
        catch (java.util.MissingFormatArgumentException error) {
            if (!error.getMessage().equals("Format specifier '%s'")) return 13;
        }
        try { format("%s:%s", new Object[]{raw}); return 14; }
        catch (IllegalArgumentException error) {
            if (!error.getMessage().equals("Format specifier '%s'")) return 15;
        }
        try { format("%s", new Object[]{new FailingError()}); return 16; }
        catch (AssertionError expected) { }
        java.util.function.Supplier<String> deferred = () -> String.format("%s");
        try { deferred.get(); return 17; }
        catch (java.util.MissingFormatArgumentException expected) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
