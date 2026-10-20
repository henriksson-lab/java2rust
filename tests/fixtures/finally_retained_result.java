import java.util.List;
import java.util.ArrayList;
class FinallyRetainedResult {
    static <T> T keep(List<T> values, boolean cleanup) {
        T value = values.remove(0);
        try { int ignored = 0; }
        finally { if (cleanup) values.add(value); }
        return value;
    }
    static int run() {
        List<String> values = new ArrayList<>();
        values.add("kept");
        return keep(values, true).length() + values.size();
    }
}
