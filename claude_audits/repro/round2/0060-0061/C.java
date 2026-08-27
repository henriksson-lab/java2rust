import java.util.List;

public class C {
    public String inCatch(List<String> xs) {
        try {
            return xs.get(0);
        } catch (RuntimeException e) {
            String s = xs.get(1);
            return s;
        }
    }

    public String inFinally(List<String> xs) {
        try {
            return xs.get(0);
        } finally {
            String t = xs.get(1);
            System.out.println(t);
        }
    }
}
