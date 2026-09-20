interface GeneralResult {
    Object value();
}

interface SpecificResult {
    String value();
}

interface GeneralFirst extends GeneralResult, SpecificResult {}
interface SpecificFirst extends SpecificResult, GeneralResult {}

class GeneralFirstConcrete implements GeneralFirst {
    public String value() { return "abc"; }
}

class SpecificFirstConcrete implements SpecificFirst {
    public String value() { return "def"; }
}

public class AnonymousCovariantParentContract {
    public static void main(String[] args) {
        GeneralFirst first = new GeneralFirst() {
            public String value() { return "hello"; }
        };
        SpecificFirst second = new SpecificFirst() {
            public String value() { return "world"; }
        };
        GeneralFirst concreteFirst = new GeneralFirstConcrete();
        SpecificFirst concreteSecond = new SpecificFirstConcrete();
        System.out.println(first.value().length() + second.value().length()
            + concreteFirst.value().length() + concreteSecond.value().length());
    }
}
