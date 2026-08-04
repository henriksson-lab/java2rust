import java.util.ArrayList;
import java.util.List;
public class SelfAlias {
    private List<String> items = new ArrayList<>();
    private String tag = "t";
    String decorate(String s) { return s + tag; }
    public void run() {
        for (String item : items) {
            System.out.println(decorate(item));
        }
    }
}
