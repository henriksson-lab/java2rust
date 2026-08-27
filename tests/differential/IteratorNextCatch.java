import java.util.ArrayList;
import java.util.Iterator;
import java.util.NoSuchElementException;

public class IteratorNextCatch {
    public static String run() {
        ArrayList<String> items = new ArrayList<String>();
        items.add("a");
        Iterator<String> iterator = items.iterator();
        String first = iterator.next();
        try {
            iterator.next();
            return "miss";
        } catch (NoSuchElementException e) {
            return first + ":caught";
        }
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
