package idiom.gaps;

import java.util.List;

public class ToArrayEmptyStringArray {
    public String[] copy(List<String> names) {
        return names.toArray(new String[]{});
    }
}
