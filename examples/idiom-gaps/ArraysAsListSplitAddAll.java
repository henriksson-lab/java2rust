package idiom.gaps;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

public class ArraysAsListSplitAddAll {
    public List<String> fields(String filterString) {
        List<String> fields = new ArrayList<>();
        fields.addAll(Arrays.asList(filterString.split(",")));
        return fields;
    }
}
