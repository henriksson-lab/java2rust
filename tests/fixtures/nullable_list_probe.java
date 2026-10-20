import java.util.List;
import java.util.ArrayList;

class NullableListProbe {
    static int run() {
        List<String> values = new ArrayList<>();
        values.add(null);
        values.add("present");
        int index = 0;
        int result = 0;
        if (values.get(index++) == null) result += 1;
        if (null != values.get(index++)) result += 2;
        try {
            if (values.get(-1) == null) result = -1000;
        } catch (IndexOutOfBoundsException expected) {
            result += 4;
        }
        try {
            if (values.get(2) != null) result = -1000;
        } catch (IndexOutOfBoundsException expected) {
            result += 8;
        }
        return result + index * 100;
    }
}
