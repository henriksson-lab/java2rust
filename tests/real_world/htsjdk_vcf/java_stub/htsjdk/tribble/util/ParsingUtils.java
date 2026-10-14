package htsjdk.tribble.util;

import java.util.ArrayList;
import java.util.Collection;
import java.util.Collections;
import java.util.List;

public final class ParsingUtils {
    public static String join(final String separator, final Collection<?> values) {
        return String.join(separator, values.stream().map(Object::toString).toArray(String[]::new));
    }

    public static String join(final char separator, final Collection<?> values) {
        return join(String.valueOf(separator), values);
    }

    public static <T extends Comparable<? super T>> List<T> sortList(final Collection<T> values) {
        final List<T> result = new ArrayList<>(values);
        Collections.sort(result);
        return result;
    }
}
