public class SourceObjectDefaultToString {
    public static String run() {
        SourceObjectDefaultToStringValue value = new SourceObjectDefaultToStringValue();
        boolean direct = value.toString().startsWith("SourceObjectDefaultToStringValue@");
        Object erased = value;
        return direct
            + ","
            + erased.toString().startsWith("SourceObjectDefaultToStringValue@");
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}

class SourceObjectDefaultToStringValue {
}
