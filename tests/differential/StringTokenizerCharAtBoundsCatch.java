import java.util.StringTokenizer;

public class StringTokenizerCharAtBoundsCatch {
    public static String run() {
        StringTokenizer tokenizer = new StringTokenizer("a", "|");
        try {
            return "value=" + tokenizer.nextToken().charAt(1);
        } catch (StringIndexOutOfBoundsException ex) {
            return "caught";
        }
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
