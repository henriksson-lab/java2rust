import java.util.NoSuchElementException;
import java.util.StringTokenizer;

public class StringTokenizerNextCatch {
    public static String run() {
        StringTokenizer tokenizer = new StringTokenizer("a", "|");
        String first = tokenizer.nextToken();
        try {
            tokenizer.nextToken();
            return "miss";
        } catch (NoSuchElementException e) {
            return first + ":caught";
        }
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
