/** Source exception constructor reduction, never a replacement exception API. */
public class ExceptionCauseConstructorProbe {
    static int evaluations;
    static String observe(String message) { evaluations++; return message; }
    static class Parsed extends NumberFormatException {
        Parsed(String message, Throwable cause) {
            super(observe(message));
            initCause(cause);
        }
    }
    static class MissingMessage extends IllegalStateException {
        MissingMessage(Throwable cause) {
            super();
            this.initCause(cause);
        }
    }
    static class NullCause extends RuntimeException {
        NullCause(String message) {
            super(message);
            super.initCause(null);
        }
    }
    static void fail(String message, Throwable cause) { throw new Parsed(message, cause); }
    public static int run() {
        String message = "outer\ud800";
        String detail = "cause\udc00";
        IllegalArgumentException cause = new IllegalArgumentException(detail);
        Parsed parsed = new Parsed(message, cause);
        if (evaluations != 1 || parsed.getMessage() != message) return 1;
        if (parsed.getCause() == null || parsed.getCause().getMessage() != detail
                || parsed.getCause().getMessage().charAt(5) != '\udc00') return 2;
        Parsed absent = new Parsed(null, null);
        if (evaluations != 2 || absent.getMessage() != null || absent.getCause() != null) return 3;
        Parsed noCause = new Parsed(message, null);
        if (evaluations != 3 || noCause.getMessage() != message || noCause.getCause() != null) return 4;
        MissingMessage missing = new MissingMessage(cause);
        if (missing.getMessage() != null || missing.getCause().getMessage() != detail) return 5;
        MissingMessage missingBoth = new MissingMessage(null);
        if (missingBoth.getMessage() != null || missingBoth.getCause() != null) return 6;
        NullCause emptyCause = new NullCause(message);
        if (emptyCause.getCause() != null || emptyCause.getMessage() != message) return 7;
        try { fail(message, cause); return 8; }
        catch (NumberFormatException caught) {
            if (caught.getMessage() != message || caught.getCause().getMessage() != detail) return 9;
        }
        if (evaluations != 4 || parsed.getMessage() != message
                || cause.getMessage() != detail || parsed.getCause().getMessage() != detail) return 10;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
