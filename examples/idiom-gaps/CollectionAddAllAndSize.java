package idiom.gaps;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

public class CollectionAddAllAndSize {
    public int count() {
        List<String> values = new ArrayList<>();
        values.addAll(Arrays.asList("a", "b"));
        return values.size();
    }
}
