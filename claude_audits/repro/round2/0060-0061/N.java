import java.util.ArrayList;
import java.util.List;

public class N {
    int n;
    List<String> items = new ArrayList<>();

    public void inCatch() {
        try {
            throw new RuntimeException("x");
        } catch (RuntimeException e) {
            n = n + 1;
            items.add("a");
        }
    }

    public void inFinally() {
        try {
            System.out.println("t");
        } finally {
            n = n + 1;
            items.add("b");
        }
    }

    public void plain() {
        n = n + 1;
        items.add("c");
    }
}
