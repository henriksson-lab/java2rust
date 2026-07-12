package examples.stdlib;

public class MultiCatch {
    public int parseOrZero(String text) {
        try {
            return Integer.parseInt(text);
        } catch (NumberFormatException | NullPointerException ex) {
            return 0;
        }
    }
}

