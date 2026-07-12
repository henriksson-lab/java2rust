package examples.methods;

public class NullOverloadAmbiguity {
    public String pick(String value) {
        return "string";
    }

    public String pick(StringBuilder value) {
        return "builder";
    }

    public String ambiguous() {
        return pick(null);
    }
}

