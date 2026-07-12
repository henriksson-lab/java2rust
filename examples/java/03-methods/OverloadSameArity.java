package examples.methods;

public class OverloadSameArity {
    public String choose(int value) {
        return "int:" + value;
    }

    public String choose(long value) {
        return "long:" + value;
    }

    public String choose(String value) {
        return "string:" + value;
    }

    public String callAll() {
        return choose(1) + "," + choose(1L) + "," + choose("one");
    }
}

