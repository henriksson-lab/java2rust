package idiom.gaps;

public class JavaStringMethods {
    public boolean sameIgnoringCase(String left, String right) {
        return left.toLowerCase().equals(right.toLowerCase());
    }

    public int lengthOf(String value) {
        return value.length();
    }
}
