public class ExceptionTextProbe {
    static void fail(String message) { throw new IllegalArgumentException(message); }
    static int run() {
        String message = "\ud800A\udc00";
        IllegalArgumentException value = new IllegalArgumentException(message);
        if (value.getMessage() != message || value.getLocalizedMessage() != message) return 1;
        if (value.getMessage().length() != 3 || value.getMessage().charAt(0) != '\ud800' || value.getMessage().charAt(2) != '\udc00') return 2;
        IllegalStateException absent = new IllegalStateException();
        if (absent.getMessage() != null || absent.getLocalizedMessage() != null) return 3;
        if (absent.getCause() != null) return 9;
        try { absent.getCause().getMessage(); return 10; }
        catch (NullPointerException expected) { }
        IllegalStateException empty = new IllegalStateException("");
        if (empty.getMessage() == null || empty.getMessage().length() != 0) return 4;
        RuntimeException wrapped = new RuntimeException(message, value);
        if (!wrapped.getMessage().equals(message) || !wrapped.getCause().getMessage().equals(message)) return 5;
        wrapped.addSuppressed(value);
        if (wrapped.getSuppressed().length != 1 || !wrapped.getSuppressed()[0].getMessage().equals(message)) return 6;
        try { fail(message); return 7; }
        catch (IllegalArgumentException caught) { if (!caught.getMessage().equals(message)) return 8; }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
