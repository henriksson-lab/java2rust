import java.util.ArrayList;
import java.util.List;

public class LoopMove {
    private List<String> items = new ArrayList<>();

    void add(String s) {
        items.add(s);
    }

    public void run(String value, int n) {
        for (int i = 0; i < n; i++) {
            add(value);
        }
    }
}
