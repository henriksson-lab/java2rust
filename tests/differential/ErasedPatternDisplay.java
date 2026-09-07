import java.util.regex.Pattern;

public class ErasedPatternDisplay {
    public static String run() {
        Object value = Pattern.compile("a+");
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
