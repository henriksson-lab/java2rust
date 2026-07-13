package examples.methods;

public class NamingPolicy {
    private String xTestString;

    public String methodMIs() {
        return xTestString;
    }

    public String call(NamingPolicy xAA) {
        return xAA.methodMIs();
    }
}
