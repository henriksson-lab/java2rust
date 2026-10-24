package identity;

public class ExceptionBinaryIdentityProbe {
    static class Parent extends RuntimeException {
        Parent(String message) { super(message); }
    }
    static class Child extends Parent {
        Child(String message) { super(message); }
    }
    static class IOException extends Exception {
        IOException(String message) { super(message); }
    }
    static class Literal$Dollar extends IllegalArgumentException {
        Literal$Dollar(String message) { super(message); }
    }
    static void fail() { throw new Child("child"); }
    static void checked() throws IOException { throw new IOException("checked"); }
    public static int run() {
        try { fail(); return 1; }
        catch (Parent caught) {
            if (!caught.getClass().getName().equals("identity.ExceptionBinaryIdentityProbe$Child")) return 2;
            if (!(caught instanceof Parent) || !(caught instanceof RuntimeException)) return 3;
            RuntimeException wrapper = new RuntimeException(caught);
            if (!wrapper.getMessage().equals("identity.ExceptionBinaryIdentityProbe$Child: child")) return 4;
            if (!wrapper.getCause().getClass().getName().equals("identity.ExceptionBinaryIdentityProbe$Child")) return 5;
            wrapper.addSuppressed(caught);
            if (!wrapper.getSuppressed()[0].getClass().getName().equals("identity.ExceptionBinaryIdentityProbe$Child")) return 6;
        }
        try { checked(); return 7; }
        catch (RuntimeException wrong) { return 8; }
        catch (IOException caught) {
            if (!caught.getClass().getName().equals("identity.ExceptionBinaryIdentityProbe$IOException")) return 9;
        }
        Exception io = new java.io.IOException("native");
        if (!io.getClass().getName().equals("java.io.IOException")) return 10;
        Exception util = new java.util.NoSuchElementException("util");
        if (!util.getClass().getName().equals("java.util.NoSuchElementException")) return 11;
        Exception dollar = new Literal$Dollar("dollar");
        if (!dollar.getClass().getName().equals("identity.ExceptionBinaryIdentityProbe$Literal$Dollar")) return 12;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
