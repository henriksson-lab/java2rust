import java.util.ArrayList;
import java.util.List;

class RetainedCollectionSearch {
    static int run() {
        List<Integer> values = new ArrayList<>();
        values.add(4);
        values.add(3);
        values.add(4);
        Iterable<Integer> view = values;
        if (!values.contains(4) || values.contains(9)) { return -1; }
        if (values.indexOf(4) != 0 || values.indexOf(9) != -1) { return -2; }
        if (values.remove(Integer.valueOf(9))) { return -3; }
        if (!values.remove(Integer.valueOf(4))) { return -4; }
        int result = values.indexOf(4) * 100;
        for (int value : view) { result += value; }
        return result + values.size() * 10 + values.get(0) + values.get(1) - 2;
    }
}
