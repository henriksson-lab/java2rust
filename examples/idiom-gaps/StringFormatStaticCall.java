package idiom.gaps;

public class StringFormatStaticCall {
    public String describe(String name, int count) {
        return String.format("%s:%d", name, count);
    }
}
