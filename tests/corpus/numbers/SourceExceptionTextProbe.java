public class SourceExceptionTextProbe {
    static class Parsed extends NumberFormatException {
        Parsed(String message) { super(message); }
    }
    static class Invalid extends IllegalArgumentException {
        Invalid(String message) { super(message); }
    }
    static class Overridden extends NumberFormatException {
        Overridden() { super("base"); }
        @Override public String getMessage() { return "own"; }
    }
    static void fail(String message) { throw new Parsed(message); }
    public static int run() {
        String message = "\ud800bad\udc00";
        Parsed parsed = new Parsed(message);
        if (parsed.getMessage() != message || parsed.getLocalizedMessage() != message) return 1;
        if (parsed.getCause() != null) return 2;
        Invalid invalid = new Invalid(null);
        if (invalid.getMessage() != null) return 3;
        Overridden own = new Overridden();
        if (own.getMessage() != "own") return 6;
        try { fail(message); return 4; }
        catch (NumberFormatException caught) {
            if (caught.getMessage() != message || caught.getMessage().charAt(0) != '\ud800') return 5;
        }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
