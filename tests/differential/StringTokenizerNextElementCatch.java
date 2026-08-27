import java.util.NoSuchElementException;
import java.util.StringTokenizer;

public class StringTokenizerNextElementCatch {
    public static String run() {
        StringTokenizer tokenizer = new StringTokenizer("a", "|");
        String first = (String) tokenizer.nextElement();
        boolean empty = tokenizer.hasMoreElements();
        try {
            tokenizer.nextElement();
            return "miss";
        } catch (NoSuchElementException e) {
            return first + ":" + empty + ":caught";
        }
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
